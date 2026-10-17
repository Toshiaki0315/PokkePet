//! お世話の質: 体重・しつけ度・お世話ミス・わがまま。進化の分岐の材料になる。

use std::ops::RangeInclusive;

use serde::{Deserialize, Serialize};

use crate::status::{Status, DIRTY_THRESHOLD};

pub const WEIGHT_START: u8 = 50;
pub const WEIGHT_MIN: u8 = 20;
pub const WEIGHT_MAX: u8 = 100;
/// 適正な体重の範囲。外れると進化の評価が下がる。
pub const IDEAL_WEIGHT: RangeInclusive<u8> = 35..=65;
const FEED_WEIGHT: u8 = 4;
const SNACK_WEIGHT: u8 = 8;
const PLAY_WEIGHT_LOSS: u8 = 3;
/// ご飯・おやつを食べると、少しあとにうんちが出る（汚れ度が増える）。増える量と、出るまでのティック数。
pub const FEED_MESS: u8 = 10;
pub const SNACK_MESS: u8 = 6;
pub const DIGEST_TICKS: u32 = 10;
/// まだ出ていない「食べたぶん」の上限（食べすぎても、一度にたまる汚れには限りがある）。
const MESS_MAX_PENDING: u8 = 40;
/// このティック数ごとに、体重が標準（`WEIGHT_START`）へ 1 近づく。
pub const WEIGHT_DRIFT_TICKS: u32 = 30;

/// 世話が必要な状態がこのティック数（減りのティック）続くたびに、お世話ミスを 1 回数える。
pub const MISS_TICKS: u32 = 30;
const MISS_PENALTY: i16 = 1;
const MISS_PENALTY_MAX: i16 = 8;
const WEIGHT_PENALTY: i16 = 10;

/// 同じ放置の中でお世話ミスがこの回数に達すると、病気になる。
pub const SICK_AFTER_MISSES: u32 = 3;
/// 病気のあいだ、ティックごとに余分に下がるご機嫌。
pub const SICK_MOOD_LOSS: u8 = 1;
const CURE_MOOD_LOSS: u8 = 5;
const SICK_WEIGHT_LOSS: u8 = 2;
const WRONG_CURE_MOOD_LOSS: u8 = 10;

/// かくしキャラに進化できる条件: お世話ミスの合計がこの回数以下。
pub const LEGEND_MAX_MISSES: u32 = 2;
/// 同じく、しつけ度がこの値以上。
pub const LEGEND_MIN_DISCIPLINE: u8 = 60;

/// 病気、またはおなかもご機嫌もゼロの状態がこのティック数（減りのティック。約 12 時間）続くと、きびしいモードではお別れになる。
pub const FATAL_TICKS: u32 = 360;
/// そのうちこのティック数に達したら「もう限界」と知らせる。
pub const DANGER_TICKS: u32 = 240;

pub const DISCIPLINE_MAX: u8 = 100;
const SCOLD_GAIN: u8 = 15;
/// しつけ度 10 ごとに、進化の評価が 1 上がる（最大 +10）。
const DISCIPLINE_PER_POINT: u8 = 10;
const RIGHT_SCOLD_MOOD_LOSS: u8 = 5;
const WRONG_SCOLD_MOOD_LOSS: u8 = 10;

/// わがままを言い出す間隔（ティック）。おなかもご機嫌も十分なときだけ。
pub const SELFISH_EVERY_TICKS: u32 = 90;
/// わがままを言い続けて待つ長さ（ティック）。過ぎると、何も起きずに収まる。
pub const SELFISH_PATIENCE_TICKS: u32 = 15;

