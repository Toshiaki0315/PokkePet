//! キャラクターの内部パラメータとユーザーアクションによる変動。
//!
//! `hunger` は「満腹側が大きい」値として扱う（ご飯で回復＝増加、遊ぶと減少）。

use serde::{Deserialize, Serialize};

use crate::expression::LOW_MAX;

pub const PARAM_MAX: u8 = 100;
const AFFINITY_LOSS: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Status {
    pub mood: u8,
    pub affinity: u8,
    pub hunger: u8,
    pub dirtiness: u8,
}

const FEED_HUNGER: u8 = 40;
const FEED_MOOD: u8 = 5;
const FEED_AFFINITY: u8 = 2;
const PLAY_MOOD: u8 = 25;
const PLAY_AFFINITY: u8 = 5;
const PLAY_HUNGER_COST: u8 = 5;
const SNACK_MOOD: u8 = 15;
const SNACK_HUNGER: u8 = 10;
const SNACK_AFFINITY: u8 = 2;
/// 空腹度（満腹側が大きい）がこの値以上のときにおやつをあげると、食べすぎになる。
pub const SNACK_FULL_AT: u8 = 80;
const OVEREAT_MOOD_LOSS: u8 = 10;
const WIN_MOOD: u8 = 10;
const WIN_AFFINITY: u8 = 3;
const PET_MOOD: u8 = 10;
const PET_AFFINITY: u8 = 3;
const STROKE_MOOD: u8 = 2;
const STROKE_AFFINITY: u8 = 1;
/// 汚れ度がこの値以上だと、ティックごとのご機嫌低下が大きくなる。
pub const DIRTY_THRESHOLD: u8 = 70;

impl Status {
    /// 世話が足りていない状態か（おなかがすいている、またはご機嫌が低い）。
    pub fn is_neglected(&self) -> bool {
        self.hunger <= LOW_MAX || self.mood <= LOW_MAX
    }

    /// ほったらかしにされて、親密度が少し下がる。
    pub fn lose_affinity(&mut self) {
        self.affinity = self.affinity.saturating_sub(AFFINITY_LOSS);
    }

    pub fn new(mood: u8, affinity: u8, hunger: u8, dirtiness: u8) -> Self {
        Self {
            mood: mood.min(PARAM_MAX),
            affinity: affinity.min(PARAM_MAX),
            hunger: hunger.min(PARAM_MAX),
            dirtiness: dirtiness.min(PARAM_MAX),
        }
    }

    pub fn feed(&mut self) {
        self.hunger = raise(self.hunger, FEED_HUNGER);
        self.mood = raise(self.mood, FEED_MOOD);
        self.affinity = raise(self.affinity, FEED_AFFINITY);
    }

    pub fn play(&mut self) {
        self.mood = raise(self.mood, PLAY_MOOD);
        self.affinity = raise(self.affinity, PLAY_AFFINITY);
        self.hunger = self.hunger.saturating_sub(PLAY_HUNGER_COST);
    }

    /// おやつ。おなかに余裕があれば喜ぶが、満腹のときは食べすぎてご機嫌が下がる。
    /// 喜んだなら true、食べすぎなら false。
    pub fn snack(&mut self) -> bool {
        if self.hunger >= SNACK_FULL_AT {
            self.mood = self.mood.saturating_sub(OVEREAT_MOOD_LOSS);
            return false;
        }
        self.mood = raise(self.mood, SNACK_MOOD);
        self.hunger = raise(self.hunger, SNACK_HUNGER);
        self.affinity = raise(self.affinity, SNACK_AFFINITY);
        true
    }

    /// ミニゲームに勝ったときのボーナス（通常の「遊ぶ」の効果に上乗せ）。
    pub fn win_bonus(&mut self) {
        self.mood = raise(self.mood, WIN_MOOD);
        self.affinity = raise(self.affinity, WIN_AFFINITY);
    }

    /// 撫でる（卵の間の「遊ぶ」）。ご機嫌と親密度が少し上がり、空腹度は減らない。
    pub fn pet(&mut self) {
        self.mood = raise(self.mood, PET_MOOD);
        self.affinity = raise(self.affinity, PET_AFFINITY);
    }

    /// マウスでなでられた。ほんの少しだけご機嫌と親密度が上がる。
    pub fn stroke(&mut self) {
        self.mood = raise(self.mood, STROKE_MOOD);
        self.affinity = raise(self.affinity, STROKE_AFFINITY);
    }

    pub fn clean(&mut self) {
        self.dirtiness = 0;
    }

    /// 1ティック分の時間経過。空腹度↓、汚れ度↑、ご機嫌↓（汚れ放置時は減少が大きい）。
    pub fn tick(&mut self) {
        let mood_loss = if self.dirtiness >= DIRTY_THRESHOLD { 2 } else { 1 };
        self.mood = self.mood.saturating_sub(mood_loss);
        self.hunger = self.hunger.saturating_sub(1);
        self.dirtiness = raise(self.dirtiness, 1);
    }
}

