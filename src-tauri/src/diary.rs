//! 思い出（日記）: 孵化・進化・名づけ・お世話の節目の記録。

use serde::{Deserialize, Serialize};

use crate::evolution::{Branch, Personality};

/// 記録の上限。超えたら古いものから捨てる。
pub const DIARY_MAX: usize = 200;
/// お世話の回数がこの値になったとき、節目として記録する。
pub const CARE_MILESTONES: [u32; 6] = [10, 50, 100, 250, 500, 1000];

pub const DAY_SECS: u64 = 24 * 60 * 60;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum DiaryEvent {
    /// 卵が届いた（始まり）。
    Born,
    /// 卵が孵った。
    Hatched,
    /// 次の段階に進化した。
    Evolved { stage: u8, branch: Branch, personality: Personality },
    /// 名前を付けた（付け直した）。
    Named { name: String },
    /// お世話の回数が節目になった。
    CareMilestone { count: u32 },
    /// いっしょに過ごして丸 `days` 日が経った（記念日）。
    Anniversary { days: u32 },
    /// 病気や衰弱で亡くなった（きびしいモード）。
    Died { name: Option<String>, personality: Personality },
    /// 旅立ちを見送った（世代交代）。
    Graduated { name: Option<String>, personality: Personality },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiaryEntry {
    pub at_unix: u64,
    pub event: DiaryEvent,
}

/// 記録を追加する。上限を超えたら古いものから捨てる。
pub fn record(entries: &mut Vec<DiaryEntry>, at_unix: u64, event: DiaryEvent) {
    entries.push(DiaryEntry { at_unix, event });
    if entries.len() > DIARY_MAX {
        let excess = entries.len() - DIARY_MAX;
        entries.drain(..excess);
    }
}

/// その回数が節目か。
pub fn is_care_milestone(total: u32) -> bool {
    CARE_MILESTONES.contains(&total)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiaryView {
    /// いっしょに過ごした日数（生まれた日が 1 日目）。
    pub days_together: u64,
    /// 新しい順。
    pub entries: Vec<DiaryEntry>,
}

pub fn build_view(entries: &[DiaryEntry], born_unix: u64, now_unix: u64) -> DiaryView {
    DiaryView {
        days_together: now_unix.saturating_sub(born_unix) / DAY_SECS + 1,
        entries: entries.iter().rev().cloned().collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(at: u64) -> DiaryEntry {
        DiaryEntry { at_unix: at, event: DiaryEvent::Hatched }
    }

    #[test]
    fn records_are_appended_in_order() {
        let mut v = Vec::new();
        record(&mut v, 10, DiaryEvent::Born);
        record(&mut v, 20, DiaryEvent::Hatched);
        assert_eq!(v, vec![DiaryEntry { at_unix: 10, event: DiaryEvent::Born }, DiaryEntry { at_unix: 20, event: DiaryEvent::Hatched }]);
    }

    #[test]
    fn the_oldest_records_are_dropped_beyond_the_limit() {
        let mut v = Vec::new();
        for i in 0..(DIARY_MAX as u64 + 5) {
            record(&mut v, i, DiaryEvent::Hatched);
        }
        assert_eq!(v.len(), DIARY_MAX);
        assert_eq!(v.first().unwrap().at_unix, 5);
        assert_eq!(v.last().unwrap().at_unix, DIARY_MAX as u64 + 4);
    }

    #[test]
    fn care_milestones() {
        for hit in [10, 50, 100, 250, 500, 1000] {
            assert!(is_care_milestone(hit), "{hit}");
        }
        for miss in [0, 1, 9, 11, 49, 51, 999, 1001] {
            assert!(!is_care_milestone(miss), "{miss}");
        }
    }

    #[test]
    fn the_view_lists_the_newest_first() {
        let v = build_view(&[entry(1), entry(2), entry(3)], 0, 10);
        let order: Vec<u64> = v.entries.iter().map(|e| e.at_unix).collect();
        assert_eq!(order, vec![3, 2, 1]);
    }

    #[test]
    fn the_day_count_starts_at_one_on_the_birthday() {
        assert_eq!(build_view(&[], 1_000, 1_000).days_together, 1);
        assert_eq!(build_view(&[], 1_000, 1_000 + DAY_SECS - 1).days_together, 1);
        assert_eq!(build_view(&[], 1_000, 1_000 + DAY_SECS).days_together, 2);
        assert_eq!(build_view(&[], 1_000, 1_000 + 9 * DAY_SECS + 5).days_together, 10);
    }

    #[test]
    fn a_clock_before_the_birth_counts_as_day_one() {
        assert_eq!(build_view(&[], 5_000, 1_000).days_together, 1);
    }

    #[test]
    fn events_serialize_with_a_kind_tag_and_camel_case_keys() {
        let evolved = DiaryEvent::Evolved { stage: 3, branch: Branch::Good, personality: Personality::Playful };
        let json = serde_json::to_value(DiaryEntry { at_unix: 7, event: evolved }).unwrap();
        assert_eq!(json["atUnix"], 7);
        assert_eq!(json["event"]["kind"], "evolved");
        assert_eq!(json["event"]["stage"], 3);
        assert_eq!(json["event"]["branch"], "good");
        assert_eq!(json["event"]["personality"], "playful");
        assert_eq!(serde_json::to_value(DiaryEvent::Born).unwrap()["kind"], "born");
        assert_eq!(serde_json::to_value(DiaryEvent::Named { name: "ごま".into() }).unwrap()["name"], "ごま");
        assert_eq!(serde_json::to_value(DiaryEvent::CareMilestone { count: 10 }).unwrap()["kind"], "careMilestone");
    }

    #[test]
    fn a_farewell_serializes_with_the_name_and_personality() {
        let named = DiaryEvent::Graduated { name: Some("ごま".into()), personality: Personality::Tidy };
        let json = serde_json::to_value(named).unwrap();
        assert_eq!(json["kind"], "graduated");
        assert_eq!(json["name"], "ごま");
        assert_eq!(json["personality"], "tidy");
        let unnamed = DiaryEvent::Graduated { name: None, personality: Personality::Calm };
        assert_eq!(serde_json::to_value(unnamed).unwrap()["name"], serde_json::Value::Null);
    }

    #[test]
    fn events_survive_a_save_round_trip() {
        let all = vec![
            DiaryEntry { at_unix: 1, event: DiaryEvent::Born },
            DiaryEntry { at_unix: 2, event: DiaryEvent::Hatched },
            DiaryEntry { at_unix: 3, event: DiaryEvent::Evolved { stage: 4, branch: Branch::Poor, personality: Personality::Tidy } },
            DiaryEntry { at_unix: 4, event: DiaryEvent::Named { name: "ごま".into() } },
            DiaryEntry { at_unix: 5, event: DiaryEvent::CareMilestone { count: 50 } },
            DiaryEntry { at_unix: 6, event: DiaryEvent::Graduated { name: Some("ごま".into()), personality: Personality::Playful } },
            DiaryEntry { at_unix: 7, event: DiaryEvent::Graduated { name: None, personality: Personality::Calm } },
        ];
        let json = serde_json::to_string(&all).unwrap();
        assert_eq!(serde_json::from_str::<Vec<DiaryEntry>>(&json).unwrap(), all);
    }
}
