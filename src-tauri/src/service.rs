//! Tauri から独立したアプリケーションサービス層。IPC 用 DTO と排他制御・永続化をまとめる。

use std::io;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

use serde::Serialize;

use crate::evolution::{Branch, Personality};
use crate::expression::{express, greeting, Bubble, Expression};
use crate::companion::{Awake, Observation};
use crate::debuglog::{describe_changes, format_stamp, param_summary, snapshot_line, DebugLog, SNAPSHOT_EVERY_SECS};
use crate::dex::{build_view, DexView};
use crate::diary::{self, DiaryView};
use crate::game::{Action, GameState, FORCED_SLEEP_SECS, WOKEN_AWAKE_SECS};
use crate::minigame::{is_win, Rng, Side};
use crate::genome::EggCode;
use crate::events::{todays_event, Event};
use crate::rhythm::{chronotype, learned_sleep_window};
use crate::seasons::{scene_for, season_of};
use crate::souvenirs::{self, SouvenirView};
use crate::system::SystemInfo;
use crate::sleep::{awake_run_between, is_sleeping, local_hour, local_month_day, local_utc_offset, SleepWindow};

impl FromStr for Action {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "feed" => Ok(Action::Feed),
            "snack" => Ok(Action::Snack),
            "play" => Ok(Action::Play),
            "clean" => Ok(Action::Clean),
            "scold" => Ok(Action::Scold),
            "cure" => Ok(Action::Cure),
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StateView {
    pub mood: u8,
    pub affinity: u8,
    pub hunger: u8,
    pub dirtiness: u8,
    /// 体重とお世話の質（しつけ度）。
    pub weight: u8,
    pub discipline: u8,
    /// 画面に出るうんちの数（汚れ度から。0〜4）。
    pub poops: u8,
    /// 病気か。
    pub sick: bool,
    /// 命が危ない（きびしいモードで、もう限界が近い）。
    pub danger: bool,
    /// 亡くなっている（きびしいモード）。新しい卵を迎えるまで操作できない。
    pub dead: bool,
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
    /// 電源ボタンで寝かされている（おやすみ時間とは関係なく）。もう一度押すと起こせる。
    pub forced_sleep: bool,
    /// おでかけ中（飼い主が席を離れていて、ペットは起きている）。画面にはいない。
    pub out: bool,
    /// 色あい（色相を回す角度。0 ならもとの色）と、飾り（絵文字）。親から受け継いだ見た目の特徴。
    pub tint_degrees: u16,
    pub charm: Option<&'static str>,
    /// 持ち帰ったばかりで、まだ見せていないおみやげ。
    pub gift: Vec<GiftItem>,
    /// 画面に飾っているおみやげ（絵文字）。
    pub displayed: Vec<&'static str>,
    /// いまの季節に合う背景（背景が「季節にまかせる」のとき、画面はこれを使う）。
    pub season_scene: &'static str,
    /// Mac の電池が少ない（電源につながっていないとき）。
    pub battery_low: bool,
    /// Mac が充電中。
    pub charging: bool,
    /// インターネットにつながっていない。
    pub offline: bool,
    /// Mac がダークモード。
    pub dark_mode: bool,
    /// 今日の季節のできごと（ない日は null）。
    pub event: Option<Event>,
    /// ペットの名前（未設定なら null）。
    pub name: Option<String>,
    /// 孵化の演出が終わったのに、まだ名前がない（名前を付けてもらう）。
    pub needs_name: bool,
    /// 旅立ちの時を迎えている（見送るまで一緒にいられる）。
    pub graduating: bool,
    /// 何代目か。
    pub generation: u32,
}

/// ミニゲームの結果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniGameReport {
    pub view: StateView,
    pub win: bool,
    /// ペットがおやつを隠していた手。
    pub answer: Side,
}

/// 持ち帰ったおみやげ 1 つ。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GiftItem {
    pub emoji: &'static str,
    pub name: &'static str,
    pub rarity: u8,
}

/// `tick` の結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tick {
    pub view: StateView,
    /// 今回の時間経過で起きた進化の回数。
    pub evolutions: u8,
    /// 卵が孵った（段階 1 → 2 になった）。
    pub hatched: bool,
    /// 今回の時間経過で、旅立ちの時を迎えた（それまでは迎えていなかった）。
    pub graduation_ready: bool,
}

/// メニューバーのアイコン横に出す目印。ポップアップを開かなくても状態がわかるようにする。
pub fn tray_title(view: &StateView) -> Option<&'static str> {
    if view.hatching {
        return Some("🐣");
    }
    if view.dead {
        return Some("🪦");
    }
    if view.graduating {
        return Some("👋");
    }
    if view.out {
        return Some("🎒");
    }
    match view.expression.bubble? {
        Bubble::Hungry => Some("🍙"),
        Bubble::Dirty => Some("🧹"),
        Bubble::Gloomy => Some("💧"),
        Bubble::Sleeping => Some("💤"),
        Bubble::Welcome => Some("🥚"),
        // あいさつはメニューバーには出さない（落ち着いて見えるように）。
        Bubble::Sick => Some("🤒"),
        Bubble::Dying => Some("🆘"),
        Bubble::Rest => Some("☕"),
        Bubble::BatteryLow => Some("🪫"),
        Bubble::Offline => Some("📡"),
        Bubble::Selfish => Some("💬"),
        Bubble::Happy | Bubble::Morning | Bubble::Noon | Bubble::Evening | Bubble::Drowsy | Bubble::WelcomeBack | Bubble::Event => None,
    }
}

/// 旅立ちの通知を出すか。ポップアップが開いていれば、お別れの画面で伝わる。
pub fn should_notify_graduation(ready: bool, popup_open: bool) -> bool {
    ready && !popup_open
}

/// 孵化の通知を出すか。ポップアップが開いていれば演出で伝わるので出さない。
pub fn should_notify_hatch(hatched: bool, popup_open: bool) -> bool {
    hatched && !popup_open
}

/// 操作の前の状態を持っておき、関数が終わって手放されるときに、前後の違いをデバッグログへ書く。
struct Trace<'a> {
    svc: &'a GameService,
    label: &'static str,
    before: Option<StateView>,
}

impl Drop for Trace<'_> {
    fn drop(&mut self) {
        if let Some(before) = self.before.take() {
            self.svc.log_change(self.label, &before);
        }
    }
}

/// 世話の名前（デバッグログ用）。
fn action_label(action: Action) -> &'static str {
    match action {
        Action::Feed => "ご飯を食べた",
        Action::Snack => "おやつを食べた",
        Action::Play => "遊んだ",
        Action::Clean => "掃除した",
        Action::Scold => "叱られた",
        Action::Cure => "治療を受けた",
    }
}

pub struct GameService {
    state: Mutex<GameState>,
    path: PathBuf,
    time_scale: u32,
    /// None ならこのマシンのローカルタイムゾーンを使う（テストでは固定値を指定する）。
    utc_offset: Option<i32>,
    /// おやすみ時間（設定ダイアログで変えられる）。
    sleep: Mutex<SleepWindow>,
    /// 生活リズムから学んだおやすみ時間（まだ学べていなければ None）。
    learned_sleep: Mutex<Option<SleepWindow>>,
    /// 学んだおやすみ時間を使うか（設定）。
    auto_sleep: AtomicBool,
    /// Mac の状態（電池・ネット・ダークモード）。反応する設定がオフなら、何も起きていない状態にしておく。
    system: Mutex<SystemInfo>,
    /// 季節のできごとを出すか（アプリでは常にオン。テストでは、1970-01-01（お正月）の時刻を使うので、既定はオフ）。
    events: AtomicBool,
    /// 飼い主のお誕生日（月, 日）。設定から与える。
    birthday: Mutex<Option<(u32, u32)>>,
    /// デバッグ用の詳細ログ（設定「デバッグログを残す」がオンのときだけ）。
    debug: Mutex<Option<DebugLog>>,
    debug_on: AtomicBool,
    /// 最後に状態の一覧をログに書いた時刻（UNIX秒）。
    last_snapshot: AtomicU64,
    /// ミニゲームの乱数。
    rng: Mutex<Rng>,
}

impl GameService {
    pub fn new(state: GameState, path: PathBuf) -> Self {
        Self { state: Mutex::new(state), path, time_scale: 1, utc_offset: None, sleep: Mutex::new(SleepWindow::default()), learned_sleep: Mutex::new(None), auto_sleep: AtomicBool::new(false), system: Mutex::new(SystemInfo::default()), events: AtomicBool::new(false), birthday: Mutex::new(None), debug: Mutex::new(None), debug_on: AtomicBool::new(false), last_snapshot: AtomicU64::new(0), rng: Mutex::new(Rng::new(time_seed())) }
    }

    /// 経過時間の倍率（動作確認用）。
    pub fn with_time_scale(mut self, scale: u32) -> Self {
        self.time_scale = scale;
        self
    }

    /// ミニゲームの乱数の種を固定する（テスト用）。
    pub fn with_seed(self, seed: u64) -> Self {
        *self.rng.lock().unwrap_or_else(|e| e.into_inner()) = Rng::new(seed);
        self
    }

    /// ミニゲーム「どっちの手？」。時間を進め、答えを決め、結果（当たり・外れ）を反映して保存する。
    pub fn play_minigame(&self, guess: Side, now_unix: u64) -> Result<MiniGameReport, String> {
        let _trace = self.trace("ミニゲームで遊んだ");
        let mut state = self.lock();
        let mut next = state.clone();
        next.step_to_in(now_unix, self.time_scale, self.offset_at(now_unix), self.sleep_window());
        let answer = self.rng.lock().unwrap_or_else(|e| e.into_inner()).next_side();
        let win = is_win(guess, answer);
        next.play_minigame(win)?;
        next.save(&self.path).map_err(|e| e.to_string())?;
        *state = next;
        Ok(MiniGameReport { view: self.view(&state), win, answer })
    }

    /// 図鑑（全 109 種と発見済みの印）。
    pub fn dex(&self) -> DexView {
        build_view(&self.lock().discovered)
    }

    /// 思い出（新しい順）といっしょの日数。
    pub fn diary(&self) -> DiaryView {
        let state = self.lock();
        let mut view = diary::build_view(&state.diary, state.born_unix, state.last_unix);
        view.together_minutes = state.companion.total_secs / 60;
        view.chronotype = chronotype(&state.companion.rhythm);
        view.lineage = state.lineage.iter().map(Into::into).collect();
        view.today = state.today;
        view
    }

    /// ローカル時刻の UTC オフセットを固定する（テスト用）。
    pub fn with_utc_offset(mut self, secs: i32) -> Self {
        self.utc_offset = Some(secs);
        self
    }

    /// おやすみ時間を切り替える。次の更新・表示からすぐ使われる。
    pub fn set_sleep_window(&self, window: SleepWindow) {
        *self.sleep.lock().unwrap_or_else(|e| e.into_inner()) = window;
    }

    /// Mac の状態を知らせる。
    pub fn set_system(&self, info: SystemInfo) {
        *self.system.lock().unwrap_or_else(|e| e.into_inner()) = info;
    }

    /// 季節のできごと（お正月・ハロウィンなど）を出すようにする。
    pub fn set_events(&self, on: bool) {
        self.events.store(on, Ordering::Relaxed);
    }

    /// デバッグログを始める／止める（`path` が Some のとき、そこへ書く）。
    pub fn set_debug_log(&self, path: Option<PathBuf>) {
        let now = self.lock().last_unix;
        let was_on = self.debug_on.load(Ordering::Relaxed);
        if was_on && path.is_none() {
            self.log_at(now, "起動", "デバッグログを止めた");
        }
        self.debug_on.store(path.is_some(), Ordering::Relaxed);
        *self.debug.lock().unwrap_or_else(|e| e.into_inner()) = path.map(DebugLog::new);
        if !was_on && self.debug_on.load(Ordering::Relaxed) {
            self.last_snapshot.store(now, Ordering::Relaxed);
            self.log_at(now, "起動", "デバッグログを始めた");
            let line = snapshot_line(&self.snapshot());
            self.log_at(now, "状態", &line);
        }
    }

