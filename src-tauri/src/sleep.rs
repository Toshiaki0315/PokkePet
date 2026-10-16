//! おやすみ時間（ローカル時刻）。この間はキャラが眠り、状態の減りがゆっくりになる。

use chrono::{Local, Offset, TimeZone};

/// 既定のおやすみ時間（開始・終了の時）。
pub const DEFAULT_SLEEP_START_HOUR: u8 = 23;
pub const DEFAULT_SLEEP_END_HOUR: u8 = 7;

/// おやすみ時間。`start_hour`（含む）から `end_hour`（含まない）まで。
/// 開始が終了より大きければ日付をまたぐ（23→7 は 23時〜翌7時）。同じ値ならおやすみなし。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SleepWindow {
    pub start_hour: u8,
    pub end_hour: u8,
}

impl Default for SleepWindow {
    fn default() -> Self {
        Self { start_hour: DEFAULT_SLEEP_START_HOUR, end_hour: DEFAULT_SLEEP_END_HOUR }
    }
}

const DAY_SECS: u64 = 24 * 60 * 60;

/// `unix_secs`（UTC）が、UTC からの差 `utc_offset_secs` のローカル時刻でおやすみ時間か。
pub fn is_sleeping(unix_secs: u64, utc_offset_secs: i32, window: SleepWindow) -> bool {
    let local = i128::from(unix_secs) + i128::from(utc_offset_secs);
    let secs_of_day = local.rem_euclid(i128::from(DAY_SECS)) as u64;
    let hour = (secs_of_day / 3600) as u8;
    let SleepWindow { start_hour, end_hour } = window;
    if start_hour == end_hour {
        return false;
    }
    if start_hour < end_hour {
        return (start_hour..end_hour).contains(&hour);
    }
    hour >= start_hour || hour < end_hour
}

/// この時刻の、このマシンのローカルタイムゾーンの UTC オフセット（秒）。取得できなければ 0。
pub fn local_utc_offset(unix_secs: u64) -> i32 {
    i64::try_from(unix_secs)
        .ok()
        .and_then(|secs| Local.timestamp_opt(secs, 0).single())
        .map_or(0, |dt| dt.offset().fix().local_minus_utc())
}

#[cfg(test)]
mod tests {
    use super::*;

    const H: u64 = 3600;

    /// 既定のおやすみ時間（23〜7時）での判定。
    fn sleeping(unix: u64, offset: i32) -> bool {
        is_sleeping(unix, offset, SleepWindow::default())
    }

    fn window(start_hour: u8, end_hour: u8) -> SleepWindow {
        SleepWindow { start_hour, end_hour }
    }

    #[test]
    fn sleeps_from_23_to_7_in_utc() {
        assert!(!sleeping(22 * H + 59 * 60 + 59, 0));
        assert!(sleeping(23 * H, 0));
        assert!(sleeping(0, 0));
        assert!(sleeping(6 * H + 59 * 60 + 59, 0));
        assert!(!sleeping(7 * H, 0));
        assert!(!sleeping(12 * H, 0));
    }

    #[test]
    fn follows_the_local_time_zone() {
        let jst = 9 * 3600;
        assert!(sleeping(14 * H, jst)); // UTC 14:00 = JST 23:00
        assert!(!sleeping(14 * H - 1, jst));
        assert!(!sleeping(22 * H, jst)); // UTC 22:00 = JST 07:00
        let est = -5 * 3600;
        assert!(sleeping(4 * H, est)); // UTC 04:00 = 前日 23:00
        assert!(!sleeping(12 * H, est));
    }

    #[test]
    fn a_custom_window_that_wraps_midnight() {
        let w = window(22, 6);
        assert!(!is_sleeping(21 * H + 3599, 0, w));
        assert!(is_sleeping(22 * H, 0, w));
        assert!(is_sleeping(5 * H + 3599, 0, w));
        assert!(!is_sleeping(6 * H, 0, w));
        // 既定では眠っている 6:30 も、この設定では起きている。
        assert!(sleeping(6 * H + 1800, 0));
        assert!(!is_sleeping(6 * H + 1800, 0, w));
    }

    #[test]
    fn a_window_within_one_day() {
        let w = window(13, 15); // 昼寝
        assert!(!is_sleeping(12 * H + 3599, 0, w));
        assert!(is_sleeping(13 * H, 0, w));
        assert!(is_sleeping(14 * H + 3599, 0, w));
        assert!(!is_sleeping(15 * H, 0, w));
        assert!(!is_sleeping(2 * H, 0, w)); // 夜は起きている
    }

    #[test]
    fn a_window_starting_at_midnight() {
        let w = window(0, 5);
        assert!(is_sleeping(0, 0, w));
        assert!(is_sleeping(4 * H + 3599, 0, w));
        assert!(!is_sleeping(5 * H, 0, w));
        assert!(!is_sleeping(23 * H, 0, w));
    }

    #[test]
    fn equal_start_and_end_means_never_sleeping() {
        for h in 0..24u8 {
            for hour in 0..24u64 {
                assert!(!is_sleeping(hour * H, 0, window(h, h)), "{h} {hour}");
            }
        }
    }

    #[test]
    fn a_custom_window_follows_the_local_time_zone_too() {
        let jst = 9 * 3600;
        let w = window(22, 6);
        assert!(is_sleeping(13 * H, jst, w)); // UTC 13:00 = JST 22:00
        assert!(!is_sleeping(13 * H - 1, jst, w));
    }

    #[test]
    fn the_default_window_is_23_to_7() {
        let w = SleepWindow::default();
        assert_eq!((w.start_hour, w.end_hour), (23, 7));
    }

    #[test]
    fn repeats_every_day() {
        for day in [0u64, 1, 365, 20_000] {
            assert!(sleeping(day * DAY_SECS + 2 * H, 0));
            assert!(!sleeping(day * DAY_SECS + 15 * H, 0));
        }
    }

    #[test]
    fn local_offset_is_a_plausible_time_zone_offset() {
        let off = local_utc_offset(1_700_000_000);
        assert!((-14 * 3600..=14 * 3600).contains(&off), "{off}");
    }

    #[test]
    fn local_offset_tolerates_absurd_timestamps() {
        let _ = local_utc_offset(u64::MAX);
    }
}
