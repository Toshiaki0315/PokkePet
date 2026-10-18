//! ユーザー設定（配色・段階/ゲージの表示）。JSON で保存し、複数ウィンドウから共有する。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use serde::{Deserialize, Serialize};

use crate::sleep::{SleepWindow, DEFAULT_SLEEP_END_HOUR, DEFAULT_SLEEP_START_HOUR};

/// ポップアップの大きさ（論理ピクセル）。ゲージを出すときは縦に広げる。
pub const POPUP_WIDTH: f64 = 320.0;
pub const POPUP_HEIGHT: f64 = 380.0;
pub const POPUP_HEIGHT_WITH_GAUGES: f64 = 470.0;
pub const DEFAULT_SOUND_VOLUME: u8 = 50;
pub const SOUND_VOLUME_MAX: u8 = 100;
pub const SOUND_PITCH_RANGE: std::ops::RangeInclusive<i8> = -2..=2;
/// 選べる背景（画面の景色）。`classic` は「きもちで色が変わる」背景、`season` は「季節にまかせる」（春は夜桜、夏はうみべ、秋はあき、冬はゆきげしき）。
pub const BACKGROUNDS: [&str; 11] = ["classic", "season", "room", "tatami", "meadow", "forest", "beach", "sakura", "snow", "autumn", "stars"];
/// 吹き出しを出しておく長さ（秒）の既定と上限。0 は「ずっと出しておく」。
pub const DEFAULT_BUBBLE_SECONDS: u8 = 8;
pub const BUBBLE_SECONDS_MAX: u8 = 60;
pub const DEFAULT_BACKGROUND: &str = "classic";
pub const DEFAULT_BODY_COLOR: &str = "#4b5fb8";
pub const DEFAULT_BUTTON_COLOR: &str = "#f280a8";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub show_gauges: bool,
    /// お世話の呼びかけ（空腹・汚れ・つまらない）をmacOSの通知で知らせる。
    pub notify_calls: bool,
    /// 鳴き声（サウンド）を鳴らす。
    pub sound_enabled: bool,
    /// 鳴き声の大きさ（0〜100。50 が標準）。
    pub sound_volume: u8,
    /// 鳴き声の高さ（-2〜2。0 が標準、1 段で半音 3 つぶん）。
    pub sound_pitch: i8,
    /// ログイン時にアプリを自動で起動する。
    pub launch_at_login: bool,
    /// きびしいモード: 病気や衰弱を放っておくと、お別れ（死）がある。
    pub strict_mode: bool,
    /// 作業に寄り添う: Mac を操作している時間（操作の有無だけ）を、いっしょにいた時間として数える。
    pub companion_enabled: bool,
    /// 長く休憩なしで作業しているとき、「ちょっと休もうよ」と声をかける。
    pub break_reminder: bool,
    /// 画面の背景（`BACKGROUNDS` のどれか）。
    pub background: String,
    /// 吹き出しを出しておく長さ（秒。0〜60）。過ぎると、すっと消える。0 なら、ずっと出しておく。
    pub bubble_seconds: u8,
    /// Mac の状態（電池・ネット・ダークモード）にペットが反応する。
    pub system_reactions: bool,
    /// 操作する時間帯から生活リズムを学んで、おやすみ時間を自動で合わせる。
    pub auto_sleep: bool,
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
            sound_enabled: true,
            sound_volume: DEFAULT_SOUND_VOLUME,
            sound_pitch: 0,
            launch_at_login: false,
            strict_mode: false,
            companion_enabled: true,
            system_reactions: true,
            background: DEFAULT_BACKGROUND.to_string(),
            bubble_seconds: DEFAULT_BUBBLE_SECONDS,
            break_reminder: true,
            auto_sleep: true,
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
        if self.sound_volume > SOUND_VOLUME_MAX {
            return Err(format!("soundVolume must be 0-{SOUND_VOLUME_MAX}: {}", self.sound_volume));
        }
        if !SOUND_PITCH_RANGE.contains(&self.sound_pitch) {
            return Err(format!("soundPitch must be -2..=2: {}", self.sound_pitch));
        }
        if !BACKGROUNDS.contains(&self.background.as_str()) {
            return Err(format!("background must be one of {BACKGROUNDS:?}: {:?}", self.background));
        }
        if self.bubble_seconds > BUBBLE_SECONDS_MAX {
            return Err(format!("bubbleSeconds must be 0-{BUBBLE_SECONDS_MAX}: {}", self.bubble_seconds));
        }
        check_color("bodyColor", &self.body_color)?;
        check_color("buttonColor", &self.button_color)
    }

    /// ポップアップの高さ。ゲージを出さないときは、空きが大きくならないよう低くする。
    pub fn popup_height(&self) -> f64 {
        if self.show_gauges { POPUP_HEIGHT_WITH_GAUGES } else { POPUP_HEIGHT }
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
    fn sound_is_on_by_default_and_can_be_turned_off() {
        assert!(Settings::default().sound_enabled);
        let p = tmp("sound");
        Settings { sound_enabled: false, ..Settings::default() }.save(&p).unwrap();
        assert!(!Settings::load(&p).unwrap().sound_enabled);
    }

    #[test]
    fn older_settings_files_get_the_sound_on() {
        let p = tmp("older-sound");
        fs::write(&p, r##"{"showGauges": true, "notifyCalls": false}"##).unwrap();
        let s = Settings::load(&p).unwrap();
        assert!(s.sound_enabled);
        assert!(!s.notify_calls);
    }

    #[test]
    fn the_cry_volume_and_pitch_default_to_standard_and_are_saved() {
        let d = Settings::default();
        assert_eq!((d.sound_volume, d.sound_pitch, d.launch_at_login), (50, 0, false));
        let p = tmp("sound-tuning");
        Settings { sound_volume: 80, sound_pitch: -1, launch_at_login: true, ..d }.save(&p).unwrap();
        let back = Settings::load(&p).unwrap();
        assert_eq!((back.sound_volume, back.sound_pitch, back.launch_at_login), (80, -1, true));
    }

    #[test]
    fn the_popup_is_taller_only_while_the_gauges_are_shown() {
        assert_eq!(Settings::default().popup_height(), POPUP_HEIGHT);
        assert_eq!(Settings { show_gauges: true, ..Settings::default() }.popup_height(), POPUP_HEIGHT_WITH_GAUGES);
    }

    #[test]
    fn the_companion_features_are_on_by_default_and_can_be_turned_off() {
        let d = Settings::default();
        assert!(d.companion_enabled && d.break_reminder && d.auto_sleep && d.system_reactions);
        let p = tmp("companion");
        Settings { companion_enabled: false, break_reminder: false, auto_sleep: false, ..d }.save(&p).unwrap();
        let back = Settings::load(&p).unwrap();
        assert!(!back.companion_enabled && !back.break_reminder && !back.auto_sleep);
        let json = serde_json::to_value(Settings::default()).unwrap();
        assert_eq!(json["systemReactions"], true);
        assert_eq!((json["companionEnabled"].as_bool(), json["breakReminder"].as_bool(), json["autoSleep"].as_bool()), (Some(true), Some(true), Some(true)));
    }

    #[test]
    fn the_background_defaults_to_the_classic_one_and_only_known_backgrounds_are_allowed() {
        assert_eq!(Settings::default().background, "classic");
        for name in BACKGROUNDS {
            assert!(Settings { background: name.to_string(), ..Settings::default() }.validate().is_ok(), "{name}");
        }
        let err = Settings { background: "space".to_string(), ..Settings::default() }.validate().unwrap_err();
        assert!(err.contains("background"));
        let p = tmp("background");
        Settings { background: "beach".to_string(), ..Settings::default() }.save(&p).unwrap();
        assert_eq!(Settings::load(&p).unwrap().background, "beach");
        assert_eq!(serde_json::to_value(Settings::default()).unwrap()["background"], "classic");
        fs::write(&p, r##"{"background": "nope"}"##).unwrap();
        assert_eq!(Settings::load(&p).unwrap_err().kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn the_bubble_lasts_eight_seconds_by_default_and_can_be_changed_or_kept_forever() {
        assert_eq!(Settings::default().bubble_seconds, 8);
        for seconds in [0, 3, 8, 60] {
            assert!(Settings { bubble_seconds: seconds, ..Settings::default() }.validate().is_ok(), "{seconds}");
        }
        assert!(Settings { bubble_seconds: 61, ..Settings::default() }.validate().unwrap_err().contains("bubbleSeconds"));
        let p = tmp("bubble-seconds");
        Settings { bubble_seconds: 15, ..Settings::default() }.save(&p).unwrap();
        assert_eq!(Settings::load(&p).unwrap().bubble_seconds, 15);
        assert_eq!(serde_json::to_value(Settings::default()).unwrap()["bubbleSeconds"], 8);
        fs::write(&p, r##"{"showGauges": true}"##).unwrap();
        assert_eq!(Settings::load(&p).unwrap().bubble_seconds, 8, "older settings files get the default");
    }

    #[test]
    fn strict_mode_is_off_by_default_and_is_saved() {
        assert!(!Settings::default().strict_mode);
        let p = tmp("strict");
        Settings { strict_mode: true, ..Settings::default() }.save(&p).unwrap();
        assert!(Settings::load(&p).unwrap().strict_mode);
        assert_eq!(serde_json::to_value(Settings::default()).unwrap()["strictMode"], false);
    }

    #[test]
    fn out_of_range_volume_or_pitch_is_rejected() {
        let base = Settings::default();
        assert!(Settings { sound_volume: 100, sound_pitch: 2, ..base.clone() }.validate().is_ok());
        assert!(Settings { sound_volume: 0, sound_pitch: -2, ..base.clone() }.validate().is_ok());
        assert!(Settings { sound_volume: 101, ..base.clone() }.validate().unwrap_err().contains("soundVolume"));
        assert!(Settings { sound_pitch: 3, ..base.clone() }.validate().unwrap_err().contains("soundPitch"));
        assert!(Settings { sound_pitch: -3, ..base }.validate().is_err());
    }

    #[test]
    fn serializes_with_camel_case_keys() {
        let json = serde_json::to_value(Settings::default()).unwrap();
        assert!(json.get("showStage").is_none());
        assert_eq!(json["showGauges"], false);
        assert_eq!(json["notifyCalls"], true);
        assert_eq!(json["soundEnabled"], true);
        assert_eq!(json["soundVolume"], 50);
        assert_eq!(json["soundPitch"], 0);
        assert_eq!(json["launchAtLogin"], false);
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
