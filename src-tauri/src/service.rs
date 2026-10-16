//! Tauri から独立したアプリケーションサービス層。IPC 用 DTO と排他制御・永続化をまとめる。

use std::io;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Mutex, MutexGuard};

use serde::Serialize;

use crate::evolution::{Branch, Personality};
use crate::expression::{express, Bubble, Expression};
use crate::game::{Action, GameState};

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
}

pub struct GameService {
    state: Mutex<GameState>,
    path: PathBuf,
    time_scale: u32,
}

impl GameService {
    pub fn new(state: GameState, path: PathBuf) -> Self {
        Self { state: Mutex::new(state), path, time_scale: 1 }
    }

    /// 経過時間の倍率（動作確認用）。
    pub fn with_time_scale(mut self, scale: u32) -> Self {
        self.time_scale = scale;
        self
    }

    pub fn snapshot(&self) -> StateView {
        view_of(&self.lock())
    }

    /// 現在時刻まで進めたうえでアクションを適用し、保存する。
    pub fn perform(&self, action: Action, now_unix: u64) -> io::Result<StateView> {
        let mut state = self.lock();
        state.step_to_scaled(now_unix, self.time_scale);
        state.apply(action);
        state.save(&self.path)?;
        Ok(view_of(&state))
    }

    /// 孵化の演出を見せたことを記録して保存する。
    pub fn ack_hatch(&self) -> io::Result<StateView> {
        let mut state = self.lock();
        state.ack_hatch();
        state.save(&self.path)?;
        Ok(view_of(&state))
    }

    /// 時間を進めて保存する。表示用スナップショットと進化回数を返す。
    pub fn tick(&self, now_unix: u64) -> io::Result<(StateView, u8)> {
        let mut state = self.lock();
        let evolutions = state.step_to_scaled(now_unix, self.time_scale);
        state.save(&self.path)?;
        Ok((view_of(&state), evolutions))
    }

    /// パニックしたスレッドがロックを汚染しても状態自体は有効なので続行する。
    fn lock(&self) -> MutexGuard<'_, GameState> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

fn view_of(s: &GameState) -> StateView {
    let mut expression = express(&s.status);
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
        play_action: if s.form.stage <= 1 { PlayAction::Pet } else { PlayAction::Play },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evolution::STAGE_SECS;
    use crate::expression::Level;
    use crate::game::TICK_SECS;
    use std::fs;
    use std::sync::Arc;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pokkepet-svc-{}-{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir.join("save.json")
    }

    fn svc(name: &str) -> GameService {
        GameService::new(GameState::new(0), tmp(name))
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
            }
        );
    }

    #[test]
    fn view_reflects_expression_after_actions() {
        let s = svc("expr");
        s.perform(Action::Clean, 0).unwrap(); // 初回オープニングを終える
        // 時間経過で汚れが溜まり、空腹になると吹き出しが変わる。
        let (v, _) = s.tick(u64::from(TICK_SECS) * 90).unwrap();
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
        let (v, _) = s.tick(u64::from(TICK_SECS)).unwrap();
        assert!(v.intro);
        assert_eq!(v.expression.bubble, Some(Bubble::Welcome));
    }

    #[test]
    fn hatching_is_reported_until_acknowledged_and_persisted() {
        let p = tmp("hatch");
        let s = GameService::new(GameState::new(0), p.clone());
        let (v, evo) = s.tick(u64::from(STAGE_SECS)).unwrap();
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
    fn play_action_is_pet_only_while_an_egg() {
        let s = svc("play-action");
        assert_eq!(s.snapshot().play_action, PlayAction::Pet);
        let (v, _) = s.tick(u64::from(STAGE_SECS)).unwrap();
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
        let (v, evo) = s.tick(u64::from(STAGE_SECS)).unwrap();
        assert_eq!((v.stage, evo), (2, 1));
    }

    #[test]
    fn time_scale_speeds_up_tick_and_perform() {
        let s = svc("scale").with_time_scale(60);
        let (v, evo) = s.tick(60).unwrap();
        assert_eq!((v.stage, evo), (2, 1));
        let s = svc("scale2").with_time_scale(60);
        assert_eq!(s.perform(Action::Clean, 60).unwrap().stage, 2);
    }

    #[test]
    fn tick_without_progress_reports_no_evolution() {
        let s = svc("tick0");
        let (v, evo) = s.tick(0).unwrap();
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
