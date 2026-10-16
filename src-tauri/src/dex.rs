//! 図鑑: これまでに出会った姿の記録。全 109 種（卵 1 + 段階 2〜10 × 分岐 3 × 性格 4）。

use std::collections::BTreeSet;

use serde::Serialize;

use crate::evolution::{Branch, Form, Personality, MAX_STAGE};

pub const BRANCHES: [Branch; 3] = [Branch::Poor, Branch::Normal, Branch::Good];
pub const PERSONALITIES: [Personality; 4] = [Personality::Calm, Personality::Playful, Personality::Gluttonous, Personality::Tidy];

/// 姿の識別子。スプライトのファイル名（拡張子なし）と同じ。卵は分岐・性格によらず "egg"。
pub fn form_key(form: &Form) -> String {
    if form.stage <= 1 {
        return "egg".to_string();
    }
    format!("{}-{}-{}", form.stage, form.branch.as_str(), form.personality.as_str())
}

/// 全ての姿の識別子（卵→段階順→分岐→性格の順）。
pub fn all_keys() -> Vec<String> {
    all_entries().into_iter().map(|(key, ..)| key).collect()
}

/// (識別子, 段階, 分岐, 性格)。卵は便宜上 分岐 Normal・性格 Calm とする。
fn all_entries() -> Vec<(String, u8, Branch, Personality)> {
    let mut out = vec![("egg".to_string(), 1, Branch::Normal, Personality::Calm)];
    for stage in 2..=MAX_STAGE {
        for branch in BRANCHES {
            for personality in PERSONALITIES {
                out.push((form_key(&Form { stage, branch, personality }), stage, branch, personality));
            }
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DexEntry {
    pub key: String,
    pub stage: u8,
    pub branch: Branch,
    pub personality: Personality,
    pub discovered: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DexView {
    pub total: usize,
    pub discovered: usize,
    pub entries: Vec<DexEntry>,
}

pub fn build_view(discovered: &BTreeSet<String>) -> DexView {
    let entries: Vec<DexEntry> = all_entries()
        .into_iter()
        .map(|(key, stage, branch, personality)| DexEntry { discovered: discovered.contains(&key), key, stage, branch, personality })
        .collect();
    DexView { total: entries.len(), discovered: entries.iter().filter(|e| e.discovered).count(), entries }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form(stage: u8, branch: Branch, personality: Personality) -> Form {
        Form { stage, branch, personality }
    }

    #[test]
    fn keys_match_the_sprite_file_names() {
        assert_eq!(form_key(&form(3, Branch::Good, Personality::Playful)), "3-good-playful");
        assert_eq!(form_key(&form(10, Branch::Poor, Personality::Tidy)), "10-poor-tidy");
        assert_eq!(form_key(&form(2, Branch::Normal, Personality::Gluttonous)), "2-normal-gluttonous");
    }

    #[test]
    fn every_egg_is_the_same_entry() {
        let a = form_key(&form(1, Branch::Poor, Personality::Tidy));
        let b = form_key(&form(1, Branch::Good, Personality::Calm));
        assert_eq!((a.as_str(), b.as_str()), ("egg", "egg"));
    }

    #[test]
    fn there_are_109_distinct_forms_starting_with_the_egg() {
        let keys = all_keys();
        assert_eq!(keys.len(), 1 + (usize::from(MAX_STAGE) - 1) * 3 * 4);
        assert_eq!(keys.len(), 109);
        assert_eq!(keys.iter().collect::<BTreeSet<_>>().len(), 109);
        assert_eq!(keys[0], "egg");
        assert_eq!(keys[1], "2-poor-calm");
        assert_eq!(keys[2], "2-poor-playful");
        assert_eq!(keys[13], "3-poor-calm");
        assert_eq!(keys[108], "10-good-tidy");
    }

    #[test]
    fn the_view_flags_discovered_forms_and_counts_them() {
        let found: BTreeSet<String> = ["egg", "2-normal-calm", "5-good-playful"].iter().map(|s| s.to_string()).collect();
        let v = build_view(&found);
        assert_eq!((v.total, v.discovered), (109, 3));
        let flagged: Vec<&str> = v.entries.iter().filter(|e| e.discovered).map(|e| e.key.as_str()).collect();
        assert_eq!(flagged, ["egg", "2-normal-calm", "5-good-playful"]);
    }

    #[test]
    fn entries_carry_the_stage_branch_and_personality() {
        let v = build_view(&BTreeSet::new());
        let egg = &v.entries[0];
        assert_eq!((egg.key.as_str(), egg.stage), ("egg", 1));
        let last = v.entries.last().unwrap();
        assert_eq!((last.key.as_str(), last.stage, last.branch, last.personality), ("10-good-tidy", 10, Branch::Good, Personality::Tidy));
        assert!(v.entries.iter().all(|e| !e.discovered));
    }

    #[test]
    fn unknown_keys_in_a_save_are_not_counted() {
        let found: BTreeSet<String> = ["egg", "99-good-calm", "garbage"].iter().map(|s| s.to_string()).collect();
        assert_eq!(build_view(&found).discovered, 1);
    }

    #[test]
    fn the_view_serializes_with_camel_case_keys() {
        let json = serde_json::to_value(build_view(&BTreeSet::new())).unwrap();
        assert_eq!(json["total"], 109);
        assert_eq!(json["discovered"], 0);
        assert_eq!(json["entries"][1]["key"], "2-poor-calm");
        assert_eq!(json["entries"][1]["branch"], "poor");
        assert_eq!(json["entries"][1]["personality"], "calm");
        assert_eq!(json["entries"][1]["discovered"], false);
    }
}
