//! 成長段階の進行と、進化時の分岐判定。

use serde::{Deserialize, Serialize};

use crate::status::Status;

pub const MAX_STAGE: u8 = 10;
/// 1段階あたりの成長に必要な秒数。
pub const STAGE_SECS: u32 = 3600;
const GOOD_MIN_AVG: u16 = 70;
const POOR_MAX_AVG: u16 = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Personality {
    Calm,
    Playful,
    Gluttonous,
    Tidy,
}

/// 行動履歴（性格形成の材料）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct History {
    pub feeds: u32,
    pub plays: u32,
    pub cleans: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Branch {
    Poor,
    Normal,
    Good,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Form {
    pub stage: u8,
    pub branch: Branch,
    pub personality: Personality,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Growth {
    pub stage: u8,
    pub elapsed_secs: u32,
}

impl Growth {
    pub fn egg() -> Self {
        Self { stage: 1, elapsed_secs: 0 }
    }

    /// 経過時間を加算し、次の段階へ進んだ場合 true を返す。最終段階では進まない。
    pub fn advance(&mut self, secs: u32) -> bool {
        if self.stage >= MAX_STAGE {
            return false;
        }
        self.elapsed_secs = self.elapsed_secs.saturating_add(secs);
        if self.elapsed_secs < STAGE_SECS {
            return false;
        }
        self.elapsed_secs -= STAGE_SECS;
        self.stage += 1;
        true
    }
}

/// 履歴から性格を決定する。最多の行動に対応し、同数・全0なら Calm。
pub fn personality_from(h: &History) -> Personality {
    let counts = [
        (h.feeds, Personality::Gluttonous),
        (h.plays, Personality::Playful),
        (h.cleans, Personality::Tidy),
    ];
    let top = counts.iter().map(|c| c.0).max().unwrap_or(0);
    let mut leaders = counts.iter().filter(|c| c.0 == top);
    match (leaders.next(), leaders.next()) {
        (Some(&(n, p)), None) if n > 0 => p,
        _ => Personality::Calm,
    }
}

/// ご機嫌と親密度の平均から分岐を決める。
pub fn branch_from(s: &Status) -> Branch {
    let avg = (u16::from(s.mood) + u16::from(s.affinity)) / 2;
    if avg >= GOOD_MIN_AVG {
        return Branch::Good;
    }
    if avg <= POOR_MAX_AVG {
        return Branch::Poor;
    }
    Branch::Normal
}

/// 進化先の形態（段階は進化後の段階）。
pub fn evolve(stage: u8, s: &Status, h: &History) -> Form {
    Form {
        stage: stage.saturating_add(1).min(MAX_STAGE),
        branch: branch_from(s),
        personality: personality_from(h),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(mood: u8, affinity: u8) -> Status {
        Status::new(mood, affinity, 50, 0)
    }

    #[test]
    fn egg_starts_at_stage_one() {
        assert_eq!(Growth::egg(), Growth { stage: 1, elapsed_secs: 0 });
    }

    #[test]
    fn advance_below_threshold_keeps_stage() {
        let mut g = Growth::egg();
        assert!(!g.advance(STAGE_SECS - 1));
        assert_eq!(g, Growth { stage: 1, elapsed_secs: STAGE_SECS - 1 });
    }

    #[test]
    fn advance_at_threshold_moves_to_next_stage_and_resets_timer() {
        let mut g = Growth::egg();
        assert!(g.advance(STAGE_SECS));
        assert_eq!(g, Growth { stage: 2, elapsed_secs: 0 });
    }

    #[test]
    fn advance_carries_over_remainder() {
        let mut g = Growth::egg();
        assert!(g.advance(STAGE_SECS + 5));
        assert_eq!(g, Growth { stage: 2, elapsed_secs: 5 });
    }

    #[test]
    fn advance_accumulates_across_calls() {
        let mut g = Growth::egg();
        assert!(!g.advance(STAGE_SECS / 2));
        assert!(g.advance(STAGE_SECS / 2));
        assert_eq!(g.stage, 2);
    }

    #[test]
    fn advance_moves_one_stage_per_call_even_after_long_sleep() {
        let mut g = Growth::egg();
        assert!(g.advance(STAGE_SECS * 3));
        assert_eq!(g, Growth { stage: 2, elapsed_secs: STAGE_SECS * 2 });
    }

    #[test]
    fn advance_does_not_exceed_final_stage() {
        let mut g = Growth { stage: MAX_STAGE, elapsed_secs: 0 };
        assert!(!g.advance(STAGE_SECS * 2));
        assert_eq!(g.stage, MAX_STAGE);
    }

    #[test]
    fn personality_is_calm_without_history() {
        assert_eq!(personality_from(&History::default()), Personality::Calm);
    }

    #[test]
    fn personality_follows_most_frequent_action() {
        let play = History { feeds: 1, plays: 5, cleans: 2 };
        let feed = History { feeds: 9, plays: 5, cleans: 2 };
        let clean = History { feeds: 1, plays: 5, cleans: 7 };
        assert_eq!(personality_from(&play), Personality::Playful);
        assert_eq!(personality_from(&feed), Personality::Gluttonous);
        assert_eq!(personality_from(&clean), Personality::Tidy);
    }

    #[test]
    fn personality_is_calm_on_tie_for_top() {
        let h = History { feeds: 4, plays: 4, cleans: 1 };
        assert_eq!(personality_from(&h), Personality::Calm);
    }

    #[test]
    fn branch_uses_average_of_mood_and_affinity() {
        assert_eq!(branch_from(&st(100, 60)), Branch::Good); // avg 80
        assert_eq!(branch_from(&st(60, 60)), Branch::Normal); // avg 60
        assert_eq!(branch_from(&st(40, 40)), Branch::Poor); // avg 40
    }

    #[test]
    fn branch_boundaries() {
        assert_eq!(branch_from(&st(70, 70)), Branch::Good);
        assert_eq!(branch_from(&st(69, 70)), Branch::Normal); // avg 69.5 -> 69
        assert_eq!(branch_from(&st(40, 40)), Branch::Poor);
        assert_eq!(branch_from(&st(41, 40)), Branch::Poor); // avg 40.5 -> 40
        assert_eq!(branch_from(&st(41, 41)), Branch::Normal);
    }

    #[test]
    fn evolve_combines_next_stage_branch_and_personality() {
        let h = History { feeds: 0, plays: 3, cleans: 0 };
        let f = evolve(2, &st(90, 90), &h);
        assert_eq!(
            f,
            Form { stage: 3, branch: Branch::Good, personality: Personality::Playful }
        );
    }

    #[test]
    fn evolve_caps_stage_at_max() {
        let f = evolve(MAX_STAGE, &st(50, 50), &History::default());
        assert_eq!(f.stage, MAX_STAGE);
    }
}
