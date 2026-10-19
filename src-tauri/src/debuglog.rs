//! デバッグ用の詳細ログ（設定「デバッグログを残す」がオンのとき）。
//! お世話・眠る・おでかけ・うんち・ご機嫌の変化など、ペットに起きたことを 1 行ずつ、`debug.log` に書き足す。

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Datelike, Timelike};

use crate::expression::Level;
use crate::service::StateView;

/// ログファイルの大きさの上限（バイト）。超えたら、古いものを `debug.log.1` にして、新しく書き始める。
pub const MAX_LOG_BYTES: u64 = 1024 * 1024;
/// 状態の一覧を書く間隔（秒）。
pub const SNAPSHOT_EVERY_SECS: u64 = 600;

/// ログファイルへの書き込み。
#[derive(Debug, Clone)]
pub struct DebugLog {
    path: PathBuf,
}

impl DebugLog {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 1 行書き足す。「2026-10-19 07:12:03 [操作] ご飯を食べた」の形。大きくなりすぎたら、先に入れ替える。
    pub fn write(&self, stamp: &str, tag: &str, text: &str) -> io::Result<()> {
        self.rotate_if_needed()?;
        let mut file = OpenOptions::new().create(true).append(true).open(&self.path)?;
        writeln!(file, "{stamp} [{tag}] {text}")
    }

    fn rotate_if_needed(&self) -> io::Result<()> {
        let too_big = fs::metadata(&self.path).is_ok_and(|m| m.len() >= MAX_LOG_BYTES);
        if too_big {
            fs::rename(&self.path, self.path.with_extension("log.1"))?;
        }
        Ok(())
    }
}

/// 「2026-10-19 07:12:03」（ローカル時刻）。
pub fn format_stamp(unix_secs: u64, utc_offset_secs: i32) -> String {
    let local = i64::try_from(unix_secs).unwrap_or(0).saturating_add(i64::from(utc_offset_secs));
    DateTime::from_timestamp(local, 0).map_or_else(
        || "0000-00-00 00:00:00".to_string(),
        |d| format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", d.year(), d.month(), d.day(), d.hour(), d.minute(), d.second()),
    )
}

fn rank(level: Level) -> u8 {
    match level {
        Level::Low => 0,
        Level::Mid => 1,
        Level::High => 2,
    }
}

fn level_name(level: Level) -> &'static str {
    match level {
        Level::Low => "低い",
        Level::Mid => "ふつう",
        Level::High => "高い",
    }
}

