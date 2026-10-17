//! 季節のできごと（お正月・ハロウィンなど）。その日だけ、ペットがひとこと言って、名前の横に飾りが出る。

use serde::Serialize;

/// その日のできごと。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Event {
    /// 名前の横に出す飾り（絵文字）。
    pub emoji: &'static str,
    /// 吹き出しで言うこと。
    pub message: &'static str,
}

/// 月と日から、その日のできごとを返す（ない日は None）。
pub fn event_for(month: u32, day: u32) -> Option<Event> {
    let (emoji, message) = match (month, day) {
        (1, 1..=3) => ("🎍", "あけましておめでとう！"),
        (2, 3) => ("👹", "おにはーそと！ふくはーうち！"),
        (2, 14) => ("🍫", "バレンタインだね"),
        (3, 3) => ("🎎", "ひなまつりだね"),
        (5, 5) => ("🎏", "こどもの日！"),
        (7, 7) => ("🎋", "たなばた！ねがいごと、した？"),
        (10, 31) => ("🎃", "トリック・オア・トリート！"),
        (12, 24 | 25) => ("🎄", "メリークリスマス！"),
        (12, 31) => ("🔔", "こんねんも ありがとう"),
        _ => return None,
    };
    Some(Event { emoji, message })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_big_days_have_their_own_event() {
        for (m, d, emoji) in [(1, 1, "🎍"), (1, 3, "🎍"), (2, 3, "👹"), (2, 14, "🍫"), (3, 3, "🎎"), (5, 5, "🎏"), (7, 7, "🎋"), (10, 31, "🎃"), (12, 24, "🎄"), (12, 25, "🎄"), (12, 31, "🔔")] {
            let e = event_for(m, d).unwrap_or_else(|| panic!("{m}/{d}"));
            assert_eq!(e.emoji, emoji, "{m}/{d}");
            assert!(!e.message.is_empty());
        }
    }

    #[test]
    fn an_ordinary_day_has_none() {
        for (m, d) in [(1, 4), (1, 31), (4, 10), (6, 15), (8, 8), (11, 11), (12, 23), (12, 26)] {
            assert_eq!(event_for(m, d), None, "{m}/{d}");
        }
    }

    #[test]
    fn every_day_of_the_year_can_be_asked_without_surprises() {
        let days_with_events = (1..=12u32).flat_map(|m| (1..=31u32).map(move |d| (m, d))).filter(|&(m, d)| event_for(m, d).is_some()).count();
        assert_eq!(days_with_events, 3 + 1 + 1 + 1 + 1 + 1 + 1 + 2 + 1);
    }
}
