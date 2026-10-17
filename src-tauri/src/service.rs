//! Tauri から独立したアプリケーションサービス層。IPC 用 DTO と排他制御・永続化をまとめる。

use std::io;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Mutex, MutexGuard};

use serde::Serialize;

use crate::evolution::{Branch, Personality};
use crate::expression::{express, greeting, Bubble, Expression};
use crate::dex::{build_view, DexView};
use crate::diary::{self, DiaryView};
use crate::game::{Action, GameState};
use crate::minigame::{is_win, Rng, Side};
use crate::sleep::{is_sleeping, local_hour, local_utc_offset, SleepWindow};

impl FromStr for Action {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "feed" => Ok(Action::Feed),
            "snack" => Ok(Action::Snack),
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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
    if view.graduating {
        return Some("👋");
    }
    match view.expression.bubble? {
        Bubble::Hungry => Some("🍙"),
        Bubble::Dirty => Some("🧹"),
        Bubble::Gloomy => Some("💧"),
        Bubble::Sleeping => Some("💤"),
        Bubble::Welcome => Some("🥚"),
        // あいさつはメニューバーには出さない（落ち着いて見えるように）。
        Bubble::Happy | Bubble::Morning | Bubble::Noon | Bubble::Evening | Bubble::Drowsy => None,
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

pub struct GameService {
    state: Mutex<GameState>,
    path: PathBuf,
    time_scale: u32,
    /// None ならこのマシンのローカルタイムゾーンを使う（テストでは固定値を指定する）。
    utc_offset: Option<i32>,
    /// おやすみ時間（設定ダイアログで変えられる）。
    sleep: Mutex<SleepWindow>,
    /// ミニゲームの乱数。
    rng: Mutex<Rng>,
}

impl GameService {
    pub fn new(state: GameState, path: PathBuf) -> Self {
        Self { state: Mutex::new(state), path, time_scale: 1, utc_offset: None, sleep: Mutex::new(SleepWindow::default()), rng: Mutex::new(Rng::new(time_seed())) }
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
        diary::build_view(&state.diary, state.born_unix, state.last_unix)
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

    fn sleep_window(&self) -> SleepWindow {
        *self.sleep.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn offset_at(&self, unix_secs: u64) -> i32 {
        self.utc_offset.unwrap_or_else(|| local_utc_offset(unix_secs))
    }

    fn view(&self, state: &GameState) -> StateView {
        view_of(state, self.offset_at(state.last_unix), self.sleep_window())
    }

    pub fn snapshot(&self) -> StateView {
        self.view(&self.lock())
    }

    /// 現在時刻まで進めたうえでアクションを適用し、保存する。
    pub fn perform(&self, action: Action, now_unix: u64) -> io::Result<StateView> {
        let mut state = self.lock();
        state.step_to_in(now_unix, self.time_scale, self.offset_at(now_unix), self.sleep_window());
        state.apply(action);
        state.save(&self.path)?;
        Ok(self.view(&state))
    }

    /// マウスでなでられた。おやすみ中は起こさないので何も変わらない。
    pub fn stroke(&self, now_unix: u64) -> io::Result<StateView> {
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
        let mut state = self.lock();
        let mut next = state.clone();
        next.set_name(raw)?;
        next.save(&self.path).map_err(|e| e.to_string())?;
        *state = next;
        Ok(self.view(&state))
    }

    /// 旅立ちを見送り、次の世代の卵を迎える。
    pub fn graduate(&self) -> Result<StateView, String> {
        let mut state = self.lock();
        let mut next = state.clone();
        next.graduate()?;
        next.save(&self.path).map_err(|e| e.to_string())?;
        *state = next;
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

fn view_of(s: &GameState, utc_offset: i32, window: SleepWindow) -> StateView {
    let sleeping = is_sleeping(s.last_unix, utc_offset, window);
    let mut expression = express(&s.status);
    if sleeping {
        expression.bubble = Some(Bubble::Sleeping);
    }
    // 世話が必要なとき以外は、時間帯のあいさつ（卵・おやすみ中は言わない）。
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
        stage: s.form.stage,
        branch: s.form.branch,
        personality: s.form.personality,
        expression,
        intro: !s.intro_seen,
        hatching: s.hatching(),
        sleeping,
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
        // 常駐中（2分おき）に 90 ティック進むと、空腹になって吹き出しが変わる。
        let mut v = s.snapshot();
        for i in 1..=90u64 {
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
        assert_eq!(at(8 - 0), Some(Bubble::Morning)); // 秒 0 + 孵化の経過で、ほぼ 8 時
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
        assert_eq!((d.total, d.discovered), (109, 2));
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
        for quiet in [Bubble::Happy, Bubble::Morning, Bubble::Noon, Bubble::Evening, Bubble::Drowsy] {
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
