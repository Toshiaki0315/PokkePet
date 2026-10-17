//! ユーザー設定（配色・段階/ゲージの表示）。JSON で保存し、複数ウィンドウから共有する。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use serde::{Deserialize, Serialize};

use crate::sleep::{SleepWindow, DEFAULT_SLEEP_END_HOUR, DEFAULT_SLEEP_START_HOUR};

pub const DEFAULT_BODY_COLOR: &str = "#4b5fb8";
pub const DEFAULT_BUTTON_COLOR: &str = "#f280a8";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub show_gauges: bool,
    /// お世話の呼びかけ（空腹・汚れ・つまらない）をmacOSの通知で知らせる。
    pub notify_calls: bool,
    /// おやすみ時間の開始・終了（0〜23時）。同じ値ならおやすみなし。
    pub sleep_start_hour: u8,
    pub sleep_end_hour: u8,
    pub body_color: String,
    pub button_color: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            show_gauges: false,
            notify_calls: true,
            sleep_start_hour: DEFAULT_SLEEP_START_HOUR,
            sleep_end_hour: DEFAULT_SLEEP_END_HOUR,
            body_color: DEFAULT_BODY_COLOR.to_string(),
            button_color: DEFAULT_BUTTON_COLOR.to_string(),
        }
    }
}

impl Settings {
    /// 色は `#rrggbb` のみ許可する（CSS に埋め込むため厳密に検証する）。
    pub fn validate(&self) -> Result<(), String> {
        check_hour("sleepStartHour", self.sleep_start_hour)?;
        check_hour("sleepEndHour", self.sleep_end_hour)?;
        check_color("bodyColor", &self.body_color)?;
        check_color("buttonColor", &self.button_color)
    }

    /// ゲームに渡すおやすみ時間。
    pub fn sleep_window(&self) -> SleepWindow {
        SleepWindow { start_hour: self.sleep_start_hour, end_hour: self.sleep_end_hour }
    }

    /// ファイルが無ければ既定値。壊れていれば InvalidData。
    pub fn load(path: &Path) -> io::Result<Self> {
        let json = match fs::read_to_string(path) {
            Ok(json) => json,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(e),
        };
        let settings: Self = serde_json::from_str(&json).map_err(invalid_data)?;
        settings.validate().map_err(invalid_data)?;
        Ok(settings)
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_string(self).map_err(invalid_data)?)?;
        fs::rename(&tmp, path)
    }
}

fn invalid_data(e: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, e)
}

fn check_hour(field: &str, hour: u8) -> Result<(), String> {
    if hour <= 23 {
        return Ok(());
    }
    Err(format!("{field} must be 0-23: {hour}"))
}

fn check_color(field: &str, value: &str) -> Result<(), String> {
    let digits = value.strip_prefix('#').unwrap_or("");
    if digits.len() == 6 && digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Ok(());
    }
    Err(format!("{field} must be #rrggbb: {value:?}"))
}

pub struct SettingsService {
    settings: Mutex<Settings>,
    path: PathBuf,
}

impl SettingsService {
    pub fn new(settings: Settings, path: PathBuf) -> Self {
        Self { settings: Mutex::new(settings), path }
    }

    pub fn get(&self) -> Settings {
        self.lock().clone()
    }

    /// 検証→保存→反映の順。失敗時は現在の設定を変えない。
    pub fn update(&self, next: Settings) -> Result<Settings, String> {
        next.validate()?;
        let mut current = self.lock();
        next.save(&self.path).map_err(|e| e.to_string())?;
        *current = next.clone();
        Ok(next)
    }

    /// パニックしたスレッドがロックを汚染しても設定自体は有効なので続行する。
    fn lock(&self) -> MutexGuard<'_, Settings> {
        self.settings.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pokkepet-settings-{}-{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir.join("settings.json")
    }

    fn with_colors(body: &str, button: &str) -> Settings {
        Settings { body_color: body.into(), button_color: button.into(), ..Settings::default() }
    }

