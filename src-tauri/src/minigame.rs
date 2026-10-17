//! ミニゲーム「どっちの手？」: ペットがおやつを隠した手を当てる。勝敗の判定と乱数をここに持つ。

use std::str::FromStr;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Left,
    Right,
}

impl FromStr for Side {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "left" => Ok(Side::Left),
            "right" => Ok(Side::Right),
            other => Err(format!("unknown side: {other}")),
        }
    }
}

/// 当たりか。
pub fn is_win(guess: Side, answer: Side) -> bool {
    guess == answer
}

/// 乱数（SplitMix64）。小さくて依存が要らず、種を決めれば結果が再現できるのでテストしやすい。
#[derive(Debug, Clone)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// 0 以上 `n` 未満の整数（`n` が 0 なら 0）。
    pub fn next_below(&mut self, n: u64) -> u64 {
        if n == 0 {
            return 0;
        }
        self.next_u64() % n
    }

    /// コイン投げ（true か false を半々で）。
    pub fn next_bool(&mut self) -> bool {
        self.next_u64() >> 63 == 0
    }

    /// 答えの手をランダムに選ぶ。
    pub fn next_side(&mut self) -> Side {
        if self.next_u64() >> 63 == 0 {
            return Side::Left;
        }
        Side::Right
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_below_a_limit_stay_below_it() {
        let mut rng = Rng::new(3);
        assert!((0..500).all(|_| rng.next_below(7) < 7));
        assert_eq!(rng.next_below(0), 0);
        assert_eq!(rng.next_below(1), 0);
    }

    #[test]
    fn coin_flips_are_reproducible_and_land_both_ways() {
        let flips = |seed| {
            let mut rng = Rng::new(seed);
            (0..64).map(|_| rng.next_bool()).collect::<Vec<_>>()
        };
        assert_eq!(flips(5), flips(5));
        assert!(flips(5).contains(&true) && flips(5).contains(&false));
    }

    #[test]
    fn sides_are_parsed_from_their_names() {
        assert_eq!("left".parse::<Side>(), Ok(Side::Left));
        assert_eq!("right".parse::<Side>(), Ok(Side::Right));
        assert!("up".parse::<Side>().is_err());
        assert!("".parse::<Side>().is_err());
    }

    #[test]
    fn sides_serialize_in_lowercase() {
        assert_eq!(serde_json::to_value(Side::Left).unwrap(), "left");
        assert_eq!(serde_json::to_value(Side::Right).unwrap(), "right");
    }

    #[test]
    fn a_guess_wins_only_when_it_matches() {
        assert!(is_win(Side::Left, Side::Left));
        assert!(is_win(Side::Right, Side::Right));
        assert!(!is_win(Side::Left, Side::Right));
        assert!(!is_win(Side::Right, Side::Left));
    }

    #[test]
    fn splitmix64_matches_the_reference_sequence() {
        // 種 0 の公式な出力（https://prng.di.unimi.it/splitmix64.c）
        let mut rng = Rng::new(0);
        assert_eq!(rng.next_u64(), 0xE220_A839_7B1D_CDAF);
        assert_eq!(rng.next_u64(), 0x6E78_9E6A_A1B9_65F4);
        assert_eq!(rng.next_u64(), 0x06C4_5D18_8009_454F);
    }

    #[test]
    fn the_same_seed_gives_the_same_sides() {
        let a: Vec<Side> = (0..20).scan(Rng::new(42), |r, _| Some(r.next_side())).collect();
        let b: Vec<Side> = (0..20).scan(Rng::new(42), |r, _| Some(r.next_side())).collect();
        assert_eq!(a, b);
    }

    #[test]
    fn both_sides_come_up_about_equally_often() {
        let mut rng = Rng::new(2026);
        let lefts = (0..10_000).filter(|_| rng.next_side() == Side::Left).count();
        assert!((4_700..=5_300).contains(&lefts), "{lefts}");
    }

    #[test]
    fn different_seeds_give_different_sequences() {
        let a: Vec<Side> = (0..32).scan(Rng::new(1), |r, _| Some(r.next_side())).collect();
        let b: Vec<Side> = (0..32).scan(Rng::new(2), |r, _| Some(r.next_side())).collect();
        assert_ne!(a, b);
    }
}
