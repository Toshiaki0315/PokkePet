//! ゲーム全体の状態。バックグラウンドの時間経過・進化適用・セーブ/ロードを担う。

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::care::{Care, SICK_MOOD_LOSS};
use crate::evolution::{evolve, Branch, Form, Growth, History, Personality};
use crate::dex::form_key;
use crate::diary::{self, DiaryEntry, DiaryEvent, DAY_SECS};
use crate::sleep::{is_sleeping, SleepWindow};
use crate::status::Status;

/// 何秒ごとに1ティック進めるか。
pub const TICK_SECS: u32 = 120;
/// 実際の経過がこれを超えたら「留守番（アプリ停止・スリープ）」として扱う。
pub const AWAY_THRESHOLD_SECS: u64 = 300;
/// 留守番中・おやすみ中は、状態の減りがこの割合（1/N）になる。両方なら掛け合わせる。
pub const AWAY_DECAY_DIV: u32 = 4;
pub const SLEEP_DECAY_DIV: u32 = 4;
/// 1回の `step` で処理するティック数の上限（長時間離席時の無駄なループを避ける）。
/// ほったらかしのとき、この回数のティックごとに親密度が 1 下がる。
pub const AFFINITY_DECAY_TICKS: u32 = 60;
/// 電源ボタンで寝かせたとき、自然に起きるまでの長さ（秒）。
pub const FORCED_SLEEP_SECS: u64 = 8 * 60 * 60;
pub const MAX_CATCHUP_TICKS: u32 = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Feed,
    Snack,
    Play,
    Clean,
    /// しつけ（叱る）。わがままを言っているときだけ効果がある。
    Scold,
    /// 治療（注射）。病気のときだけ効果がある。
    Cure,
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
    /// 初回オープニング（「卵が届いたよ」）を済ませたか。最初の操作で済みになる。旧セーブは済み扱い。
    #[serde(default = "yes")]
    pub intro_seen: bool,
    /// 孵化の演出を見せたか。旧セーブは済み扱い。
    #[serde(default = "yes")]
    pub hatch_shown: bool,
    /// これまでに進めたティック数。減りをゆっくりにする間引きに使う。
    #[serde(default)]
    pub tick_count: u32,
    /// ペットの名前。孵化後に付ける。
    #[serde(default)]
    pub name: Option<String>,
    /// 図鑑: これまでに出会った姿（`dex::form_key`）。
    #[serde(default)]
    pub discovered: BTreeSet<String>,
    /// 思い出（日記）。古いものから順。
    #[serde(default)]
    pub diary: Vec<DiaryEntry>,
    /// 卵が届いた時刻（UNIX秒）。いっしょの日数の起点。古いセーブでは読み込み時に補う。
    #[serde(default)]
    pub born_unix: u64,
    /// 記念日として記録した、いっしょの日数（丸何日か）。
    #[serde(default)]
    pub anniversaries: u32,
    /// お世話の質（体重・しつけ度・お世話ミス・わがまま）。
    #[serde(default)]
    pub care: Care,
    /// 亡くなっている（きびしいモードで、病気や衰弱を放置した）。新しい卵を迎えるまで、時間は止まる。
    #[serde(default)]
    pub dead: bool,
    /// 電源ボタンで寝かされている間の終わり（UNIX秒）。おやすみ時間とは関係なく眠っている。
    #[serde(default)]
    pub forced_sleep_until: Option<u64>,
    /// きびしいモードか。設定から与えられる実行時の値で、セーブには含めない。
    #[serde(skip)]
    pub strict: bool,
    /// 何代目か（最初のペットが 1）。
    #[serde(default = "first_generation")]
    pub generation: u32,
    /// 前の世代から受け継いだ性格。新しいペットは、何もしなければこの性格に育つ。
    #[serde(default)]
    pub legacy: Option<Personality>,
}

/// 名前の最大文字数（バイトではなく文字数）。
pub const NAME_MAX_CHARS: usize = 8;

/// 名前を検証して、前後の空白を除いたものを返す。
pub fn validate_name(raw: &str) -> Result<String, String> {
    let name = raw.trim();
    if name.is_empty() {
        return Err("name must not be empty".to_string());
    }
    if name.chars().count() > NAME_MAX_CHARS {
        return Err(format!("name must be at most {NAME_MAX_CHARS} characters"));
    }
    if name.chars().any(char::is_control) {
        return Err("name must not contain control characters".to_string());
    }
    Ok(name.to_string())
}

/// 時間を進めるときの文脈（おやすみ・留守番の判定に使う）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timeline {
    /// 進め始める時刻（UNIX秒）。
    pub start_unix: u64,
    /// ローカル時刻の UTC からの差（秒）。
    pub utc_offset: i32,
    /// 留守番（アプリ停止・スリープ明け）か。
    pub away: bool,
    /// おやすみ時間の設定。
    pub sleep: SleepWindow,
    /// 電源ボタンで寝かされている間の終わり（UNIX秒）。
    pub forced_until: Option<u64>,
}

fn yes() -> bool {
    true
}

fn first_generation() -> u32 {
    1
}

/// k 番目のティックで状態を減らす間隔（1=毎回、4=4回に1回…）。おやすみ・留守番で間引く。
fn decay_divisor(timeline: Option<Timeline>, tick_index: u32, carried_secs: u32) -> u32 {
    let Some(t) = timeline else {
        return 1;
    };
    let offset = (u64::from(tick_index) + 1) * u64::from(TICK_SECS) - u64::from(carried_secs.min(TICK_SECS));
    let at = t.start_unix.saturating_add(offset);
    let mut div = 1;
    if t.away {
        div *= AWAY_DECAY_DIV;
    }
    if is_sleeping(at, t.utc_offset, t.sleep) || t.forced_until.is_some_and(|until| at < until) {
        div *= SLEEP_DECAY_DIV;
    }
    div
}

impl GameState {
    pub fn new(now_unix: u64) -> Self {
        let mut state = Self {
            status: Status::new(50, 0, 80, 0),
            growth: Growth::egg(),
            history: History::default(),
            form: Form { stage: 1, branch: Branch::Normal, personality: Personality::Calm },
            tick_acc: 0,
            last_unix: now_unix,
            intro_seen: false,
            hatch_shown: false,
            tick_count: 0,
            name: None,
            discovered: BTreeSet::new(),
            diary: Vec::new(),
            born_unix: now_unix,
            anniversaries: 0,
            care: Care::default(),
            dead: false,
            forced_sleep_until: None,
            strict: false,
            generation: 1,
            legacy: None,
        };
        state.record_form();
        state.remember(DiaryEvent::Born);
        state
    }

    /// 旅立ちの時か（最終段階になって 1 時間が経った）。見送るまで、そのまま一緒にいられる。
    pub fn graduating(&self) -> bool {
        !self.dead && self.growth.ready_to_graduate()
    }

    /// 旅立ちを見送り、次の世代の卵を迎える。図鑑と思い出は引き継ぎ、性格が少し受け継がれる。
    pub fn graduate(&mut self) -> Result<(), String> {
        if !self.graduating() {
            return Err("it is not time to say goodbye yet".to_string());
        }
        let farewell = DiaryEvent::Graduated { name: self.name.take(), personality: self.form.personality };
        self.remember(farewell);
        self.start_next_generation(Some(self.form.personality));
        Ok(())
    }

