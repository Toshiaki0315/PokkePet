//! 遺伝と家系図、たまごコード。色あい（`tint`）と飾り（`charm`）が、世代から世代へ、少しずつ変わりながら受け継がれる。
//! 「たまごコード」は、いまのペットの血筋を短い文字列にしたもので、友だちに送って、そのたまごを迎えてもらえる。

use serde::{Deserialize, Serialize};

use crate::evolution::{Branch, Personality};
use crate::minigame::Rng;

/// 色あいの種類（0 = もとの色）。画面では、この番号ぶん色相を回して表示する（度）。
pub const TINT_DEGREES: [u16; 8] = [0, 40, 80, 130, 180, 230, 280, 320];
/// 飾りの種類（0 = なし）。
pub const CHARMS: [&str; 6] = ["", "🎀", "🎩", "👓", "🌸", "⭐"];

/// 受け継がれる、見た目の特徴。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Genome {
    /// 色あい（0〜7）。
    #[serde(default)]
    pub tint: u8,
    /// 飾り（0〜5。0 はなし）。
    #[serde(default)]
    pub charm: u8,
}

impl Genome {
    /// 子の遺伝子。色あいは 6 割そのまま、3 割は隣の色へ、1 割はまったく別の色になる。
    /// 飾りは 6 割そのまま、4 割で別の飾り（なしを含む）に変わる。
    pub fn child(&self, rng: &mut Rng) -> Genome {
        let tint = match rng.next_below(10) {
            0..=5 => self.tint,
            6..=8 => (self.tint + if rng.next_bool() { 1 } else { 7 }) % 8,
            _ => rng.next_below(8) as u8,
        };
        let charm = if rng.next_below(10) < 6 { self.charm } else { rng.next_below(CHARMS.len() as u64) as u8 };
        Genome { tint, charm }
    }

    /// 色相を回す角度（度）。
    pub fn tint_degrees(&self) -> u16 {
        TINT_DEGREES.get(usize::from(self.tint)).copied().unwrap_or(0)
    }

    /// 飾りの絵文字（なければ None）。
    pub fn charm_emoji(&self) -> Option<&'static str> {
        CHARMS.get(usize::from(self.charm)).copied().filter(|c| !c.is_empty())
    }
}

/// 家系図の 1 人。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ancestor {
    pub generation: u32,
    pub name: Option<String>,
    pub personality: Personality,
    pub branch: Branch,
    pub genome: Genome,
    /// 友だちからもらったたまご（たまごコード）の親か。
    #[serde(default)]
    pub from_friend: bool,
}

/// 家系図を残す世代数の上限。
pub const LINEAGE_MAX: usize = 30;

/// 家系図に 1 人を足す（上限を超えたら、古い方から捨てる）。
pub fn record_ancestor(lineage: &mut Vec<Ancestor>, ancestor: Ancestor) {
    lineage.push(ancestor);
    if lineage.len() > LINEAGE_MAX {
        let excess = lineage.len() - LINEAGE_MAX;
        lineage.drain(..excess);
    }
}

/// たまごコードに入っているもの。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EggCode {
    /// 血筋の性格（このたまごが孵ると、何もしなければこの性格に育つ）。
    pub personality: Personality,
    pub genome: Genome,
    /// 何代目の血筋か（255 まで）。
    pub generation: u8,
}

const CODE_VERSION: u8 = 1;
/// 紛らわしい文字（0/O・1/I）を除いた 32 文字。
const ALPHABET: &[u8; 32] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

fn personality_index(p: Personality) -> u8 {
    match p {
        Personality::Calm => 0,
        Personality::Playful => 1,
        Personality::Gluttonous => 2,
        Personality::Tidy => 3,
    }
}

fn personality_from_index(i: u8) -> Option<Personality> {
    [Personality::Calm, Personality::Playful, Personality::Gluttonous, Personality::Tidy].get(usize::from(i)).copied()
}

fn checksum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0x5Au8, |acc, b| acc.wrapping_mul(31).wrapping_add(*b) ^ 0xA7)
}

impl EggCode {
    /// 「ABCDE-FGHIJ」の形の文字列にする。
    pub fn encode(&self) -> String {
        let body = [CODE_VERSION, personality_index(self.personality), self.genome.tint, self.genome.charm, self.generation];
        let mut bytes = body.to_vec();
        bytes.push(checksum(&body));
        // 6 バイト = 48 ビットを、5 ビットずつ 10 文字に（先頭から詰める。最後は 2 ビット余るので 0 で埋める）
        let mut bits: u64 = 0;
        for b in &bytes {
            bits = (bits << 8) | u64::from(*b);
        }
        bits <<= 2; // 50 ビットにそろえる
        let chars: Vec<char> = (0..10).map(|i| char::from(ALPHABET[((bits >> (45 - 5 * i)) & 31) as usize])).collect();
        format!("{}-{}", chars[..5].iter().collect::<String>(), chars[5..].iter().collect::<String>())
    }