    #[test]
    fn defaults_hide_the_gauges_and_use_the_blue_pink_scheme() {
        let s = Settings::default();
        assert!(!s.show_gauges);
        assert_eq!((s.body_color.as_str(), s.button_color.as_str()), ("#4b5fb8", "#f280a8"));
        assert_eq!(s.validate(), Ok(()));
    }

    #[test]
    fn validate_accepts_six_digit_hex_in_either_case() {
        assert_eq!(with_colors("#AABBCC", "#0f9e3a").validate(), Ok(()));
    }

    #[test]
    fn validate_rejects_malformed_colors() {
        for bad in ["red", "#fff", "4b5fb8", "#12345g", "#1234567", "", "#ああああああ", "#4b5fb8;x", "url(x)"] {
            assert!(with_colors(bad, "#000000").validate().is_err(), "body {bad:?}");
            assert!(with_colors("#000000", bad).validate().is_err(), "button {bad:?}");
        }
    }

    #[test]
    fn validate_names_the_offending_field() {
        assert!(with_colors("x", "#000000").validate().unwrap_err().contains("bodyColor"));
        assert!(with_colors("#000000", "x").validate().unwrap_err().contains("buttonColor"));
    }

    #[test]
    fn call_notifications_are_on_by_default_and_can_be_turned_off() {
        assert!(Settings::default().notify_calls);
        let p = tmp("notify");
        let off = Settings { notify_calls: false, ..Settings::default() };
        off.save(&p).unwrap();
        assert!(!Settings::load(&p).unwrap().notify_calls);
    }

