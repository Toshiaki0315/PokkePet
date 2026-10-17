//! おでかけとおみやげ: 飼い主が席を離れている間、ペットはおでかけして、戻ると、おみやげを持ち帰る。
//! おみやげは集めてコレクションにでき、好きなものを画面に飾れる。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::minigame::Rng;

/// おみやげ 1 種類。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Souvenir {
    pub id: &'static str,
    pub name: &'static str,
    pub emoji: &'static str,
    /// 0 = よくある、1 = めずらしい、2 = とてもめずらしい。
    pub rarity: u8,
}

const fn s(id: &'static str, name: &'static str, emoji: &'static str, rarity: u8) -> Souvenir {
    Souvenir { id, name, emoji, rarity }
}

/// おみやげの一覧（全 24 種）。
pub const CATALOG: [Souvenir; 24] = [
    s("shell", "かいがら", "🐚", 0),
    s("acorn", "どんぐり", "🌰", 0),
    s("leaf", "おちば", "🍃", 0),
    s("stone", "きれいな石", "🪨", 0),
    s("mushroom", "きのこ", "🍄", 0),
    s("dandelion", "たんぽぽ", "🌼", 0),
    s("feather", "はね", "🪶", 0),
    s("maple", "もみじ", "🍂", 0),
    s("clover", "四つ葉のクローバー", "🍀", 1),
    s("marble", "ビー玉", "🔮", 1),
    s("kite", "たこ", "🪁", 1),
    s("windbell", "ふうりん", "🎐", 1),
    s("plush", "ぬいぐるみ", "🧸", 1),
    s("balloon", "ふうせん", "🎈", 1),
    s("candy", "あめ", "🍬", 1),
    s("key", "ふるいカギ", "🔑", 1),
    s("gem", "ほうせき", "💎", 2),
    s("stardust", "ほしのかけら", "🌟", 2),
    s("medal", "きんのメダル", "🥇", 2),
    s("map", "たからの地図", "📜", 2),
    s("wand", "まほうのつえ", "🪄", 2),
    s("scale", "りゅうのうろこ", "🐉", 2),
    s("mane", "ユニコーンのたてがみ", "🦄", 2),
    s("rainbow", "にじのかけら", "🌈", 2),
];

pub fn find(id: &str) -> Option<&'static Souvenir> {
    CATALOG.iter().find(|x| x.id == id)
}

/// おでかけと認める、離席の最短の長さ（秒）。
pub const OUTING_MIN_SECS: u64 = 30 * 60;
/// 画面に飾れる数。
pub const MAX_DISPLAYED: usize = 3;

/// 離れていた時間に応じた、持ち帰る数（長いほど多い）。
pub fn count_for(away_secs: u64) -> usize {
    match away_secs {
        0..=10_799 => 1,
        10_800..=28_799 => 2,
        _ => 3,
    }
}

/// 離れていた時間に応じた、（よくある, めずらしい, とてもめずらしい）の出やすさ。長いほど、めずらしいものが出やすい。
pub fn weights(away_secs: u64) -> [u64; 3] {
    match away_secs {
        0..=7_199 => [70, 25, 5],
        7_200..=21_599 => [50, 38, 12],
        _ => [30, 45, 25],
    }
}

/// 出やすさに従って、おみやげを 1 つ選ぶ。
pub fn pick(rng: &mut Rng, away_secs: u64) -> &'static Souvenir {
    let w = weights(away_secs);
    let mut roll = rng.next_below(w.iter().sum());
    let mut rarity = 2u8;
    for (i, weight) in w.iter().enumerate() {
        if roll < *weight {
            rarity = i as u8;
            break;
        }
        roll -= weight;
    }
    let pool: Vec<&'static Souvenir> = CATALOG.iter().filter(|x| x.rarity == rarity).collect();
    pool[usize::try_from(rng.next_below(pool.len() as u64)).unwrap_or(0)]
}

/// 集めたおみやげ（個数）と、画面に飾るもの。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SouvenirBook {
    /// 種類（`Souvenir::id`）ごとの個数。
    #[serde(default)]
    pub counts: BTreeMap<String, u32>,
    /// 画面に飾っているもの（`Souvenir::id`。最大 `MAX_DISPLAYED`）。
    #[serde(default)]
    pub displayed: Vec<String>,
}

impl SouvenirBook {
    pub fn add(&mut self, id: &str) {
        *self.counts.entry(id.to_string()).or_insert(0) += 1;
    }

    /// 飾る／飾るのをやめる。持っていないものは飾れない。上限を超えるときはエラー。
    pub fn toggle_display(&mut self, id: &str) -> Result<(), String> {
        if let Some(i) = self.displayed.iter().position(|d| d == id) {
            self.displayed.remove(i);
            return Ok(());
        }
        if !self.counts.contains_key(id) {
            return Err(format!("you do not have {id} yet"));
        }
        if self.displayed.len() >= MAX_DISPLAYED {
            return Err(format!("only {MAX_DISPLAYED} souvenirs can be displayed at once"));
        }
        self.displayed.push(id.to_string());
        Ok(())
    }