fn raise(value: u8, amount: u8) -> u8 {
    value.saturating_add(amount).min(PARAM_MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Status {
        Status::new(50, 50, 50, 50)
    }

    #[test]
    fn new_clamps_values_to_max() {
        let s = Status::new(200, 101, 255, 100);
        assert_eq!(s, Status { mood: 100, affinity: 100, hunger: 100, dirtiness: 100 });
    }

    #[test]
    fn new_keeps_values_in_range() {
        assert_eq!(base(), Status { mood: 50, affinity: 50, hunger: 50, dirtiness: 50 });
    }

    #[test]
    fn feed_restores_hunger_and_slightly_raises_mood_and_affinity() {
        let mut s = base();
        s.feed();
        assert_eq!((s.hunger, s.mood, s.affinity), (90, 55, 52));
        assert_eq!(s.dirtiness, 50);
    }

    #[test]
    fn stroking_raises_mood_and_affinity_a_little_and_saturates() {
        let mut s = base();
        s.stroke();
        assert_eq!((s.mood, s.affinity, s.hunger, s.dirtiness), (52, 51, 50, 50));
        let mut full = Status::new(99, 100, 50, 0);
        full.stroke();
        assert_eq!((full.mood, full.affinity), (100, 100));
    }

    #[test]
    fn feed_saturates_at_max() {
        let mut s = Status::new(99, 99, 90, 0);
        s.feed();
        assert_eq!((s.hunger, s.mood, s.affinity), (100, 100, 100));
    }

    #[test]
    fn play_raises_mood_and_affinity_and_lowers_hunger() {
        let mut s = base();
        s.play();
        assert_eq!((s.mood, s.affinity, s.hunger), (75, 55, 45));
        assert_eq!(s.dirtiness, 50);
    }

    #[test]
    fn play_saturates_at_both_bounds() {
        let mut s = Status::new(95, 98, 5, 0);
        s.play();
        assert_eq!((s.mood, s.affinity, s.hunger), (100, 100, 0));
    }

    #[test]
    fn tick_lowers_hunger_and_mood_and_raises_dirtiness() {
        let mut s = base();
        s.tick();
        assert_eq!((s.hunger, s.dirtiness, s.mood, s.affinity), (49, 51, 49, 50));
    }

    #[test]
    fn tick_lowers_mood_faster_when_room_is_dirty() {
        let mut s = Status::new(50, 50, 50, DIRTY_THRESHOLD);
        s.tick();
        assert_eq!(s.mood, 48);
    }

    #[test]
    fn tick_is_gentle_just_below_dirty_threshold() {
        let mut s = Status::new(50, 50, 50, DIRTY_THRESHOLD - 2);
        s.tick();
        assert_eq!(s.mood, 49);
    }

    #[test]
    fn neglect_means_hungry_or_gloomy() {
        assert!(!Status::new(31, 50, 31, 0).is_neglected());
        assert!(Status::new(30, 50, 80, 0).is_neglected());
        assert!(Status::new(80, 50, 30, 0).is_neglected());
    }

    #[test]
    fn losing_affinity_goes_down_one_and_stops_at_zero() {
        let mut s = Status::new(50, 1, 50, 0);
        s.lose_affinity();
        assert_eq!(s.affinity, 0);
        s.lose_affinity();
        assert_eq!(s.affinity, 0);
    }

    #[test]
    fn tick_saturates_at_bounds() {
        let mut s = Status::new(0, 50, 0, 100);
        s.tick();
        assert_eq!((s.mood, s.hunger, s.dirtiness), (0, 0, 100));
    }

    #[test]
    fn a_snack_cheers_the_pet_up_a_lot_and_fills_it_a_little() {
        let mut s = base();
        assert!(s.snack());
        assert_eq!(s, Status { mood: 65, affinity: 52, hunger: 60, dirtiness: 50 });
    }

    #[test]
    fn a_snack_is_still_enjoyed_just_below_the_full_mark() {
        let mut s = Status::new(50, 50, SNACK_FULL_AT - 1, 0);
        assert!(s.snack());
        assert_eq!((s.mood, s.hunger), (65, SNACK_FULL_AT + 9));
    }

    #[test]
    fn a_snack_when_full_causes_overeating_and_a_worse_mood() {
        let mut s = Status::new(50, 50, SNACK_FULL_AT, 0);
        assert!(!s.snack());
        assert_eq!(s, Status { mood: 40, affinity: 50, hunger: SNACK_FULL_AT, dirtiness: 0 });
    }

    #[test]
    fn overeating_cannot_push_the_mood_below_zero() {
        let mut s = Status::new(4, 0, 100, 0);
        assert!(!s.snack());
        assert_eq!(s.mood, 0);
    }

    #[test]
    fn a_snack_saturates_at_the_maximum() {
        let mut s = Status::new(95, 99, 75, 0);
        assert!(s.snack());
        assert_eq!((s.mood, s.affinity, s.hunger), (100, 100, 85));
    }

    #[test]
    fn winning_a_game_adds_mood_and_affinity() {
        let mut s = base();
        s.win_bonus();
        assert_eq!(s, Status { mood: 60, affinity: 53, hunger: 50, dirtiness: 50 });
    }

    #[test]
    fn the_win_bonus_saturates() {
        let mut s = Status::new(98, 99, 10, 0);
        s.win_bonus();
        assert_eq!((s.mood, s.affinity), (100, 100));
    }

    #[test]
    fn pet_raises_mood_and_affinity_without_costing_hunger() {
        let mut s = base();
        s.pet();
        assert_eq!(s, Status { mood: 60, affinity: 53, hunger: 50, dirtiness: 50 });
    }

    #[test]
    fn pet_saturates_at_max() {
        let mut s = Status::new(95, 99, 10, 0);
        s.pet();
        assert_eq!((s.mood, s.affinity), (100, 100));
    }

    #[test]
    fn clean_resets_dirtiness_only() {
        let mut s = base();
        s.clean();
        assert_eq!(s, Status { mood: 50, affinity: 50, hunger: 50, dirtiness: 0 });
    }
}
