//! 生活リズム: 操作していた時間帯のヒストグラムから、おやすみ時間（と朝型・夜型）を学ぶ。

use serde::Serialize;

use crate::sleep::SleepWindow;

/// 学ぶのに必要な、操作時間の合計（秒）。これより少ないうちは、設定のおやすみ時間を使う。
pub const MIN_DATA_SECS: u64 = 2 * 3600;
/// おやすみ時間の長さ（時間）。
pub const SLEEP_SPAN_HOURS: u8 = 8;
/// 同じくらい静かな時間帯が複数あるときに、近いものを選ぶ目安の時刻。
const USUAL_BEDTIME: u8 = 23;

/// 朝型・夜型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Chronotype {
    Morning,
    Night,
    Neutral,
}

pub fn total_secs(rhythm: &[u32; 24]) -> u64 {
    rhythm.iter().map(|&s| u64::from(s)).sum()
}

fn span_sum(rhythm: &[u32; 24], start: u8) -> u64 {
    (0..SLEEP_SPAN_HOURS).map(|i| u64::from(rhythm[usize::from((start + i) % 24)])).sum()
}

/// 23 時から、時計まわりに何時間ずれているか（近い向きで 0〜12）。
fn distance_from_usual(start: u8) -> u8 {
    let d = (start + 24 - USUAL_BEDTIME) % 24;
    d.min(24 - d)
}

/// いちばん操作していない連続 8 時間を、おやすみ時間として返す。データが足りなければ None。
pub fn learned_sleep_window(rhythm: &[u32; 24]) -> Option<SleepWindow> {
    if total_secs(rhythm) < MIN_DATA_SECS {
        return None;
    }
    let start = (0..24u8).min_by_key(|&s| (span_sum(rhythm, s), distance_from_usual(s)))?;
    Some(SleepWindow { start_hour: start, end_hour: (start + SLEEP_SPAN_HOURS) % 24 })
}

/// 朝（5〜10 時）に多ければ朝型、夜（20〜2 時）に多ければ夜型。データが足りなければ None。
pub fn chronotype(rhythm: &[u32; 24]) -> Option<Chronotype> {
    let total = total_secs(rhythm);
    if total < MIN_DATA_SECS {
        return None;
    }
    let share = |hours: &[usize]| hours.iter().map(|&h| u64::from(rhythm[h])).sum::<u64>() as f64 / total as f64;
    if share(&[20, 21, 22, 23, 0, 1, 2]) >= 0.4 {
        return Some(Chronotype::Night);
    }
    if share(&[5, 6, 7, 8, 9, 10]) >= 0.45 {
        return Some(Chronotype::Morning);
    }
    Some(Chronotype::Neutral)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 指定の時間帯だけ、1 時間あたり 1 時間ぶん操作したヒストグラム。
    fn active_during(hours: &[usize]) -> [u32; 24] {
        let mut r = [0; 24];
        for &h in hours {
            r[h] = 3600;
        }
        r
    }

    #[test]
    fn nothing_is_learned_from_too_little_data() {
        let mut r = [0; 24];
        r[10] = 3600;
        assert_eq!(learned_sleep_window(&r), None);
        assert_eq!(chronotype(&r), None);
    }

    #[test]
    fn an_office_worker_sleeps_around_the_usual_night_hours() {
        let r = active_during(&[9, 10, 11, 12, 13, 14, 15, 16, 17, 18]);
        let w = learned_sleep_window(&r).unwrap();
        // 19〜翌 2 時、20〜翌 3 時、…、1〜8 時などすべて静か → 23 時に近い 22 時か 23 時台が選ばれる
        assert_eq!(w.end_hour, (w.start_hour + 8) % 24);
        assert!((21..=24).contains(&(if w.start_hour == 0 { 24 } else { w.start_hour })), "{w:?}");
    }

    #[test]
    fn a_night_owl_sleeps_in_the_morning() {
        let r = active_during(&[14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 0, 1, 2, 3, 4]);
        let w = learned_sleep_window(&r).unwrap();
        assert_eq!(w, SleepWindow { start_hour: 5, end_hour: 13 });
        assert_eq!(chronotype(&r), Some(Chronotype::Night));
    }

    #[test]
    fn an_early_bird_goes_to_bed_early() {
        let r = active_during(&[5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
        let w = learned_sleep_window(&r).unwrap();
        assert!(w.start_hour >= 17 && w.start_hour <= 21, "{w:?}");
        assert_eq!(chronotype(&r), Some(Chronotype::Morning));
    }

    #[test]
    fn the_window_wraps_around_midnight() {
        let r = active_during(&[8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21]);
        let w = learned_sleep_window(&r).unwrap();
        // 22〜7 時は静か。その中で 23 時にいちばん近い 23 時始まりを選ぶ
        assert_eq!(w, SleepWindow { start_hour: 23, end_hour: 7 });
    }

    #[test]
    fn a_steady_daytime_user_is_neither_morning_nor_night() {
        let r = active_during(&[11, 12, 13, 14, 15, 16, 17]);
        assert_eq!(chronotype(&r), Some(Chronotype::Neutral));
    }

    #[test]
    fn equally_quiet_candidates_are_resolved_towards_the_usual_bedtime() {
        let r = active_during(&[12, 13]); // どこも同じくらい静か
        let w = learned_sleep_window(&r).unwrap();
        assert_eq!(w.start_hour, 23, "{w:?}"); // 12〜13 時を避けられる窓のうち、23 時にいちばん近いもの
    }

    #[test]
    fn distance_from_the_usual_bedtime_goes_both_ways_round_the_clock() {
        assert_eq!(distance_from_usual(23), 0);
        assert_eq!(distance_from_usual(1), 2);
        assert_eq!(distance_from_usual(21), 2);
        assert_eq!(distance_from_usual(11), 12);
    }

    #[test]
    fn chronotypes_serialize_in_lowercase() {
        assert_eq!(serde_json::to_value(Chronotype::Night).unwrap(), "night");
        assert_eq!(serde_json::to_value(Chronotype::Morning).unwrap(), "morning");
        assert_eq!(serde_json::to_value(Chronotype::Neutral).unwrap(), "neutral");
    }
}