    /// デバッグログに 1 行書く（ログがオフなら何もしない）。書けなくても、ゲームには影響させない。
    pub fn log_note(&self, tag: &str, text: &str) {
        if self.debug_on.load(Ordering::Relaxed) {
            let now = self.lock().last_unix;
            self.log_at(now, tag, text);
        }
    }

    fn log_at(&self, unix: u64, tag: &str, text: &str) {
        let guard = self.debug.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(log) = guard.as_ref() {
            let stamp = format_stamp(unix, self.offset_at(unix));
            if let Err(e) = log.write(&stamp, tag, text) {
                eprintln!("failed to write the debug log: {e}");
            }
        }
    }

    /// 変更を伴う操作の前後の状態を比べて、ログに書く準備（ログがオフなら、何もしない）。
    /// 返した値を、関数の頭で持っておくと、関数が終わるときに（ロックを手放したあとで）書く。
    fn trace(&self, label: &'static str) -> Trace<'_> {
        let before = self.debug_on.load(Ordering::Relaxed).then(|| self.snapshot());
        Trace { svc: self, label, before }
    }

    fn log_change(&self, label: &str, before: &StateView) {
        let after = self.snapshot();
        let now = self.lock().last_unix;
        if !label.is_empty() {
            let params = param_summary(before, &after);
            let text = if params.is_empty() { format!("{label}（数値の変化なし）") } else { format!("{label}  {params}") };
            self.log_at(now, "操作", &text);
        }
        for line in describe_changes(before, &after) {
            self.log_at(now, "変化", &line);
        }
        let last = self.last_snapshot.load(Ordering::Relaxed);
        if now >= last + SNAPSHOT_EVERY_SECS {
            self.last_snapshot.store(now, Ordering::Relaxed);
            self.log_at(now, "状態", &snapshot_line(&after));
        }
    }

    /// 飼い主のお誕生日を知らせる（その日、ペットがお祝いする）。
    pub fn set_birthday(&self, birthday: Option<(u32, u32)>) {
        *self.birthday.lock().unwrap_or_else(|e| e.into_inner()) = birthday;
    }

    fn birthday(&self) -> Option<(u32, u32)> {
        *self.birthday.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn system_info(&self) -> SystemInfo {
        *self.system.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// 学んだおやすみ時間を使うかどうか（設定）。
    pub fn set_auto_sleep(&self, on: bool) {
        self.auto_sleep.store(on, Ordering::Relaxed);
    }

    /// いま使うおやすみ時間: 自動で合わせる設定で、学べていれば学んだもの。そうでなければ設定のもの。
    fn sleep_window(&self) -> SleepWindow {
        if self.auto_sleep.load(Ordering::Relaxed)
            && let Some(learned) = *self.learned_sleep.lock().unwrap_or_else(|e| e.into_inner())
        {
            return learned;
        }
        *self.sleep.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Mac の操作を観測する（10 秒ごとに呼ぶ）。いっしょにいた時間・生活リズムを数え、休憩の声かけ・おかえりの材料を返す。
    /// 保存は、そのあとの `tick` がまとめて行う。
    pub fn observe_activity(&self, now_unix: u64, idle_secs: u64) -> Observation {
        let _trace = self.trace(""); // おでかけの始まり・終わり、おみやげなど、観測で起きた変化も書く
        let mut state = self.lock();
        let offset = self.offset_at(now_unix);
        let window = self.sleep_window();
        let hour = local_hour(now_unix, offset);
        // 席を離れている間にペットが眠っていた時間（おやすみ時間・電源ボタンで寝かせた間）は、おでかけにならない。
        let awake = Awake {
            now: (!is_sleeping(now_unix, offset, window) || state.awake_override_until.is_some_and(|until| until > now_unix)) && state.forced_sleep_until.is_none_or(|until| until <= now_unix),
            during_gap: state.companion.last_observed().map(|prev| {
                let forced = state.forced_sleep_until.map(|until| (until.saturating_sub(FORCED_SLEEP_SECS), until));
                let woken = state.awake_override_until.map(|until| (until.saturating_sub(WOKEN_AWAKE_SECS), until));
                awake_run_between(prev, now_unix, offset, window, forced, woken)
            }),
        };
        let was_away = state.companion.is_away();
        let gift_before = state.gift.len();
        let obs = state.observe_activity_with(now_unix, idle_secs, hour, awake);
        if obs.returned_after.is_some() {
            state.bring_back_in(obs.outing_secs, now_unix, season_of(local_month_day(now_unix, offset).0));
        }
        if self.debug_on.load(Ordering::Relaxed) {
            let pet = if awake.now { "起きている" } else { "眠っている" };
            if !was_away && state.companion.is_away() {
                self.log_at(now_unix, "観測", &format!("席を離れたと判断（無操作 {idle_secs}秒。ペットは{pet}）"));
            }
            if let Some(away) = obs.returned_after {
                let souvenirs = state.gift.len().saturating_sub(gift_before);
                self.log_at(now_unix, "観測", &format!("飼い主が戻った（離席 {away}秒。眠るまでに起きていた時間 {}秒 → おみやげ {souvenirs}個）", obs.outing_secs));
            }
            if obs.break_due {
                self.log_at(now_unix, "観測", &format!("休憩の声かけの時間（休まず作業 {}分）", state.companion.session_secs / 60));
            }
            if obs.affinity_gain > 0 {
                self.log_at(now_unix, "観測", &format!("作業に寄り添って親密度 +{}", obs.affinity_gain));
            }
        }
        *self.learned_sleep.lock().unwrap_or_else(|e| e.into_inner()) = learned_sleep_window(&state.companion.rhythm);
        obs
    }

    fn offset_at(&self, unix_secs: u64) -> i32 {
        self.utc_offset.unwrap_or_else(|| local_utc_offset(unix_secs))
    }

    fn view(&self, state: &GameState) -> StateView {
        view_of(state, self.offset_at(state.last_unix), self.sleep_window(), self.system_info(), self.events.load(Ordering::Relaxed), self.birthday())
    }

    pub fn snapshot(&self) -> StateView {
        self.view(&self.lock())
    }

    /// 現在時刻まで進めたうえでアクションを適用し、保存する。
    pub fn perform(&self, action: Action, now_unix: u64) -> io::Result<StateView> {
        let _trace = self.trace(action_label(action));
        let mut state = self.lock();
        state.step_to_in(now_unix, self.time_scale, self.offset_at(now_unix), self.sleep_window());
        state.apply(action);
        state.save(&self.path)?;
        Ok(self.view(&state))
    }

    /// 電源ボタン。おやすみ時間とは関係なく寝かせる（ご機嫌が上がるか下がるかはランダム）。寝ている最中に押すと起こす
    /// （おやすみ時間で眠っているのを起こすと、ご機嫌が大きく下がり、親密度も少し下がる）。
    pub fn toggle_power(&self, now_unix: u64) -> Result<StateView, String> {
        let _trace = self.trace("電源ボタンを押した");
        let mut state = self.lock();
        let mut next = state.clone();
        next.step_to_in(now_unix, self.time_scale, self.offset_at(now_unix), self.sleep_window());
        let scheduled = is_sleeping(next.last_unix, self.offset_at(next.last_unix), self.sleep_window());
        let good = self.rng.lock().unwrap_or_else(|e| e.into_inner()).next_bool();
        next.toggle_power_in(good, scheduled)?;
        next.save(&self.path).map_err(|e| e.to_string())?;
        *state = next;
        Ok(self.view(&state))
    }

    /// マウスでなでられた。おやすみ中は起こさないので何も変わらない。
    pub fn stroke(&self, now_unix: u64) -> io::Result<StateView> {
        let _trace = self.trace("なでられた");
        let mut state = self.lock();
        state.step_to_in(now_unix, self.time_scale, self.offset_at(now_unix), self.sleep_window());
        if !self.view(&state).sleeping {
            state.stroke();
            state.save(&self.path)?;
        }
        Ok(self.view(&state))
    }

    /// 名前を付けて（または付け直して）保存する。
    pub fn set_name(&self, raw: &str) -> Result<StateView, String> {
        let _trace = self.trace("名前をつけた");
        let mut state = self.lock();
        let mut next = state.clone();
        next.set_name(raw)?;
        next.save(&self.path).map_err(|e| e.to_string())?;
        *state = next;
        Ok(self.view(&state))
    }

    /// 旅立ちを見送り、次の世代の卵を迎える。
    pub fn graduate(&self) -> Result<StateView, String> {
        let _trace = self.trace("旅立ちを見送った");
        let mut state = self.lock();
        let mut next = state.clone();
        next.graduate()?;
        next.save(&self.path).map_err(|e| e.to_string())?;
        *state = next;
        Ok(self.view(&state))
    }

    /// 旅立った親の性格を子が受け継ぐ強さ（加点）。設定から与える。
    pub fn set_legacy_bonus(&self, bonus: u32) {
        self.lock().legacy_bonus = bonus;
    }

    /// きびしいモード（お別れがある）か。設定から与える。
    pub fn set_strict(&self, strict: bool) {
        self.lock().strict = strict;
    }

    /// 最初からやり直す（隠しコマンド）。今のペットとお別れして、新しい卵を迎える。
    pub fn restart(&self) -> Result<StateView, String> {
        let _trace = self.trace("最初からやり直した");
        let mut state = self.lock();
        let mut next = state.clone();
        next.restart();
        next.save(&self.path).map_err(|e| e.to_string())?;
        *state = next;
        Ok(self.view(&state))
    }

    /// 亡くなったペットを見送り、新しい卵を迎える。
    pub fn bury(&self) -> Result<StateView, String> {
        let _trace = self.trace("お墓から新しい卵を迎えた");
        let mut state = self.lock();
        let mut next = state.clone();
        next.bury()?;
        next.save(&self.path).map_err(|e| e.to_string())?;
        *state = next;
        Ok(self.view(&state))
    }

    /// いまのペットの血筋の「たまごコード」。
    pub fn egg_code(&self) -> String {
        self.lock().egg_code().encode()
    }

    /// 友だちのたまごコードのたまごを迎える。いまのペットとはお別れになる。コードが正しくなければ、何も変えない。
    pub fn receive_egg(&self, text: &str) -> Result<StateView, String> {
        let _trace = self.trace("友だちのたまごを迎えた");
        let code = EggCode::decode(text)?;
        let mut state = self.lock();
        let mut next = state.clone();
        next.receive_egg(code);
        next.save(&self.path).map_err(|e| e.to_string())?;
        *state = next;
        Ok(self.view(&state))
    }

    /// 持ち帰ったおみやげを見せたことを記録して保存する。
    pub fn ack_gift(&self) -> io::Result<StateView> {
        let _trace = self.trace("おみやげを受け取った");
        let mut state = self.lock();
        state.ack_gift();
        state.save(&self.path)?;
        Ok(self.view(&state))
    }

    /// おみやげのコレクション。
    pub fn souvenirs(&self) -> SouvenirView {
        souvenirs::build_view(&self.lock().souvenirs)
    }

    /// おみやげを飾る／しまう。
    pub fn toggle_souvenir_display(&self, id: &str) -> Result<SouvenirView, String> {
        let _trace = self.trace("おみやげの飾りを変えた");
        let mut state = self.lock();
        let mut next = state.clone();
        next.souvenirs.toggle_display(id)?;
        next.save(&self.path).map_err(|e| e.to_string())?;
        *state = next;
        Ok(souvenirs::build_view(&state.souvenirs))
    }

    /// 孵化の演出を見せたことを記録して保存する。
    pub fn ack_hatch(&self) -> io::Result<StateView> {
        let _trace = self.trace("孵化の演出を見せた");
        let mut state = self.lock();
        state.ack_hatch();
        state.save(&self.path)?;
        Ok(self.view(&state))
    }

    /// 時間を進めて保存する。表示用スナップショットと進化回数を返す。
    pub fn tick(&self, now_unix: u64) -> io::Result<Tick> {
        let _trace = self.trace("");
        let mut state = self.lock();
        let was_egg = state.form.stage <= 1;
        let was_graduating = state.graduating();
        let evolutions = state.step_to_in(now_unix, self.time_scale, self.offset_at(now_unix), self.sleep_window());
        state.save(&self.path)?;
        Ok(Tick { view: self.view(&state), evolutions, hatched: was_egg && evolutions > 0, graduation_ready: !was_graduating && state.graduating() })
    }

    /// パニックしたスレッドがロックを汚染しても状態自体は有効なので続行する。
    fn lock(&self) -> MutexGuard<'_, GameState> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// 乱数の種に使う、いまの時刻（ナノ秒）。
fn time_seed() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64)
}

/// 汚れ度から、画面に出すうんちの数を決める（25 たまるごとに 1 つ、最大 4 つ）。卵はうんちをしない。
fn poops(stage: u8, dirtiness: u8) -> u8 {
    if stage <= 1 {
        return 0;
    }
    (dirtiness / 25).min(4)
}

/// 今日（ローカル時刻）の季節のできごと。
fn event_for_today(s: &GameState, utc_offset: i32, birthday: Option<(u32, u32)>) -> Option<Event> {
    let (month, day) = local_month_day(s.last_unix, utc_offset);
    todays_event(month, day, birthday, s.days_since_start(utc_offset))
}

fn view_of(s: &GameState, utc_offset: i32, window: SleepWindow, system: SystemInfo, events: bool, birthday: Option<(u32, u32)>) -> StateView {
    let forced_sleep = s.forced_asleep();
    let sleeping = forced_sleep || (is_sleeping(s.last_unix, utc_offset, window) && !s.woken_by_force());
    let mut expression = express(&s.status);
    if sleeping {
        expression.bubble = Some(Bubble::Sleeping);
    }
    // 世話が必要なとき以外は、時間帯のあいさつ（卵・おやすみ中は言わない）。
    if !sleeping && s.form.stage >= 2 && s.care.sick {
        expression.bubble = Some(if s.strict && s.care.in_danger() { Bubble::Dying } else { Bubble::Sick });
    }
    if s.dead {
        expression.bubble = None;
    }
    if !sleeping && s.form.stage >= 2 && s.care.is_selfish() && matches!(expression.bubble, None | Some(Bubble::Happy)) {
        expression.bubble = Some(Bubble::Selfish);
    }
    // 飼い主を気づかう一言（世話が必要なときは言わない）。休憩 > おかえり。
    if !sleeping && s.form.stage >= 2 && matches!(expression.bubble, None | Some(Bubble::Happy)) {
        if s.companion.needs_a_break() {
            expression.bubble = Some(Bubble::Rest);
        } else if s.companion.is_welcoming_back(s.last_unix) {
            expression.bubble = Some(Bubble::WelcomeBack);
        } else if system.battery_low() {
            expression.bubble = Some(Bubble::BatteryLow);
        } else if !system.online {
            expression.bubble = Some(Bubble::Offline);
        }
    }
    let event = events.then(|| event_for_today(s, utc_offset, birthday)).flatten();
    if !sleeping && s.form.stage >= 2 && event.is_some() && matches!(expression.bubble, None | Some(Bubble::Happy)) {
        expression.bubble = Some(Bubble::Event);
    }
    if !sleeping && s.form.stage >= 2 && expression.bubble.is_none() {
        expression.bubble = greeting(local_hour(s.last_unix, utc_offset), window);
    }
    if !s.intro_seen {
        expression.bubble = Some(Bubble::Welcome);
    }
    StateView {
        mood: s.status.mood,
        affinity: s.status.affinity,
        hunger: s.status.hunger,
        dirtiness: s.status.dirtiness,
        weight: s.care.weight,
        discipline: s.care.discipline,
        poops: poops(s.form.stage, s.status.dirtiness),
        sick: s.care.sick,
        danger: s.strict && !s.dead && s.care.in_danger(),
        dead: s.dead,
        stage: s.form.stage,
        branch: s.form.branch,
        personality: s.form.personality,
        expression,
        intro: !s.intro_seen,
        hatching: s.hatching(),
        sleeping,
        forced_sleep,
        out: s.companion.is_away() && s.form.stage >= 2 && !sleeping && !s.dead && !s.hatching() && !s.graduating() && !(s.form.stage >= 2 && s.hatch_shown && s.name.is_none()),
        tint_degrees: s.genome.tint_degrees(),
        charm: s.genome.charm_emoji(),
        gift: s.gift.iter().filter_map(|id| souvenirs::find(id)).map(|x| GiftItem { emoji: x.emoji, name: x.name, rarity: x.rarity }).collect(),
        displayed: s.souvenirs.displayed.iter().filter_map(|id| souvenirs::find(id)).map(|x| x.emoji).collect(),
        season_scene: scene_for(season_of(local_month_day(s.last_unix, utc_offset).0)),
        battery_low: system.battery_low(),
        charging: system.charging(),
        offline: !system.online,
        dark_mode: system.dark_mode,
        event,
        name: s.name.clone(),
        needs_name: s.form.stage >= 2 && s.hatch_shown && s.name.is_none(),
        graduating: s.graduating(),
        generation: s.generation,
        play_action: if s.form.stage <= 1 { PlayAction::Pet } else { PlayAction::Play },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evolution::{secs_until_graduation, stage_secs, EGG_SECS};
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

    /// UTC+15 に固定: UNIX 秒 0 が昼の 15:00 になり、おやすみ時間でも、あいさつの時間帯でもない。
    const NOON_AT_ZERO: i32 = 15 * 3600;

    fn svc(name: &str) -> GameService {
        GameService::new(GameState::new(0), tmp(name)).with_utc_offset(NOON_AT_ZERO)
    }

    #[test]
    fn action_parses_known_names() {
        assert_eq!("feed".parse::<Action>(), Ok(Action::Feed));
        assert_eq!("snack".parse::<Action>(), Ok(Action::Snack));
        assert_eq!("play".parse::<Action>(), Ok(Action::Play));
        assert_eq!("clean".parse::<Action>(), Ok(Action::Clean));
        assert_eq!("scold".parse::<Action>(), Ok(Action::Scold));
        assert_eq!("cure".parse::<Action>(), Ok(Action::Cure));
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
                weight: 50,
                discipline: 0,
                poops: 0,
                sick: false,
                danger: false,
                dead: false,
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
                forced_sleep: false,
                out: false,
                tint_degrees: 0,
                charm: None,
                gift: Vec::new(),
                displayed: Vec::new(),
                season_scene: "snow", // UTC+15 の 1970-01-01 は冬
                battery_low: false,
                charging: false,
                offline: false,
                dark_mode: false,
                event: None,
                name: None,
                needs_name: false,
                graduating: false,
                generation: 1,
            }
        );
    }

    #[test]
    fn view_reflects_expression_after_actions() {
        let s = svc("expr");
        s.perform(Action::Clean, 0).unwrap(); // 初回オープニングを終える
        // 常駐中（2分おき）に 60 ティック進むと、空腹になって吹き出しが変わる（まだ病気にはならない）。
        let mut v = s.snapshot();
        for i in 1..=60u64 {
            v = s.tick(i * u64::from(TICK_SECS)).unwrap().view;
        }
        assert_eq!(v.expression.hunger, Level::Low);
        assert_eq!(v.expression.bubble, Some(Bubble::Hungry));
    }

    #[test]
    fn a_hatched_pet_greets_by_the_time_of_day_when_nothing_else_needs_saying() {
        let mut state = GameState::new(0);
        state.step(crate::evolution::EGG_SECS); // 孵化
        state.ack_hatch();
        let at = |hour: i32| GameService::new(state.clone(), tmp(&format!("greet-{hour}"))).with_utc_offset(hour * 3600).snapshot().expression.bubble;
        assert_eq!(at(8), Some(Bubble::Morning)); // 秒 0 + 孵化の経過で、ほぼ 8 時
        assert_eq!(at(15), None);
    }

    #[test]
    fn a_need_comes_before_the_greeting_and_an_egg_does_not_greet() {
        let mut hungry = GameState::new(0);
        hungry.step(crate::evolution::EGG_SECS);
        hungry.ack_hatch();
        hungry.status = crate::status::Status::new(50, 50, 0, 0);
        let v = GameService::new(hungry, tmp("greet-hungry")).with_utc_offset(8 * 3600).snapshot();
        assert_eq!(v.expression.bubble, Some(Bubble::Hungry));
        let mut egg = GameState::new(0);
        egg.intro_seen = true;
        let v = GameService::new(egg, tmp("greet-egg")).with_utc_offset(8 * 3600).snapshot();
        assert_eq!(v.expression.bubble, None);
    }

    fn selfish_pet(name: &str) -> GameService {
        let mut state = GameState::new(0);
        state.step(crate::evolution::EGG_SECS);
        state.ack_hatch();
        state.intro_seen = true;
        state.status = crate::status::Status::new(100, 50, 100, 0);
        state.tick_count = crate::care::SELFISH_EVERY_TICKS - 1;
        state.step(crate::game::TICK_SECS);
        GameService::new(state, tmp(name)).with_utc_offset(NOON_AT_ZERO)
    }

    fn grown_pet(name: &str) -> GameService {
        let mut state = GameState::new(0);
        state.step(crate::evolution::EGG_SECS);
        state.ack_hatch();
        state.intro_seen = true;
        state.status = crate::status::Status::new(50, 50, 80, 0);
        GameService::new(state, tmp(name)).with_utc_offset(NOON_AT_ZERO)
    }

    #[test]
    fn the_power_button_puts_an_awake_pet_to_sleep_whatever_the_time_and_wakes_it_again() {
        let s = grown_pet("power");
        assert!(!s.snapshot().sleeping);
        let v = s.toggle_power(700).unwrap();
        assert!(v.sleeping && v.forced_sleep);
        assert_eq!(v.expression.bubble, Some(Bubble::Sleeping));
        let v = s.toggle_power(710).unwrap();
        assert!(!v.sleeping && !v.forced_sleep);
    }

    #[test]
    fn the_mood_after_being_put_to_bed_is_random_and_goes_both_ways() {
        let mut moods = std::collections::BTreeSet::new();
        for seed in 0..32 {
            let s = grown_pet(&format!("power-rng-{seed}")).with_seed(seed);
            let before = s.snapshot().mood;
            let after = s.toggle_power(700).unwrap().mood;
            // 寝かしつけの ±15 に、数ティックぶんの自然な減りが重なる
            assert!(after >= before + 9 || after + 15 <= before + 2, "{before} -> {after}");
            moods.insert(after > before);
        }
        assert_eq!(moods.len(), 2, "both outcomes happen");
    }

    #[test]
    fn the_pet_wakes_up_by_itself_after_a_long_sleep() {
        let s = grown_pet("power-auto").with_utc_offset(7 * 3600); // 朝 7 時台に寝かせると、8 時間後も昼
        s.toggle_power(700).unwrap();
        let v = s.tick(700 + crate::game::FORCED_SLEEP_SECS - 1).unwrap().view;
        assert!(v.forced_sleep);
        let v = s.tick(700 + crate::game::FORCED_SLEEP_SECS).unwrap().view;
        assert!(!v.forced_sleep && !v.sleeping);
    }

    /// おやすみ時間（UTC の 0 時台）に、孵化した、名前のあるペット。
    fn pet_asleep_for_the_night(name: &str) -> GameService {
        let s = GameService::new(GameState::new(0), tmp(name)).with_utc_offset(0);
        {
            let mut state = s.lock();
            state.step(crate::evolution::EGG_SECS);
            state.ack_hatch();
            state.status = crate::status::Status::new(80, 50, 80, 0);
        }
        s
    }

    #[test]
    fn the_power_button_wakes_a_pet_that_sleeps_for_the_night_but_it_wakes_up_grumpy() {
        let s = pet_asleep_for_the_night("wake-night");
        let before = s.snapshot();
        assert!(before.sleeping && !before.forced_sleep);
        let v = s.toggle_power(700).unwrap();
        assert!(!v.sleeping, "woken up by force");
        assert!(v.mood + 15 <= before.mood, "{} -> {}", before.mood, v.mood);
        assert_eq!(v.affinity, before.affinity - 3);
        assert_eq!(v.expression.bubble, None, "not the Zzz bubble any more");
    }

    #[test]
    fn a_woken_pet_stays_up_for_two_hours_and_then_falls_asleep_again_if_it_is_still_night() {
        let s = pet_asleep_for_the_night("wake-night-again");
        s.toggle_power(700).unwrap();
        assert!(!s.tick(700 + crate::game::WOKEN_AWAKE_SECS - 10).unwrap().view.sleeping);
        assert!(s.tick(700 + crate::game::WOKEN_AWAKE_SECS + 10).unwrap().view.sleeping, "back to sleep: it is still 2 am");
    }

    #[test]
    fn pressing_the_power_button_again_after_waking_the_pet_puts_it_to_sleep() {
        let s = pet_asleep_for_the_night("wake-then-sleep");
        s.toggle_power(700).unwrap();
        let v = s.toggle_power(800).unwrap();
        assert!(v.sleeping && v.forced_sleep);
    }

    #[test]
    fn waking_a_pet_that_was_put_to_bed_in_the_daytime_costs_nothing_but_at_night_it_does() {
        let day = grown_pet("wake-day");
        day.toggle_power(700).unwrap();
        let before = day.snapshot();
        let v = day.toggle_power(710).unwrap();
        assert!(!v.sleeping);
        assert_eq!((v.affinity, v.forced_sleep), (before.affinity, false));
        let night = pet_asleep_for_the_night("wake-forced-night");
        night.toggle_power(700).unwrap(); // 起こす
        night.toggle_power(710).unwrap(); // 寝かせる
        let before = night.snapshot();
        let v = night.toggle_power(720).unwrap(); // もう一度起こす（夜なので、無理やり）
        assert!(!v.sleeping);
        assert_eq!(v.affinity, before.affinity - 3);
    }

    #[test]
    fn the_power_button_is_refused_for_an_egg_even_at_night() {
        let s = GameService::new(GameState::new(0), tmp("wake-egg")).with_utc_offset(0);
        assert!(s.toggle_power(10).is_err());
    }

    #[test]
    fn the_power_button_is_refused_for_an_egg() {
        let s = svc("power-egg");
        assert!(s.toggle_power(10).is_err());
        assert!(!s.snapshot().forced_sleep);
    }

    #[test]
    fn the_power_button_returns_an_error_when_saving_fails() {
        let bad = tmp("power-bad").parent().unwrap().join("no/such/dir/save.json");
        let mut state = GameState::new(0);
        state.step(crate::evolution::EGG_SECS);
        let s = GameService::new(state, bad).with_utc_offset(NOON_AT_ZERO);
        assert!(s.toggle_power(700).is_err());
        assert!(!s.snapshot().forced_sleep, "the pet stays as it was");
    }

    /// 昼間（UTC+15 の 15 時）に、`secs` 秒ぶん操作し続ける。
    fn work_for(s: &GameService, start: u64, secs: u64) -> u64 {
        let mut now = start;
        s.observe_activity(now, 0);
        for _ in 0..secs / 10 {
            now += 10;
            s.observe_activity(now, 0);
        }
        now
    }

    #[test]
    fn after_working_a_long_time_the_pet_asks_the_owner_to_rest() {
        let s = grown_pet("rest");
        assert_ne!(s.snapshot().expression.bubble, Some(Bubble::Rest));
        work_for(&s, 1_000, crate::companion::BREAK_AFTER_SECS + 60);
        assert_eq!(s.snapshot().expression.bubble, Some(Bubble::Rest));
    }

    #[test]
    fn a_need_comes_before_the_rest_suggestion() {
        let s = grown_pet("rest-hungry");
        s.lock().status = crate::status::Status::new(50, 50, 0, 0);
        work_for(&s, 1_000, crate::companion::BREAK_AFTER_SECS + 60);
        assert_eq!(s.snapshot().expression.bubble, Some(Bubble::Hungry));
    }

    #[test]
    fn the_pet_welcomes_the_owner_back_after_a_long_absence() {
        let s = grown_pet("welcome-back");
        s.observe_activity(1_000, 0);
        s.observe_activity(1_010, crate::companion::AWAY_AFTER_SECS);
        let back = 1_000 + 2 * 3600;
        let obs = s.observe_activity(back, 0);
        assert!(obs.returned_after.is_some());
        let v = s.tick(back).unwrap().view;
        assert_eq!(v.expression.bubble, Some(Bubble::WelcomeBack));
        let later = s.tick(back + crate::companion::WELCOME_BACK_SECS + 10).unwrap().view;
        assert_ne!(later.expression.bubble, Some(Bubble::WelcomeBack));
    }

    #[test]
    fn a_sleeping_pet_does_not_say_either() {
        let s = grown_pet("rest-asleep");
        work_for(&s, 1_000, crate::companion::BREAK_AFTER_SECS + 60);
        s.toggle_power(1_000 + crate::companion::BREAK_AFTER_SECS + 70).unwrap();
        assert_eq!(s.snapshot().expression.bubble, Some(Bubble::Sleeping));
    }

    /// 毎日 9〜18 時だけ操作しているヒストグラムを、オフセットなしの UTC として覚えさせる。
    fn learn_an_office_rhythm(s: &GameService) {
        let mut now = 0u64;
        for day in 0..3u64 {
            for hour in 9..18u64 {
                let start = day * 86_400 + hour * 3600 + 5;
                now = work_for(s, start, 600).max(now);
            }
        }
    }

    #[test]
    fn the_learned_sleep_window_is_used_when_auto_sleep_is_on() {
        let s = GameService::new(GameState::new(0), tmp("auto-sleep")).with_utc_offset(0);
        s.set_sleep_window(SleepWindow { start_hour: 1, end_hour: 2 }); // 設定は 1〜2 時だけ
        learn_an_office_rhythm(&s);
        s.set_auto_sleep(true);
        // 学んだ窓は 23〜7 時ごろ。設定（1〜2 時）とは別なので、3 日目の 0 時台に眠っているかで見分けられる
        let window = s.sleep_window();
        assert_ne!(window, SleepWindow { start_hour: 1, end_hour: 2 });
        assert_eq!(window.end_hour, (window.start_hour + 8) % 24);
        s.set_auto_sleep(false);
        assert_eq!(s.sleep_window(), SleepWindow { start_hour: 1, end_hour: 2 }, "the setting is back in charge");
    }

    #[test]
    fn until_enough_has_been_seen_the_setting_is_used_even_with_auto_sleep_on() {
        let s = GameService::new(GameState::new(0), tmp("auto-sleep-early")).with_utc_offset(0);
        s.set_sleep_window(SleepWindow { start_hour: 1, end_hour: 2 });
        s.set_auto_sleep(true);
        work_for(&s, 100, 120);
        assert_eq!(s.sleep_window(), SleepWindow { start_hour: 1, end_hour: 2 });
    }

    #[test]
    fn the_diary_reports_the_time_spent_together_and_the_learned_chronotype() {
        let s = GameService::new(GameState::new(0), tmp("diary-together")).with_utc_offset(0);
        assert_eq!((s.diary().together_minutes, s.diary().chronotype), (0, None));
        learn_an_office_rhythm(&s);
        let d = s.diary();
        assert_eq!(d.together_minutes, 3 * 9 * 10);
        assert!(d.chronotype.is_some());
        let json = serde_json::to_value(&d).unwrap();
        assert_eq!(json["togetherMinutes"], 270);
    }

    fn system_with(battery: Option<crate::system::Battery>, online: bool, dark_mode: bool) -> SystemInfo {
        SystemInfo { battery, online, dark_mode }
    }

    #[test]
    fn the_pet_complains_about_a_low_battery_and_a_lost_connection() {
        let s = grown_pet("system");
        s.set_system(system_with(Some(crate::system::Battery { percent: 15, on_ac: false }), true, false));
        let v = s.snapshot();
        assert_eq!((v.battery_low, v.expression.bubble), (true, Some(Bubble::BatteryLow)));
        s.set_system(system_with(None, false, false));
        let v = s.snapshot();
        assert_eq!((v.offline, v.battery_low, v.expression.bubble), (true, false, Some(Bubble::Offline)));
        s.set_system(system_with(Some(crate::system::Battery { percent: 5, on_ac: false }), false, false));
        assert_eq!(s.snapshot().expression.bubble, Some(Bubble::BatteryLow), "the battery is the bigger worry");
    }

    #[test]
    fn being_plugged_in_or_in_the_dark_shows_up_in_the_view_without_a_bubble() {
        let s = grown_pet("system-charging");
        s.set_system(system_with(Some(crate::system::Battery { percent: 30, on_ac: true }), true, true));
        let v = s.snapshot();
        assert_eq!((v.charging, v.battery_low, v.dark_mode, v.offline), (true, false, true, false));
        assert_eq!(v.expression.bubble, None);
        let json = serde_json::to_value(&v).unwrap();
        assert_eq!((json["charging"].as_bool(), json["darkMode"].as_bool(), json["batteryLow"].as_bool(), json["offline"].as_bool()), (Some(true), Some(true), Some(false), Some(false)));
    }

    #[test]
    fn a_need_or_sleep_comes_before_the_system_remarks() {
        let s = grown_pet("system-need");
        s.lock().status = crate::status::Status::new(50, 50, 0, 0);
        s.set_system(system_with(Some(crate::system::Battery { percent: 5, on_ac: false }), false, false));
        assert_eq!(s.snapshot().expression.bubble, Some(Bubble::Hungry));
        let asleep = grown_pet("system-asleep");
        asleep.toggle_power(700).unwrap();
        asleep.set_system(system_with(None, false, false));
        assert_eq!(asleep.snapshot().expression.bubble, Some(Bubble::Sleeping));
    }

    #[test]
    fn on_a_big_day_the_pet_says_so_and_wears_the_decoration() {
        let s = grown_pet("event").with_utc_offset(15 * 3600); // 1970-01-01 15 時 = お正月
        s.set_events(true);
        let v = s.snapshot();
        assert_eq!(v.expression.bubble, Some(Bubble::Event));
        let event = v.event.clone().unwrap();
        assert_eq!((event.emoji, event.message.as_str()), ("🎍", "あけましておめでとう！"));
        let json = serde_json::to_value(&v).unwrap();
        assert_eq!(json["event"]["emoji"], "🎍");
        assert_eq!(json["event"]["message"], "あけましておめでとう！");
    }

    #[test]
    fn events_stay_quiet_when_off_when_hungry_when_asleep_and_for_eggs() {
        let off = grown_pet("event-off");
        assert!(off.snapshot().event.is_none());
        let hungry = grown_pet("event-hungry");
        hungry.set_events(true);
        hungry.lock().status = crate::status::Status::new(50, 50, 0, 0);
        assert_eq!(hungry.snapshot().expression.bubble, Some(Bubble::Hungry));
        assert!(hungry.snapshot().event.is_some(), "the decoration is still there");
        let asleep = grown_pet("event-asleep");
        asleep.set_events(true);
        asleep.toggle_power(700).unwrap();
        assert_eq!(asleep.snapshot().expression.bubble, Some(Bubble::Sleeping));
        let egg = svc("event-egg");
        egg.set_events(true);
        assert_ne!(egg.snapshot().expression.bubble, Some(Bubble::Event));
    }

    #[test]
    fn the_pet_celebrates_the_owners_birthday_and_the_days_since_the_start() {
        let s = grown_pet("birthday").with_utc_offset(15 * 3600); // 1970-01-01 が使い始め
        s.set_events(true);
        s.lock().last_unix = 100 * 86_400; // 1970-04-11 = 使い始めから 100 日
        let v = s.snapshot();
        assert_eq!(v.event.as_ref().map(|e| e.emoji), Some("🎉"), "{:?}", v.event);
        s.lock().last_unix = 110 * 86_400; // 1970-04-21
        assert!(s.snapshot().event.is_none());
        s.set_birthday(Some((4, 21)));
        let v = s.snapshot();
        assert_eq!(v.expression.bubble, Some(Bubble::Event));
        assert_eq!(v.event.as_ref().map(|e| e.emoji), Some("🎂"));
        s.set_birthday(None);
        assert!(s.snapshot().event.is_none());
    }

    #[test]
    fn an_ordinary_day_has_no_event() {
        let s = grown_pet("event-ordinary").with_utc_offset(15 * 3600);
        s.set_events(true);
        s.lock().last_unix = 101 * 86_400; // 1970-04-12
        assert!(s.snapshot().event.is_none());
    }

    /// 飼い主が `secs` 秒ぶん席を離れて戻る（昼間に出かける）。
    fn go_out_and_come_back(s: &GameService, secs: u64) {
        let start = 1_000;
        s.observe_activity(start, 0);
        s.observe_activity(start + 10, crate::companion::AWAY_AFTER_SECS);
        s.observe_activity(start + secs, 0);
    }

    #[test]
    fn the_pet_brings_back_souvenirs_when_the_owner_returns_and_they_can_be_acknowledged() {
        let s = grown_pet("gift");
        go_out_and_come_back(&s, 3 * 3600);
        let v = s.snapshot();
        assert_eq!(v.gift.len(), 2);
        let json = serde_json::to_value(&v).unwrap();
        assert!(json["gift"][0]["emoji"].is_string() && json["gift"][0]["name"].is_string());
        let v = s.ack_gift().unwrap();
        assert!(v.gift.is_empty());
        assert_eq!(s.souvenirs().entries.iter().map(|e| e.count).sum::<u32>(), 2);
    }

    #[test]
    fn a_pet_that_was_asleep_when_the_owner_left_goes_nowhere() {
        let s = GameService::new(GameState::new(0), tmp("gift-asleep")).with_utc_offset(0); // 0 時台はおやすみ中
        {
            let mut state = s.lock();
            state.step(crate::evolution::EGG_SECS);
            state.ack_hatch();
        }
        go_out_and_come_back(&s, 3 * 3600);
        assert!(s.snapshot().gift.is_empty());
    }

    /// 飼い主が `leaves_at` に席を離れて（10 秒ごとに観測され）、`back_at` に戻る。UTC 0 時 = ローカル 0 時の設定で使う。
    fn observed_absence(s: &GameService, leaves_at: u64, back_at: u64) {
        s.observe_activity(leaves_at, 0);
        let mut now = leaves_at;
        while now + 10 < back_at {
            now += 10;
            s.observe_activity(now, now - leaves_at);
        }
        s.observe_activity(back_at, 0);
    }

    fn night_owner_service(name: &str) -> GameService {
        let s = GameService::new(GameState::new(0), tmp(name)).with_utc_offset(0); // おやすみは 23〜7 時
        {
            let mut state = s.lock();
            state.step(crate::evolution::EGG_SECS);
            state.ack_hatch();
        }
        s
    }

    #[test]
    fn leaving_just_before_bedtime_and_coming_back_in_the_morning_brings_nothing() {
        // 22:30 に席を離れる（ペットはまだ起きている）→ 23 時に眠る → 朝 7 時すぎに戻る。起きていたのは最初の 30 分だけ。
        let s = night_owner_service("night-away");
        let day = 5 * 86_400;
        observed_absence(&s, day + 22 * 3600 + 1800, day + 7 * 3600 + 300 + 86_400);
        assert!(s.snapshot().gift.is_empty(), "{:?}", s.snapshot().gift);
    }

    #[test]
    fn a_pet_that_wakes_up_in_the_morning_while_the_owner_is_still_away_does_not_go_out_then() {
        // 22:30 に離席 → 23 時に眠る → 朝 7 時に起きる → 8 時に飼い主が戻る。出かけたのは、寝るまでの 30 分だけ
        let s = night_owner_service("night-away-late");
        let day = 5 * 86_400;
        observed_absence(&s, day + 22 * 3600 + 1800, day + 8 * 3600 + 86_400);
        assert!(s.snapshot().gift.is_empty(), "it went home to sleep and did not go out again: {:?}", s.snapshot().gift);
    }

    #[test]
    fn putting_the_pet_to_bed_with_the_power_button_before_bedtime_and_sleeping_the_mac_brings_nothing() {
        // 21:00 に電源ボタンで寝かせて（8 時間）、Mac もスリープ。朝 8 時に戻る。おやすみ時間の前の 2 時間も、眠っている
        let s = night_owner_service("forced-mac-sleep");
        let day = 5 * 86_400;
        s.lock().last_unix = day + 21 * 3600 - 10;
        s.observe_activity(day + 21 * 3600 - 10, 0);
        s.toggle_power(day + 21 * 3600).unwrap();
        s.observe_activity(day + 21 * 3600, 0);
        let obs = s.observe_activity(day + 8 * 3600 + 86_400, 5); // Mac のスリープ明け
        assert!(obs.returned_after.is_some());
        assert!(s.snapshot().gift.is_empty(), "{:?}", s.snapshot().gift);
    }

    #[test]
    fn a_mac_that_slept_while_the_pet_was_still_awake_gives_only_the_time_until_the_pet_slept() {
        // 21:00 に離席して Mac がスリープ → 翌朝 8 時に戻る。ペットが起きていたのは、23 時に眠るまでの 2 時間
        let s = night_owner_service("mac-sleep-evening");
        let day = 5 * 86_400;
        s.observe_activity(day + 21 * 3600, 0);
        s.observe_activity(day + 8 * 3600 + 86_400, 5);
        assert_eq!(s.snapshot().gift.len(), 1, "2 hours of an outing (not the whole night): {:?}", s.snapshot().gift);
    }

    #[test]
    fn a_short_stretch_awake_before_bedtime_is_not_enough_for_an_outing_either() {
        let s = night_owner_service("night-away-short");
        let day = 5 * 86_400;
        observed_absence(&s, day + 22 * 3600 + 1800, day + 23 * 3600 + 1700); // 起きていたのは 30 分だけ
        assert!(s.snapshot().gift.is_empty());
    }

    #[test]
    fn only_the_awake_hours_are_counted_when_the_owner_was_away_through_the_evening_and_night() {
        // 18:00 に離れて、翌朝 8 時に戻る: 起きていたのは 18〜23 時の 5 時間 + 7〜8 時の 1 時間（戻る前は観測されている）
        let s = night_owner_service("evening-away");
        let day = 5 * 86_400;
        observed_absence(&s, day + 18 * 3600, day + 8 * 3600 + 86_400);
        let gift = s.snapshot().gift;
        assert_eq!(gift.len(), 2, "5 hours awake: two souvenirs, not the three of a whole night: {gift:?}");
    }

    #[test]
    fn a_pet_put_to_bed_with_the_power_button_does_not_go_out_either() {
        let s = grown_pet("forced-away");
        s.toggle_power(900).unwrap();
        observed_absence(&s, 1_000, 1_000 + 3 * 3600); // ずっと強制睡眠中（8 時間）
        assert!(s.snapshot().gift.is_empty());
    }

    #[test]
    fn the_pet_is_out_while_the_owner_is_away_and_back_when_the_owner_returns() {
        let s = grown_pet("out");
        s.lock().name = Some("ごま".to_string());
        s.observe_activity(1_000, 0);
        assert!(!s.snapshot().out);
        s.observe_activity(1_010, crate::companion::AWAY_AFTER_SECS);
        let v = s.snapshot();
        assert!(v.out);
        assert_eq!(tray_title(&v), Some("🎒"));
        assert_eq!(serde_json::to_value(&v).unwrap()["out"], true);
        s.observe_activity(1_020, 0);
        assert!(!s.snapshot().out);
    }

    #[test]
    fn a_sleeping_pet_an_egg_a_dead_pet_and_one_without_a_name_are_never_out() {
        let away = |s: &GameService| {
            s.observe_activity(1_000, 0);
            s.observe_activity(1_010, crate::companion::AWAY_AFTER_SECS);
        };
        let asleep = grown_pet("out-asleep");
        asleep.lock().name = Some("ごま".to_string());
        asleep.toggle_power(900).unwrap();
        away(&asleep);
        assert!(!asleep.snapshot().out);
        let egg = svc("out-egg");
        egg.lock().name = Some("ごま".to_string());
        away(&egg);
        assert!(!egg.snapshot().out);
        let unnamed = hatched_and_shown("out-unnamed");
        away(&unnamed);
        assert!(!unnamed.snapshot().out, "the name has to be decided first");
        let mut state = GameState::new(0);
        state.step(crate::evolution::EGG_SECS);
        state.name = Some("ごま".to_string());
        state.dead = true;
        let dead = GameService::new(state, tmp("out-dead")).with_utc_offset(NOON_AT_ZERO);
        away(&dead);
        assert!(!dead.snapshot().out);
    }

    #[test]
    fn souvenirs_can_be_displayed_and_the_pet_shows_them_on_screen() {
        let s = grown_pet("display");
        go_out_and_come_back(&s, 3 * 3600);
        let first = s.souvenirs().entries.into_iter().find(|e| e.count > 0).unwrap();
        let v = s.toggle_souvenir_display(first.id).unwrap();
        assert!(v.entries.iter().any(|e| e.id == first.id && e.displayed));
        assert_eq!(s.snapshot().displayed, [first.emoji]);
        s.toggle_souvenir_display(first.id).unwrap();
        assert!(s.snapshot().displayed.is_empty());
        assert!(s.toggle_souvenir_display("nothing").is_err());
    }

    #[test]
    fn displaying_returns_an_error_when_saving_fails() {
        let bad = tmp("display-bad").parent().unwrap().join("no/such/dir/save.json");
        let mut state = GameState::new(0);
        state.souvenirs.add("shell");
        let s = GameService::new(state, bad);
        assert!(s.toggle_souvenir_display("shell").is_err());
        assert!(s.souvenirs().entries[0].count == 1 && !s.souvenirs().entries[0].displayed);
    }

    #[test]
    fn acknowledging_the_gift_returns_an_error_when_saving_fails() {
        let bad = tmp("gift-bad").parent().unwrap().join("no/such/dir/save.json");
        let s = GameService::new(GameState::new(0), bad);
        assert!(s.ack_gift().is_err());
    }

    #[test]
    fn the_view_tells_the_background_of_the_season() {
        let s = svc("season-scene");
        assert_eq!(s.snapshot().season_scene, "snow", "1 月");
        let spring = svc("season-spring");
        spring.lock().last_unix = 100 * 86_400; // 1970-04-11
        assert_eq!(spring.snapshot().season_scene, "sakura");
        let summer = svc("season-summer");
        summer.lock().last_unix = 190 * 86_400; // 1970-07-10
        assert_eq!(serde_json::to_value(summer.snapshot()).unwrap()["seasonScene"], "beach");
        let autumn = svc("season-autumn");
        autumn.lock().last_unix = 280 * 86_400; // 1970-10-08
        assert_eq!(autumn.snapshot().season_scene, "autumn");
    }

    #[test]
    fn the_inherited_look_is_in_the_view() {
        let s = grown_pet("look");
        s.lock().genome = crate::genome::Genome { tint: 3, charm: 1 };
        let v = s.snapshot();
        assert_eq!((v.tint_degrees, v.charm), (130, Some("🎀")));
        let json = serde_json::to_value(&v).unwrap();
        assert_eq!((json["tintDegrees"].as_u64(), json["charm"].as_str()), (Some(130), Some("🎀")));
        assert_eq!(svc("look-plain").snapshot().charm, None);
    }

    #[test]
    fn the_egg_code_can_be_shared_and_welcomed_by_another_game() {
        let mine = grown_pet("code-mine");
        {
            let mut state = mine.lock();
            state.genome = crate::genome::Genome { tint: 5, charm: 3 };
            state.form.personality = Personality::Tidy;
        }
        let code = mine.egg_code();
        let friend = grown_pet("code-friend");
        let v = friend.receive_egg(&code.to_lowercase()).unwrap();
        assert_eq!((v.stage, v.generation, v.tint_degrees, v.charm), (1, 2, 230, Some("👓")));
        assert_eq!(friend.lock().legacy, Some(Personality::Tidy));
        let diary = friend.diary();
        assert_eq!(diary.lineage.len(), 1);
        let ancestor = &diary.lineage[0];
        assert!(ancestor.from_friend && ancestor.charm == Some("👓") && ancestor.tint_degrees == 230);
        let json = serde_json::to_value(&diary).unwrap();
        assert_eq!(json["lineage"][0]["fromFriend"], true);
        assert_eq!(json["lineage"][0]["tintDegrees"], 230);
    }

    #[test]
    fn a_wrong_egg_code_changes_nothing() {
        let s = grown_pet("code-wrong");
        let before = s.snapshot();
        assert!(s.receive_egg("NOT-A-CODE").is_err());
        assert!(s.receive_egg("AAAAA-AAAAA").is_err());
        assert_eq!(s.snapshot(), before);
    }

    #[test]
    fn receiving_an_egg_returns_an_error_when_saving_fails() {
        let code = grown_pet("code-src").egg_code();
        let bad = tmp("code-bad").parent().unwrap().join("no/such/dir/save.json");
        let s = GameService::new(GameState::new(0), bad);
        assert!(s.receive_egg(&code).is_err());
        assert_eq!(s.snapshot().generation, 1, "the pet stays as it was");
    }

    #[test]
    fn the_family_tree_is_in_the_diary_oldest_first() {
        let mut state = GameState::new(0);
        state.step(secs_until_graduation());
        state.graduate().unwrap();
        state.step(secs_until_graduation());
        state.graduate().unwrap();
        let s = GameService::new(state, tmp("tree")).with_utc_offset(NOON_AT_ZERO);
        let tree = s.diary().lineage;
        assert_eq!(tree.iter().map(|a| a.generation).collect::<Vec<_>>(), [1, 2]);
    }

    #[test]
    fn the_diary_shows_todays_page_so_far() {
        let s = grown_pet("today");
        s.perform(Action::Feed, 700).unwrap();
        s.perform(Action::Play, 710).unwrap();
        s.stroke(720).unwrap();
        let today = s.diary().today;
        assert_eq!((today.feeds, today.plays, today.strokes), (1, 1, 1));
        let json = serde_json::to_value(s.diary()).unwrap();
        assert_eq!((json["today"]["feeds"].as_u64(), json["today"]["strokes"].as_u64()), (Some(1), Some(1)));
    }

    #[test]
    fn the_inheritance_strength_from_the_settings_reaches_the_game() {
        let s = svc("legacy-bonus");
        assert_eq!(s.lock().legacy_bonus, crate::evolution::DEFAULT_LEGACY_BONUS);
        s.set_legacy_bonus(60);
        assert_eq!(s.lock().legacy_bonus, 60);
    }

    fn read_log(path: &std::path::Path) -> String {
        std::fs::read_to_string(path).unwrap_or_default()
    }

    fn pet_with_log(name: &str) -> (GameService, PathBuf) {
        let s = grown_pet(name);
        let path = tmp(&format!("{name}-log")).with_file_name("debug.log");
        s.set_debug_log(Some(path.clone()));
        (s, path)
    }

    #[test]
    fn nothing_is_written_while_the_debug_log_is_off() {
        let s = grown_pet("log-off");
        let path = tmp("log-off-file").with_file_name("debug.log");
        s.perform(Action::Feed, 700).unwrap();
        s.log_note("設定", "x");
        assert!(!path.exists());
    }

    #[test]
    fn turning_the_log_on_and_off_is_noted_with_a_snapshot() {
        let (s, path) = pet_with_log("log-onoff");
        let text = read_log(&path);
        assert!(text.contains("[起動] デバッグログを始めた"), "{text}");
        assert!(text.contains("[状態] ご機嫌 50 親密度 50 空腹度 80"), "{text}");
        s.set_debug_log(Some(path.clone())); // すでにオン: 重ねては書かない
        assert_eq!(read_log(&path).matches("デバッグログを始めた").count(), 1);
        s.set_debug_log(None);
        assert!(read_log(&path).contains("[起動] デバッグログを止めた"));
        let before = read_log(&path);
        s.perform(Action::Feed, 700).unwrap();
        assert_eq!(read_log(&path), before, "off again: nothing more");
        s.set_debug_log(None); // 止まっているものを止めても、何も起きない
        assert_eq!(read_log(&path), before);
    }

    #[test]
    fn care_is_logged_with_the_numbers_that_changed() {
        let (s, path) = pet_with_log("log-care");
        s.lock().status = crate::status::Status::new(50, 50, 40, 0);
        s.perform(Action::Feed, 700).unwrap();
        s.perform(Action::Play, 710).unwrap();
        let text = read_log(&path);
        assert!(text.contains("[操作] ご飯を食べた  "), "{text}");
        assert!(text.contains("空腹度 "), "{text}");
        assert!(text.contains("[操作] 遊んだ  "), "{text}");
        assert!(text.contains("機嫌が良くなった"), "{text}");
        assert!(text.contains("1970-01-01 15:11:"), "the local time stamp: {text}");
    }

    #[test]
    fn an_action_that_changes_no_number_says_so() {
        let (s, path) = pet_with_log("log-nochange");
        s.perform(Action::Scold, 700).unwrap(); // 叱っても、わがままでなければ機嫌が下がる…が、卵でなければ数値は動く
        s.lock().status = crate::status::Status::new(0, 0, 80, 0);
        s.perform(Action::Scold, 710).unwrap(); // もう下がらない
        let text = read_log(&path);
        assert!(text.contains("[操作] 叱られた（数値の変化なし）"), "{text}");
    }

    #[test]
    fn every_kind_of_operation_leaves_a_line() {
        let (s, path) = pet_with_log("log-all");
        s.perform(Action::Snack, 700).unwrap();
        s.perform(Action::Clean, 705).unwrap();
        s.perform(Action::Cure, 710).unwrap();
        s.stroke(720).unwrap();
        s.toggle_power(730).unwrap();
        s.toggle_power(740).unwrap();
        s.set_name("ごま").unwrap();
        s.play_minigame(Side::Left, 750).unwrap();
        s.ack_hatch().unwrap();
        s.ack_gift().unwrap();
        s.toggle_souvenir_display("nothing").unwrap_err();
        let text = read_log(&path);
        for label in ["おやつを食べた", "掃除した", "治療を受けた", "なでられた", "電源ボタンを押した", "名前をつけた", "ミニゲームで遊んだ", "孵化の演出を見せた", "おみやげを受け取った", "おみやげの飾りを変えた"] {
            assert!(text.contains(label), "{label}: {text}");
        }
    }

    #[test]
    fn the_big_moments_are_logged_too() {
        let (s, path) = pet_with_log("log-moments");
        s.restart().unwrap();
        let egg_code = s.egg_code();
        s.receive_egg(&egg_code).unwrap();
        let text = read_log(&path);
        assert!(text.contains("最初からやり直した") && text.contains("友だちのたまごを迎えた"), "{text}");
        assert!(text.contains("世代が変わった（1→2代目）"), "{text}");
        let elder = GameService::new({ let mut g = GameState::new(0); g.step(crate::evolution::secs_until_graduation()); g }, tmp("log-elder").with_file_name("save.json")).with_utc_offset(NOON_AT_ZERO);
        let elder_log = tmp("log-elder-file").with_file_name("debug.log");
        elder.set_debug_log(Some(elder_log.clone()));
        elder.graduate().unwrap();
        assert!(read_log(&elder_log).contains("旅立ちを見送った"));
        let mut dead = GameState::new(0);
        dead.step(crate::evolution::EGG_SECS);
        dead.dead = true;
        let grave = GameService::new(dead, tmp("log-grave").with_file_name("save.json")).with_utc_offset(NOON_AT_ZERO);
        let grave_log = tmp("log-grave-file").with_file_name("debug.log");
        grave.set_debug_log(Some(grave_log.clone()));
        grave.bury().unwrap();
        let text = read_log(&grave_log);
        assert!(text.contains("お墓から新しい卵を迎えた") && text.contains("新しい卵を迎えた"), "{text}");
    }

    #[test]
    fn going_to_sleep_and_waking_up_are_logged_by_the_ticks() {
        let s = GameService::new(GameState::new(0), tmp("log-sleep").with_file_name("save.json")).with_utc_offset(0); // 0 時台はおやすみ中
        {
            let mut state = s.lock();
            state.step(crate::evolution::EGG_SECS);
            state.ack_hatch();
            state.last_unix = 22 * 3600; // 22 時（まだ起きている）
        }
        let path = tmp("log-sleep-file").with_file_name("debug.log");
        s.set_debug_log(Some(path.clone()));
        s.tick(23 * 3600 + 60).unwrap();
        s.toggle_power(23 * 3600 + 120).unwrap(); // 起こす
        s.toggle_power(23 * 3600 + 180).unwrap(); // 寝かせる
        s.toggle_power(23 * 3600 + 240).unwrap(); // 起こす（まだ夜）
        let text = read_log(&path);
        assert!(text.contains("[変化] ねた（おやすみ時間）"), "{text}");
        assert!(text.contains("[変化] おきた"), "{text}");
        assert!(text.contains("ねた（電源ボタンで寝かされた）"), "{text}");
    }

    #[test]
    fn poops_hunger_and_mood_changes_are_logged_by_the_ticks() {
        let (s, path) = pet_with_log("log-ticks");
        s.lock().status = crate::status::Status::new(31, 50, 31, 24);
        s.tick(240).unwrap(); // 2 ティック: 汚れ度 24→26（うんち 1 個）、空腹度・ご機嫌も下がる
        let text = read_log(&path);
        assert!(text.contains("[変化] うんちが増えた（0→1個"), "{text}");
        assert!(text.contains("[変化] おなかがすいた"), "{text}");
        assert!(text.contains("[変化] 機嫌が悪くなった"), "{text}");
    }

    #[test]
    fn the_state_is_listed_every_ten_minutes_and_not_in_between() {
        let (s, path) = pet_with_log("log-snapshot");
        let start = read_log(&path).matches("[状態]").count();
        s.tick(crate::debuglog::SNAPSHOT_EVERY_SECS / 2).unwrap();
        assert_eq!(read_log(&path).matches("[状態]").count(), start);
        s.tick(crate::debuglog::SNAPSHOT_EVERY_SECS + 700).unwrap();
        assert_eq!(read_log(&path).matches("[状態]").count(), start + 1);
    }

    #[test]
    fn leaving_and_returning_are_logged_with_the_time_the_pet_was_awake() {
        let (s, path) = pet_with_log("log-outing");
        s.lock().name = Some("ごま".to_string());
        s.observe_activity(1_000, 0);
        s.observe_activity(1_010, crate::companion::AWAY_AFTER_SECS);
        s.tick(1_020).unwrap();
        s.observe_activity(1_000 + 3 * 3600, 0);
        s.tick(1_000 + 3 * 3600 + 10).unwrap();
        let text = read_log(&path);
        assert!(text.contains("[観測] 席を離れたと判断（無操作 1800秒。ペットは起きている）"), "{text}");
        assert!(text.contains("[変化] おでかけした"), "{text}");
        assert!(text.contains("[観測] 飼い主が戻った（離席 "), "{text}");
        assert!(text.contains("眠るまでに起きていた時間 "), "{text}");
        assert!(text.contains("おみやげ 2個"), "{text}");
        assert!(text.contains("[変化] おでかけから戻った"), "{text}");
        assert!(text.contains("[変化] おみやげを持ち帰った（2個: "), "{text}");
    }

    #[test]
    fn a_pet_that_was_asleep_when_the_owner_left_is_noted_as_asleep() {
        let s = GameService::new(GameState::new(0), tmp("log-asleep-away").with_file_name("save.json")).with_utc_offset(0);
        {
            let mut state = s.lock();
            state.step(crate::evolution::EGG_SECS);
            state.ack_hatch();
        }
        let path = tmp("log-asleep-away-file").with_file_name("debug.log");
        s.set_debug_log(Some(path.clone()));
        s.observe_activity(1_000, 0);
        s.observe_activity(1_010, crate::companion::AWAY_AFTER_SECS);
        assert!(read_log(&path).contains("ペットは眠っている"));
    }

    #[test]
    fn long_work_is_logged_as_a_break_time_and_affinity_gain() {
        let (s, path) = pet_with_log("log-work");
        work_for(&s, 1_000, crate::companion::BREAK_AFTER_SECS + 60);
        let text = read_log(&path);
        assert!(text.contains("[観測] 作業に寄り添って親密度 +1"), "{text}");
        assert!(text.contains("[観測] 休憩の声かけの時間（休まず作業 90分）"), "{text}");
    }

    #[test]
    fn notes_from_outside_are_written_with_their_tag() {
        let (s, path) = pet_with_log("log-note");
        s.log_note("設定", "設定を保存した");
        assert!(read_log(&path).contains("[設定] 設定を保存した"));
    }

    #[test]
    fn a_log_that_cannot_be_written_never_breaks_the_game() {
        let s = grown_pet("log-broken");
        let bad = tmp("log-broken-file").parent().unwrap().join("no/such/dir/debug.log");
        s.set_debug_log(Some(bad));
        assert!(s.perform(Action::Feed, 700).is_ok());
        s.log_note("設定", "x");
    }

    #[test]
    fn a_selfish_pet_says_so_until_it_is_scolded() {
        let s = selfish_pet("selfish");
        assert_eq!(s.snapshot().expression.bubble, Some(Bubble::Selfish));
        let v = s.perform(Action::Scold, 700).unwrap();
        assert_eq!(v.discipline, 15);
        assert_ne!(v.expression.bubble, Some(Bubble::Selfish));
    }

    #[test]
    fn the_number_of_poops_follows_the_dirtiness_and_eggs_make_none() {
        for (dirtiness, shown) in [(0, 0), (1, 0), (24, 0), (25, 1), (49, 1), (50, 2), (74, 2), (75, 3), (99, 3), (100, 4)] {
            assert_eq!(poops(2, dirtiness), shown, "dirtiness {dirtiness}");
            assert_eq!(poops(1, dirtiness), 0, "an egg at dirtiness {dirtiness}");
        }
    }

    fn sick_pet(name: &str, offset: i32) -> GameService {
        let mut state = GameState::new(0);
        state.step(crate::evolution::EGG_SECS);
        state.ack_hatch();
        state.intro_seen = true;
        state.care.sick = true;
        GameService::new(state, tmp(name)).with_utc_offset(offset)
    }

    #[test]
    fn a_sick_pet_asks_for_a_shot_until_it_is_cured() {
        let s = sick_pet("sick", NOON_AT_ZERO);
        let v = s.snapshot();
        assert!(v.sick);
        assert_eq!(v.expression.bubble, Some(Bubble::Sick));
        let v = s.perform(Action::Cure, 700).unwrap();
        assert!(!v.sick);
        assert_ne!(v.expression.bubble, Some(Bubble::Sick));
    }

    #[test]
    fn a_sleeping_sick_pet_is_left_to_sleep() {
        let s = sick_pet("sick-sleep", 0); // UTC の 0 時台はおやすみ中
        assert_eq!(s.snapshot().expression.bubble, Some(Bubble::Sleeping));
    }

    fn doomed_service(name: &str) -> GameService {
        let mut state = GameState::new(0);
        state.step(crate::evolution::EGG_SECS);
        state.ack_hatch();
        state.intro_seen = true;
        state.care.sick = true;
        let s = GameService::new(state, tmp(name)).with_utc_offset(NOON_AT_ZERO);
        s.set_strict(true);
        s
    }

    #[test]
    fn a_pet_close_to_the_end_cries_for_help_but_only_in_strict_mode() {
        let s = doomed_service("danger");
        assert_eq!(s.snapshot().expression.bubble, Some(Bubble::Sick));
        assert!(!s.snapshot().danger);
        s.lock().care.fatal_ticks = crate::care::DANGER_TICKS;
        let v = s.snapshot();
        assert_eq!((v.danger, v.expression.bubble), (true, Some(Bubble::Dying)));
        s.set_strict(false);
        let v = s.snapshot();
        assert_eq!((v.danger, v.expression.bubble), (false, Some(Bubble::Sick)));
    }

    #[test]
    fn a_dead_pet_shows_the_grave_and_can_be_buried_into_a_new_egg() {
        let s = doomed_service("grave");
        s.lock().care.fatal_ticks = crate::care::FATAL_TICKS - 1;
        let tick = s.tick(u64::from(TICK_SECS) * 2).unwrap();
        assert!(tick.view.dead);
        assert_eq!(tick.view.expression.bubble, None);
        assert_eq!(tray_title(&tick.view), Some("🪦"));
        assert!(!tick.view.danger);
        let json = serde_json::to_value(&tick.view).unwrap();
        assert_eq!((json["dead"].as_bool(), json["danger"].as_bool()), (Some(true), Some(false)));
        let v = s.bury().unwrap();
        assert!(!v.dead);
        assert_eq!((v.generation, v.stage), (2, 1));
        assert!(s.bury().is_err(), "nothing to bury now");
    }

    #[test]
    fn burying_returns_an_error_when_saving_fails() {
        let bad = tmp("bury-bad").parent().unwrap().join("no/such/dir/save.json");
        let mut state = GameState::new(0);
        state.dead = true;
        let s = GameService::new(state, bad);
        assert!(s.bury().is_err());
        assert!(s.snapshot().dead, "the pet stays as it was");
    }

    #[test]
    fn the_view_reports_weight_and_discipline() {
        let v = svc("weight").snapshot();
        assert_eq!((v.weight, v.discipline), (50, 0));
        let json = serde_json::to_value(&v).unwrap();
        assert_eq!(json["weight"], 50);
        assert_eq!(json["discipline"], 0);
    }

    #[test]
    fn stroking_cheers_the_pet_up_without_counting_as_care() {
        let s = svc("stroke");
        let before = s.snapshot();
        let v = s.stroke(0).unwrap();
        assert_eq!((v.mood, v.affinity), (before.mood + 2, before.affinity + 1));
        assert!(v.intro, "stroking does not end the opening");
        assert_eq!(s.snapshot().mood, v.mood);
    }

    #[test]
    fn stroking_returns_an_error_when_saving_fails() {
        let bad = tmp("stroke-bad").parent().unwrap().join("no/such/dir/save.json");
        let s = GameService::new(GameState::new(0), bad).with_utc_offset(NOON_AT_ZERO); // 昼でないと、おやすみ中で保存されない
        assert!(s.stroke(0).is_err());
    }

    #[test]
    fn a_sleeping_pet_is_not_cheered_by_stroking() {
        let s = svc("stroke-sleep");
        s.set_sleep_window(SleepWindow { start_hour: 0, end_hour: 24 });
        let before = s.snapshot();
        let v = s.stroke(0).unwrap();
        assert!(v.sleeping);
        assert_eq!(v.mood, before.mood);
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
        let Tick { view: v, evolutions: evo, .. } = s.tick(u64::from(EGG_SECS)).unwrap();
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

    fn night_service(name: &str) -> GameService {
        // UTC+0 で UNIX 秒 0 = 0:00（既定のおやすみ時間の真ん中）。
        GameService::new(GameState::new(0), tmp(name)).with_utc_offset(0)
    }

    #[test]
    fn a_custom_sleep_window_decides_whether_the_pet_is_asleep() {
        let s = night_service("win-custom");
        assert!(s.snapshot().sleeping); // 既定 23〜7 では 0:00 は夜
        s.set_sleep_window(SleepWindow { start_hour: 1, end_hour: 5 });
        assert!(!s.snapshot().sleeping); // 1〜5時にすると 0:00 は起きている
        s.set_sleep_window(SleepWindow { start_hour: 22, end_hour: 6 });
        assert!(s.snapshot().sleeping);
    }

    #[test]
    fn a_window_with_equal_hours_means_the_pet_never_sleeps() {
        let s = night_service("win-never");
        s.set_sleep_window(SleepWindow { start_hour: 0, end_hour: 0 });
        assert!(!s.snapshot().sleeping);
    }

    #[test]
    fn the_sleep_window_also_changes_how_fast_the_pet_gets_hungry() {
        fn hunger_after_four_hours(window: Option<SleepWindow>) -> u8 {
            let mut state = GameState::new(0);
            state.status.hunger = 100;
            let s = GameService::new(state, tmp("win-decay")).with_utc_offset(0);
            if let Some(w) = window {
                s.set_sleep_window(w);
            }
            for i in 1..=120u64 {
                s.tick(i * u64::from(TICK_SECS)).unwrap();
            }
            s.snapshot().hunger
        }
        let asleep = hunger_after_four_hours(None); // 既定 23〜7: 0〜4時はずっと寝ている
        let awake = hunger_after_four_hours(Some(SleepWindow { start_hour: 0, end_hour: 0 }));
        assert_eq!(100 - asleep, 30);
        assert_eq!(awake, 0);
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

    fn hatched_and_shown(name: &str) -> GameService {
        let s = svc(name);
        s.tick(u64::from(EGG_SECS)).unwrap();
        s.ack_hatch().unwrap();
        s
    }

    #[test]
    fn nobody_is_asked_for_a_name_while_still_an_egg() {
        let v = svc("name-egg").snapshot();
        assert_eq!((v.name.clone(), v.needs_name), (None, false));
    }

    #[test]
    fn the_name_is_asked_for_only_after_the_hatch_animation_has_been_shown() {
        let s = svc("name-ask");
        let t = s.tick(u64::from(EGG_SECS)).unwrap();
        assert!(t.view.hatching);
        assert!(!t.view.needs_name); // 演出が先
        assert!(s.ack_hatch().unwrap().needs_name);
    }

    #[test]
    fn naming_the_pet_stops_the_question_and_exposes_the_name() {
        let s = hatched_and_shown("name-set");
        let v = s.set_name("  ごま ").unwrap();
        assert_eq!((v.name.as_deref(), v.needs_name), (Some("ごま"), false));
        assert_eq!(s.snapshot().name.as_deref(), Some("ごま"));
    }

    #[test]
    fn the_name_cannot_be_changed_once_it_is_set() {
        let s = hatched_and_shown("name-fixed");
        s.set_name("ごま").unwrap();
        assert!(s.set_name("もち").is_err());
        assert_eq!(s.snapshot().name.as_deref(), Some("ごま"));
    }

    #[test]
    fn restarting_brings_a_new_unnamed_egg_that_needs_no_name_yet() {
        let s = hatched_and_shown("restart");
        s.set_name("ごま").unwrap();
        let v = s.restart().unwrap();
        assert_eq!((v.stage, v.generation, v.name, v.needs_name, v.intro), (1, 2, None, false, true));
        assert!(GameState::load(&s.path, 0).unwrap().name.is_none(), "saved");
    }

    #[test]
    fn restarting_returns_an_error_when_saving_fails() {
        let bad = tmp("restart-bad").parent().unwrap().join("no/such/dir/save.json");
        let s = GameService::new(GameState::new(0), bad);
        assert!(s.restart().is_err());
        assert_eq!(s.snapshot().generation, 1, "the pet stays as it was");
    }

    #[test]
    fn the_name_is_persisted() {
        let p = tmp("name-persist");
        let s = GameService::new(GameState::new(0), p.clone()).with_utc_offset(NOON_AT_ZERO);
        s.set_name("ごま").unwrap();
        assert_eq!(GameState::load(&p, 0).unwrap().name.as_deref(), Some("ごま"));
    }

    #[test]
    fn an_invalid_name_is_an_error_and_changes_nothing() {
        let s = hatched_and_shown("name-bad");
        assert!(s.set_name("").is_err());
        assert!(s.set_name(&"あ".repeat(50)).is_err());
        assert_eq!(s.snapshot().name, None);
    }

    #[test]
    fn a_failure_to_save_the_name_is_reported() {
        let bad = tmp("name-save").parent().unwrap().join("no/such/dir/save.json");
        let s = GameService::new(GameState::new(0), bad);
        assert!(s.set_name("ごま").is_err());
    }

    #[test]
    fn the_view_json_exposes_the_name_fields() {
        let json = serde_json::to_value(svc("name-json").snapshot()).unwrap();
        assert_eq!(json["name"], serde_json::Value::Null);
        assert_eq!(json["needsName"], false);
    }

    fn hatched_service(name: &str, seed: u64) -> GameService {
        let s = svc(name).with_seed(seed);
        s.tick(u64::from(EGG_SECS)).unwrap();
        s.ack_hatch().unwrap();
        s
    }

    fn other(side: Side) -> Side {
        match side {
            Side::Left => Side::Right,
            Side::Right => Side::Left,
        }
    }

    #[test]
    fn guessing_the_hidden_hand_wins_and_the_report_says_where_it_was() {
        let answer = Rng::new(7).next_side();
        let s = hatched_service("game-win", 7);
        let r = s.play_minigame(answer, 1000).unwrap();
        assert!(r.win);
        assert_eq!(r.answer, answer);
    }

    #[test]
    fn guessing_wrong_loses_but_still_reveals_the_answer() {
        let answer = Rng::new(7).next_side();
        let s = hatched_service("game-lose", 7);
        let r = s.play_minigame(other(answer), 1000).unwrap();
        assert!(!r.win);
        assert_eq!(r.answer, answer);
    }

    #[test]
    fn a_win_cheers_the_pet_up_more_than_a_loss() {
        let answer = Rng::new(7).next_side();
        let won = hatched_service("game-cmp-a", 7).play_minigame(answer, 1000).unwrap().view;
        let lost = hatched_service("game-cmp-b", 7).play_minigame(other(answer), 1000).unwrap().view;
        assert!(won.mood > lost.mood);
        assert!(won.affinity > lost.affinity);
    }

    #[test]
    fn every_round_gets_a_fresh_answer() {
        let mut rng = Rng::new(99);
        let (first, second) = (rng.next_side(), rng.next_side());
        let s = hatched_service("game-rounds", 99);
        assert_eq!(s.play_minigame(first, 1000).unwrap().answer, first);
        assert_eq!(s.play_minigame(second, 1001).unwrap().answer, second);
    }

    #[test]
    fn the_game_result_is_saved() {
        let p = tmp("game-save");
        let s = GameService::new(GameState::new(0), p.clone()).with_utc_offset(NOON_AT_ZERO).with_seed(3);
        s.tick(u64::from(EGG_SECS)).unwrap();
        s.play_minigame(Side::Left, 1000).unwrap();
        assert_eq!(GameState::load(&p, 0).unwrap().history.plays, 1);
    }

    #[test]
    fn an_egg_cannot_play_and_nothing_changes() {
        let s = svc("game-egg").with_seed(1);
        assert!(s.play_minigame(Side::Left, 0).is_err());
        assert_eq!(s.snapshot().affinity, 0);
    }

    #[test]
    fn a_failure_to_save_the_game_is_reported() {
        let bad = tmp("game-bad").parent().unwrap().join("no/such/dir/save.json");
        let mut state = GameState::new(0);
        state.step(u32::MAX); // 孵化させる
        let s = GameService::new(state, bad).with_seed(1);
        assert!(s.play_minigame(Side::Left, 0).is_err());
    }

    #[test]
    fn the_report_serializes_with_camel_case_keys() {
        let s = hatched_service("game-json", 5);
        let json = serde_json::to_value(s.play_minigame(Side::Right, 1000).unwrap()).unwrap();
        assert!(json["win"].is_boolean());
        assert!(json["answer"] == "left" || json["answer"] == "right");
        assert_eq!(json["view"]["stage"], 2);
    }

    fn elder_service(name: &str) -> GameService {
        let mut state = GameState::new(0);
        state.step(secs_until_graduation());
        state.intro_seen = true;
        GameService::new(state, tmp(name)).with_utc_offset(NOON_AT_ZERO)
    }

    #[test]
    fn a_young_pet_is_not_graduating_and_is_the_first_generation() {
        let v = svc("grad-young").snapshot();
        assert_eq!((v.graduating, v.generation), (false, 1));
    }

    #[test]
    fn an_elder_pet_is_graduating() {
        assert!(elder_service("grad-elder").snapshot().graduating);
    }

    #[test]
    fn saying_goodbye_returns_the_view_of_a_new_egg() {
        let s = elder_service("grad-bye");
        let v = s.graduate().unwrap();
        assert_eq!((v.stage, v.generation, v.graduating, v.intro), (1, 2, false, true));
        assert_eq!(v.expression.bubble, Some(Bubble::Welcome));
        assert_eq!(v.name, None);
        assert_eq!(s.snapshot().generation, 2);
    }

    #[test]
    fn it_is_an_error_to_say_goodbye_too_early() {
        let s = svc("grad-early");
        assert!(s.graduate().is_err());
        assert_eq!(s.snapshot().generation, 1);
    }

    #[test]
    fn the_farewell_is_saved() {
        let p = tmp("grad-save");
        let mut state = GameState::new(0);
        state.step(secs_until_graduation());
        let s = GameService::new(state, p.clone()).with_utc_offset(NOON_AT_ZERO);
        s.graduate().unwrap();
        let saved = GameState::load(&p, 0).unwrap();
        assert_eq!((saved.generation, saved.growth.stage), (2, 1));
    }

    #[test]
    fn a_failure_to_save_the_farewell_leaves_the_pet_as_it_was() {
        let bad = tmp("grad-bad").parent().unwrap().join("no/such/dir/save.json");
        let mut state = GameState::new(0);
        state.step(secs_until_graduation());
        let s = GameService::new(state, bad).with_utc_offset(NOON_AT_ZERO);
        assert!(s.graduate().is_err());
        assert!(s.snapshot().graduating);
    }

    #[test]
    fn the_tick_that_makes_the_pet_ready_to_leave_says_so_once() {
        let mut state = GameState::new(0);
        state.step(secs_until_graduation() - 1); // あと 1 秒
        let s = GameService::new(state, tmp("grad-tick")).with_utc_offset(NOON_AT_ZERO);
        let first = s.tick(1).unwrap();
        assert!(first.graduation_ready);
        assert!(first.view.graduating);
        let second = s.tick(2).unwrap();
        assert!(!second.graduation_ready);
        assert!(second.view.graduating);
    }

    #[test]
    fn nothing_to_report_for_an_ordinary_tick() {
        assert!(!svc("grad-normal").tick(10).unwrap().graduation_ready);
    }

    #[test]
    fn the_menu_bar_waves_goodbye_for_a_graduating_pet() {
        let mut v = view_with(Some(Bubble::Hungry));
        v.graduating = true;
        assert_eq!(tray_title(&v), Some("👋"));
        v.hatching = true;
        assert_eq!(tray_title(&v), Some("🐣")); // 孵化の確認が先
    }

    #[test]
    fn the_farewell_notification_is_only_for_a_closed_popup() {
        assert!(should_notify_graduation(true, false));
        assert!(!should_notify_graduation(true, true));
        assert!(!should_notify_graduation(false, false));
        assert!(!should_notify_graduation(false, true));
    }

    #[test]
    fn the_diary_view_lists_memories_newest_first_with_the_day_count() {
        let s = svc("diary");
        let d = s.diary();
        assert_eq!((d.days_together, d.entries.len()), (1, 1)); // 卵が届いた
        s.tick(u64::from(EGG_SECS)).unwrap();
        s.set_name("ごま").unwrap();
        let d = s.diary();
        assert_eq!(d.entries.len(), 3); // 届いた / 孵った / 名づけ
        assert!(d.entries[0].at_unix >= d.entries[2].at_unix);
        assert!(matches!(d.entries[0].event, crate::diary::DiaryEvent::Named { .. }));
    }

    #[test]
    fn the_day_count_follows_the_clock() {
        let s = svc("diary-days");
        s.tick(3 * 24 * 3600 + 10).unwrap();
        assert_eq!(s.diary().days_together, 4);
    }

    #[test]
    fn the_dex_starts_with_the_egg_and_grows_with_each_evolution() {
        let s = svc("dex");
        assert_eq!(s.dex().discovered, 1);
        s.tick(u64::from(EGG_SECS)).unwrap();
        let d = s.dex();
        assert_eq!((d.total, d.discovered), (113, 2));
        assert!(d.entries.iter().filter(|e| e.discovered).any(|e| e.stage == 2));
    }

    #[test]
    fn the_dex_is_saved_with_the_game() {
        let p = tmp("dex-save");
        let s = GameService::new(GameState::new(0), p.clone()).with_utc_offset(NOON_AT_ZERO);
        s.tick(u64::from(EGG_SECS)).unwrap();
        assert_eq!(GameState::load(&p, 0).unwrap().discovered.len(), 2);
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
        let Tick { view: v, .. } = s.tick(u64::from(EGG_SECS)).unwrap();
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
        let Tick { view: v, evolutions: evo, .. } = s.tick(u64::from(EGG_SECS)).unwrap();
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
        let t = s.tick(u64::from(EGG_SECS) + u64::from(stage_secs(2))).unwrap();
        assert_eq!((t.evolutions, t.hatched), (1, false));
    }

    #[test]
    fn tick_reports_the_hatch_after_a_long_absence_too() {
        let s = svc("hatched-long");
        let t = s.tick(u64::from(EGG_SECS) + 3 * u64::from(stage_secs(2))).unwrap();
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
        assert_eq!(tray_title(&view_with(Some(Bubble::Selfish))), Some("💬"));
        assert_eq!(tray_title(&view_with(Some(Bubble::Sick))), Some("🤒"));
        assert_eq!(tray_title(&view_with(Some(Bubble::Dying))), Some("🆘"));
        assert_eq!(tray_title(&view_with(Some(Bubble::Rest))), Some("☕"));
        let mut out = view_with(Some(Bubble::Hungry));
        out.out = true;
        assert_eq!(tray_title(&out), Some("🎒"), "while out, the bag shows instead of the needs");
        assert_eq!(tray_title(&view_with(Some(Bubble::BatteryLow))), Some("🪫"));
        assert_eq!(tray_title(&view_with(Some(Bubble::Offline))), Some("📡"));
        for quiet in [Bubble::Happy, Bubble::Morning, Bubble::Noon, Bubble::Evening, Bubble::Drowsy, Bubble::WelcomeBack, Bubble::Event] {
            assert_eq!(tray_title(&view_with(Some(quiet))), None);
        }
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
