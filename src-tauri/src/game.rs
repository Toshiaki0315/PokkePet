//! ゲーム全体の状態。バックグラウンドの時間経過・進化適用・セーブ/ロードを担う。

use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::evolution::{evolve, Branch, Form, Growth, History, Personality};
use crate::status::Status;

/// 何秒ごとに1ティック進めるか。
pub const TICK_SECS: u32 = 120;
/// 1回の `step` で処理するティック数の上限（長時間離席時の無駄なループを避ける）。
pub const MAX_CATCHUP_TICKS: u32 = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Feed,
    Play,
    Clean,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameState {
    pub status: Status,
    pub growth: Growth,
    pub history: History,
    pub form: Form,
    /// ティック未満の端数秒。
    pub tick_acc: u32,
    /// 最後に時間を進めた時刻（UNIX秒）。
    pub last_unix: u64,
}

impl GameState {
    pub fn new(now_unix: u64) -> Self {
        Self {
            status: Status::new(50, 0, 80, 0),
            growth: Growth::egg(),
            history: History::default(),
            form: Form { stage: 1, branch: Branch::Normal, personality: Personality::Calm },
            tick_acc: 0,
            last_unix: now_unix,
        }
    }

    pub fn apply(&mut self, action: Action) {
        match action {
            Action::Feed => {
                self.status.feed();
                self.history.feeds += 1;
            }
            Action::Play => {
                self.status.play();
                self.history.plays += 1;
            }
            Action::Clean => {
                self.status.clean();
                self.history.cleans += 1;
            }
        }
    }

    /// 経過秒を進め、ティックと進化を適用する。発生した進化の回数を返す。
    pub fn step(&mut self, secs: u32) -> u8 {
        let acc = self.tick_acc.saturating_add(secs);
        self.tick_acc = acc % TICK_SECS;
        for _ in 0..(acc / TICK_SECS).min(MAX_CATCHUP_TICKS) {
            self.status.tick();
        }
        self.evolve_for(secs)
    }

    fn evolve_for(&mut self, secs: u32) -> u8 {
        let mut evolutions = 0;
        let mut advanced = self.growth.advance(secs);
        while advanced {
            self.form = evolve(self.growth.stage - 1, &self.status, &self.history);
            evolutions += 1;
            advanced = self.growth.advance(0);
        }
        evolutions
    }

    /// 現在時刻まで進める（アプリ停止中・スリープ中の経過分も含む）。
    /// 時計が巻き戻っていた場合は何も進めず時刻だけ更新する。
    pub fn step_to(&mut self, now_unix: u64) -> u8 {
        self.step_to_scaled(now_unix, 1)
    }

