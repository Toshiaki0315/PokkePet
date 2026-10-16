//! おやすみ時間（ローカル時刻）。この間はキャラが眠り、状態の減りがゆっくりになる。

use chrono::{Local, Offset, TimeZone};

/// おやすみの開始・終了（時）。開始が終了より大きい場合は日付をまたぐ（23時〜翌7時）。
pub const SLEEP_START_HOUR: u32 = 23;
pub const SLEEP_END_HOUR: u32 = 7;

const DAY_SECS: u64 = 24 * 60 * 60;

/// `unix_secs`（UTC）が、UTC からの差 `utc_offset_secs` のローカル時刻でおやすみ時間か。
pub fn is_sleeping(unix_secs: u64, utc_offset_secs: i32) -> bool {
    let local = i128::from(unix_secs) + i128::from(utc_offset_secs);
    let secs_of_day = local.rem_euclid(i128::from(DAY_SECS)) as u64;
    let hour = (secs_of_day / 3600) as u32;
    hour >= SLEEP_START_HOUR || hour < SLEEP_END_HOUR
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

    #[test]
    fn sleeps_from_23_to_7_in_utc() {
        assert!(!is_sleeping(22 * H + 59 * 60 + 59, 0));
        assert!(is_sleeping(23 * H, 0));
        assert!(is_sleeping(0, 0));
        assert!(is_sleeping(6 * H + 59 * 60 + 59, 0));
        assert!(!is_sleeping(7 * H, 0));
        assert!(!is_sleeping(12 * H, 0));
    }

    #[test]
    fn follows_the_local_time_zone() {
        let jst = 9 * 3600;
        assert!(is_sleeping(14 * H, jst)); // UTC 14:00 = JST 23:00
        assert!(!is_sleeping(14 * H - 1, jst));
        assert!(!is_sleeping(22 * H, jst)); // UTC 22:00 = JST 07:00
        let est = -5 * 3600;
        assert!(is_sleeping(4 * H, est)); // UTC 04:00 = 前日 23:00
        assert!(!is_sleeping(12 * H, est));
    }

    #[test]
    fn repeats_every_day() {
        for day in [0u64, 1, 365, 20_000] {
            assert!(is_sleeping(day * DAY_SECS + 2 * H, 0));
            assert!(!is_sleeping(day * DAY_SECS + 15 * H, 0));
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
