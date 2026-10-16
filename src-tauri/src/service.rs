//! Tauri から独立したアプリケーションサービス層。IPC 用 DTO と排他制御・永続化をまとめる。

use std::io;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Mutex, MutexGuard};

use serde::Serialize;

use crate::evolution::{Branch, Personality};
use crate::expression::{express, Bubble, Expression};
use crate::game::{Action, GameState};
use crate::sleep::{is_sleeping, local_utc_offset};

impl FromStr for Action {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "feed" => Ok(Action::Feed),
            "play" => Ok(Action::Play),
            "clean" => Ok(Action::Clean),
            other => Err(format!("unknown action: {other}")),
        }
    }
}

/// フロントエンドへ渡す表示用スナップショット。
/// 「遊ぶ」ボタンの中身。卵の間は「撫でる」(pet)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PlayAction {
    Play,
    Pet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StateView {
    pub mood: u8,
    pub affinity: u8,
    pub hunger: u8,
    pub dirtiness: u8,
    pub stage: u8,
    pub branch: Branch,
    pub personality: Personality,
    pub expression: Expression,
    pub play_action: PlayAction,
    /// 初回オープニング中（最初の操作まで）。
    pub intro: bool,
    /// 孵化したが、演出をまだ見せていない。
    pub hatching: bool,
    /// おやすみ時間中（ローカル時刻）。
    pub sleeping: bool,
}

/// `tick` の結果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tick {
    pub view: StateView,
    /// 今回の時間経過で起きた進化の回数。
    pub evolutions: u8,
    /// 卵が孵った（段階 1 → 2 になった）。
    pub hatched: bool,
}

/// メニューバーのアイコン横に出す目印。ポップアップを開かなくても状態がわかるようにする。
pub fn tray_title(view: &StateView) -> Option<&'static str> {
    if view.hatching {
        return Some("🐣");
    }
    match view.expression.bubble? {
        Bubble::Hungry => Some("🍙"),
        Bubble::Dirty => Some("🧹"),
        Bubble::Gloomy => Some("💧"),
        Bubble::Sleeping => Some("💤"),
        Bubble::Welcome => Some("🥚"),
        Bubble::Happy => None,
    }
}

/// 孵化の通知を出すか。ポップアップが開いていれば演出で伝わるので出さない。
pub fn should_notify_hatch(hatched: bool, popup_open: bool) -> bool {
    hatched && !popup_open
}

pub struct GameService {
    state: Mutex<GameState>,
    path: PathBuf,
    time_scale: u32,
    /// None ならこのマシンのローカルタイムゾーンを使う（テストでは固定値を指定する）。
    utc_offset: Option<i32>,
}

impl GameService {
    pub fn new(state: GameState, path: PathBuf) -> Self {
        Self { state: Mutex::new(state), path, time_scale: 1, utc_offset: None }
    }

    /// 経過時間の倍率（動作確認用）。
    pub fn with_time_scale(mut self, scale: u32) -> Self {
        self.time_scale = scale;
        self
    }

    /// ローカル時刻の UTC オフセットを固定する（テスト用）。
    pub fn with_utc_offset(mut self, secs: i32) -> Self {
        self.utc_offset = Some(secs);
        self
    }

    fn offset_at(&self, unix_secs: u64) -> i32 {
        self.utc_offset.unwrap_or_else(|| local_utc_offset(unix_secs))
    }

    fn view(&self, state: &GameState) -> StateView {
        view_of(state, self.offset_at(state.last_unix))
    }

    pub fn snapshot(&self) -> StateView {
        self.view(&self.lock())
    }

    /// 現在時刻まで進めたうえでアクションを適用し、保存する。
    pub fn perform(&self, action: Action, now_unix: u64) -> io::Result<StateView> {
        let mut state = self.lock();
        state.step_to_in(now_unix, self.time_scale, self.offset_at(now_unix));
        state.apply(action);
        state.save(&self.path)?;
        Ok(self.view(&state))
    }

    /// 孵化の演出を見せたことを記録して保存する。
    pub fn ack_hatch(&self) -> io::Result<StateView> {
        let mut state = self.lock();
        state.ack_hatch();
        state.save(&self.path)?;
        Ok(self.view(&state))
    }

    /// 時間を進めて保存する。表示用スナップショットと進化回数を返す。
    pub fn tick(&self, now_unix: u64) -> io::Result<Tick> {
        let mut state = self.lock();
        let was_egg = state.form.stage <= 1;
        let evolutions = state.step_to_in(now_unix, self.time_scale, self.offset_at(now_unix));
        state.save(&self.path)?;
        Ok(Tick { view: self.view(&state), evolutions, hatched: was_egg && evolutions > 0 })
    }