    #[test]
    fn older_settings_files_get_call_notifications_on() {
        let p = tmp("older");
        fs::write(&p, r##"{"showGauges": true, "bodyColor": "#010203"}"##).unwrap();
        assert!(Settings::load(&p).unwrap().notify_calls);
    }

    #[test]
    fn the_default_sleep_hours_are_23_to_7() {
        let s = Settings::default();
        assert_eq!((s.sleep_start_hour, s.sleep_end_hour), (23, 7));
        assert_eq!(s.sleep_window(), SleepWindow { start_hour: 23, end_hour: 7 });
    }

    #[test]
    fn any_hour_from_0_to_23_is_valid() {
        for h in 0..=23 {
            let s = Settings { sleep_start_hour: h, sleep_end_hour: 23 - h, ..Settings::default() };
            assert_eq!(s.validate(), Ok(()), "{h}");
        }
    }

    #[test]
    fn hours_above_23_are_rejected_and_named() {
        let start = Settings { sleep_start_hour: 24, ..Settings::default() };
        assert!(start.validate().unwrap_err().contains("sleepStartHour"));
        let end = Settings { sleep_end_hour: 99, ..Settings::default() };
        assert!(end.validate().unwrap_err().contains("sleepEndHour"));
    }

    #[test]
    fn sleep_hours_are_saved_and_converted_to_a_window() {
        let p = tmp("sleep-hours");
        let s = Settings { sleep_start_hour: 21, sleep_end_hour: 5, ..Settings::default() };
        s.save(&p).unwrap();
        let back = Settings::load(&p).unwrap();
        assert_eq!(back.sleep_window(), SleepWindow { start_hour: 21, end_hour: 5 });
    }

    #[test]
    fn older_settings_files_get_the_default_sleep_hours() {
        let p = tmp("older-sleep");
        fs::write(&p, r##"{"showGauges": true}"##).unwrap();
        assert_eq!(Settings::load(&p).unwrap().sleep_window(), SleepWindow::default());
    }

    #[test]
    fn a_settings_file_with_invalid_sleep_hours_is_invalid_data() {
        let p = tmp("bad-sleep");
        fs::write(&p, r##"{"sleepStartHour": 30}"##).unwrap();
        assert_eq!(Settings::load(&p).unwrap_err().kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn settings_files_from_when_the_stage_could_be_shown_still_load() {
        let p = tmp("old-stage");
        fs::write(&p, r##"{"showStage": true, "showGauges": true, "bodyColor": "#010203"}"##).unwrap();
        let s = Settings::load(&p).unwrap();
        assert!(s.show_gauges);
        assert_eq!(s.body_color, "#010203");
    }

    #[test]
    fn serializes_with_camel_case_keys() {
        let json = serde_json::to_value(Settings::default()).unwrap();
        assert!(json.get("showStage").is_none());
        assert_eq!(json["showGauges"], false);
        assert_eq!(json["notifyCalls"], true);
        assert_eq!(json["sleepStartHour"], 23);
        assert_eq!(json["sleepEndHour"], 7);
        assert_eq!(json["bodyColor"], "#4b5fb8");
        assert_eq!(json["buttonColor"], "#f280a8");
    }

    #[test]
    fn load_missing_file_returns_defaults() {
        assert_eq!(Settings::load(&tmp("missing")).unwrap(), Settings::default());
    }

    #[test]
    fn load_fills_missing_fields_with_defaults() {
        let p = tmp("partial");
        fs::write(&p, r##"{"showGauges": true}"##).unwrap();
        let s = Settings::load(&p).unwrap();
        assert!(s.show_gauges);
        assert_eq!(s.body_color, DEFAULT_BODY_COLOR);
    }

    #[test]
    fn load_corrupt_file_is_invalid_data() {
        let p = tmp("corrupt");
        fs::write(&p, "not json").unwrap();
        assert_eq!(Settings::load(&p).unwrap_err().kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn load_rejects_files_with_invalid_colors() {
        let p = tmp("badcolor");
        fs::write(&p, r##"{"bodyColor": "red"}"##).unwrap();
        assert_eq!(Settings::load(&p).unwrap_err().kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn load_propagates_other_io_errors() {
        let dir = tmp("isdir").parent().unwrap().to_path_buf();
        assert_ne!(Settings::load(&dir).unwrap_err().kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn save_then_load_roundtrips_and_leaves_no_temp_file() {
        let p = tmp("roundtrip");
        let s = Settings { show_gauges: true, ..with_colors("#112233", "#445566") };
        s.save(&p).unwrap();
        assert_eq!(Settings::load(&p).unwrap(), s);
        assert!(!p.with_extension("json.tmp").exists());
    }

    #[test]
    fn save_into_missing_directory_fails() {
        let p = tmp("nodir").parent().unwrap().join("no/such/dir/settings.json");
        assert!(Settings::default().save(&p).is_err());
    }

    #[test]
    fn service_update_validates_saves_and_applies() {
        let p = tmp("svc");
        let svc = SettingsService::new(Settings::default(), p.clone());
        let next = Settings { show_gauges: true, ..with_colors("#010203", "#040506") };
        assert_eq!(svc.update(next.clone()), Ok(next.clone()));
        assert_eq!(svc.get(), next);
        assert_eq!(Settings::load(&p).unwrap(), next);
    }

    #[test]
    fn service_update_rejects_invalid_settings_without_changing_anything() {
        let p = tmp("svc-invalid");
        let svc = SettingsService::new(Settings::default(), p.clone());
        assert!(svc.update(with_colors("nope", "#000000")).is_err());
        assert_eq!(svc.get(), Settings::default());
        assert!(!p.exists());
    }

    #[test]
    fn service_update_keeps_current_settings_when_saving_fails() {
        let bad = tmp("svc-save").parent().unwrap().join("no/such/dir/settings.json");
        let svc = SettingsService::new(Settings::default(), bad);
        assert!(svc.update(Settings { show_gauges: true, ..Settings::default() }).is_err());
        assert_eq!(svc.get(), Settings::default());
    }

    #[test]
    fn service_recovers_from_a_poisoned_lock() {
        let svc = Arc::new(SettingsService::new(Settings::default(), tmp("poison")));
        let s2 = Arc::clone(&svc);
        let _ = std::thread::spawn(move || {
            let _guard = s2.lock();
            panic!("poison");
        })
        .join();
        assert_eq!(svc.get(), Settings::default());
    }
}
