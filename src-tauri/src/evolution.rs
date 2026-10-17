//! 成長段階の進行と、進化時の分岐判定。

use serde::{Deserialize, Serialize};

use crate::status::Status;

pub const MAX_STAGE: u8 = 10;
/// 卵（段階 1）が孵るまでの秒数。初回に孵化を見られるよう短くしてある。
pub const EGG_SECS: u32 = 600;
/// 孵化後、1段階あたりの成長に必要な秒数。
pub const HOUR_SECS: u32 = 3600;
/// 最終段階になってから旅立ちの時までの長さ。
pub const GRADUATION_SECS: u32 = 24 * HOUR_SECS;
const GOOD_MIN_AVG: u16 = 70;
const POOR_MAX_AVG: u16 = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Personality {
    Calm,
    Playful,
    Gluttonous,
    Tidy,
}

impl Personality {
    /// JSON（serde）・スプライトのファイル名で使う小文字の名前。
    pub fn as_str(self) -> &'static str {
        match self {
            Personality::Calm => "calm",
            Personality::Playful => "playful",
            Personality::Gluttonous => "gluttonous",
            Personality::Tidy => "tidy",
        }
    }
}

/// 行動履歴（性格形成の材料）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct History {
    pub feeds: u32,
    pub plays: u32,
    pub cleans: u32,
    /// おやつの回数。性格の判定では「ご飯」と合わせて食いしん坊の材料になる。
    #[serde(default)]
    pub snacks: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Branch {
    Poor,
    Normal,
    Good,
}

impl Branch {
    pub fn as_str(self) -> &'static str {
        match self {
            Branch::Poor => "poor",
            Branch::Normal => "normal",
            Branch::Good => "good",
        }
    }
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

/// その段階から次の段階へ進むのに必要な秒数。
///
/// 序盤は進化が早く、育つほど間が空く（卵 10分 → 段階2〜4: 3時間 → 5〜7: 12時間 → 8〜9: 24時間）。
/// 最終段階は旅立ちまでの時間（`GRADUATION_SECS`）。
pub fn stage_secs(stage: u8) -> u32 {
    match stage {
        0..=1 => EGG_SECS,
        2..=4 => 3 * HOUR_SECS,
        5..=7 => 12 * HOUR_SECS,
        8..=9 => 24 * HOUR_SECS,
        _ => GRADUATION_SECS,
    }
}

/// 卵が届いてから旅立ちの時までの長さ（秒）。
pub fn secs_until_graduation() -> u32 {
    (1..MAX_STAGE).map(stage_secs).sum::<u32>() + GRADUATION_SECS
}

impl Growth {
    pub fn egg() -> Self {
        Self { stage: 1, elapsed_secs: 0 }
    }

    /// 旅立ちの時か（最終段階になってから `GRADUATION_SECS` が経った）。
    pub fn ready_to_graduate(&self) -> bool {
        self.stage >= MAX_STAGE && self.elapsed_secs >= GRADUATION_SECS
    }

    /// 経過時間を加算し、次の段階へ進んだ場合 true を返す。最終段階では進まない
    /// （代わりに旅立ちまでの時間を数える）。
    pub fn advance(&mut self, secs: u32) -> bool {
        if self.stage >= MAX_STAGE {
            self.elapsed_secs = self.elapsed_secs.saturating_add(secs).min(GRADUATION_SECS);
            return false;
        }
        self.elapsed_secs = self.elapsed_secs.saturating_add(secs);
        let needed = stage_secs(self.stage);
        if self.elapsed_secs < needed {
            return false;
        }
        self.elapsed_secs -= needed;
        self.stage += 1;
        true
    }
}

