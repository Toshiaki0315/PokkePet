//! 絵日記: 1 日ごとの出来事（お世話の回数・いっしょにいた時間・おみやげ）を数え、日が変わると思い出に書き残す。

use serde::{Deserialize, Serialize};

const DAY_SECS: i64 = 86_400;

/// その日（ローカル時刻）の出来事の数。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DayStats {
    #[serde(default)]
    pub feeds: u32,
    #[serde(default)]
    pub snacks: u32,
    #[serde(default)]
    pub plays: u32,
    #[serde(default)]
    pub cleans: u32,
    /// なでてもらった回数。
    #[serde(default)]
    pub strokes: u32,
    /// いっしょに作業した時間（秒）。
    #[serde(default)]
    pub together_secs: u64,
    /// 持ち帰ったおみやげの数。
    #[serde(default)]
    pub souvenirs: u32,
}

impl DayStats {
    /// 何か書くことがあるか（何もない日は、日記に残さない）。
    pub fn has_anything(&self) -> bool {
        self.feeds + self.snacks + self.plays + self.cleans + self.strokes + self.souvenirs > 0 || self.together_secs >= 60
    }

    pub fn together_minutes(&self) -> u32 {
        u32::try_from(self.together_secs / 60).unwrap_or(u32::MAX)
    }
}

/// ローカル時刻の「日番号」（1970-01-01 が 0）。日が変わったかの判定に使う。
pub fn day_number(unix_secs: u64, utc_offset_secs: i32) -> i64 {
    (i64::try_from(unix_secs).unwrap_or(i64::MAX / 2).saturating_add(i64::from(utc_offset_secs))).div_euclid(DAY_SECS)
}

/// 日番号から（月, 日）。
pub fn month_day(day_number: i64) -> (u8, u8) {
    use chrono::{DateTime, Datelike};
    DateTime::from_timestamp(day_number.saturating_mul(DAY_SECS), 0).map_or((1, 1), |d| (d.month() as u8, d.day() as u8))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_day_with_nothing_to_tell_is_not_written() {
        assert!(!DayStats::default().has_anything());
        assert!(!DayStats { together_secs: 59, ..DayStats::default() }.has_anything());
    }

    #[test]
    fn any_care_a_stroke_a_souvenir_or_a_minute_together_is_worth_writing() {
        for stats in [
            DayStats { feeds: 1, ..DayStats::default() },
            DayStats { snacks: 1, ..DayStats::default() },
            DayStats { plays: 1, ..DayStats::default() },
            DayStats { cleans: 1, ..DayStats::default() },
            DayStats { strokes: 1, ..DayStats::default() },
            DayStats { souvenirs: 1, ..DayStats::default() },
            DayStats { together_secs: 60, ..DayStats::default() },
        ] {
            assert!(stats.has_anything(), "{stats:?}");
        }
    }

    #[test]
    fn the_time_together_is_in_whole_minutes() {
        assert_eq!(DayStats { together_secs: 3_599, ..DayStats::default() }.together_minutes(), 59);
        assert_eq!(DayStats { together_secs: 7_260, ..DayStats::default() }.together_minutes(), 121);
    }

    #[test]
    fn the_day_number_changes_at_local_midnight() {
        assert_eq!(day_number(0, 0), 0);
        assert_eq!(day_number(86_399, 0), 0);
        assert_eq!(day_number(86_400, 0), 1);
        assert_eq!(day_number(86_399, 9 * 3600), 1, "already tomorrow in Tokyo");
        assert_eq!(day_number(0, -3600), -1);
    }

    #[test]
    fn the_month_and_day_come_from_the_day_number() {
        assert_eq!(month_day(0), (1, 1));
        assert_eq!(month_day(31), (2, 1));
        assert_eq!(month_day(day_number(1_767_225_600, 0)), (1, 1)); // 2026-01-01
        assert_eq!(month_day(day_number(1_767_225_600 - 1, 0)), (12, 31));
    }

    #[test]
    fn stats_load_from_an_empty_object() {
        let s: DayStats = serde_json::from_str("{}").unwrap();
        assert_eq!(s, DayStats::default());
        let json = serde_json::to_value(DayStats { together_secs: 5, ..DayStats::default() }).unwrap();
        assert_eq!(json["togetherSecs"], 5);
    }
}