/// 2 つの状態の違いを、1 行ずつの文章にする（何も変わっていなければ空）。
pub fn describe_changes(before: &StateView, after: &StateView) -> Vec<String> {
    let mut out = Vec::new();
    if !before.sleeping && after.sleeping {
        out.push(if after.forced_sleep { "ねた（電源ボタンで寝かされた）".to_string() } else { "ねた（おやすみ時間）".to_string() });
    }
    if before.sleeping && !after.sleeping {
        out.push(if before.forced_sleep { "おきた（電源ボタンで起こされた・自然に起きた）".to_string() } else { "おきた".to_string() });
    }
    if !before.out && after.out {
        out.push("おでかけした".to_string());
    }
    if before.out && !after.out {
        out.push("おでかけから戻った".to_string());
    }
    if after.poops > before.poops {
        out.push(format!("うんちが増えた（{}→{}個。汚れ度 {}→{}）", before.poops, after.poops, before.dirtiness, after.dirtiness));
    }
    if after.poops < before.poops {
        out.push(format!("うんちが減った（{}→{}個）", before.poops, after.poops));
    }
    let (b, a) = (before.expression.mood, after.expression.mood);
    if rank(a) > rank(b) {
        out.push(format!("機嫌が良くなった（{}→{}、ご機嫌 {}→{}）", level_name(b), level_name(a), before.mood, after.mood));
    }
    if rank(a) < rank(b) {
        out.push(format!("機嫌が悪くなった（{}→{}、ご機嫌 {}→{}）", level_name(b), level_name(a), before.mood, after.mood));
    }
    let (b, a) = (before.expression.hunger, after.expression.hunger);
    if rank(a) < rank(b) {
        let what = if a == Level::Low { "おなかがすいた" } else { "おなかが減ってきた" };
        out.push(format!("{what}（{}→{}、空腹度 {}→{}）", level_name(b), level_name(a), before.hunger, after.hunger));
    }
    if rank(a) > rank(b) {
        let what = if a == Level::High { "おなかがいっぱいになった" } else { "おなかが少しふくれた" };
        out.push(format!("{what}（{}→{}、空腹度 {}→{}）", level_name(b), level_name(a), before.hunger, after.hunger));
    }
    if !before.sick && after.sick {
        out.push("病気になった".to_string());
    }
    if before.sick && !after.sick {
        out.push("病気が治った".to_string());
    }
    if !before.danger && after.danger {
        out.push("命が危ない（もう限界が近い）".to_string());
    }
    if !before.dead && after.dead {
        out.push("亡くなった".to_string());
    }
    if before.dead && !after.dead {
        out.push("新しい卵を迎えた".to_string());
    }
    let selfish = |v: &StateView| v.expression.bubble == Some(crate::expression::Bubble::Selfish);
    if !selfish(before) && selfish(after) {
        out.push("わがままを言い出した".to_string());
    }
    if selfish(before) && !selfish(after) {
        out.push("わがままが収まった".to_string());
    }
    if after.stage != before.stage {
        out.push(format!("成長した（段階 {}→{}。{:?}・{:?}）", before.stage, after.stage, after.branch, after.personality));
    }
    if after.generation != before.generation {
        out.push(format!("世代が変わった（{}→{}代目）", before.generation, after.generation));
    }
    if after.name != before.name {
        out.push(format!("名前が変わった（{:?}→{:?}）", before.name, after.name));
    }
    if after.gift.len() > before.gift.len() {
        let names: Vec<&str> = after.gift.iter().map(|g| g.name).collect();
        out.push(format!("おみやげを持ち帰った（{}個: {}）", after.gift.len(), names.join("、")));
    }
    if after.event != before.event {
        out.push(format!("できごと: {}", after.event.as_ref().map_or("なし".to_string(), |e| format!("{} {}", e.emoji, e.message))));
    }
    for (label, b, a) in [
        ("Mac の電池が少ない", before.battery_low, after.battery_low),
        ("Mac を充電中", before.charging, after.charging),
        ("ネットにつながっていない", before.offline, after.offline),
        ("Mac がダークモード", before.dark_mode, after.dark_mode),
    ] {
        if a != b {
            out.push(format!("{label}: {}", if a { "なった" } else { "なくなった" }));
        }
    }
    out
}

/// 変わった数値だけを「ご機嫌 50→55」のように並べる（何も変わっていなければ空）。
pub fn param_summary(before: &StateView, after: &StateView) -> String {
    let pairs = [
        ("ご機嫌", before.mood, after.mood),
        ("親密度", before.affinity, after.affinity),
        ("空腹度", before.hunger, after.hunger),
        ("汚れ度", before.dirtiness, after.dirtiness),
        ("体重", before.weight, after.weight),
        ("しつけ度", before.discipline, after.discipline),
    ];
    pairs.iter().filter(|(_, b, a)| a != b).map(|(label, b, a)| format!("{label} {b}→{a}")).collect::<Vec<_>>().join(" ")
}