    /// パニックしたスレッドがロックを汚染しても状態自体は有効なので続行する。
    fn lock(&self) -> MutexGuard<'_, GameState> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

fn view_of(s: &GameState, utc_offset: i32) -> StateView {
    let sleeping = is_sleeping(s.last_unix, utc_offset);
    let mut expression = express(&s.status);
    if sleeping {
        expression.bubble = Some(Bubble::Sleeping);
    }
    if !s.intro_seen {
        expression.bubble = Some(Bubble::Welcome);
    }
    StateView {
        mood: s.status.mood,
        affinity: s.status.affinity,
        hunger: s.status.hunger,
        dirtiness: s.status.dirtiness,
        stage: s.form.stage,
        branch: s.form.branch,
        personality: s.form.personality,
        expression,
        intro: !s.intro_seen,
        hatching: s.hatching(),
        sleeping,
        play_action: if s.form.stage <= 1 { PlayAction::Pet } else { PlayAction::Play },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evolution::{EGG_SECS, STAGE_SECS};
    use crate::expression::{Bubble, Level};
    use crate::game::TICK_SECS;
    use std::fs;
    use std::sync::Arc;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pokkepet-svc-{}-{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir.join("save.json")
    }

    /// UTC+12 に固定: UNIX 秒 0 が昼の 12:00 になり、テスト中はおやすみ時間にならない。
    const NOON_AT_ZERO: i32 = 12 * 3600;

    fn svc(name: &str) -> GameService {
        GameService::new(GameState::new(0), tmp(name)).with_utc_offset(NOON_AT_ZERO)
    }

    #[test]
    fn action_parses_known_names() {
        assert_eq!("feed".parse::<Action>(), Ok(Action::Feed));
        assert_eq!("play".parse::<Action>(), Ok(Action::Play));
        assert_eq!("clean".parse::<Action>(), Ok(Action::Clean));
    }

    #[test]
    fn action_rejects_unknown_name() {
        assert!("dance".parse::<Action>().is_err());
    }

    #[test]
    fn snapshot_maps_state_to_view() {
        let v = svc("snap").snapshot();
        assert_eq!(
            v,
            StateView {
                mood: 50,
                affinity: 0,
                hunger: 80,
                dirtiness: 0,
                stage: 1,
                branch: Branch::Normal,
                personality: Personality::Calm,
                expression: Expression {
                    mood: Level::Mid,
                    hunger: Level::High,
                    dirtiness: Level::Low,
                    bubble: Some(Bubble::Welcome),
                },
                play_action: PlayAction::Pet,
                intro: true,
                hatching: false,
                sleeping: false,
            }
        );
    }

    #[test]
    fn view_reflects_expression_after_actions() {
        let s = svc("expr");
        s.perform(Action::Clean, 0).unwrap(); // 初回オープニングを終える
        // 常駐中（2分おき）に 90 ティック進むと、空腹になって吹き出しが変わる。
        let mut v = s.snapshot();
        for i in 1..=90u64 {
            v = s.tick(i * u64::from(TICK_SECS)).unwrap().view;
        }
        assert_eq!(v.expression.hunger, Level::Low);
        assert_eq!(v.expression.bubble, Some(Bubble::Hungry));
    }

    #[test]
    fn the_first_view_shows_the_welcome_bubble_until_the_first_action() {
        let s = svc("welcome");
        assert_eq!(s.snapshot().expression.bubble, Some(Bubble::Welcome));
        assert!(s.snapshot().intro);
        let v = s.perform(Action::Play, 0).unwrap();
        assert!(!v.intro);
        assert_eq!(v.expression.bubble, None);
    }

    #[test]
    fn time_passing_keeps_the_welcome_bubble() {
        let s = svc("welcome-tick");
        let Tick { view: v, .. } = s.tick(u64::from(TICK_SECS)).unwrap();
        assert!(v.intro);
        assert_eq!(v.expression.bubble, Some(Bubble::Welcome));
    }

    #[test]
    fn hatching_is_reported_until_acknowledged_and_persisted() {
        let p = tmp("hatch");
        let s = GameService::new(GameState::new(0), p.clone());
        let Tick { view: v, evolutions: evo, .. } = s.tick(u64::from(STAGE_SECS)).unwrap();
        assert_eq!((evo, v.hatching), (1, true));
        let v = s.ack_hatch().unwrap();
        assert!(!v.hatching);
        assert!(GameState::load(&p, 0).unwrap().hatch_shown);
    }

    #[test]
    fn ack_hatch_returns_an_error_when_saving_fails() {
        let bad = tmp("hatch-bad").parent().unwrap().join("no/such/dir/save.json");
        let s = GameService::new(GameState::new(0), bad);
        assert!(s.ack_hatch().is_err());
    }

    #[test]
    fn view_json_exposes_intro_and_hatching() {
        let json = serde_json::to_value(svc("intro-json").snapshot()).unwrap();
        assert_eq!(json["intro"], true);
        assert_eq!(json["hatching"], false);
        assert_eq!(json["expression"]["bubble"], "welcome");
    }

    #[test]
    fn the_view_says_sleeping_during_the_local_night() {
        let night = GameService::new(GameState::new(0), tmp("night")).with_utc_offset(0); // 0:00
        assert!(night.snapshot().sleeping);
        assert!(!svc("day").snapshot().sleeping);
    }

    #[test]
    fn a_sleeping_pet_shows_the_sleeping_bubble_instead_of_its_needs() {
        let mut state = GameState::new(0);
        state.intro_seen = true;
        state.status.hunger = 10; // おなかがすいている
        let night = GameService::new(state, tmp("night-bubble")).with_utc_offset(0); // 0:00
        let v = night.snapshot();
        assert_eq!(v.expression.hunger, Level::Low);
        assert_eq!(v.expression.bubble, Some(Bubble::Sleeping));
    }

    #[test]
    fn an_awake_hungry_pet_still_asks_for_food() {
        let mut state = GameState::new(0);
        state.intro_seen = true;
        state.status.hunger = 10;
        let day = GameService::new(state, tmp("day-bubble")).with_utc_offset(NOON_AT_ZERO);
        assert_eq!(day.snapshot().expression.bubble, Some(Bubble::Hungry));
    }

    #[test]
    fn the_welcome_bubble_still_wins_over_sleeping() {
        let night = GameService::new(GameState::new(0), tmp("night-welcome")).with_utc_offset(0);
        assert_eq!(night.snapshot().expression.bubble, Some(Bubble::Welcome));
    }

    #[test]
    fn view_json_exposes_sleeping() {
        let json = serde_json::to_value(svc("sleep-json").snapshot()).unwrap();
        assert_eq!(json["sleeping"], false);
    }

    #[test]
    fn play_action_is_pet_only_while_an_egg() {
        let s = svc("play-action");
        assert_eq!(s.snapshot().play_action, PlayAction::Pet);
        let Tick { view: v, .. } = s.tick(u64::from(STAGE_SECS)).unwrap();
        assert_eq!(v.play_action, PlayAction::Play);
    }

    #[test]
    fn view_json_exposes_play_action() {
        let json = serde_json::to_value(svc("pa-json").snapshot()).unwrap();
        assert_eq!(json["playAction"], "pet");
    }

    #[test]
    fn view_json_exposes_expression() {
        let s = svc("expr-json");
        s.perform(Action::Clean, 0).unwrap(); // 初回オープニングを終える
        let json = serde_json::to_value(s.snapshot()).unwrap();
        assert_eq!(json["expression"]["mood"], "mid");
        assert_eq!(json["expression"]["dirtiness"], "low");
        assert_eq!(json["expression"]["bubble"], serde_json::Value::Null);
    }

    #[test]
    fn view_serializes_camel_case_with_lowercase_enums() {
        let json = serde_json::to_value(svc("json").snapshot()).unwrap();
        assert_eq!(json["dirtiness"], 0);
        assert_eq!(json["stage"], 1);
        assert_eq!(json["branch"], "normal");
        assert_eq!(json["personality"], "calm");
    }

    #[test]
    fn perform_applies_action_and_persists() {
        let p = tmp("perform");
        let s = GameService::new(GameState::new(0), p.clone());
        let v = s.perform(Action::Feed, 0).unwrap();
        assert_eq!(v.hunger, 100);
        assert_eq!(GameState::load(&p, 0).unwrap().history.feeds, 1);
    }

    #[test]
    fn perform_advances_time_before_applying() {
        let s = svc("perform-time");
        let v = s.perform(Action::Clean, u64::from(TICK_SECS)).unwrap();
        // 1ティックぶん空腹度が減っている。
        assert_eq!(v.hunger, 79);
        assert_eq!(v.dirtiness, 0);
    }

    #[test]
    fn perform_returns_error_when_save_fails() {
        let bad = tmp("bad").parent().unwrap().join("no/such/dir/save.json");
        let s = GameService::new(GameState::new(0), bad);
        assert!(s.perform(Action::Play, 0).is_err());
    }

    #[test]
    fn tick_advances_time_and_reports_evolutions() {
        let s = svc("tick");
        let Tick { view: v, evolutions: evo, .. } = s.tick(u64::from(STAGE_SECS)).unwrap();
        assert_eq!((v.stage, evo), (2, 1));
    }

    #[test]
    fn time_scale_speeds_up_tick_and_perform() {
        let s = svc("scale").with_time_scale(60);
        let Tick { view: v, evolutions: evo, .. } = s.tick(60).unwrap();
        assert_eq!((v.stage, evo), (2, 1));
        let s = svc("scale2").with_time_scale(60);
        assert_eq!(s.perform(Action::Clean, 60).unwrap().stage, 2);
    }

    #[test]
    fn tick_reports_the_hatch_only_for_the_first_evolution() {
        let s = svc("hatched-flag");
        let t = s.tick(u64::from(EGG_SECS)).unwrap();
        assert_eq!((t.evolutions, t.hatched), (1, true));
        let t = s.tick(u64::from(EGG_SECS) + u64::from(STAGE_SECS)).unwrap();
        assert_eq!((t.evolutions, t.hatched), (1, false));
    }

    #[test]
    fn tick_reports_the_hatch_after_a_long_absence_too() {
        let s = svc("hatched-long");
        let t = s.tick(u64::from(EGG_SECS) + u64::from(STAGE_SECS) * 3).unwrap();
        assert_eq!(t.evolutions, 4);
        assert!(t.hatched);
    }

    #[test]
    fn tick_does_not_report_a_hatch_while_the_egg_is_still_an_egg() {
        let t = svc("no-hatch").tick(u64::from(EGG_SECS) - 1).unwrap();
        assert!(!t.hatched);
    }

    fn view_with(bubble: Option<Bubble>) -> StateView {
        let mut v = svc("tray").snapshot();
        v.intro = false;
        v.expression.bubble = bubble;
        v
    }

    #[test]
    fn the_menu_bar_mark_follows_the_bubble() {
        assert_eq!(tray_title(&view_with(None)), None);
        assert_eq!(tray_title(&view_with(Some(Bubble::Happy))), None);
        assert_eq!(tray_title(&view_with(Some(Bubble::Hungry))), Some("🍙"));
        assert_eq!(tray_title(&view_with(Some(Bubble::Dirty))), Some("🧹"));
        assert_eq!(tray_title(&view_with(Some(Bubble::Gloomy))), Some("💧"));
        assert_eq!(tray_title(&view_with(Some(Bubble::Sleeping))), Some("💤"));
        assert_eq!(tray_title(&view_with(Some(Bubble::Welcome))), Some("🥚"));
    }

    #[test]
    fn an_unseen_hatch_beats_every_other_mark() {
        let mut v = view_with(Some(Bubble::Hungry));
        v.hatching = true;
        assert_eq!(tray_title(&v), Some("🐣"));
    }

    #[test]
    fn hatch_notification_is_only_for_a_closed_popup() {
        assert!(should_notify_hatch(true, false));
        assert!(!should_notify_hatch(true, true)); // 開いていれば演出で伝わる
        assert!(!should_notify_hatch(false, false));
        assert!(!should_notify_hatch(false, true));
    }

    #[test]
    fn tick_without_progress_reports_no_evolution() {
        let s = svc("tick0");
        let Tick { view: v, evolutions: evo, .. } = s.tick(0).unwrap();
        assert_eq!((v.stage, evo), (1, 0));
    }

    #[test]
    fn tick_returns_error_when_save_fails() {
        let bad = tmp("bad2").parent().unwrap().join("no/such/dir/save.json");
        let s = GameService::new(GameState::new(0), bad);
        assert!(s.tick(1).is_err());
    }

    #[test]
    fn poisoned_lock_is_recovered() {
        let s = Arc::new(svc("poison"));
        let s2 = Arc::clone(&s);
        let _ = std::thread::spawn(move || {
            let _guard = s2.lock();
            panic!("poison the mutex");
        })
        .join();
        assert_eq!(s.snapshot().stage, 1);
    }
}