    /// 文字列から読み取る。小文字・ハイフンや空白の有無は問わない。壊れている・打ち間違いのときはエラー。
    pub fn decode(text: &str) -> Result<EggCode, String> {
        let cleaned: Vec<char> = text.chars().filter(|c| !c.is_whitespace() && *c != '-').map(|c| c.to_ascii_uppercase()).collect();
        if cleaned.len() != 10 {
            return Err("an egg code has 10 letters".to_string());
        }
        let mut bits: u64 = 0;
        for c in &cleaned {
            let value = ALPHABET.iter().position(|a| char::from(*a) == *c).ok_or_else(|| format!("unexpected letter in the egg code: {c}"))?;
            bits = (bits << 5) | value as u64;
        }
        if bits & 3 != 0 {
            return Err("the egg code is not valid".to_string());
        }
        bits >>= 2;
        let bytes: Vec<u8> = (0..6).map(|i| ((bits >> (40 - 8 * i)) & 0xFF) as u8).collect();
        if checksum(&bytes[..5]) != bytes[5] {
            return Err("the egg code does not match (a typo?)".to_string());
        }
        if bytes[0] != CODE_VERSION {
            return Err("this egg code is from a different version".to_string());
        }
        let personality = personality_from_index(bytes[1]).ok_or("the egg code is not valid")?;
        if usize::from(bytes[2]) >= TINT_DEGREES.len() || usize::from(bytes[3]) >= CHARMS.len() {
            return Err("the egg code is not valid".to_string());
        }
        Ok(EggCode { personality, genome: Genome { tint: bytes[2], charm: bytes[3] }, generation: bytes[4] })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code(personality: Personality, tint: u8, charm: u8, generation: u8) -> EggCode {
        EggCode { personality, genome: Genome { tint, charm }, generation }
    }

    #[test]
    fn an_egg_code_survives_a_round_trip_for_every_combination() {
        for p in [Personality::Calm, Personality::Playful, Personality::Gluttonous, Personality::Tidy] {
            for tint in 0..8 {
                for charm in 0..6 {
                    for generation in [1u8, 2, 17, 255] {
                        let original = code(p, tint, charm, generation);
                        assert_eq!(EggCode::decode(&original.encode()), Ok(original), "{original:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn the_code_looks_like_two_groups_of_five_easy_to_read_letters() {
        let text = code(Personality::Playful, 3, 2, 4).encode();
        assert_eq!(text.len(), 11);
        assert_eq!(text.as_bytes()[5], b'-');
        assert!(text.chars().filter(|c| *c != '-').all(|c| ALPHABET.contains(&(c as u8))));
        assert_eq!(text, code(Personality::Playful, 3, 2, 4).encode(), "always the same for the same egg");
    }

    #[test]
    fn different_eggs_have_different_codes() {
        let a = code(Personality::Calm, 0, 0, 1).encode();
        assert_ne!(a, code(Personality::Playful, 0, 0, 1).encode());
        assert_ne!(a, code(Personality::Calm, 1, 0, 1).encode());
        assert_ne!(a, code(Personality::Calm, 0, 1, 1).encode());
        assert_ne!(a, code(Personality::Calm, 0, 0, 2).encode());
    }

    #[test]
    fn lower_case_spaces_and_missing_hyphens_are_forgiven() {
        let original = code(Personality::Tidy, 5, 4, 9);
        let text = original.encode();
        assert_eq!(EggCode::decode(&text.to_lowercase()), Ok(original));
        assert_eq!(EggCode::decode(&text.replace('-', "")), Ok(original));
        assert_eq!(EggCode::decode(&format!("  {}  ", text.replace('-', " - "))), Ok(original));
    }

    #[test]
    fn a_typo_is_caught() {
        let text = code(Personality::Calm, 2, 1, 3).encode();
        let mut caught = 0;
        let letters: Vec<char> = text.chars().collect();
        for i in (0..11).filter(|&i| letters[i] != '-') {
            let mut broken = letters.clone();
            broken[i] = if broken[i] == 'A' { 'B' } else { 'A' };
            if EggCode::decode(&broken.iter().collect::<String>()).is_err() {
                caught += 1;
            }
        }
        assert_eq!(caught, 10, "every single-letter slip is noticed");
    }

    #[test]
    fn nonsense_is_rejected_with_a_reason() {
        assert!(EggCode::decode("").unwrap_err().contains("10 letters"));
        assert!(EggCode::decode("ABCDE-FGHIJK").unwrap_err().contains("10 letters"));
        assert!(EggCode::decode("0OIL1-00000").unwrap_err().contains("unexpected letter"));
        assert!(EggCode::decode("AAAAA-AAAAA").is_err());
    }

    #[test]
    fn out_of_range_values_and_other_versions_are_rejected() {
        let encode_raw = |body: [u8; 5]| {
            let mut bytes = body.to_vec();
            bytes.push(checksum(&body));
            let mut bits: u64 = 0;
            for b in &bytes {
                bits = (bits << 8) | u64::from(*b);
            }
            bits <<= 2;
            (0..10).map(|i| char::from(ALPHABET[((bits >> (45 - 5 * i)) & 31) as usize])).collect::<String>()
        };
        assert!(EggCode::decode(&encode_raw([1, 0, 0, 0, 1])).is_ok());
        assert!(EggCode::decode(&encode_raw([2, 0, 0, 0, 1])).unwrap_err().contains("different version"));
        assert!(EggCode::decode(&encode_raw([1, 4, 0, 0, 1])).is_err(), "no such personality");
        assert!(EggCode::decode(&encode_raw([1, 0, 8, 0, 1])).is_err(), "no such tint");
        assert!(EggCode::decode(&encode_raw([1, 0, 0, 6, 1])).is_err(), "no such charm");
    }

    #[test]
    fn a_stray_trailing_bit_makes_the_code_invalid() {
        // 末尾の 1 文字の下 2 ビットは 0 のはず。立っていたら、別の文字列と見なす。
        let text = code(Personality::Calm, 0, 0, 1).encode();
        let last = text.chars().last().unwrap();
        let index = ALPHABET.iter().position(|a| char::from(*a) == last).unwrap();
        let tampered = char::from(ALPHABET[index ^ 1]);
        let broken = format!("{}{}", &text[..text.len() - 1], tampered);
        assert!(EggCode::decode(&broken).is_err());
    }

    #[test]
    fn a_child_mostly_keeps_the_parents_colour_and_sometimes_changes_it() {
        let parent = Genome { tint: 3, charm: 2 };
        let mut rng = Rng::new(11);
        let children: Vec<Genome> = (0..400).map(|_| parent.child(&mut rng)).collect();
        let same_tint = children.iter().filter(|c| c.tint == 3).count();
        assert!(same_tint > 200 && same_tint < 330, "{same_tint}");
        assert!(children.iter().any(|c| c.tint == 2) && children.iter().any(|c| c.tint == 4), "neighbouring colours appear");
        assert!(children.iter().all(|c| usize::from(c.tint) < TINT_DEGREES.len() && usize::from(c.charm) < CHARMS.len()));
        let same_charm = children.iter().filter(|c| c.charm == 2).count();
        assert!(same_charm > 220, "{same_charm}");
        assert!(children.iter().any(|c| c.charm == 0), "a charm can be lost");
    }

    #[test]
    fn inheritance_is_reproducible() {
        let run = |seed| {
            let mut rng = Rng::new(seed);
            let mut g = Genome::default();
            (0..10).map(|_| {
                g = g.child(&mut rng);
                g
            }).collect::<Vec<_>>()
        };
        assert_eq!(run(5), run(5));
    }

    #[test]
    fn the_tint_is_a_hue_rotation_in_degrees() {
        assert_eq!(Genome { tint: 0, charm: 0 }.tint_degrees(), 0);
        assert_eq!(Genome { tint: 3, charm: 0 }.tint_degrees(), 130);
        assert_eq!(Genome { tint: 99, charm: 0 }.tint_degrees(), 0);
    }

    #[test]
    fn charms_have_emoji_except_none() {
        assert_eq!(Genome { tint: 0, charm: 0 }.charm_emoji(), None);
        assert_eq!(Genome { tint: 0, charm: 1 }.charm_emoji(), Some("🎀"));
        assert_eq!(Genome { tint: 0, charm: 99 }.charm_emoji(), None);
    }

    #[test]
    fn the_family_tree_keeps_only_the_latest_generations() {
        let mut tree = Vec::new();
        for g in 1..=(LINEAGE_MAX as u32 + 5) {
            record_ancestor(&mut tree, Ancestor { generation: g, name: None, personality: Personality::Calm, branch: Branch::Normal, genome: Genome::default(), from_friend: false });
        }
        assert_eq!((tree.len(), tree[0].generation, tree.last().unwrap().generation), (LINEAGE_MAX, 6, 35));
    }
}