    /// 経過時間を `scale` 倍して進める（動作確認用の時間短縮）。0 は 1 として扱う。
    pub fn step_to_scaled(&mut self, now_unix: u64, scale: u32) -> u8 {
        let elapsed = now_unix.saturating_sub(self.last_unix);
        self.last_unix = now_unix;
        if elapsed == 0 {
            return 0;
        }
        let secs = elapsed.saturating_mul(u64::from(scale.max(1)));
        self.step(u32::try_from(secs).unwrap_or(u32::MAX))
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn from_json(json: &str) -> io::Result<Self> {
        serde_json::from_str(json).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    /// 一時ファイルに書いてからリネームし、書き込み途中の破損を避ける。
    pub fn save(&self, path: &Path) -> io::Result<()> {
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, self.to_json())?;
        fs::rename(&tmp, path)
    }

    /// ファイルが無ければ新規状態を返す。壊れていれば InvalidData。
    pub fn load(path: &Path, now_unix: u64) -> io::Result<Self> {
        match fs::read_to_string(path) {
            Ok(json) => Self::from_json(&json),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Self::new(now_unix)),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evolution::STAGE_SECS;
    use std::path::PathBuf;

    fn new_state() -> GameState {
        GameState::new(1_000)
    }

    fn tmp_path(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pokkepet-test-{}-{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir.join("save.json")
    }

    #[test]
    fn new_starts_as_egg_with_neutral_values() {
        let s = new_state();
        assert_eq!(s.status, Status::new(50, 0, 80, 0));
        assert_eq!(s.growth, Growth::egg());
        assert_eq!(s.history, History::default());
        assert_eq!(
            s.form,
            Form { stage: 1, branch: Branch::Normal, personality: Personality::Calm }
        );
        assert_eq!((s.tick_acc, s.last_unix), (0, 1_000));
    }

    #[test]
    fn apply_updates_status_and_history() {
        let mut s = new_state();
        s.apply(Action::Feed);
        s.apply(Action::Play);
        s.apply(Action::Play);
        s.apply(Action::Clean);
        assert_eq!(s.history, History { feeds: 1, plays: 2, cleans: 1 });
        assert_eq!(s.status.dirtiness, 0);
        assert!(s.status.affinity > 0);
    }

    #[test]
    fn step_below_tick_only_accumulates() {
        let mut s = new_state();
        let before = s.status;
        assert_eq!(s.step(TICK_SECS - 1), 0);
        assert_eq!(s.status, before);
        assert_eq!(s.tick_acc, TICK_SECS - 1);
    }

    #[test]
    fn step_runs_one_tick_per_interval_and_keeps_remainder() {
        let mut s = new_state();
        s.step(TICK_SECS + 7);
        assert_eq!(s.status.hunger, 79);
        assert_eq!(s.status.dirtiness, 1);
        assert_eq!(s.tick_acc, 7);
    }

    #[test]
    fn step_remainders_carry_across_calls() {
        let mut s = new_state();
        s.step(TICK_SECS / 2);
        s.step(TICK_SECS / 2);
        assert_eq!(s.status.dirtiness, 1);
        assert_eq!(s.tick_acc, 0);
    }

    #[test]
    fn step_caps_catchup_ticks() {
        let mut s = new_state();
        s.step(TICK_SECS * (MAX_CATCHUP_TICKS + 500));
        // 上限ぶんしか進まない（hunger 80 は 80 ティックで 0 になるので飽和）。
        assert_eq!(s.status.hunger, 0);
        assert_eq!(s.status.dirtiness, 100);
        assert_eq!(s.tick_acc, 0);
    }

    #[test]
    fn step_evolves_when_stage_time_passes() {
        let mut s = new_state();
        assert_eq!(s.step(STAGE_SECS), 1);
        assert_eq!(s.growth.stage, 2);
        assert_eq!(s.form.stage, 2);
    }

    #[test]
    fn step_applies_personality_from_history_on_evolution() {
        let mut s = new_state();
        for _ in 0..3 {
            s.apply(Action::Play);
        }
        s.step(STAGE_SECS);
        assert_eq!(s.form.personality, Personality::Playful);
    }

    #[test]
    fn step_returns_multiple_evolutions_after_long_absence() {
        let mut s = new_state();
        assert_eq!(s.step(STAGE_SECS * 3), 3);
        assert_eq!(s.growth.stage, 4);
        assert_eq!(s.form.stage, 4);
    }

    #[test]
    fn step_stops_evolving_at_final_stage() {
        let mut s = new_state();
        s.step(STAGE_SECS * 20);
        assert_eq!(s.growth.stage, 10);
        assert_eq!(s.step(STAGE_SECS), 0);
        assert_eq!(s.form.stage, 10);
    }

    #[test]
    fn step_to_advances_by_elapsed_seconds_and_updates_time() {
        let mut s = new_state();
        assert_eq!(s.step_to(1_000 + u64::from(STAGE_SECS)), 1);
        assert_eq!(s.last_unix, 1_000 + u64::from(STAGE_SECS));
    }

    #[test]
    fn step_to_ignores_clock_rollback() {
        let mut s = new_state();
        let before = s.status;
        assert_eq!(s.step_to(500), 0);
        assert_eq!(s.status, before);
        assert_eq!(s.last_unix, 500);
    }

    #[test]
    fn step_to_clamps_huge_gaps_without_overflow() {
        let mut s = new_state();
        s.step_to(u64::MAX);
        assert_eq!(s.growth.stage, 10);
    }

    #[test]
    fn step_to_scaled_multiplies_elapsed_time() {
        let mut s = new_state();
        assert_eq!(s.step_to_scaled(1_000 + 60, 60), 1);
        assert_eq!(s.growth.stage, 2);
        assert_eq!(s.last_unix, 1_060);
    }

    #[test]
    fn step_to_scaled_treats_zero_as_one() {
        let mut s = new_state();
        assert_eq!(s.step_to_scaled(1_000 + 60, 0), 0);
        assert_eq!(s.growth.elapsed_secs, 60);
    }

    #[test]
    fn step_to_scaled_saturates_instead_of_overflowing() {
        let mut s = new_state();
        s.step_to_scaled(1_000 + u64::from(u32::MAX), u32::MAX);
        assert_eq!(s.growth.stage, 10);
    }

    #[test]
    fn json_roundtrip_preserves_state() {
        let mut s = new_state();
        s.apply(Action::Feed);
        s.step(STAGE_SECS + 10);
        let back = GameState::from_json(&s.to_json()).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn from_json_rejects_garbage_as_invalid_data() {
        let err = GameState::from_json("{not json").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn save_then_load_restores_state() {
        let path = tmp_path("roundtrip");
        let mut s = new_state();
        s.apply(Action::Play);
        s.save(&path).unwrap();
        assert_eq!(GameState::load(&path, 9_999).unwrap(), s);
        assert!(!path.with_extension("json.tmp").exists());
    }

    #[test]
    fn save_overwrites_existing_file() {
        let path = tmp_path("overwrite");
        let mut s = new_state();
        s.save(&path).unwrap();
        s.apply(Action::Feed);
        s.save(&path).unwrap();
        assert_eq!(GameState::load(&path, 0).unwrap(), s);
    }

    #[test]
    fn load_missing_file_returns_fresh_state() {
        let path = tmp_path("missing");
        assert_eq!(GameState::load(&path, 42).unwrap(), GameState::new(42));
    }

    #[test]
    fn load_corrupt_file_is_invalid_data() {
        let path = tmp_path("corrupt");
        fs::write(&path, "garbage").unwrap();
        let err = GameState::load(&path, 0).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn save_into_missing_directory_fails() {
        let path = tmp_path("nodir").parent().unwrap().join("no/such/dir/save.json");
        assert!(new_state().save(&path).is_err());
    }

    #[test]
    fn load_propagates_other_io_errors() {
        let dir = tmp_path("isdir").parent().unwrap().to_path_buf();
        let err = GameState::load(&dir, 0).unwrap_err();
        assert_ne!(err.kind(), io::ErrorKind::NotFound);
    }

    /// ゲームバランス: 1分刻みで10時間（全10段階）遊んだときの最終状態。
    fn simulate(mut policy: impl FnMut(u32, &mut GameState)) -> GameState {
        let mut s = new_state();
        for minute in 0..600 {
            s.step(60);
            policy(minute, &mut s);
        }
        s
    }

    fn care_every(minutes: u32) -> impl FnMut(u32, &mut GameState) {
        move |m, s| {
            if m % minutes != 0 {
                return;
            }
            s.apply(Action::Feed);
            s.apply(Action::Play);
            s.apply(Action::Clean);
        }
    }

    #[test]
    fn balance_neglected_pet_grows_up_but_ends_poor() {
        let s = simulate(|_, _| {});
        assert_eq!(s.form.stage, 10);
        assert_eq!(s.form.branch, Branch::Poor);
        assert_eq!(s.status.mood, 0);
    }

    #[test]
    fn balance_hourly_care_keeps_pet_normal() {
        let s = simulate(care_every(60));
        assert_eq!(s.form.branch, Branch::Normal);
        assert!(s.status.hunger > 0);
    }

    #[test]
    fn balance_half_hourly_care_reaches_good() {
        let s = simulate(care_every(30));
        assert_eq!(s.form.branch, Branch::Good);
    }

    #[test]
    fn balance_reactive_player_never_lets_mood_collapse() {
        let mut min_mood = 100;
        let s = simulate(|_, s| {
            if s.status.hunger < 40 {
                s.apply(Action::Feed);
            }
            if s.status.mood < 50 {
                s.apply(Action::Play);
            }
            if s.status.dirtiness >= 60 {
                s.apply(Action::Clean);
            }
            min_mood = min_mood.min(s.status.mood);
        });
        assert!(min_mood >= 40);
        assert_ne!(s.form.branch, Branch::Poor);
    }
}
