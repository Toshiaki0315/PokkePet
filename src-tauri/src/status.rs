//! キャラクターの内部パラメータとユーザーアクションによる変動。
//!
//! `hunger` は「満腹側が大きい」値として扱う（ご飯で回復＝増加、遊ぶと減少）。

pub const PARAM_MAX: u8 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Status {
    pub mood: u8,
    pub affinity: u8,
    pub hunger: u8,
    pub dirtiness: u8,
}

const FEED_HUNGER: u8 = 30;
const FEED_MOOD: u8 = 3;
const FEED_AFFINITY: u8 = 2;
const PLAY_MOOD: u8 = 15;
const PLAY_AFFINITY: u8 = 5;
const PLAY_HUNGER_COST: u8 = 10;

impl Status {
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

    pub fn clean(&mut self) {
        self.dirtiness = 0;
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
        assert_eq!((s.hunger, s.mood, s.affinity), (80, 53, 52));
        assert_eq!(s.dirtiness, 50);
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
        assert_eq!((s.mood, s.affinity, s.hunger), (65, 55, 40));
        assert_eq!(s.dirtiness, 50);
    }

    #[test]
    fn play_saturates_at_both_bounds() {
        let mut s = Status::new(95, 98, 5, 0);
        s.play();
        assert_eq!((s.mood, s.affinity, s.hunger), (100, 100, 0));
    }

    #[test]
    fn clean_resets_dirtiness_only() {
        let mut s = base();
        s.clean();
        assert_eq!(s, Status { mood: 50, affinity: 50, hunger: 50, dirtiness: 0 });
    }
}
