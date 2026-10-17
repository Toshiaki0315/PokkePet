//! お世話の質: 体重・しつけ度・お世話ミス・わがまま。進化の分岐の材料になる。

use std::ops::RangeInclusive;

use serde::{Deserialize, Serialize};

use crate::status::Status;

pub const WEIGHT_START: u8 = 50;
pub const WEIGHT_MIN: u8 = 20;
pub const WEIGHT_MAX: u8 = 100;
/// 適正な体重の範囲。外れると進化の評価が下がる。
pub const IDEAL_WEIGHT: RangeInclusive<u8> = 35..=65;
const FEED_WEIGHT: u8 = 4;
const SNACK_WEIGHT: u8 = 8;
const PLAY_WEIGHT_LOSS: u8 = 3;
/// このティック数ごとに、体重が標準（`WEIGHT_START`）へ 1 近づく。
pub const WEIGHT_DRIFT_TICKS: u32 = 30;

/// 世話が必要な状態がこのティック数（減りのティック）続くたびに、お世話ミスを 1 回数える。
pub const MISS_TICKS: u32 = 30;
const MISS_PENALTY: i16 = 1;
const MISS_PENALTY_MAX: i16 = 8;
const WEIGHT_PENALTY: i16 = 10;

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
    /// 前の進化から数えたお世話ミス。進化の評価に使う。
    #[serde(default)]
    pub recent_misses: u32,
    /// 世話が必要な状態が続いているティック数。
    #[serde(default)]
    need_ticks: u32,
    /// わがままを言っている間の、残りのティック数（0 なら言っていない）。
    #[serde(default)]
    selfish_left: u32,
}

impl Default for Care {
    fn default() -> Self {
        Self { weight: WEIGHT_START, discipline: 0, misses: 0, recent_misses: 0, need_ticks: 0, selfish_left: 0 }
    }
}

impl Care {
    pub fn fed(&mut self) {
        self.weight = self.weight.saturating_add(FEED_WEIGHT).min(WEIGHT_MAX);
    }

    pub fn snacked(&mut self) {
        self.weight = self.weight.saturating_add(SNACK_WEIGHT).min(WEIGHT_MAX);
    }

    /// 遊ぶと痩せる。適正体重の下限（`WEIGHT_MIN`）を下回ることはない。
    pub fn played(&mut self) {
        self.weight = self.weight.saturating_sub(PLAY_WEIGHT_LOSS).max(WEIGHT_MIN);
    }

    /// 減りのティックが 1 回進んだ。卵の間は何も数えない。
    pub fn tick(&mut self, status: &Status, tick_count: u32, grown: bool) {
        if !grown {
            return;
        }
        self.count_misses(status);
        if tick_count.is_multiple_of(WEIGHT_DRIFT_TICKS) {
            self.weight = match self.weight.cmp(&WEIGHT_START) {
                std::cmp::Ordering::Less => self.weight + 1,
                std::cmp::Ordering::Greater => self.weight - 1,
                std::cmp::Ordering::Equal => self.weight,
            };
        }
        self.update_selfish(status, tick_count);
    }

    fn count_misses(&mut self, status: &Status) {
        if !status.is_neglected() {
            self.need_ticks = 0;
            return;
        }
        self.need_ticks += 1;
        if self.need_ticks >= MISS_TICKS {
            self.need_ticks = 0;
            self.misses += 1;
            self.recent_misses += 1;
        }
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
    fn evolving_clears_only_the_recent_misses() {
        let mut c = Care { misses: 5, recent_misses: 3, ..Care::default() };
        c.evolved();
        assert_eq!((c.misses, c.recent_misses), (5, 0));
    }

    #[test]
    fn old_saves_without_care_fields_load_with_standard_values() {
        let c: Care = serde_json::from_str("{}").unwrap();
        assert_eq!(c, Care::default());
    }
}