    /// 電源ボタンで寝かされている最中か。
    pub fn forced_asleep(&self) -> bool {
        self.forced_sleep_until.is_some_and(|until| until > self.last_unix)
    }

    /// 電源ボタン。起きているペットを、おやすみ時間と関係なく寝かせる（無理やりなので、ご機嫌が上がるか下がるかは
    /// `good` しだい）。寝かされている最中に押すと起こす。卵と、亡くなったペットには使えない。
    pub fn toggle_power(&mut self, good: bool) -> Result<(), String> {
        if self.dead || self.form.stage <= 1 {
            return Err("only a living pet that has hatched can be put to bed".to_string());
        }
        if self.forced_asleep() {
            self.forced_sleep_until = None;
            return Ok(());
        }
        self.forced_sleep_until = Some(self.last_unix.saturating_add(FORCED_SLEEP_SECS));
        self.status.put_to_bed(good);
        Ok(())
    }

    /// 亡くなった。時間が止まり、思い出に記録される。
    fn die(&mut self) {
        self.dead = true;
        let farewell = DiaryEvent::Died { name: self.name.clone(), personality: self.form.personality };
        self.remember(farewell);
    }

    /// 亡くなったペットを見送り、新しい卵を迎える。性格は受け継がない。
    pub fn bury(&mut self) -> Result<(), String> {
        if !self.dead {
            return Err("the pet is still alive".to_string());
        }
        self.name = None;
        self.start_next_generation(None);
        Ok(())
    }

    fn start_next_generation(&mut self, legacy: Option<Personality>) {
        self.dead = false;
        self.legacy = legacy;
        self.generation += 1;
        self.status = Status::new(50, 0, 80, 0);
        self.growth = Growth::egg();
        self.history = History::default();
        self.form = Form { stage: 1, branch: Branch::Normal, personality: Personality::Calm };
        self.tick_acc = 0;
        self.intro_seen = false;
        self.hatch_shown = false;
        self.born_unix = self.last_unix;
        self.anniversaries = 0;
        self.care = Care::default();
        self.record_form();
        self.remember(DiaryEvent::Born);
    }

    /// いっしょに丸 1 日・2 日…が経っていたら、記念日として思い出に残す（まとめて経ったときは最新の 1 件）。
    fn remember_anniversary(&mut self) {
        let days = u32::try_from(self.last_unix.saturating_sub(self.born_unix) / DAY_SECS).unwrap_or(u32::MAX);
        if days > self.anniversaries {
            self.anniversaries = days;
            self.remember(DiaryEvent::Anniversary { days });
        }
    }

    /// 思い出を記録する（時刻は最後に時間を進めた時刻）。
    fn remember(&mut self, event: DiaryEvent) {
        diary::record(&mut self.diary, self.last_unix, event);
    }

    /// 今の姿を図鑑に記録する（すでにあれば何もしない）。
    pub fn record_form(&mut self) {
        self.discovered.insert(form_key(&self.form));
    }

    /// ミニゲームの結果を反映する。普通に遊んだ効果に加えて、勝てばボーナス。卵は遊べない。
    pub fn play_minigame(&mut self, win: bool) -> Result<(), String> {
        if self.dead {
            return Err("the pet has passed away".to_string());
        }
        if self.form.stage <= 1 {
            return Err("an egg cannot play games".to_string());
        }
        self.apply(Action::Play);
        if win {
            self.status.win_bonus();
        }
        Ok(())
    }

    /// 名前を付ける（付け直しもできる）。
    pub fn set_name(&mut self, raw: &str) -> Result<(), String> {
        let name = validate_name(raw)?;
        self.name = Some(name.clone());
        self.remember(DiaryEvent::Named { name });
        Ok(())
    }

    /// 孵化したが、まだ演出を見せていない。
    pub fn hatching(&self) -> bool {
        self.form.stage >= 2 && !self.hatch_shown
    }

    /// 孵化の演出を見せたことを記録する（卵のままなら何もしない）。
    pub fn ack_hatch(&mut self) {
        if self.form.stage >= 2 {
            self.hatch_shown = true;
        }
    }

    pub fn apply(&mut self, action: Action) {
        if self.dead {
            return;
        }
        self.intro_seen = true;
        match action {
            Action::Feed => {
                self.status.feed();
                self.care.fed();
                self.history.feeds += 1;
            }
            Action::Snack => {
                self.status.snack();
                self.care.snacked();
                self.history.snacks += 1;
            }
            Action::Play if self.form.stage <= 1 => self.status.pet(),
            Action::Play => {
                self.status.play();
                self.care.played();
                self.history.plays += 1;
            }
            Action::Clean => {
                self.status.clean();
                self.history.cleans += 1;
            }
            Action::Scold if self.form.stage >= 2 => {
                self.care.scold(&mut self.status);
            }
            Action::Cure if self.form.stage >= 2 => {
                self.care.cure(&mut self.status);
            }
            Action::Scold | Action::Cure => {}
        }
        let was_petting = matches!(action, Action::Scold | Action::Cure) || (action == Action::Play && self.form.stage <= 1);
        let total = self.care_total();
        if !was_petting && diary::is_care_milestone(total) {
            self.remember(DiaryEvent::CareMilestone { count: total });
        }
    }

    /// マウスでなでられた。お世話の回数には数えず、オープニングも終わらせない。
    pub fn stroke(&mut self) {
        if self.dead {
            return;
        }
        self.status.stroke();
    }

    /// これまでのお世話の回数（ご飯・おやつ・遊ぶ・掃除。卵を撫でるのは含めない）。
    fn care_total(&self) -> u32 {
        let h = &self.history;
        h.feeds + h.snacks + h.plays + h.cleans
    }

    /// 経過秒を進め、ティックと進化を適用する。発生した進化の回数を返す。
    /// 昼間・在宅として扱う（おやすみ・留守番の間引きなし）。
    pub fn step(&mut self, secs: u32) -> u8 {
        self.step_in(secs, None)
    }

    /// `step` に、おやすみ・留守番の文脈を与えたもの。成長（進化）の時間は文脈に関係なく進む。
    pub fn step_in(&mut self, secs: u32, timeline: Option<Timeline>) -> u8 {
        if self.dead {
            return 0;
        }
        let carried = self.tick_acc;
        let acc = carried.saturating_add(secs);
        self.tick_acc = acc % TICK_SECS;
        for k in 0..(acc / TICK_SECS).min(MAX_CATCHUP_TICKS) {
            self.tick_count = self.tick_count.wrapping_add(1);
            if self.tick_count.is_multiple_of(decay_divisor(timeline, k, carried)) {
                self.status.tick();
                self.care.tick(&self.status, self.tick_count, self.form.stage >= 2);
                if self.care.sick {
                    self.status.mood = self.status.mood.saturating_sub(SICK_MOOD_LOSS);
                }
                if self.strict && self.care.is_fatal() {
                    self.die();
                    return 0;
                }
                // ほったらかしにすると、少しずつ親密度が下がる（世話を続けていれば下がらない）。
                if self.tick_count.is_multiple_of(AFFINITY_DECAY_TICKS) && self.status.is_neglected() {
                    self.status.lose_affinity();
                }
            }
        }
        self.evolve_for(secs)
    }

