//! 季節のできごと（お正月・ハロウィンなど）と、飼い主のお誕生日・使い始めの記念日。
//! その日だけ、ペットがひとこと言って、名前の横に飾りが出る。

use serde::Serialize;

/// その日のできごと。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Event {
    /// 名前の横に出す飾り（絵文字）。
    pub emoji: &'static str,
    /// 吹き出しで言うこと。
    pub message: String,
}

fn event(emoji: &'static str, message: &str) -> Event {
    Event { emoji, message: message.to_string() }
}

/// 月と日から、その日の季節のできごとを返す（ない日は None）。
pub fn event_for(month: u32, day: u32) -> Option<Event> {
    let (emoji, message) = match (month, day) {
        (1, 1..=3) => ("🎍", "あけましておめでとう！"),
        (1, 7) => ("🌿", "きょうは ななくさの日だよ"),
        (2, 3) => ("👹", "おにはーそと！ふくはーうち！"),
        (2, 14) => ("🍫", "バレンタインだね"),
        (3, 3) => ("🎎", "ひなまつりだね"),
        (3, 14) => ("🍬", "ホワイトデーだね"),
        (4, 1) => ("😜", "きょうは エイプリルフール！"),
        (4, 29) | (5, 3 | 4) => ("🌤", "ゴールデンウィーク！どこかいく？"),
        (5, 5) => ("🎏", "こどもの日！"),
        (7, 7) => ("🎋", "たなばた！ねがいごと、した？"),
        (8, 13..=16) => ("🏮", "おぼんだね"),
        (10, 31) => ("🎃", "トリック・オア・トリート！"),
        (11, 3) => ("🎨", "文化の日だよ"),
        (11, 15) => ("👘", "七五三だね"),
        (12, 22) => ("🍊", "きょうは とうじ。ゆずゆに はいろう！"),
        (12, 24 | 25) => ("🎄", "メリークリスマス！"),
        (12, 26..=30) => ("🧹", "おおそうじ しよう！"),
        (12, 31) => ("🔔", "こんねんも ありがとう"),
        _ => return None,
    };
    Some(event(emoji, message))
}

/// 使い始めてから、お祝いする日数。
pub const ANNIVERSARY_DAYS: [i64; 7] = [100, 200, 300, 365, 500, 730, 1000];

/// 使い始めの記念日（その日だけ）。1 年・2 年は「年」で言う。
pub fn anniversary_event(days_since_start: i64) -> Option<Event> {
    if !ANNIVERSARY_DAYS.contains(&days_since_start) {
        return None;
    }
    let how_long = match days_since_start {
        365 => "1年".to_string(),
        730 => "2年".to_string(),
        n => format!("{n}日"),
    };
    Some(event("🎉", &format!("いっしょに {how_long} たったね！ありがとう！")))
}

/// 飼い主のお誕生日。
pub fn birthday_event() -> Event {
    event("🎂", "おたんじょうび おめでとう！")
}

/// その日のできごと。お誕生日 ＞ 使い始めの記念日 ＞ 季節のできごと の順に選ぶ。
/// `birthday` は飼い主のお誕生日（月, 日）。`days_since_start` は使い始めから何日目か。
pub fn todays_event(month: u32, day: u32, birthday: Option<(u32, u32)>, days_since_start: i64) -> Option<Event> {
    if birthday == Some((month, day)) {
        return Some(birthday_event());
    }
    anniversary_event(days_since_start).or_else(|| event_for(month, day))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_big_days_have_their_own_event() {
        for (m, d, emoji) in [
            (1, 1, "🎍"), (1, 3, "🎍"), (1, 7, "🌿"), (2, 3, "👹"), (2, 14, "🍫"), (3, 3, "🎎"), (3, 14, "🍬"), (4, 1, "😜"),
            (4, 29, "🌤"), (5, 3, "🌤"), (5, 4, "🌤"), (5, 5, "🎏"), (7, 7, "🎋"), (8, 13, "🏮"), (8, 16, "🏮"), (10, 31, "🎃"),
            (11, 3, "🎨"), (11, 15, "👘"), (12, 22, "🍊"), (12, 24, "🎄"), (12, 25, "🎄"), (12, 26, "🧹"), (12, 30, "🧹"), (12, 31, "🔔"),
        ] {
            let e = event_for(m, d).unwrap_or_else(|| panic!("{m}/{d}"));
            assert_eq!(e.emoji, emoji, "{m}/{d}");
            assert!(!e.message.is_empty());
        }
    }

    #[test]
    fn an_ordinary_day_has_none() {
        for (m, d) in [(1, 4), (1, 31), (4, 10), (6, 15), (8, 8), (8, 12), (8, 17), (11, 11), (12, 23), (12, 21)] {
            assert_eq!(event_for(m, d), None, "{m}/{d}");
        }
    }

    #[test]
    fn the_number_of_days_with_an_event_is_what_the_table_says() {
        let days = (1..=12u32).flat_map(|m| (1..=31u32).map(move |d| (m, d))).filter(|&(m, d)| event_for(m, d).is_some()).count();
        // 1/1-3, 1/7, 2/3, 2/14, 3/3, 3/14, 4/1, 4/29, 5/3-5, 7/7, 8/13-16, 10/31, 11/3, 11/15, 12/22, 12/24-25, 12/26-30, 12/31
        assert_eq!(days, 3 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 3 + 1 + 4 + 1 + 1 + 1 + 1 + 2 + 5 + 1);
    }

    #[test]
    fn the_owners_birthday_beats_everything_else() {
        let e = todays_event(12, 25, Some((12, 25)), 100).unwrap();
        assert_eq!((e.emoji, e.message.as_str()), ("🎂", "おたんじょうび おめでとう！"));
        assert_eq!(todays_event(12, 24, Some((12, 25)), 5).unwrap().emoji, "🎄", "not the birthday: the season");
        assert!(todays_event(6, 15, None, 5).is_none());
        assert!(todays_event(6, 15, Some((6, 16)), 5).is_none());
    }

    #[test]
    fn the_anniversary_of_starting_beats_the_season() {
        let e = todays_event(1, 1, None, 100).unwrap();
        assert_eq!(e.emoji, "🎉");
        assert!(e.message.contains("100日"));
        assert_eq!(todays_event(3, 3, None, 99).unwrap().emoji, "🎎");
    }

    #[test]
    fn long_anniversaries_are_told_in_years() {
        assert!(anniversary_event(365).unwrap().message.contains("1年"));
        assert!(anniversary_event(730).unwrap().message.contains("2年"));
        assert!(anniversary_event(500).unwrap().message.contains("500日"));
        for days in ANNIVERSARY_DAYS {
            assert!(anniversary_event(days).is_some(), "{days}");
        }
        for days in [0, 1, 99, 101, 364, 366, 999, 1001, -5] {
            assert!(anniversary_event(days).is_none(), "{days}");
        }
    }

    #[test]
    fn the_birthday_event_is_a_cake() {
        assert_eq!(birthday_event().emoji, "🎂");
    }
}