/// いまの状態の一覧（定期的に書く）。
pub fn snapshot_line(view: &StateView) -> String {
    let state = if view.out {
        "おでかけ中"
    } else if view.sleeping {
        if view.forced_sleep { "ねている（電源ボタン）" } else { "ねている" }
    } else {
        "起きている"
    };
    format!(
        "ご機嫌 {} 親密度 {} 空腹度 {} 汚れ度 {} 体重 {} しつけ度 {} うんち {}個 段階 {} {}{}",
        view.mood,
        view.affinity,
        view.hunger,
        view.dirtiness,
        view.weight,
        view.discipline,
        view.poops,
        view.stage,
        state,
        if view.sick { " 病気" } else { "" },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expression::{Bubble, Expression};
    use crate::service::{GiftItem, PlayAction};
    use crate::evolution::{Branch, Personality};

    fn view() -> StateView {
        StateView {
            mood: 50, affinity: 50, hunger: 50, dirtiness: 10, weight: 50, discipline: 0, poops: 0, sick: false, danger: false, dead: false,
            stage: 3, branch: Branch::Normal, personality: Personality::Calm,
            expression: Expression { mood: Level::Mid, hunger: Level::Mid, dirtiness: Level::Low, bubble: None },
            play_action: PlayAction::Play, intro: false, hatching: false, sleeping: false, forced_sleep: false, out: false,
            tint_degrees: 0, charm: None, gift: Vec::new(), displayed: Vec::new(), season_scene: "snow", battery_low: false, charging: false,
            offline: false, dark_mode: false, event: None, name: Some("ごま".to_string()), needs_name: false, graduating: false, generation: 1,
        }
    }

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pokkepet-debuglog-{}-{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir.join("debug.log")
    }

    #[test]
    fn nothing_changed_nothing_to_say() {
        assert!(describe_changes(&view(), &view()).is_empty());
        assert_eq!(param_summary(&view(), &view()), "");
    }

    #[test]
    fn going_to_sleep_and_waking_up_say_why() {
        let awake = view();
        let mut asleep = view();
        asleep.sleeping = true;
        assert_eq!(describe_changes(&awake, &asleep), ["ねた（おやすみ時間）"]);
        asleep.forced_sleep = true;
        assert_eq!(describe_changes(&awake, &asleep), ["ねた（電源ボタンで寝かされた）"]);
        assert_eq!(describe_changes(&asleep, &awake), ["おきた（電源ボタンで起こされた・自然に起きた）"]);
        asleep.forced_sleep = false;
        assert_eq!(describe_changes(&asleep, &awake), ["おきた"]);
    }

    #[test]
    fn leaving_and_coming_back_are_logged() {
        let home = view();
        let mut out = view();
        out.out = true;
        assert_eq!(describe_changes(&home, &out), ["おでかけした"]);
        assert_eq!(describe_changes(&out, &home), ["おでかけから戻った"]);
    }

    #[test]
    fn poops_appear_and_get_cleaned_up() {
        let before = view();
        let mut after = view();
        after.poops = 2;
        after.dirtiness = 55;
        assert_eq!(describe_changes(&before, &after), ["うんちが増えた（0→2個。汚れ度 10→55）"]);
        assert_eq!(describe_changes(&after, &before), ["うんちが減った（2→0個）"]);
    }

    #[test]
    fn the_mood_getting_better_or_worse_is_logged_with_the_numbers() {
        let mid = view();
        let mut high = view();
        high.mood = 80;
        high.expression.mood = Level::High;
        assert_eq!(describe_changes(&mid, &high), ["機嫌が良くなった（ふつう→高い、ご機嫌 50→80）"]);
        assert_eq!(describe_changes(&high, &mid), ["機嫌が悪くなった（高い→ふつう、ご機嫌 80→50）"]);
    }

    #[test]
    fn getting_hungry_and_full_are_logged() {
        let mid = view();
        let mut hungry = view();
        hungry.hunger = 20;
        hungry.expression.hunger = Level::Low;
        assert_eq!(describe_changes(&mid, &hungry), ["おなかがすいた（ふつう→低い、空腹度 50→20）"]);
        assert_eq!(describe_changes(&hungry, &mid), ["おなかが少しふくれた（低い→ふつう、空腹度 20→50）"]);
        let mut full = view();
        full.hunger = 90;
        full.expression.hunger = Level::High;
        assert_eq!(describe_changes(&mid, &full), ["おなかがいっぱいになった（ふつう→高い、空腹度 50→90）"]);
        assert_eq!(describe_changes(&full, &mid), ["おなかが減ってきた（高い→ふつう、空腹度 90→50）"]);
    }

    #[test]
    fn sickness_selfishness_and_death_are_logged() {
        let well = view();
        let mut sick = view();
        sick.sick = true;
        assert_eq!(describe_changes(&well, &sick), ["病気になった"]);
        assert_eq!(describe_changes(&sick, &well), ["病気が治った"]);
        let mut selfish = view();
        selfish.expression.bubble = Some(Bubble::Selfish);
        assert_eq!(describe_changes(&well, &selfish), ["わがままを言い出した"]);
        assert_eq!(describe_changes(&selfish, &well), ["わがままが収まった"]);
        let mut dying = view();
        dying.danger = true;
        assert_eq!(describe_changes(&well, &dying), ["命が危ない（もう限界が近い）"]);
        let mut dead = view();
        dead.dead = true;
        assert_eq!(describe_changes(&well, &dead), ["亡くなった"]);
        assert_eq!(describe_changes(&dead, &well), ["新しい卵を迎えた"]);
    }

    #[test]
    fn growing_up_changing_generation_and_naming_are_logged() {
        let before = view();
        let mut after = view();
        after.stage = 4;
        after.generation = 2;
        after.name = None;
        let lines = describe_changes(&before, &after);
        assert_eq!(lines.len(), 3);
        assert!(lines[0].starts_with("成長した（段階 3→4"));
        assert_eq!(lines[1], "世代が変わった（1→2代目）");
        assert_eq!(lines[2], "名前が変わった（Some(\"ごま\")→None）");
    }

    #[test]
    fn souvenirs_and_events_are_logged() {
        let before = view();
        let mut after = view();
        after.gift = vec![GiftItem { emoji: "🐚", name: "かいがら", rarity: 0 }, GiftItem { emoji: "💎", name: "ほうせき", rarity: 2 }];
        after.event = Some(crate::events::birthday_event());
        let lines = describe_changes(&before, &after);
        assert_eq!(lines, ["おみやげを持ち帰った（2個: かいがら、ほうせき）", "できごと: 🎂 おたんじょうび おめでとう！"]);
        assert_eq!(describe_changes(&after, &before), ["できごと: なし"]);
    }

    #[test]
    fn the_macs_state_is_logged() {
        let before = view();
        let mut after = view();
        after.battery_low = true;
        after.charging = true;
        after.offline = true;
        after.dark_mode = true;
        assert_eq!(
            describe_changes(&before, &after),
            ["Mac の電池が少ない: なった", "Mac を充電中: なった", "ネットにつながっていない: なった", "Mac がダークモード: なった"]
        );
        assert_eq!(describe_changes(&after, &before)[0], "Mac の電池が少ない: なくなった");
    }

    #[test]
    fn only_the_numbers_that_changed_are_listed() {
        let before = view();
        let mut after = view();
        after.hunger = 90;
        after.mood = 55;
        after.weight = 54;
        assert_eq!(param_summary(&before, &after), "ご機嫌 50→55 空腹度 50→90 体重 50→54");
        after.affinity = 52;
        after.dirtiness = 0;
        after.discipline = 15;
        assert_eq!(param_summary(&before, &after), "ご機嫌 50→55 親密度 50→52 空腹度 50→90 汚れ度 10→0 体重 50→54 しつけ度 0→15");
    }

    #[test]
    fn the_snapshot_says_where_the_pet_is_and_how_it_feels() {
        let mut v = view();
        assert!(snapshot_line(&v).contains("起きている"));
        assert!(snapshot_line(&v).starts_with("ご機嫌 50 親密度 50 空腹度 50 汚れ度 10 体重 50 しつけ度 0 うんち 0個 段階 3"));
        v.sleeping = true;
        assert!(snapshot_line(&v).contains("ねている") && !snapshot_line(&v).contains("電源"));
        v.forced_sleep = true;
        assert!(snapshot_line(&v).contains("ねている（電源ボタン）"));
        v.out = true;
        v.sick = true;
        assert!(snapshot_line(&v).contains("おでかけ中") && snapshot_line(&v).ends_with("病気"));
    }

    #[test]
    fn the_time_stamp_is_the_local_time() {
        assert_eq!(format_stamp(1_792_218_645, 0), "2026-10-17 06:30:45");
        assert_eq!(format_stamp(1_792_218_645, 9 * 3600), "2026-10-17 15:30:45");
        assert_eq!(format_stamp(i64::MAX as u64, 0), "0000-00-00 00:00:00");
    }

    #[test]
    fn lines_are_appended_with_the_stamp_and_a_tag() {
        let log = DebugLog::new(tmp("append"));
        log.write("2026-10-19 07:12:03", "操作", "ご飯を食べた").unwrap();
        log.write("2026-10-19 07:12:04", "変化", "うんちが増えた").unwrap();
        assert_eq!(fs::read_to_string(log.path()).unwrap(), "2026-10-19 07:12:03 [操作] ご飯を食べた\n2026-10-19 07:12:04 [変化] うんちが増えた\n");
    }

    #[test]
    fn a_log_that_grows_too_big_is_moved_aside_and_a_new_one_started() {
        let path = tmp("rotate");
        fs::write(&path, vec![b'x'; MAX_LOG_BYTES as usize]).unwrap();
        let log = DebugLog::new(path.clone());
        log.write("t", "操作", "新しい").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "t [操作] 新しい\n");
        assert_eq!(fs::metadata(path.with_extension("log.1")).unwrap().len(), MAX_LOG_BYTES);
    }

    #[test]
    fn writing_to_an_unwritable_place_is_an_error() {
        let bad = tmp("bad").parent().unwrap().join("no/such/dir/debug.log");
        assert!(DebugLog::new(bad).write("t", "操作", "x").is_err());
    }
}