    /// 集めた種類の数。
    pub fn found(&self) -> usize {
        CATALOG.iter().filter(|x| self.counts.contains_key(x.id)).count()
    }
}

/// コレクション画面用の 1 件。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SouvenirEntry {
    pub id: &'static str,
    pub name: &'static str,
    pub emoji: &'static str,
    pub rarity: u8,
    pub count: u32,
    pub displayed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SouvenirView {
    pub total: usize,
    pub found: usize,
    pub max_displayed: usize,
    pub entries: Vec<SouvenirEntry>,
}

pub fn build_view(book: &SouvenirBook) -> SouvenirView {
    let entries = CATALOG
        .iter()
        .map(|x| SouvenirEntry { id: x.id, name: x.name, emoji: x.emoji, rarity: x.rarity, count: book.counts.get(x.id).copied().unwrap_or(0), displayed: book.displayed.iter().any(|d| d == x.id) })
        .collect();
    SouvenirView { total: CATALOG.len(), found: book.found(), max_displayed: MAX_DISPLAYED, entries }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn the_catalog_has_24_distinct_items_in_three_rarities() {
        assert_eq!(CATALOG.iter().map(|x| x.id).collect::<BTreeSet<_>>().len(), 24);
        assert_eq!(CATALOG.iter().map(|x| x.emoji).collect::<BTreeSet<_>>().len(), 24);
        for rarity in 0..=2 {
            assert_eq!(CATALOG.iter().filter(|x| x.rarity == rarity).count(), 8, "rarity {rarity}");
        }
        assert_eq!(find("gem").unwrap().name, "ほうせき");
        assert!(find("nothing").is_none());
    }

    #[test]
    fn the_constructor_fills_every_field() {
        assert_eq!(s("x", "えっくす", "❌", 1), Souvenir { id: "x", name: "えっくす", emoji: "❌", rarity: 1 });
    }

    #[test]
    fn a_longer_outing_brings_back_more() {
        assert_eq!([count_for(0), count_for(10_799), count_for(10_800), count_for(28_799), count_for(28_800), count_for(99_999)], [1, 1, 2, 2, 3, 3]);
    }

    #[test]
    fn a_longer_outing_finds_rarer_things() {
        let rare_share = |secs| {
            let mut rng = Rng::new(42);
            (0..2000).filter(|_| pick(&mut rng, secs).rarity == 2).count()
        };
        assert!(rare_share(1_800) < rare_share(10_000));
        assert!(rare_share(10_000) < rare_share(40_000));
        for secs in [1_800, 10_000, 40_000] {
            assert_eq!(weights(secs).iter().sum::<u64>(), 100);
        }
    }

    #[test]
    fn picking_is_reproducible_and_covers_every_rarity() {
        let run = |seed| {
            let mut rng = Rng::new(seed);
            (0..40).map(|_| pick(&mut rng, 40_000).id).collect::<Vec<_>>()
        };
        assert_eq!(run(7), run(7));
        let mut rng = Rng::new(1);
        let rarities: BTreeSet<u8> = (0..400).map(|_| pick(&mut rng, 40_000).rarity).collect();
        assert_eq!(rarities.len(), 3);
    }

    #[test]
    fn collecting_counts_each_kind_and_remembers_how_many() {
        let mut book = SouvenirBook::default();
        book.add("shell");
        book.add("shell");
        book.add("gem");
        assert_eq!((book.counts["shell"], book.counts["gem"], book.found()), (2, 1, 2));
    }

    #[test]
    fn only_owned_items_can_be_displayed_and_only_a_few_at_a_time() {
        let mut book = SouvenirBook::default();
        assert!(book.toggle_display("shell").is_err(), "not collected yet");
        for id in ["shell", "acorn", "leaf", "gem"] {
            book.add(id);
        }
        for id in ["shell", "acorn", "leaf"] {
            book.toggle_display(id).unwrap();
        }
        assert!(book.toggle_display("gem").is_err(), "the shelf is full");
        book.toggle_display("acorn").unwrap(); // しまう
        book.toggle_display("gem").unwrap();
        assert_eq!(book.displayed, ["shell", "leaf", "gem"]);
    }

    #[test]
    fn the_view_lists_every_item_with_its_count_and_whether_it_is_displayed() {
        let mut book = SouvenirBook::default();
        book.add("shell");
        book.add("shell");
        book.toggle_display("shell").unwrap();
        let v = build_view(&book);
        assert_eq!((v.total, v.found, v.max_displayed, v.entries.len()), (24, 1, 3, 24));
        let shell = v.entries.iter().find(|e| e.id == "shell").unwrap();
        assert_eq!((shell.count, shell.displayed, shell.emoji), (2, true, "🐚"));
        assert!(v.entries.iter().filter(|e| e.id != "shell").all(|e| e.count == 0 && !e.displayed));
        let json = serde_json::to_value(&v).unwrap();
        assert_eq!((json["maxDisplayed"].as_u64(), json["entries"][0]["id"].as_str()), (Some(3), Some("shell")));
    }

    #[test]
    fn an_empty_book_loads_from_an_old_save() {
        let book: SouvenirBook = serde_json::from_str("{}").unwrap();
        assert_eq!(book, SouvenirBook::default());
    }
}
