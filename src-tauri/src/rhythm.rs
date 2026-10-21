//! 生活リズム: 操作していた時間帯のヒストグラムから、おやすみ時間（と朝型・夜型）を学ぶ。

use serde::Serialize;

use crate::sleep::SleepWindow;

/// 学ぶのに必要な、操作時間の合計（秒）。これより少ないうちは、設定のおやすみ時間を使う。
pub const MIN_DATA_SECS: u64 = 2 * 3600;
/// おやすみ時間の長さ（時間）。
pub const SLEEP_SPAN_HOURS: u8 = 8;
/// 学んだおやすみ時間の始まりは、設定の始まりから、これだけ（時間）までしかずらさない。
/// 昼に操作が少ないだけの人が、昼に眠らされてしまわないため（昼夜が逆の人は、設定のおやすみ時間を直す）。
pub const MAX_SHIFT_HOURS: u8 = 4;

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

/// `usual` の始まりの時刻から、時計のどちらかまわりに何時間ずれているか（近い向きで 0〜12）。
fn distance_from(start: u8, usual: u8) -> u8 {
    let d = (start + 24 - usual % 24) % 24;
    d.min(24 - d)
}

/// 設定のおやすみ時間（`usual`）の近く（始まりが `MAX_SHIFT_HOURS` 時間以内）で、いちばん操作していない連続 8 時間を、
/// おやすみ時間として返す。同じくらい静かなら、設定に近いほう。データが足りなければ None。
pub fn learned_sleep_window(rhythm: &[u32; 24], usual: SleepWindow) -> Option<SleepWindow> {
    if total_secs(rhythm) < MIN_DATA_SECS {
        return None;
    }
    let start = (0..24u8).filter(|&s| distance_from(s, usual.start_hour) <= MAX_SHIFT_HOURS).min_by_key(|&s| (span_sum(rhythm, s), distance_from(s, usual.start_hour)))?;
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

    const USUAL: SleepWindow = SleepWindow { start_hour: 23, end_hour: 7 };

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
        assert_eq!(learned_sleep_window(&r, USUAL), None);
        assert_eq!(chronotype(&r), None);
    }

    #[test]
    fn an_office_worker_sleeps_around_the_usual_night_hours() {
        let r = active_during(&[9, 10, 11, 12, 13, 14, 15, 16, 17, 18]);
        let w = learned_sleep_window(&r, USUAL).unwrap();
        // 19〜翌 2 時、20〜翌 3 時、…、1〜8 時などすべて静か → 23 時に近い 22 時か 23 時台が選ばれる
        assert_eq!(w.end_hour, (w.start_hour + 8) % 24);
        assert!((21..=24).contains(&(if w.start_hour == 0 { 24 } else { w.start_hour })), "{w:?}");
    }

    #[test]
    fn a_night_owl_who_set_a_morning_bedtime_sleeps_in_the_morning() {
        let r = active_during(&[14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 0, 1, 2, 3, 4]);
        let w = learned_sleep_window(&r, SleepWindow { start_hour: 5, end_hour: 13 }).unwrap();
        assert_eq!(w, SleepWindow { start_hour: 5, end_hour: 13 });
        assert_eq!(chronotype(&r), Some(Chronotype::Night));
    }

    #[test]
    fn an_early_bird_goes_to_bed_early() {
        let r = active_during(&[5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
        let w = learned_sleep_window(&r, USUAL).unwrap();
        assert!(w.start_hour >= 17 && w.start_hour <= 21, "{w:?}");
        assert_eq!(chronotype(&r), Some(Chronotype::Morning));
    }

    #[test]
    fn a_window_in_the_quiet_daytime_is_not_learned_when_the_set_bedtime_is_at_night() {
        // 昼（10〜18 時）がいちばん静かで、夜も朝もよく操作する人
        let mut r = [3600u32; 24];
        r[10..18].fill(300);
        let w = learned_sleep_window(&r, USUAL).unwrap();
        assert!(distance_from(w.start_hour, 23) <= MAX_SHIFT_HOURS, "{w:?}");
        assert!(w.start_hour >= 19 || w.start_hour <= 3, "stays at night: {w:?}");
        // 設定が昼なら、昼になる
        let day = learned_sleep_window(&r, SleepWindow { start_hour: 10, end_hour: 18 }).unwrap();
        assert_eq!(day, SleepWindow { start_hour: 10, end_hour: 18 });
    }

    #[test]
    fn the_window_wraps_around_midnight() {
        let r = active_during(&[8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21]);
        let w = learned_sleep_window(&r, USUAL).unwrap();
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
        let w = learned_sleep_window(&r, USUAL).unwrap();
        assert_eq!(w.start_hour, 23, "{w:?}"); // 12〜13 時を避けられる窓のうち、23 時にいちばん近いもの
    }

    #[test]
    fn distance_from_the_usual_bedtime_goes_both_ways_round_the_clock() {
        assert_eq!(distance_from(23, 23), 0);
        assert_eq!(distance_from(1, 23), 2);
        assert_eq!(distance_from(21, 23), 2);
        assert_eq!(distance_from(11, 23), 12);
    }

    #[test]
    fn chronotypes_serialize_in_lowercase() {
        assert_eq!(serde_json::to_value(Chronotype::Night).unwrap(), "night");
        assert_eq!(serde_json::to_value(Chronotype::Morning).unwrap(), "morning");
        assert_eq!(serde_json::to_value(Chronotype::Neutral).unwrap(), "neutral");
    }
}