    fn evolve_for(&mut self, secs: u32) -> u8 {
        let mut evolutions = 0;
        let mut advanced = self.growth.advance(secs);
        while advanced {
            let previous_stage = self.growth.stage - 1;
            self.form = evolve(previous_stage, &self.status, &self.history, self.legacy, self.care.score_adjustment(), self.care.is_legendary());
            self.care.evolved();
            self.record_form();
            self.remember(if previous_stage <= 1 {
                DiaryEvent::Hatched
            } else {
                DiaryEvent::Evolved { stage: self.form.stage, branch: self.form.branch, personality: self.form.personality }
            });
            evolutions += 1;
            advanced = self.growth.advance(0);
        }
        if evolutions > 0 {
            self.intro_seen = true; // 卵が孵ったら「卵が届いたよ」は役目を終える
        }
        evolutions
    }

    /// 現在時刻まで進める（アプリ停止中・スリープ中の経過分も含む）。
    /// 時計が巻き戻っていた場合は何も進めず時刻だけ更新する。
    pub fn step_to(&mut self, now_unix: u64) -> u8 {
        self.step_to_scaled(now_unix, 1)
    }

    /// 経過時間を `scale` 倍して進める（動作確認用の時間短縮）。0 は 1 として扱う。UTC として扱う。
    pub fn step_to_scaled(&mut self, now_unix: u64, scale: u32) -> u8 {
        self.step_to_in(now_unix, scale, 0, SleepWindow::default())
    }