/// 履歴から性格を決定する。最多の行動に対応し、同数・全0なら `legacy`（親から受け継いだ性格）、
/// それも無ければ Calm。
pub fn personality_from(h: &History, legacy: Option<Personality>) -> Personality {
    let counts = [
        (h.feeds + h.snacks, Personality::Gluttonous),
        (h.plays, Personality::Playful),
        (h.cleans, Personality::Tidy),
    ];
    let top = counts.iter().map(|c| c.0).max().unwrap_or(0);
    let mut leaders = counts.iter().filter(|c| c.0 == top);
    match (leaders.next(), leaders.next()) {
        (Some(&(n, p)), None) if n > 0 => p,
        _ => legacy.unwrap_or(Personality::Calm),
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
pub fn evolve(stage: u8, s: &Status, h: &History, legacy: Option<Personality>) -> Form {
    Form {
        stage: stage.saturating_add(1).min(MAX_STAGE),
        branch: branch_from(s),
        personality: personality_from(h, legacy),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(mood: u8, affinity: u8) -> Status {
        Status::new(mood, affinity, 50, 0)
    }

    #[test]
    fn names_used_for_files_match_the_json_names() {
        for b in [Branch::Poor, Branch::Normal, Branch::Good] {
            assert_eq!(serde_json::to_value(b).unwrap(), b.as_str());
        }
        for p in [Personality::Calm, Personality::Playful, Personality::Gluttonous, Personality::Tidy] {
            assert_eq!(serde_json::to_value(p).unwrap(), p.as_str());
        }
    }

    #[test]
    fn egg_starts_at_stage_one() {
        assert_eq!(Growth::egg(), Growth { stage: 1, elapsed_secs: 0 });
    }

    const H: u32 = HOUR_SECS;
    const DAY: u32 = 24 * HOUR_SECS;

    #[test]
    fn the_schedule_slows_down_as_the_pet_grows() {
        assert_eq!(stage_secs(1), 10 * 60);
        for stage in 2..=4 {
            assert_eq!(stage_secs(stage), 3 * H, "stage {stage}");
        }
        for stage in 5..=7 {
            assert_eq!(stage_secs(stage), 12 * H, "stage {stage}");
        }
        for stage in 8..=9 {
            assert_eq!(stage_secs(stage), DAY, "stage {stage}");
        }
        assert_eq!(GRADUATION_SECS, DAY);
        assert_eq!(stage_secs(MAX_STAGE), GRADUATION_SECS);
    }

    #[test]
    fn a_stage_never_takes_less_time_than_the_one_before() {
        for stage in 1..MAX_STAGE {
            assert!(stage_secs(stage) <= stage_secs(stage + 1), "{stage}");
        }
    }

    #[test]
    fn the_whole_life_takes_almost_five_days() {
        // 10分 + 3×3時間 + 3×12時間 + 2×24時間 + 24時間 = 117時間10分
        assert_eq!(secs_until_graduation(), 10 * 60 + 117 * H);
        assert_eq!(secs_until_graduation(), 421_800);
        assert!((4 * DAY..5 * DAY).contains(&secs_until_graduation()));
    }

    #[test]
    fn each_stage_waits_for_its_own_length() {
        for (stage, len) in [(2u8, 3 * H), (5, 12 * H), (8, DAY)] {
            let mut g = Growth { stage, elapsed_secs: 0 };
            assert!(!g.advance(len - 1), "stage {stage}");
            assert!(g.advance(1), "stage {stage}");
            assert_eq!(g, Growth { stage: stage + 1, elapsed_secs: 0 });
        }
    }

    #[test]
    fn the_farewell_comes_a_day_after_the_final_stage() {
        let mut g = Growth { stage: MAX_STAGE, elapsed_secs: 0 };
        g.advance(DAY - 1);
        assert!(!g.ready_to_graduate());
        g.advance(1);
        assert!(g.ready_to_graduate());
    }

    #[test]
    fn advance_below_threshold_keeps_stage() {
        let mut g = Growth::egg();
        assert!(!g.advance(EGG_SECS - 1));
        assert_eq!(g, Growth { stage: 1, elapsed_secs: EGG_SECS - 1 });
    }

    #[test]
    fn advance_at_threshold_moves_to_next_stage_and_resets_timer() {
        let mut g = Growth::egg();
        assert!(g.advance(EGG_SECS));
        assert_eq!(g, Growth { stage: 2, elapsed_secs: 0 });
    }

    #[test]
    fn advance_carries_over_remainder() {
        let mut g = Growth::egg();
        assert!(g.advance(EGG_SECS + 5));
        assert_eq!(g, Growth { stage: 2, elapsed_secs: 5 });
    }

    #[test]
    fn advance_accumulates_across_calls() {
        let mut g = Growth::egg();
        assert!(!g.advance(EGG_SECS / 2));
        assert!(g.advance(EGG_SECS / 2));
        assert_eq!(g.stage, 2);
    }

    #[test]
    fn advance_moves_one_stage_per_call_even_after_long_sleep() {
        let mut g = Growth::egg();
        assert!(g.advance(EGG_SECS + 2 * DAY));
        assert_eq!(g, Growth { stage: 2, elapsed_secs: 2 * DAY });
    }

    #[test]
    fn the_final_stage_counts_down_to_the_farewell() {
        let mut g = Growth { stage: MAX_STAGE, elapsed_secs: 0 };
        assert!(!g.ready_to_graduate());
        assert!(!g.advance(GRADUATION_SECS - 1));
        assert!(!g.ready_to_graduate());
        assert!(!g.advance(1)); // 進化はしない
        assert!(g.ready_to_graduate());
        assert_eq!(g.stage, MAX_STAGE);
    }

    #[test]
    fn the_farewell_clock_stops_at_the_limit() {
        let mut g = Growth { stage: MAX_STAGE, elapsed_secs: 0 };
        g.advance(u32::MAX);
        assert_eq!(g.elapsed_secs, GRADUATION_SECS);
        assert!(g.ready_to_graduate());
    }

    #[test]
    fn earlier_stages_are_never_ready_to_graduate() {
        let g = Growth { stage: MAX_STAGE - 1, elapsed_secs: u32::MAX };
        assert!(!g.ready_to_graduate());
        assert!(!Growth::egg().ready_to_graduate());
    }

    #[test]
    fn the_parents_personality_is_the_default_for_a_new_pet() {
        assert_eq!(personality_from(&History::default(), Some(Personality::Playful)), Personality::Playful);
        let tie = History { feeds: 3, plays: 3, cleans: 0, snacks: 0 };
        assert_eq!(personality_from(&tie, Some(Personality::Tidy)), Personality::Tidy);
    }

    #[test]
    fn what_the_new_pet_actually_does_overrides_the_inherited_personality() {
        let h = History { feeds: 0, plays: 0, cleans: 5, snacks: 0 };
        assert_eq!(personality_from(&h, Some(Personality::Playful)), Personality::Tidy);
    }

    #[test]
    fn without_a_legacy_the_default_is_still_calm() {
        assert_eq!(personality_from(&History::default(), None), Personality::Calm);
    }

    #[test]
    fn evolving_passes_the_legacy_on() {
        let f = evolve(2, &st(90, 90), &History::default(), Some(Personality::Gluttonous));
        assert_eq!(f.personality, Personality::Gluttonous);
    }

    #[test]
    fn advance_does_not_exceed_final_stage() {
        let mut g = Growth { stage: MAX_STAGE, elapsed_secs: 0 };
        assert!(!g.advance(GRADUATION_SECS * 2));
        assert_eq!(g.stage, MAX_STAGE);
    }

    #[test]
    fn personality_is_calm_without_history() {
        assert_eq!(personality_from(&History::default(), None), Personality::Calm);
    }

    #[test]
    fn personality_follows_most_frequent_action() {
        let play = History { feeds: 1, plays: 5, cleans: 2, snacks: 0 };
        let feed = History { feeds: 9, plays: 5, cleans: 2, snacks: 0 };
        let clean = History { feeds: 1, plays: 5, cleans: 7, snacks: 0 };
        assert_eq!(personality_from(&play, None), Personality::Playful);
        assert_eq!(personality_from(&feed, None), Personality::Gluttonous);
        assert_eq!(personality_from(&clean, None), Personality::Tidy);
    }

    #[test]
    fn snacks_count_towards_a_gluttonous_personality() {
        let h = History { feeds: 1, plays: 4, cleans: 2, snacks: 4 };
        assert_eq!(personality_from(&h, None), Personality::Gluttonous); // 1 + 4 = 5 > 4
        let only_snacks = History { snacks: 3, ..History::default() };
        assert_eq!(personality_from(&only_snacks, None), Personality::Gluttonous);
    }

    #[test]
    fn snacks_and_feeds_add_up_in_a_tie_check() {
        let h = History { feeds: 2, plays: 4, cleans: 0, snacks: 2 };
        assert_eq!(personality_from(&h, None), Personality::Calm); // 2 + 2 = 4 は遊ぶと同数
    }

    #[test]
    fn histories_saved_before_snacks_existed_still_load() {
        let h: History = serde_json::from_str(r#"{"feeds":3,"plays":1,"cleans":0}"#).unwrap();
        assert_eq!(h, History { feeds: 3, plays: 1, cleans: 0, snacks: 0 });
    }

    #[test]
    fn personality_is_calm_on_tie_for_top() {
        let h = History { feeds: 4, plays: 4, cleans: 1, snacks: 0 };
        assert_eq!(personality_from(&h, None), Personality::Calm);
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
        let h = History { feeds: 0, plays: 3, cleans: 0, snacks: 0 };
        let f = evolve(2, &st(90, 90), &h, None);
        assert_eq!(
            f,
            Form { stage: 3, branch: Branch::Good, personality: Personality::Playful }
        );
    }

    #[test]
    fn evolve_caps_stage_at_max() {
        let f = evolve(MAX_STAGE, &st(50, 50), &History::default(), None);
        assert_eq!(f.stage, MAX_STAGE);
    }
}