fn start_weight() -> u8 {
    WEIGHT_START
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Care {
    /// 体重。ご飯・おやつで増え、遊ぶと減る。
    #[serde(default = "start_weight")]
    pub weight: u8,
    /// しつけ度（0〜100）。わがままを叱ると上がる。
    #[serde(default)]
    pub discipline: u8,
    /// お世話ミスの合計。
    #[serde(default)]
    pub misses: u32,
    /// 食べて、まだうんちになっていない分（汚れ度に換算）。
    #[serde(default)]
    pub pending_mess: u8,
    /// うんちが出るまでの、残りのティック数（0 なら食べたものはない）。
    #[serde(default)]
    digest_left: u32,
    /// 前の進化から数えたお世話ミス。進化の評価に使う。
    #[serde(default)]
    pub recent_misses: u32,
    /// 世話が必要な状態が続いているティック数。
    #[serde(default)]
    need_ticks: u32,
    /// いまの放置（世話が必要な状態が続いている間）で数えたお世話ミス。
    #[serde(default)]
    episode_misses: u32,
    /// 病気か。治療するまで、ご機嫌が下がり続け、放っておくとお世話ミスも増える。
    #[serde(default)]
    pub sick: bool,
    /// 病気を放っているティック数。
    #[serde(default)]
    sick_ticks: u32,
    /// 命にかかわる状態（病気、またはおなかもご機嫌もゼロ）が続いているティック数。
    #[serde(default)]
    pub fatal_ticks: u32,
    /// わがままを言っている間の、残りのティック数（0 なら言っていない）。
    #[serde(default)]
    selfish_left: u32,
}

impl Default for Care {
    fn default() -> Self {
        Self { weight: WEIGHT_START, discipline: 0, misses: 0, pending_mess: 0, digest_left: 0, recent_misses: 0, need_ticks: 0, episode_misses: 0, sick: false, sick_ticks: 0, fatal_ticks: 0, selfish_left: 0 }
    }
}

impl Care {
    pub fn fed(&mut self) {
        self.weight = self.weight.saturating_add(FEED_WEIGHT).min(WEIGHT_MAX);
        self.digest(FEED_MESS);
    }

    pub fn snacked(&mut self) {
        self.weight = self.weight.saturating_add(SNACK_WEIGHT).min(WEIGHT_MAX);
        self.digest(SNACK_MESS);
    }

    /// 食べたものが、しばらくしてうんちになる。食べ続けても、うんちが出るまでの時間は延びない。
    fn digest(&mut self, mess: u8) {
        self.pending_mess = self.pending_mess.saturating_add(mess).min(MESS_MAX_PENDING);
        if self.digest_left == 0 {
            self.digest_left = DIGEST_TICKS;
        }
    }

    /// 遊ぶと痩せる。適正体重の下限（`WEIGHT_MIN`）を下回ることはない。
    pub fn played(&mut self) {
        self.weight = self.weight.saturating_sub(PLAY_WEIGHT_LOSS).max(WEIGHT_MIN);
    }

    /// 減りのティックが 1 回進んだ。卵の間は何も数えない。
    /// 食べたものがうんちになったときは、増える汚れ度（それ以外は 0）を返す。
    pub fn tick(&mut self, status: &Status, tick_count: u32, grown: bool) -> u8 {
        if !grown {
            self.pending_mess = 0; // 卵はうんちをしない
            self.digest_left = 0;
            return 0;
        }
        self.count_misses(status);
        self.count_sickness();
        self.count_fatal(status);
        if tick_count.is_multiple_of(WEIGHT_DRIFT_TICKS) {
            self.weight = match self.weight.cmp(&WEIGHT_START) {
                std::cmp::Ordering::Less => self.weight + 1,
                std::cmp::Ordering::Greater => self.weight - 1,
                std::cmp::Ordering::Equal => self.weight,
            };
        }
        self.update_selfish(status, tick_count);
        self.digested()
    }

    /// 食べてから `DIGEST_TICKS` が経ったら、たまった分をまとめて汚れにする。
    fn digested(&mut self) -> u8 {
        if self.digest_left == 0 {
            return 0;
        }
        self.digest_left -= 1;
        if self.digest_left > 0 {
            return 0;
        }
        std::mem::take(&mut self.pending_mess)
    }

    fn count_misses(&mut self, status: &Status) {
        if !status.is_neglected() && status.dirtiness < DIRTY_THRESHOLD {
            self.need_ticks = 0;
            self.episode_misses = 0;
            return;
        }
        self.need_ticks += 1;
        if self.need_ticks >= MISS_TICKS {
            self.need_ticks = 0;
            self.add_miss();
            self.episode_misses += 1;
            // 放置が続いたとき、または部屋が汚れたままのとき、具合が悪くなる。
            if self.episode_misses >= SICK_AFTER_MISSES || status.dirtiness >= DIRTY_THRESHOLD {
                self.fall_sick();
            }
        }
    }

    fn add_miss(&mut self) {
        self.misses += 1;
        self.recent_misses += 1;
    }

    fn fall_sick(&mut self) {
        if !self.sick {
            self.sick = true;
            self.sick_ticks = 0;
            self.weight = self.weight.saturating_sub(SICK_WEIGHT_LOSS).max(WEIGHT_MIN);
        }
    }

    fn count_fatal(&mut self, status: &Status) {
        if self.sick || (status.hunger == 0 && status.mood == 0) {
            self.fatal_ticks += 1;
        } else {
            self.fatal_ticks = 0;
        }
    }

    /// 命にかかわる状態が長く続いたか。
    pub fn is_fatal(&self) -> bool {
        self.fatal_ticks >= FATAL_TICKS
    }

    /// そろそろ限界か（知らせる段階）。
    pub fn in_danger(&self) -> bool {
        self.fatal_ticks >= DANGER_TICKS
    }

    /// 病気を放っておくと、その間もお世話ミスが増える。
    fn count_sickness(&mut self) {
        if !self.sick {
            return;
        }
        self.sick_ticks += 1;
        if self.sick_ticks >= MISS_TICKS {
            self.sick_ticks = 0;
            self.add_miss();
        }
    }

    /// 治療する（注射）。病気のときだけ治る（注射は痛いので機嫌は少し下がる）。
    /// 病気でないのに治療すると、機嫌が大きく下がるだけ。治ったかを返す。
    pub fn cure(&mut self, status: &mut Status) -> bool {
        if self.sick {
            self.sick = false;
            self.sick_ticks = 0;
            self.episode_misses = 0;
            status.mood = status.mood.saturating_sub(CURE_MOOD_LOSS);
            return true;
        }
        status.mood = status.mood.saturating_sub(WRONG_CURE_MOOD_LOSS);
        false
    }

    fn update_selfish(&mut self, status: &Status, tick_count: u32) {
        if self.selfish_left > 0 {
            self.selfish_left -= 1;
            return;
        }
        if tick_count.is_multiple_of(SELFISH_EVERY_TICKS) && status.is_content() {
            self.selfish_left = SELFISH_PATIENCE_TICKS;
        }
    }

    /// いま、わがままを言っているか。
    pub fn is_selfish(&self) -> bool {
        self.selfish_left > 0
    }

    /// 叱る。わがままを言っているときだけ、しつけ度が上がる（機嫌は少し下がる）。
    /// 理由のない叱りは、機嫌が大きく下がるだけ。叱ったのが正しかったか（わがままだったか）を返す。
    pub fn scold(&mut self, status: &mut Status) -> bool {
        if self.is_selfish() {
            self.selfish_left = 0;
            self.discipline = self.discipline.saturating_add(SCOLD_GAIN).min(DISCIPLINE_MAX);
            status.mood = status.mood.saturating_sub(RIGHT_SCOLD_MOOD_LOSS);
            return true;
        }
        status.mood = status.mood.saturating_sub(WRONG_SCOLD_MOOD_LOSS);
        false
    }

    /// 進化した。次の評価は、ここからのお世話ミスで決まる。
    pub fn evolved(&mut self) {
        self.recent_misses = 0;
    }

    /// 体重が適正範囲か。
    pub fn weight_is_ideal(&self) -> bool {
        IDEAL_WEIGHT.contains(&self.weight)
    }

    /// かくしキャラになれるお世話か: ミスがほとんどなく、しつけがよく、体重が適正。
    pub fn is_legendary(&self) -> bool {
        self.misses <= LEGEND_MAX_MISSES && self.discipline >= LEGEND_MIN_DISCIPLINE && self.weight_is_ideal()
    }

    /// 進化の評価（ご機嫌と親密度の平均）への加減点。しつけ度で加点、ミスと体重の外れで減点。
    pub fn score_adjustment(&self) -> i16 {
        let bonus = i16::from(self.discipline / DISCIPLINE_PER_POINT);
        let misses = i16::try_from(self.recent_misses).unwrap_or(i16::MAX).saturating_mul(MISS_PENALTY).min(MISS_PENALTY_MAX);
        let weight = if self.weight_is_ideal() { 0 } else { WEIGHT_PENALTY };
        bonus - misses - weight
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hungry() -> Status {
        Status::new(80, 50, 10, 0)
    }

    fn content() -> Status {
        Status::new(90, 50, 90, 0)
    }

    /// ティックを `n` 回進めて、出た汚れの合計を返す。
    fn digest_for(c: &mut Care, n: u32) -> u32 {
        (1..=n).map(|i| u32::from(c.tick(&content(), i, true))).sum()
    }

    #[test]
    fn food_becomes_poop_a_little_while_after_eating() {
        let mut c = Care::default();
        c.fed();
        assert_eq!(c.pending_mess, FEED_MESS);
        assert_eq!(digest_for(&mut c, DIGEST_TICKS - 1), 0, "not yet");
        assert_eq!(u32::from(c.tick(&content(), DIGEST_TICKS, true)), u32::from(FEED_MESS));
        assert_eq!(c.pending_mess, 0);
        assert_eq!(digest_for(&mut c, 50), 0, "and nothing more until the next meal");
    }

    #[test]
    fn a_snack_leaves_less_than_a_meal_and_eating_again_does_not_delay_the_poop() {
        let mut c = Care::default();
        c.snacked();
        digest_for(&mut c, 4);
        c.fed(); // 食べ足しても、最初のぶんの時間で出る
        assert_eq!(digest_for(&mut c, DIGEST_TICKS - 4), u32::from(FEED_MESS + SNACK_MESS));
    }

    #[test]
    fn stuffing_the_pet_has_a_limit_on_the_mess() {
        let mut c = Care::default();
        for _ in 0..20 {
            c.fed();
        }
        assert_eq!(c.pending_mess, 40);
        assert_eq!(digest_for(&mut c, DIGEST_TICKS), 40);
    }

    #[test]
    fn eggs_do_not_poop_even_if_fed() {
        let mut c = Care::default();
        c.fed();
        assert_eq!(c.tick(&content(), 1, false), 0);
        assert_eq!(c.pending_mess, 0);
        assert_eq!(digest_for(&mut c, DIGEST_TICKS * 2), 0);
    }

    #[test]
    fn a_new_pet_has_a_standard_weight_and_no_discipline() {
        let c = Care::default();
        assert_eq!((c.weight, c.discipline, c.misses, c.recent_misses), (50, 0, 0, 0));
        assert!(c.weight_is_ideal());
        assert_eq!(c.score_adjustment(), 0);
    }

    #[test]
    fn eating_adds_weight_and_playing_takes_it_off() {
        let mut c = Care::default();
        c.fed();
        assert_eq!(c.weight, 54);
        c.snacked();
        assert_eq!(c.weight, 62);
        c.played();
        assert_eq!(c.weight, 59);
    }

    #[test]
    fn weight_stays_between_the_minimum_and_the_maximum() {
        let mut c = Care { weight: 99, ..Care::default() };
        c.snacked();
        assert_eq!(c.weight, WEIGHT_MAX);
        let mut c = Care { weight: 21, ..Care::default() };
        c.played();
        assert_eq!(c.weight, WEIGHT_MIN);
        c.played();
        assert_eq!(c.weight, WEIGHT_MIN);
    }

    #[test]
    fn weight_drifts_back_toward_standard_once_an_hour() {
        let mut c = Care { weight: 60, ..Care::default() };
        c.tick(&content(), 1, true);
        assert_eq!(c.weight, 60);
        c.tick(&content(), WEIGHT_DRIFT_TICKS, true);
        assert_eq!(c.weight, 59);
        let mut thin = Care { weight: 40, ..Care::default() };
        thin.tick(&content(), WEIGHT_DRIFT_TICKS, true);
        assert_eq!(thin.weight, 41);
        let mut std = Care::default();
        std.tick(&content(), WEIGHT_DRIFT_TICKS, true);
        assert_eq!(std.weight, 50);
    }

    #[test]
    fn eggs_do_not_count_anything() {
        let mut c = Care::default();
        for i in 1..=200 {
            c.tick(&hungry(), i, false);
        }
        assert_eq!(c, Care::default());
    }

    #[test]
    fn a_need_left_alone_for_a_while_is_a_care_miss_and_keeps_counting() {
        let mut c = Care::default();
        for i in 1..MISS_TICKS {
            c.tick(&hungry(), i, true);
        }
        assert_eq!(c.misses, 0);
        c.tick(&hungry(), MISS_TICKS, true);
        assert_eq!((c.misses, c.recent_misses), (1, 1));
        for i in 0..MISS_TICKS {
            c.tick(&hungry(), 100 + i, true);
        }
        assert_eq!(c.misses, 2);
    }

    #[test]
    fn answering_a_need_in_time_is_not_a_miss() {
        let mut c = Care::default();
        for i in 1..MISS_TICKS {
            c.tick(&hungry(), i, true);
        }
        c.tick(&content(), 50, true); // 世話をした
        for i in 1..MISS_TICKS {
            c.tick(&hungry(), 60 + i, true);
        }
        assert_eq!(c.misses, 0);
    }

    #[test]
    fn a_content_pet_asks_for_attention_every_so_often_and_then_gives_up() {
        let mut c = Care::default();
        c.tick(&content(), SELFISH_EVERY_TICKS - 1, true);
        assert!(!c.is_selfish());
        c.tick(&content(), SELFISH_EVERY_TICKS, true);
        assert!(c.is_selfish());
        for i in 1..SELFISH_PATIENCE_TICKS {
            c.tick(&content(), SELFISH_EVERY_TICKS + i, true);
            assert!(c.is_selfish(), "still asking at {i}");
        }
        c.tick(&content(), SELFISH_EVERY_TICKS + SELFISH_PATIENCE_TICKS, true);
        assert!(!c.is_selfish());
        assert_eq!(c.discipline, 0, "ignoring it costs nothing");
    }

    #[test]
    fn a_hungry_pet_is_not_selfish() {
        let mut c = Care::default();
        c.tick(&hungry(), SELFISH_EVERY_TICKS, true);
        assert!(!c.is_selfish());
    }

    #[test]
    fn scolding_a_selfish_pet_teaches_it() {
        let mut c = Care::default();
        c.tick(&content(), SELFISH_EVERY_TICKS, true);
        let mut s = content();
        assert!(c.scold(&mut s));
        assert_eq!((c.discipline, s.mood, c.is_selfish()), (15, 85, false));
    }

    #[test]
    fn scolding_for_no_reason_only_upsets_the_pet() {
        let mut c = Care::default();
        let mut s = content();
        assert!(!c.scold(&mut s));
        assert_eq!((c.discipline, s.mood), (0, 80));
        let mut gloomy = Status::new(4, 50, 50, 0);
        c.scold(&mut gloomy);
        assert_eq!(gloomy.mood, 0);
    }

    #[test]
    fn discipline_stops_at_the_maximum() {
        let mut c = Care { discipline: 95, ..Care::default() };
        c.tick(&content(), SELFISH_EVERY_TICKS, true);
        c.scold(&mut content());
        assert_eq!(c.discipline, DISCIPLINE_MAX);
    }

    #[test]
    fn the_score_rewards_discipline_and_punishes_misses_and_weight() {
        assert_eq!(Care { discipline: 100, ..Care::default() }.score_adjustment(), 10);
        assert_eq!(Care { discipline: 45, ..Care::default() }.score_adjustment(), 4);
        assert_eq!(Care { recent_misses: 3, ..Care::default() }.score_adjustment(), -3);
        assert_eq!(Care { recent_misses: 99, ..Care::default() }.score_adjustment(), -8);
        assert_eq!(Care { weight: 66, ..Care::default() }.score_adjustment(), -10);
        assert_eq!(Care { weight: 34, ..Care::default() }.score_adjustment(), -10);
        assert_eq!(Care { weight: 65, discipline: 100, recent_misses: 1, ..Care::default() }.score_adjustment(), 9);
    }

    #[test]
    fn a_legendary_care_record_needs_few_misses_good_discipline_and_a_healthy_weight() {
        let ok = Care { misses: 2, discipline: 60, ..Care::default() };
        assert!(ok.is_legendary());
        assert!(!Care { misses: 3, ..ok.clone() }.is_legendary());
        assert!(!Care { discipline: 59, ..ok.clone() }.is_legendary());
        assert!(!Care { weight: 70, ..ok }.is_legendary());
    }

    #[test]
    fn a_sick_pet_left_alone_reaches_the_danger_zone_and_then_the_end() {
        let mut c = Care { sick: true, ..Care::default() };
        neglect(&mut c, &content(), DANGER_TICKS - 1);
        assert!(!c.in_danger());
        neglect(&mut c, &content(), 1);
        assert!(c.in_danger() && !c.is_fatal());
        let mut c = Care { sick: true, ..Care::default() };
        c.fatal_ticks = FATAL_TICKS - 1;
        c.tick(&content(), 1, true);
        assert!(c.is_fatal());
    }

    #[test]
    fn being_starved_of_food_and_mood_is_also_fatal_but_recovery_resets_it() {
        let mut c = Care::default();
        let starving = Status::new(0, 50, 0, 0);
        neglect(&mut c, &starving, 100);
        assert_eq!(c.fatal_ticks, 100);
        assert!(c.sick, "starving also makes the pet ill");
        c.cure(&mut content());
        c.tick(&content(), 101, true); // 治療して、世話もした
        assert_eq!(c.fatal_ticks, 0);
        let mut only_hungry = Care::default();
        neglect(&mut only_hungry, &Status::new(50, 50, 0, 0), 50);
        assert_eq!(only_hungry.fatal_ticks, 0, "hunger alone is not fatal");
    }

    #[test]
    fn a_cure_stops_the_countdown() {
        let mut c = Care { sick: true, fatal_ticks: 300, ..Care::default() };
        c.cure(&mut content());
        c.tick(&content(), 1, true);
        assert_eq!(c.fatal_ticks, 0);
    }

    #[test]
    fn evolving_clears_only_the_recent_misses() {
        let mut c = Care { misses: 5, recent_misses: 3, ..Care::default() };
        c.evolved();
        assert_eq!((c.misses, c.recent_misses), (5, 0));
    }

    fn dirty() -> Status {
        Status::new(80, 50, 80, DIRTY_THRESHOLD)
    }

    fn neglect(c: &mut Care, status: &Status, ticks: u32) {
        for i in 1..=ticks {
            c.tick(status, i, true);
        }
    }

    #[test]
    fn a_need_left_for_a_long_time_makes_the_pet_sick() {
        let mut c = Care::default();
        neglect(&mut c, &hungry(), MISS_TICKS * (SICK_AFTER_MISSES - 1));
        assert!(!c.sick);
        neglect(&mut c, &hungry(), MISS_TICKS);
        assert!(c.sick);
        assert!(c.weight < WEIGHT_START, "being ill takes weight off");
    }

    #[test]
    fn answering_the_need_resets_the_run_towards_sickness() {
        let mut c = Care::default();
        neglect(&mut c, &hungry(), MISS_TICKS * (SICK_AFTER_MISSES - 1));
        neglect(&mut c, &content(), 1); // 世話をした
        neglect(&mut c, &hungry(), MISS_TICKS * (SICK_AFTER_MISSES - 1));
        assert!(!c.sick);
    }

    #[test]
    fn a_dirty_room_counts_as_neglect_and_makes_the_pet_sick_at_the_first_miss() {
        let mut c = Care::default();
        neglect(&mut c, &dirty(), MISS_TICKS - 1);
        assert_eq!((c.misses, c.sick), (0, false));
        neglect(&mut c, &dirty(), 1);
        assert_eq!((c.misses, c.sick), (1, true));
    }

    #[test]
    fn an_untreated_illness_keeps_adding_care_misses() {
        let mut c = Care { sick: true, ..Care::default() };
        neglect(&mut c, &content(), MISS_TICKS * 2);
        assert_eq!(c.misses, 2);
        assert!(c.sick);
    }

    #[test]
    fn being_ill_only_happens_once_until_cured() {
        let mut c = Care::default();
        c.fall_sick();
        c.fall_sick();
        assert_eq!(c.weight, 48);
    }

    #[test]
    fn a_shot_cures_an_ill_pet_but_costs_some_mood() {
        let mut c = Care { sick: true, ..Care::default() };
        let mut s = Status::new(50, 50, 50, 0);
        assert!(c.cure(&mut s));
        assert_eq!((c.sick, s.mood), (false, 45));
        // 治ったあとは、また放置が続くまで病気にならない
        neglect(&mut c, &hungry(), MISS_TICKS * (SICK_AFTER_MISSES - 1));
        assert!(!c.sick);
    }

    #[test]
    fn a_shot_for_a_healthy_pet_only_upsets_it() {
        let mut c = Care::default();
        let mut s = Status::new(50, 50, 50, 0);
        assert!(!c.cure(&mut s));
        assert_eq!(s.mood, 40);
    }

    #[test]
    fn old_saves_without_care_fields_load_with_standard_values() {
        let c: Care = serde_json::from_str("{}").unwrap();
        assert_eq!(c, Care::default());
    }
}
