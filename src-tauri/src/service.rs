//! Tauri から独立したアプリケーションサービス層。IPC 用 DTO と排他制御・永続化をまとめる。

use std::io;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Mutex, MutexGuard};

use serde::Serialize;

use crate::evolution::{Branch, Personality};
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
}

pub struct GameService {
    state: Mutex<GameState>,
    path: PathBuf,
}

impl GameService {
    pub fn new(state: GameState, path: PathBuf) -> Self {
        Self { state: Mutex::new(state), path }
    }

    pub fn snapshot(&self) -> StateView {
        view_of(&self.lock())
    }

    /// 現在時刻まで進めたうえでアクションを適用し、保存する。
    pub fn perform(&self, action: Action, now_unix: u64) -> io::Result<StateView> {
        let mut state = self.lock();
        state.step_to(now_unix);
        state.apply(action);
        state.save(&self.path)?;
        Ok(view_of(&state))
    }

    /// 時間を進めて保存する。表示用スナップショットと進化回数を返す。
    pub fn tick(&self, now_unix: u64) -> io::Result<(StateView, u8)> {
        let mut state = self.lock();
        let evolutions = state.step_to(now_unix);
        state.save(&self.path)?;
        Ok((view_of(&state), evolutions))
    }

    /// パニックしたスレッドがロックを汚染しても状態自体は有効なので続行する。
    fn lock(&self) -> MutexGuard<'_, GameState> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

fn view_of(s: &GameState) -> StateView {
    StateView {
        mood: s.status.mood,
        affinity: s.status.affinity,
        hunger: s.status.hunger,
        dirtiness: s.status.dirtiness,
        stage: s.form.stage,
        branch: s.form.branch,
        personality: s.form.personality,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evolution::STAGE_SECS;
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
            }
        );
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
        let v = s.perform(Action::Clean, 60).unwrap();
        // 60秒=1ティックぶん空腹度が減っている。
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
