//! おやすみ時間（ローカル時刻）。この間はキャラが眠り、状態の減りがゆっくりになる。

use chrono::{DateTime, Datelike, Local, Offset, TimeZone};

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

/// `unix_secs`（UTC）の、UTC からの差 `utc_offset_secs` のローカル時刻での「時」（0〜23）。
pub fn local_hour(unix_secs: u64, utc_offset_secs: i32) -> u8 {
    let local = i128::from(unix_secs) + i128::from(utc_offset_secs);
    (local.rem_euclid(i128::from(DAY_SECS)) / 3600) as u8
}

/// `unix_secs`（UTC）の、UTC からの差 `utc_offset_secs` のローカル時刻での（月, 日）。
pub fn local_month_day(unix_secs: u64, utc_offset_secs: i32) -> (u32, u32) {
    let local = i64::try_from(unix_secs).unwrap_or(0).saturating_add(i64::from(utc_offset_secs));
    DateTime::from_timestamp(local, 0).map_or((1, 1), |dt| (dt.month(), dt.day()))
}

/// `unix_secs`（UTC）が、UTC からの差 `utc_offset_secs` のローカル時刻でおやすみ時間か。
pub fn is_sleeping(unix_secs: u64, utc_offset_secs: i32, window: SleepWindow) -> bool {
    let hour = local_hour(unix_secs, utc_offset_secs);
    let SleepWindow { start_hour, end_hour } = window;
    if start_hour == end_hour {
        return false;
    }
    if start_hour < end_hour {
        return (start_hour..end_hour).contains(&hour);
    }
    hour >= start_hour || hour < end_hour
}

/// ペットが眠っている期間（UNIX 秒、開始以上・終了未満）。電源ボタンで寝かされた間、または起こされた間。
pub type Span = (u64, u64);

fn within(span: Option<Span>, t: u64) -> bool {
    span.is_some_and(|(from, to)| (from..to).contains(&t))
}

/// `start` から `end`（どちらも UTC の UNIX 秒）まで、ペットが**眠らずに起きていた時間**（`start` から、最初に眠るまで）。
/// 眠っているのは、おやすみ時間（ただし `woken` の間は起こされている）か、電源ボタンで寝かされた間（`forced`）。
/// 5 分きざみで調べる。`start` の時点で眠っていたら 0。
pub fn awake_run_between(start_unix: u64, end_unix: u64, utc_offset_secs: i32, window: SleepWindow, forced: Option<Span>, woken: Option<Span>) -> u64 {
    const CHUNK: u64 = 300;
    let mut awake = 0;
    let mut t = start_unix;
    while t < end_unix {
        let asleep = within(forced, t) || (is_sleeping(t, utc_offset_secs, window) && !within(woken, t));
        if asleep {
            break;
        }
        let chunk = CHUNK.min(end_unix - t);
        awake += chunk;
        t += chunk;
    }
    awake
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
    fn the_local_date_follows_the_offset() {
        // 2026-01-01 00:00:00 UTC
        let new_year = 1_767_225_600;
        assert_eq!(local_month_day(new_year, 0), (1, 1));
        assert_eq!(local_month_day(new_year - 1, 0), (12, 31));
        assert_eq!(local_month_day(new_year - 1, 9 * 3600), (1, 1), "already the new year in Tokyo");
        assert_eq!(local_month_day(new_year, -3600), (12, 31));
    }

    #[test]
    fn the_awake_run_stops_when_the_pet_falls_asleep() {
        let night = SleepWindow { start_hour: 23, end_hour: 7 };
        // 22:00 から 10 時間: 23 時に眠るので、起きていたのは最初の 1 時間（朝に起きた分は数えない）
        assert_eq!(awake_run_between(22 * H, 32 * H, 0, night, None, None), H);
        assert_eq!(awake_run_between(10 * H, 12 * H, 0, night, None, None), 2 * H, "all day");
        assert_eq!(awake_run_between(0, 6 * H, 0, night, None, None), 0, "asleep from the start");
        assert_eq!(awake_run_between(100, 100, 0, night, None, None), 0);
        assert_eq!(awake_run_between(200, 100, 0, night, None, None), 0);
        assert_eq!(awake_run_between(0, 10 * H, 0, SleepWindow { start_hour: 5, end_hour: 5 }, None, None), 10 * H, "a pet that never sleeps");
    }

    #[test]
    fn a_span_that_is_not_a_multiple_of_the_step_is_counted_to_the_second() {
        let night = SleepWindow { start_hour: 23, end_hour: 7 };
        assert_eq!(awake_run_between(12 * H, 12 * H + 421, 0, night, None, None), 421);
    }

    #[test]
    fn a_pet_put_to_bed_with_the_power_button_is_asleep_whatever_the_clock_says() {
        let night = SleepWindow { start_hour: 23, end_hour: 7 };
        // 21:00 に寝かされた（8 時間）。おやすみ時間の前から眠っている
        let forced = Some((21 * H, 29 * H));
        assert_eq!(awake_run_between(21 * H, 33 * H, 0, night, forced, None), 0);
        // 20:00 から見ると、寝かされるまでの 1 時間だけ起きていた
        assert_eq!(awake_run_between(20 * H, 33 * H, 0, night, forced, None), H);
    }

    #[test]
    fn a_pet_woken_in_the_night_is_awake_until_the_woken_span_ends() {
        let night = SleepWindow { start_hour: 23, end_hour: 7 };
        let woken = Some((24 * H, 26 * H)); // 0 時から 2 時間、起こされていた
        assert_eq!(awake_run_between(24 * H, 30 * H, 0, night, None, woken), 2 * H);
        assert_eq!(awake_run_between(24 * H, 30 * H, 0, night, None, None), 0);
    }

    #[test]
    fn local_hour_follows_the_offset_and_wraps_around_midnight() {
        assert_eq!(local_hour(0, 0), 0);
        assert_eq!(local_hour(9 * H + 1_800, 0), 9);
        assert_eq!(local_hour(23 * H, 2 * 3600), 1);
        assert_eq!(local_hour(H, -2 * 3600), 23);
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