    /// 現在時刻まで進める。`utc_offset` はローカル時刻の UTC からの差（秒）。
    /// 実際の経過が `AWAY_THRESHOLD_SECS` を超えていたら留守番として減りを緩める。
    pub fn step_to_in(&mut self, now_unix: u64, scale: u32, utc_offset: i32, sleep: SleepWindow) -> u8 {
        let start_unix = self.last_unix;
        let elapsed = now_unix.saturating_sub(start_unix);
        self.last_unix = now_unix;
        if elapsed == 0 {
            return 0;
        }
        let secs = elapsed.saturating_mul(u64::from(scale.max(1)));
        let away = elapsed > AWAY_THRESHOLD_SECS;
        let evolutions = self.step_in(u32::try_from(secs).unwrap_or(u32::MAX), Some(Timeline { start_unix, utc_offset, away, sleep, forced_until: self.forced_sleep_until }));
        if self.forced_sleep_until.is_some_and(|until| until <= now_unix) {
            self.forced_sleep_until = None; // 自然に起きた
        }
        self.remember_anniversary();
        evolutions
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn from_json(json: &str) -> io::Result<Self> {
        let mut state: Self = serde_json::from_str(json).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        state.record_form(); // 図鑑のない古いセーブでも、今の姿は記録される
        if state.born_unix == 0 {
            state.born_unix = state.last_unix; // 起点のない古いセーブは、最後に時間を進めた時刻から数える
        }
        Ok(state)
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
    use crate::evolution::{secs_until_graduation, stage_secs, EGG_SECS, HOUR_SECS};
    const DAY: u32 = 24 * HOUR_SECS;
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

    fn kinds(s: &GameState) -> Vec<String> {
        s.diary
            .iter()
            .map(|e| serde_json::to_value(&e.event).unwrap()["kind"].as_str().unwrap().to_string())
            .collect()
    }

    fn anniversaries(s: &GameState) -> Vec<u32> {
        s.diary.iter().filter_map(|e| if let DiaryEvent::Anniversary { days } = e.event { Some(days) } else { None }).collect()
    }

    #[test]
    fn a_whole_day_together_is_remembered_once() {
        let mut s = new_state();
        s.step_to_in(1_000 + DAY_SECS - 1, 1, 0, SleepWindow::default());
        assert!(anniversaries(&s).is_empty());
        s.step_to_in(1_000 + DAY_SECS, 1, 0, SleepWindow::default());
        assert_eq!(anniversaries(&s), [1]);
        s.step_to_in(1_000 + DAY_SECS + 60, 1, 0, SleepWindow::default());
        assert_eq!(anniversaries(&s), [1]);
        s.step_to_in(1_000 + 2 * DAY_SECS, 1, 0, SleepWindow::default());
        assert_eq!(anniversaries(&s), [1, 2]);
    }

    #[test]
    fn days_skipped_while_the_app_was_closed_leave_one_entry_with_the_latest_count() {
        let mut s = new_state();
        s.step_to_in(1_000 + 3 * DAY_SECS + 5, 1, 0, SleepWindow::default());
        assert_eq!(anniversaries(&s), [3]);
    }

    #[test]
    fn the_next_pet_counts_its_own_days() {
        let mut s = elder_state();
        s.anniversaries = 4;
        s.graduate().unwrap();
        assert_eq!(s.anniversaries, 0);
    }

    #[test]
    fn a_new_game_starts_the_diary_with_the_arrival_of_the_egg() {
        let s = new_state();
        assert_eq!(s.diary, vec![DiaryEntry { at_unix: 1_000, event: DiaryEvent::Born }]);
        assert_eq!(s.born_unix, 1_000);
    }

    #[test]
    fn neglect_slowly_wears_affinity_down() {
        let mut s = new_state();
        s.status = Status::new(80, 50, 20, 0); // 空腹
        s.step(TICK_SECS * AFFINITY_DECAY_TICKS * 2);
        assert!(s.status.affinity < 50, "neglected for a while: {}", s.status.affinity);
        assert!(s.status.affinity >= 47);
    }

    #[test]
    fn a_pet_that_is_looked_after_keeps_its_affinity() {
        let mut s = new_state();
        s.status = Status::new(100, 50, 100, 0);
        s.step(TICK_SECS * AFFINITY_DECAY_TICKS);
        assert_eq!(s.status.affinity, 50);
    }

    #[test]
    fn hatching_is_remembered_once_and_is_not_called_an_evolution() {
        let mut s = new_state();
        s.step(EGG_SECS);
        assert_eq!(kinds(&s), ["born", "hatched"]);
    }

    #[test]
    fn each_evolution_is_remembered_with_the_new_form() {
        let mut s = new_state();
        s.step(EGG_SECS);
        s.step(stage_secs(2));
        let last = s.diary.last().unwrap();
        assert_eq!(last.event, DiaryEvent::Evolved { stage: 3, branch: s.form.branch, personality: s.form.personality });
    }

    #[test]
    fn evolutions_skipped_during_a_long_absence_are_all_remembered() {
        let mut s = new_state();
        s.step(EGG_SECS + 3 * stage_secs(2)); // 2〜4 段階はどれも 3 時間
        assert_eq!(kinds(&s), ["born", "hatched", "evolved", "evolved", "evolved"]);
        let stages: Vec<u8> = s
            .diary
            .iter()
            .filter_map(|e| match e.event {
                DiaryEvent::Evolved { stage, .. } => Some(stage),
                _ => None,
            })
            .collect();
        assert_eq!(stages, [3, 4, 5]);
    }

    #[test]
    fn memories_are_stamped_with_the_time_the_game_was_advanced_to() {
        let mut s = new_state();
        s.step_to_in(5_000 + u64::from(EGG_SECS), 1, 0, SleepWindow::default());
        assert_eq!(s.diary.last().unwrap().at_unix, 5_000 + u64::from(EGG_SECS));
    }

    #[test]
    fn naming_and_renaming_are_remembered_but_failures_are_not() {
        let mut s = new_state();
        s.set_name("ごま").unwrap();
        s.set_name("もち").unwrap();
        assert!(s.set_name("").is_err());
        let names: Vec<&str> = s
            .diary
            .iter()
            .filter_map(|e| match &e.event {
                DiaryEvent::Named { name } => Some(name.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(names, ["ごま", "もち"]);
    }

    #[test]
    fn the_tenth_care_action_is_a_milestone() {
        let mut s = new_state();
        s.step(EGG_SECS); // 孵化
        for i in 1..=12 {
            s.apply(if i % 2 == 0 { Action::Feed } else { Action::Clean });
        }
        let milestones: Vec<u32> = s
            .diary
            .iter()
            .filter_map(|e| match e.event {
                DiaryEvent::CareMilestone { count } => Some(count),
                _ => None,
            })
            .collect();
        assert_eq!(milestones, [10]);
    }

    #[test]
    fn petting_an_egg_does_not_count_as_care_for_milestones() {
        let mut s = new_state();
        for _ in 0..30 {
            s.apply(Action::Play); // 卵の間は撫でる
        }
        assert!(!kinds(&s).contains(&"careMilestone".to_string()));
    }

    #[test]
    fn the_diary_never_grows_beyond_its_limit() {
        let mut s = new_state();
        for _ in 0..(diary::DIARY_MAX + 30) {
            s.set_name("ごま").unwrap();
        }
        assert_eq!(s.diary.len(), diary::DIARY_MAX);
    }

    #[test]
    fn saves_from_before_the_diary_existed_start_counting_days_from_their_last_update() {
        let mut json = serde_json::to_value(new_state()).unwrap();
        json.as_object_mut().unwrap().remove("diary");
        json.as_object_mut().unwrap().remove("born_unix");
        let s = GameState::from_json(&json.to_string()).unwrap();
        assert!(s.diary.is_empty());
        assert_eq!(s.born_unix, s.last_unix);
    }

    #[test]
    fn the_diary_survives_a_save_round_trip() {
        let mut s = new_state();
        s.step(EGG_SECS + stage_secs(2));
        s.set_name("ごま").unwrap();
        let back = GameState::from_json(&s.to_json()).unwrap();
        assert_eq!((back.diary, back.born_unix), (s.diary, s.born_unix));
    }

    /// 最終段階になって旅立ちの時を迎えるまで育てた状態。
    fn elder_state() -> GameState {
        let mut s = new_state();
        s.step(secs_until_graduation());
        s
    }

    #[test]
    fn a_pet_is_ready_to_leave_an_hour_after_reaching_the_final_stage() {
        let mut s = new_state();
        s.step(secs_until_graduation() - 1);
        assert_eq!(s.form.stage, 10);
        assert!(!s.graduating());
        s.step(1);
        assert!(s.graduating());
    }

    #[test]
    fn nobody_leaves_before_their_time() {
        for mut young in [new_state(), hatched_state()] {
            let before = young.clone();
            assert!(!young.graduating());
            assert!(young.graduate().is_err());
            assert_eq!(young, before);
        }
    }

    #[test]
    fn saying_goodbye_brings_a_new_egg_and_a_new_generation() {
        let mut s = elder_state();
        s.set_name("ごま").unwrap();
        s.apply(Action::Feed);
        s.graduate().unwrap();
        assert_eq!(s.generation, 2);
        assert_eq!((s.form.stage, s.growth.stage, s.growth.elapsed_secs), (1, 1, 0));
        assert_eq!(s.name, None);
        assert_eq!(s.history, History::default());
        assert_eq!(s.status, Status::new(50, 0, 80, 0));
        assert!(!s.graduating());
        assert!(!s.hatching());
        assert!(!s.intro_seen); // 「卵が届いたよ」から始まる
        assert!(!s.hatch_shown);
        assert_eq!(s.born_unix, s.last_unix);
    }

    #[test]
    fn the_dex_and_the_diary_are_kept_for_the_next_generation() {
        let mut s = elder_state();
        let dex = s.discovered.clone();
        let memories = s.diary.len();
        s.graduate().unwrap();
        assert_eq!(s.discovered, dex);
        assert_eq!(s.diary.len(), memories + 2); // 旅立ち + 新しい卵
    }

    #[test]
    fn the_farewell_is_remembered_with_the_name_and_personality() {
        let mut s = elder_state();
        s.set_name("ごま").unwrap();
        let personality = s.form.personality;
        s.graduate().unwrap();
        let tail: Vec<&DiaryEvent> = s.diary.iter().rev().take(2).map(|e| &e.event).collect();
        assert_eq!(tail[0], &DiaryEvent::Born);
        assert_eq!(tail[1], &DiaryEvent::Graduated { name: Some("ごま".to_string()), personality });
    }

    #[test]
    fn an_unnamed_pet_leaves_without_a_name_in_the_diary() {
        let mut s = elder_state();
        s.graduate().unwrap();
        assert!(s.diary.iter().any(|e| matches!(e.event, DiaryEvent::Graduated { name: None, .. })));
    }

    #[test]
    fn the_next_pet_inherits_the_personality_when_nothing_else_decides_it() {
        let mut s = new_state();
        s.step(EGG_SECS);
        for _ in 0..3 {
            s.apply(Action::Clean); // 掃除好き（うさぎ）に育てる
        }
        s.step(secs_until_graduation());
        assert_eq!(s.form.personality, Personality::Tidy);
        s.graduate().unwrap();
        assert_eq!(s.legacy, Some(Personality::Tidy));
        s.step(EGG_SECS); // 何もしないまま孵化
        assert_eq!(s.form.personality, Personality::Tidy);
    }

    #[test]
    fn what_the_next_pet_does_overrides_the_inheritance() {
        let mut s = elder_state();
        s.graduate().unwrap();
        s.legacy = Some(Personality::Tidy);
        s.step(EGG_SECS);
        for _ in 0..4 {
            s.apply(Action::Snack);
        }
        s.step(stage_secs(2));
        assert_eq!(s.form.personality, Personality::Gluttonous);
    }

    #[test]
    fn generations_count_up_and_the_second_pet_can_leave_too() {
        let mut s = elder_state();
        s.graduate().unwrap();
        s.step(secs_until_graduation());
        assert!(s.graduating());
        s.graduate().unwrap();
        assert_eq!(s.generation, 3);
    }

    #[test]
    fn saves_from_before_generations_start_as_the_first_generation() {
        let mut json = serde_json::to_value(new_state()).unwrap();
        json.as_object_mut().unwrap().remove("generation");
        json.as_object_mut().unwrap().remove("legacy");
        let s = GameState::from_json(&json.to_string()).unwrap();
        assert_eq!((s.generation, s.legacy), (1, None));
    }

    #[test]
    fn the_generation_survives_a_save_round_trip() {
        let mut s = elder_state();
        s.graduate().unwrap();
        let back = GameState::from_json(&s.to_json()).unwrap();
        assert_eq!((back.generation, back.legacy), (2, s.legacy));
    }

    #[test]
    fn a_new_game_has_only_the_egg_in_the_dex() {
        assert_eq!(new_state().discovered, ["egg".to_string()].into_iter().collect());
    }

    #[test]
    fn every_form_the_pet_evolves_into_is_recorded() {
        let mut s = new_state();
        s.step(EGG_SECS);
        assert_eq!(s.discovered.len(), 2);
        assert!(s.discovered.contains(&form_key(&s.form)));
        s.step(stage_secs(2));
        assert_eq!(s.discovered.len(), 3);
        assert!(s.discovered.contains(&form_key(&s.form)));
    }

    #[test]
    fn forms_skipped_over_during_a_long_absence_are_recorded_too() {
        let mut s = new_state();
        s.step(EGG_SECS + 3 * stage_secs(2)); // 卵 → 2 → 3 → 4 → 5
        assert_eq!(s.form.stage, 5);
        assert_eq!(s.discovered.len(), 5); // 卵 + 段階2〜5
    }

    #[test]
    fn meeting_the_same_form_twice_counts_once() {
        let mut s = new_state();
        s.record_form();
        s.record_form();
        assert_eq!(s.discovered.len(), 1);
    }

    #[test]
    fn old_saves_start_the_dex_with_the_current_form() {
        let mut s = new_state();
        s.step(EGG_SECS + stage_secs(2)); // 段階 3
        let mut json = serde_json::to_value(&s).unwrap();
        json.as_object_mut().unwrap().remove("discovered");
        let loaded = GameState::from_json(&json.to_string()).unwrap();
        assert_eq!(loaded.discovered, [form_key(&loaded.form)].into_iter().collect());
    }

    #[test]
    fn the_dex_survives_a_save_round_trip() {
        let mut s = new_state();
        s.step(EGG_SECS + stage_secs(2));
        assert_eq!(GameState::from_json(&s.to_json()).unwrap().discovered, s.discovered);
    }

    #[test]
    fn a_new_pet_has_no_name() {
        assert_eq!(new_state().name, None);
    }

    #[test]
    fn names_are_trimmed() {
        assert_eq!(validate_name("  ごま \u{3000}"), Ok("ごま".to_string()));
    }

    #[test]
    fn names_may_contain_inner_spaces_and_any_script() {
        assert_eq!(validate_name("Mochi 2"), Ok("Mochi 2".to_string()));
        assert_eq!(validate_name("ぽっけ🐣"), Ok("ぽっけ🐣".to_string()));
    }

    #[test]
    fn empty_or_blank_names_are_rejected() {
        assert!(validate_name("").is_err());
        assert!(validate_name("   \u{3000} ").is_err());
    }

    #[test]
    fn names_are_limited_by_characters_not_bytes() {
        let ok: String = "あ".repeat(NAME_MAX_CHARS);
        assert_eq!(validate_name(&ok), Ok(ok.clone()));
        assert!(validate_name(&"あ".repeat(NAME_MAX_CHARS + 1)).is_err());
        assert!(validate_name(&"a".repeat(NAME_MAX_CHARS + 1)).is_err());
    }

    #[test]
    fn names_with_control_characters_are_rejected() {
        assert!(validate_name("ご\nま").is_err());
        assert!(validate_name("ご\tま").is_err());
        assert!(validate_name("\u{0}x").is_err());
    }

    #[test]
    fn set_name_stores_the_trimmed_name_and_can_rename() {
        let mut s = new_state();
        s.set_name("  ごま ").unwrap();
        assert_eq!(s.name.as_deref(), Some("ごま"));
        s.set_name("もち").unwrap();
        assert_eq!(s.name.as_deref(), Some("もち"));
    }

    #[test]
    fn set_name_rejects_bad_names_and_keeps_the_old_one() {
        let mut s = new_state();
        s.set_name("ごま").unwrap();
        assert!(s.set_name("").is_err());
        assert!(s.set_name(&"あ".repeat(99)).is_err());
        assert_eq!(s.name.as_deref(), Some("ごま"));
    }

    #[test]
    fn the_name_survives_a_save_round_trip_and_old_saves_have_none() {
        let mut s = new_state();
        s.set_name("ごま").unwrap();
        assert_eq!(GameState::from_json(&s.to_json()).unwrap().name.as_deref(), Some("ごま"));
        let mut json = serde_json::to_value(new_state()).unwrap();
        json.as_object_mut().unwrap().remove("name");
        assert_eq!(GameState::from_json(&json.to_string()).unwrap().name, None);
    }

    #[test]
    fn a_new_game_starts_with_the_intro_and_the_hatch_pending() {
        let s = new_state();
        assert_eq!((s.intro_seen, s.hatch_shown), (false, false));
        assert!(!s.hatching()); // まだ卵なので孵化はしていない
    }

    #[test]
    fn a_snack_is_recorded_separately_from_meals() {
        let mut s = new_state();
        s.status.hunger = 30; // 新しいペットは満腹気味(80)なので、おなかをすかせておく
        s.apply(Action::Snack);
        s.apply(Action::Snack);
        s.apply(Action::Feed);
        assert_eq!(s.history, History { feeds: 1, snacks: 2, ..History::default() });
        assert_eq!(s.status.affinity, 2 + 2 + 2); // おやつ2回 + ご飯1回
    }

    #[test]
    fn eating_snacks_while_full_makes_the_pet_grumpy() {
        let mut s = new_state();
        s.status.hunger = 100;
        let mood = s.status.mood;
        s.apply(Action::Snack);
        assert_eq!(s.status.mood, mood - 10);
        assert_eq!(s.history.snacks, 1); // 食べすぎでも回数には数える
    }

    #[test]
    fn many_snacks_make_a_gluttonous_pet() {
        let mut s = new_state();
        s.step(EGG_SECS); // 孵化
        for _ in 0..3 {
            s.apply(Action::Snack);
        }
        s.step(stage_secs(2));
        assert_eq!(s.form.personality, Personality::Gluttonous);
    }

    fn hatched_state() -> GameState {
        let mut s = new_state();
        s.step(EGG_SECS);
        s
    }

    #[test]
    fn eating_and_playing_move_the_weight() {
        let mut s = hatched_state();
        s.apply(Action::Feed);
        s.apply(Action::Snack);
        assert_eq!(s.care.weight, 62);
        s.apply(Action::Play);
        assert_eq!(s.care.weight, 59);
        s.play_minigame(false).unwrap();
        assert_eq!(s.care.weight, 56);
    }

    #[test]
    fn scolding_an_egg_does_nothing() {
        let mut s = new_state();
        let status = s.status;
        s.apply(Action::Scold);
        assert_eq!((s.status, s.care.discipline), (status, 0));
        assert_eq!(s.care_total(), 0);
    }

    #[test]
    fn a_content_pet_gets_selfish_and_a_scolding_teaches_it() {
        let mut s = hatched_state();
        s.status = Status::new(100, 50, 100, 0);
        s.tick_count = crate::care::SELFISH_EVERY_TICKS - 1;
        s.step(TICK_SECS);
        assert!(s.care.is_selfish());
        s.apply(Action::Scold);
        assert_eq!(s.care.discipline, 15);
        assert!(!s.care.is_selfish());
        assert_eq!(s.care_total(), 0, "scolding is not care");
    }

    #[test]
    fn a_shot_cures_a_sick_pet_and_does_nothing_to_an_egg() {
        let mut s = hatched_state();
        s.care.sick = true;
        s.apply(Action::Cure);
        assert!(!s.care.sick);
        assert_eq!(s.care_total(), 0, "treatment is not routine care");
        let mut egg = new_state();
        egg.care.sick = true;
        let status = egg.status;
        egg.apply(Action::Cure);
        assert_eq!((egg.care.sick, egg.status), (true, status));
    }

    #[test]
    fn being_sick_drags_the_mood_down_faster() {
        let mut well = hatched_state();
        well.status = Status::new(80, 50, 80, 0);
        let mut ill = well.clone();
        ill.care.sick = true;
        well.step(TICK_SECS * 5);
        ill.step(TICK_SECS * 5);
        assert_eq!(well.status.mood - ill.status.mood, 5);
    }

    #[test]
    fn a_long_neglect_ends_in_sickness() {
        let mut s = hatched_state();
        s.status = Status::new(80, 50, 20, 0);
        s.step(TICK_SECS * crate::care::MISS_TICKS * 3);
        assert!(s.care.sick);
    }

    #[test]
    fn a_neglected_pet_racks_up_care_misses() {
        let mut s = hatched_state();
        s.status = Status::new(80, 50, 20, 0);
        s.step(TICK_SECS * crate::care::MISS_TICKS * 2);
        assert!(s.care.misses >= 1);
        assert_eq!(s.care.misses, s.care.recent_misses);
    }

    #[test]
    fn evolution_counts_care_misses_since_the_last_evolution_only() {
        let mut s = hatched_state();
        s.care.recent_misses = 5;
        s.care.misses = 5;
        s.evolve_for(stage_secs(2));
        assert_eq!((s.care.misses, s.care.recent_misses), (5, 0));
    }

    #[test]
    fn good_discipline_and_an_ideal_weight_lift_the_evolution_but_an_unfit_pet_falls() {
        let evolve_with = |discipline: u8, weight: u8| {
            let mut s = hatched_state();
            s.status = Status::new(60, 60, 50, 0);
            s.care.discipline = discipline;
            s.care.weight = weight;
            s.evolve_for(stage_secs(2));
            s.form.branch
        };
        assert_eq!(evolve_with(0, 50), Branch::Normal);
        assert_eq!(evolve_with(100, 50), Branch::Good);
        assert_eq!(evolve_with(0, 90), Branch::Normal); // 60 - 10 = 50
        assert_eq!(evolve_with(0, 20), Branch::Normal);
    }

    #[test]
    fn a_pet_raised_with_great_care_evolves_into_the_hidden_form_at_the_last_stage() {
        let mut s = new_state();
        s.growth.stage = crate::evolution::MAX_STAGE - 1;
        s.form.stage = crate::evolution::MAX_STAGE - 1;
        s.status = Status::new(90, 90, 50, 0);
        s.care.discipline = 80;
        s.evolve_for(stage_secs(crate::evolution::MAX_STAGE - 1));
        assert_eq!((s.form.stage, s.form.branch), (crate::evolution::MAX_STAGE, Branch::Legend));
        assert!(s.discovered.contains("10-legend-calm"));
    }

    #[test]
    fn without_enough_discipline_the_same_pet_is_only_good() {
        let mut s = new_state();
        s.growth.stage = crate::evolution::MAX_STAGE - 1;
        s.form.stage = crate::evolution::MAX_STAGE - 1;
        s.status = Status::new(90, 90, 50, 0);
        s.care.discipline = 10;
        s.evolve_for(stage_secs(crate::evolution::MAX_STAGE - 1));
        assert_eq!(s.form.branch, Branch::Good);
    }

    /// 2 分ずつ `n` ティック進める（1 回の `step` で進められるティックには上限があるため）。
    fn run_ticks(s: &mut GameState, n: u32) {
        for _ in 0..n {
            s.step(TICK_SECS);
        }
    }

    fn doomed_state() -> GameState {
        let mut s = hatched_state();
        s.strict = true;
        s.care.sick = true;
        s
    }

    #[test]
    fn in_strict_mode_a_sick_pet_left_untreated_eventually_dies() {
        let mut s = doomed_state();
        run_ticks(&mut s, crate::care::FATAL_TICKS - 1);
        assert!(!s.dead);
        run_ticks(&mut s, 1);
        assert!(s.dead);
        assert_eq!(kinds(&s).last().map(String::as_str), Some("died"));
        assert!(!s.graduating());
    }

    #[test]
    fn without_strict_mode_the_pet_never_dies() {
        let mut s = doomed_state();
        s.strict = false;
        run_ticks(&mut s, crate::care::FATAL_TICKS * 2);
        assert!(!s.dead);
    }

    #[test]
    fn a_dead_pet_is_frozen_in_time_and_ignores_everything() {
        let mut s = doomed_state();
        run_ticks(&mut s, crate::care::FATAL_TICKS);
        let frozen = s.clone();
        assert_eq!(s.step(secs_until_graduation()), 0);
        s.apply(Action::Feed);
        s.stroke();
        assert!(s.play_minigame(true).is_err());
        assert_eq!(s, frozen);
    }

    #[test]
    fn dying_with_an_unnamed_pet_still_leaves_a_memory() {
        let mut s = doomed_state();
        s.name = Some("ごま".to_string());
        run_ticks(&mut s, crate::care::FATAL_TICKS);
        let died = s.diary.iter().rev().find_map(|e| if let DiaryEvent::Died { name, .. } = &e.event { Some(name.clone()) } else { None });
        assert_eq!(died, Some(Some("ごま".to_string())));
    }

    #[test]
    fn burying_starts_a_new_egg_without_inheriting_the_personality() {
        let mut s = doomed_state();
        s.form.personality = Personality::Tidy;
        s.name = Some("ごま".to_string());
        run_ticks(&mut s, crate::care::FATAL_TICKS);
        s.bury().unwrap();
        assert!(!s.dead);
        assert_eq!((s.generation, s.form.stage, s.name.clone(), s.legacy), (2, 1, None, None));
        assert_eq!(s.care, Care::default());
        assert_eq!(kinds(&s).last().map(String::as_str), Some("born"));
        assert!(s.strict, "the mode itself carries over");
    }

    #[test]
    fn only_a_dead_pet_can_be_buried() {
        let mut s = hatched_state();
        let before = s.clone();
        assert!(s.bury().is_err());
        assert_eq!(s, before);
    }

    #[test]
    fn the_strict_flag_is_not_saved_and_old_saves_are_alive() {
        let mut s = new_state();
        s.strict = true;
        let mut json: serde_json::Value = serde_json::from_str(&s.to_json()).unwrap();
        assert!(json.get("strict").is_none());
        json.as_object_mut().unwrap().remove("dead");
        let back: GameState = serde_json::from_value(json).unwrap();
        assert!(!back.dead && !back.strict);
    }

    #[test]
    fn balance_a_fully_neglected_pet_dies_in_strict_mode_but_careful_players_survive() {
        type Policy = Box<dyn FnMut(u32, &mut GameState)>;
        let strict_sim = |mut policy: Policy| {
            let mut s = new_state();
            s.strict = true;
            for minute in 0..(secs_until_graduation() / 60) {
                s.step(60);
                policy(minute, &mut s);
            }
            s
        };
        assert!(strict_sim(Box::new(|_, _| {})).dead);
        assert!(!strict_sim(Box::new(care_every(60))).dead);
        assert!(!strict_sim(Box::new(care_every(240))).dead);
    }

    #[test]
    fn putting_the_pet_to_bed_slows_the_decay_whatever_the_clock_says() {
        let mut awake = hatched_state();
        awake.status = Status::new(80, 50, 80, 0);
        let mut forced = awake.clone();
        forced.toggle_power(true).unwrap();
        forced.status.mood = 80; // 寝かしつけの効果は除いて比べる
        let start = awake.last_unix;
        awake.step_to_in(start + 4 * 3600, 1, 0, SleepWindow { start_hour: 5, end_hour: 5 });
        forced.step_to_in(start + 4 * 3600, 1, 0, SleepWindow { start_hour: 5, end_hour: 5 });
        let lost = |s: &GameState| 80 - s.status.hunger;
        assert!(lost(&forced) * 3 < lost(&awake), "asleep: {} awake: {}", lost(&forced), lost(&awake));
    }

    #[test]
    fn the_forced_sleep_ends_on_its_own_and_the_pet_can_be_woken_early() {
        let mut s = hatched_state();
        s.toggle_power(true).unwrap();
        assert!(s.forced_asleep());
        s.toggle_power(true).unwrap();
        assert!(!s.forced_asleep());
        s.toggle_power(false).unwrap();
        let end = s.forced_sleep_until.unwrap();
        s.step_to(end - 1);
        assert!(s.forced_asleep());
        s.step_to(end);
        assert!(s.forced_sleep_until.is_none());
    }

    #[test]
    fn only_a_living_hatched_pet_can_be_put_to_bed() {
        let mut egg = new_state();
        assert!(egg.toggle_power(true).is_err());
        let mut dead = doomed_state();
        run_ticks(&mut dead, crate::care::FATAL_TICKS);
        assert!(dead.toggle_power(true).is_err());
        assert!(dead.forced_sleep_until.is_none());
    }

    #[test]
    fn the_next_pet_starts_with_a_fresh_care_record() {
        let mut s = elder_state();
        s.care.misses = 9;
        s.care.discipline = 80;
        s.graduate().unwrap();
        assert_eq!(s.care, Care::default());
    }

    #[test]
    fn an_old_save_without_care_data_still_loads() {
        let s = new_state();
        let mut json: serde_json::Value = serde_json::from_str(&s.to_json()).unwrap();
        json.as_object_mut().unwrap().remove("care");
        let back: GameState = serde_json::from_value(json).unwrap();
        assert_eq!(back.care, Care::default());
    }

    #[test]
    fn balance_hourly_care_keeps_a_healthy_weight() {
        let s = simulate(care_every(60));
        assert!(s.care.weight_is_ideal(), "weight {}", s.care.weight);
    }

    #[test]
    fn balance_snack_spam_makes_the_pet_heavy() {
        let s = simulate(|m, s| {
            if m % 30 == 0 {
                s.apply(Action::Snack);
                s.apply(Action::Clean);
            }
        });
        assert!(!s.care.weight_is_ideal());
    }

    #[test]
    fn an_egg_cannot_play_the_game() {
        let mut s = new_state();
        let before = s.clone();
        assert!(s.play_minigame(true).is_err());
        assert_eq!(s, before);
    }

    #[test]
    fn losing_a_game_still_counts_as_playing() {
        let mut played = hatched_state();
        let mut lost = played.clone();
        played.apply(Action::Play);
        lost.play_minigame(false).unwrap();
        assert_eq!(lost.status, played.status);
        assert_eq!(lost.history.plays, 1);
    }

    #[test]
    fn winning_a_game_adds_the_bonus_to_playing() {
        let mut played = hatched_state();
        let mut won = played.clone();
        played.apply(Action::Play);
        won.play_minigame(true).unwrap();
        assert_eq!(won.status.mood, (played.status.mood + 10).min(100));
        assert_eq!(won.status.affinity, played.status.affinity + 3);
        assert_eq!(won.status.hunger, played.status.hunger);
        assert_eq!(won.history.plays, 1);
    }

    #[test]
    fn playing_the_game_ends_the_intro_and_counts_as_care() {
        let mut s = hatched_state();
        s.intro_seen = false;
        for _ in 0..10 {
            s.play_minigame(false).unwrap();
        }
        assert!(s.intro_seen);
        assert!(kinds(&s).contains(&"careMilestone".to_string()));
    }

    #[test]
    fn any_action_ends_the_intro() {
        for action in [Action::Feed, Action::Snack, Action::Play, Action::Clean] {
            let mut s = new_state();
            s.apply(action);
            assert!(s.intro_seen, "{action:?}");
        }
    }

    #[test]
    fn the_egg_hatches_after_ten_minutes_and_the_next_stage_takes_an_hour() {
        let mut s = new_state();
        assert_eq!(s.step(EGG_SECS - 1), 0);
        assert_eq!(s.step(1), 1);
        assert_eq!(s.form.stage, 2);
        assert_eq!(s.step(stage_secs(2) - 1), 0);
        assert_eq!(s.step(1), 1);
        assert_eq!(s.form.stage, 3);
    }

    #[test]
    fn hatching_ends_the_intro_too() {
        let mut s = new_state();
        s.step(EGG_SECS);
        assert!(s.intro_seen);
    }

    #[test]
    fn time_passing_alone_does_not_end_the_intro() {
        let mut s = new_state();
        s.step(EGG_SECS / 2);
        assert!(!s.intro_seen);
    }

    #[test]
    fn hatching_lasts_from_the_hatch_until_it_is_acknowledged() {
        let mut s = new_state();
        s.step(EGG_SECS);
        assert!(s.hatching());
        s.ack_hatch();
        assert!(!s.hatching());
        assert!(s.hatch_shown);
    }

    #[test]
    fn acknowledging_before_the_egg_hatches_does_nothing() {
        let mut s = new_state();
        s.ack_hatch();
        assert!(!s.hatch_shown);
        s.step(EGG_SECS);
        assert!(s.hatching());
    }

    #[test]
    fn later_evolutions_do_not_bring_the_hatch_back() {
        let mut s = new_state();
        s.step(EGG_SECS);
        s.ack_hatch();
        s.step(3 * DAY);
        assert!(!s.hatching());
    }

    #[test]
    fn saves_from_before_the_intro_existed_count_as_seen() {
        let mut json = serde_json::to_value(new_state()).unwrap();
        json.as_object_mut().unwrap().remove("intro_seen");
        json.as_object_mut().unwrap().remove("hatch_shown");
        let s = GameState::from_json(&json.to_string()).unwrap();
        assert_eq!((s.intro_seen, s.hatch_shown), (true, true));
        assert!(!s.hatching());
    }

    #[test]
    fn apply_updates_status_and_history() {
        let mut s = new_state();
        s.step(EGG_SECS); // 卵の間の「遊ぶ」は撫でる扱いなので、孵化させてから確認する。
        s.apply(Action::Feed);
        s.apply(Action::Play);
        s.apply(Action::Play);
        s.apply(Action::Clean);
        assert_eq!(s.history, History { feeds: 1, plays: 2, cleans: 1, snacks: 0 });
        assert_eq!(s.status.dirtiness, 0);
        assert!(s.status.affinity > 0);
    }

    #[test]
    fn play_on_an_egg_is_petting() {
        let mut s = new_state();
        s.apply(Action::Play);
        assert_eq!(s.status, Status::new(60, 3, 80, 0));
        assert_eq!(s.history.plays, 0);
    }

    #[test]
    fn play_after_hatching_is_real_play() {
        let mut s = new_state();
        s.step(EGG_SECS);
        let hunger = s.status.hunger;
        s.apply(Action::Play);
        assert_eq!(s.history.plays, 1);
        assert_eq!(s.status.hunger, hunger - 5);
    }

    // --- おやすみ・留守番 ---------------------------------------------------

    const H: u64 = 3600;
    const FOUR_HOURS: u32 = 4 * 3600; // 120 ティック

    fn timeline(start_hour: u64, away: bool) -> Option<Timeline> {
        Some(Timeline { start_unix: start_hour * H, utc_offset: 0, away, sleep: SleepWindow::default(), forced_until: None })
    }

    fn hunger_drop(timeline: Option<Timeline>, secs: u32) -> u8 {
        let mut s = new_state();
        s.status.hunger = 100;
        s.step_in(secs, timeline);
        100 - s.status.hunger
    }

    #[test]
    fn daytime_decay_is_unchanged() {
        assert_eq!(hunger_drop(timeline(12, false), FOUR_HOURS), 100); // 120 ティックで底まで減る
    }

    #[test]
    fn sleeping_slows_the_decay_to_a_quarter() {
        // 0時〜4時はずっとおやすみ: 120 ティックのうち 30 回だけ減る。
        assert_eq!(hunger_drop(timeline(0, false), FOUR_HOURS), 30);
    }

    #[test]
    fn being_away_slows_the_decay_to_a_quarter() {
        assert_eq!(hunger_drop(timeline(12, true), FOUR_HOURS), 30);
    }

    #[test]
    fn sleeping_while_away_multiplies_the_slowdown() {
        // 120 ティックのうち 16 の倍数は 7 回。
        assert_eq!(hunger_drop(timeline(0, true), FOUR_HOURS), 7);
    }

    #[test]
    fn decay_slows_down_only_once_the_pet_goes_to_sleep() {
        // 22時から2時間: 23:00 ちょうどの 30 回目からおやすみ。通常は 1〜29 回目の 29 回、
        // おやすみ中は 30〜60 回目のうち 4 の倍数 (32,36,…,60) の 8 回。
        assert_eq!(hunger_drop(timeline(22, false), 2 * 3600), 29 + 8);
    }

    #[test]
    fn growth_is_not_slowed_by_sleep_or_absence() {
        for tl in [timeline(0, false), timeline(12, true), timeline(0, true)] {
            let mut s = new_state();
            assert_eq!(s.step_in(EGG_SECS, tl), 1);
            assert_eq!(s.form.stage, 2);
        }
    }

    #[test]
    fn the_tick_counter_survives_across_calls() {
        // 1 ティックずつ 120 回進めても、まとめて進めたときと同じ回数だけ減る。
        let mut one_by_one = new_state();
        one_by_one.status.hunger = 100;
        for _ in 0..120 {
            one_by_one.step_in(TICK_SECS, timeline(0, false));
        }
        assert_eq!(100 - one_by_one.status.hunger, 30);
        assert_eq!(one_by_one.tick_count, 120);
    }

    #[test]
    fn step_to_in_treats_a_long_real_gap_as_being_away() {
        let mut s = GameState::new(12 * H); // 昼12時
        s.status.hunger = 100;
        s.step_to_in(12 * H + 4 * H, 1, 0, SleepWindow::default()); // 4時間の空白 → 留守番(減り1/4)
        assert_eq!(100 - s.status.hunger, 30);
    }

    #[test]
    fn step_to_in_does_not_treat_the_regular_loop_interval_as_away() {
        let mut s = GameState::new(12 * H);
        s.status.hunger = 100;
        for i in 1..=120u64 {
            s.step_to_in(12 * H + i * u64::from(TICK_SECS), 1, 0, SleepWindow::default()); // 2分おき（常駐中）
        }
        assert_eq!(100 - s.status.hunger, 100);
    }

    #[test]
    fn step_to_in_judges_being_away_by_real_time_not_scaled_time() {
        let mut s = GameState::new(12 * H);
        s.status.hunger = 100;
        s.step_to_in(12 * H + 10, 240, 0, SleepWindow::default()); // 実際は10秒。時間短縮で 40 分進むが留守番ではない
        assert_eq!(100 - s.status.hunger, 20); // 2400 秒 = 20 ティック
    }

    #[test]
    fn step_to_in_uses_the_local_time_zone_for_sleep() {
        // UTC 14:00 は JST 23:00 でおやすみ開始。4時間ぶん（120ティック）は 30 回だけ減る。
        let jst = 9 * 3600;
        let mut s = GameState::new(14 * H);
        s.status.hunger = 100;
        for i in 1..=120u64 {
            s.step_to_in(14 * H + i * u64::from(TICK_SECS), 1, jst, SleepWindow::default());
        }
        assert_eq!(100 - s.status.hunger, 30);
    }

    #[test]
    fn saves_without_a_tick_counter_still_load() {
        let mut json = serde_json::to_value(new_state()).unwrap();
        json.as_object_mut().unwrap().remove("tick_count");
        assert_eq!(GameState::from_json(&json.to_string()).unwrap().tick_count, 0);
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
        assert_eq!(s.step(EGG_SECS), 1);
        assert_eq!(s.growth.stage, 2);
        assert_eq!(s.form.stage, 2);
    }

    #[test]
    fn step_applies_personality_from_history_on_evolution() {
        let mut s = new_state();
        s.step(EGG_SECS); // 孵化
        for _ in 0..3 {
            s.apply(Action::Play);
        }
        s.step(stage_secs(2));
        assert_eq!(s.form.personality, Personality::Playful);
    }

    #[test]
    fn step_returns_multiple_evolutions_after_long_absence() {
        let mut s = new_state();
        assert_eq!(s.step(EGG_SECS + 2 * stage_secs(2)), 3);
        assert_eq!(s.growth.stage, 4);
        assert_eq!(s.form.stage, 4);
    }

    #[test]
    fn step_stops_evolving_at_final_stage() {
        let mut s = new_state();
        s.step(30 * DAY);
        assert_eq!(s.growth.stage, 10);
        assert_eq!(s.step(DAY), 0);
        assert_eq!(s.form.stage, 10);
    }

    #[test]
    fn step_to_advances_by_elapsed_seconds_and_updates_time() {
        let mut s = new_state();
        assert_eq!(s.step_to(1_000 + u64::from(EGG_SECS)), 1);
        assert_eq!(s.last_unix, 1_000 + u64::from(EGG_SECS));
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
        s.step(EGG_SECS + 10);
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

    /// ゲームバランス: 1分刻みで、卵が届いてから旅立ちの時まで（約 117 時間）遊んだときの最終状態。
    fn simulate(mut policy: impl FnMut(u32, &mut GameState)) -> GameState {
        let mut s = new_state();
        for minute in 0..(secs_until_graduation() / 60) {
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
            if s.care.sick {
                s.apply(Action::Cure);
            }
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
    fn balance_care_every_few_hours_gives_a_normal_pet() {
        // 4時間おきのお世話では機嫌は保てないが、親密度は積み上がる。ポッとしない「ふつう」の姿になる。
        let s = simulate(care_every(240));
        assert_eq!(s.form.stage, 10);
        assert_eq!(s.form.branch, Branch::Normal);
    }

    #[test]
    fn balance_hourly_care_raises_a_happy_pet() {
        // 長い期間では親密度が高くなるので、毎時間お世話をすれば最終的に「きらきら」の姿になる。
        let s = simulate(care_every(60));
        assert_eq!(s.form.branch, Branch::Good);
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
