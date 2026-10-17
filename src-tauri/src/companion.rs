//! 作業に寄り添う: Mac を触っている時間（操作の有無だけ。何を打ったか・どのアプリかは見ない）を
//! 「いっしょにいた時間」として数え、親密度・休憩の声かけ・おかえりに使う。

use serde::{Deserialize, Serialize};

/// 最後の操作からこの秒数より短ければ「操作している」とみなす。
pub const ACTIVE_IDLE_MAX_SECS: u64 = 120;
/// この秒数以上、操作がなければ「席を離れた」（おでかけ・おかえりの対象）。
pub const AWAY_AFTER_SECS: u64 = 30 * 60;
/// 休憩なしで操作し続けて、この秒数が経ったら「ちょっと休もうよ」。
pub const BREAK_AFTER_SECS: u64 = 90 * 60;
/// この秒数以上、操作がなければ休憩したことになり、連続作業の数え直しになる。
pub const BREAK_RESET_IDLE_SECS: u64 = 5 * 60;
/// 操作している時間がこの秒数たまるごとに、親密度が 1 上がる。
pub const AFFINITY_EVERY_SECS: u64 = 30 * 60;
/// 戻ってきたあと、「おかえり」を言う長さ。
pub const WELCOME_BACK_SECS: u64 = 10 * 60;
/// 前回の観測からこれ以上空いたら、その間は数えない（スリープ明けなどで、長い空白をまるごと数えないため）。
const MAX_STEP_SECS: u64 = 60;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Companion {
    /// いっしょに作業した時間の合計（秒）。
    #[serde(default)]
    pub total_secs: u64,
    /// 休憩なしで続けている作業の長さ（秒）。
    #[serde(default)]
    pub session_secs: u64,
    #[serde(default)]
    last_observed: Option<u64>,
    #[serde(default)]
    affinity_acc: u64,
    /// 席を離れた時刻（戻ってくるまで）。
    #[serde(default)]
    away_since: Option<u64>,
    /// 「おかえり」を言う期限（UNIX秒）。
    #[serde(default)]
    pub back_until: u64,
    #[serde(default)]
    break_notified: bool,
    /// 時刻（0〜23 時）ごとの、操作していた時間（秒）。生活リズムを学ぶ材料。
    #[serde(default)]
    pub rhythm: [u32; 24],
    /// 席を離れている間に、ペットが起きていた時間（秒）。眠っている間は、おでかけにならない。
    #[serde(default)]
    away_awake_secs: u64,
}

/// ペットが起きているか（観測のたびに、呼び出し側が教える）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Awake {
    /// いま起きているか。
    pub now: bool,
    /// 前回の観測から長く空いたとき（Mac のスリープなど）、その間に起きていた秒数。None なら、ずっと起きていたことにする。
    pub during_gap: Option<u64>,
}

impl Awake {
    /// いつも起きている（おやすみ時間がない）。
    pub const ALWAYS: Awake = Awake { now: true, during_gap: None };
}

/// 1 回の観測で起きたこと。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Observation {
    /// 増えた親密度。
    pub affinity_gain: u8,
    /// いま初めて「休憩の時間」になった（通知を出すのは一度だけ）。
    pub break_due: bool,
    /// 席を離れていたあと戻ってきた。離れていた秒数。
    pub returned_after: Option<u64>,
    /// そのうち、ペットが起きていた秒数（おでかけと認めるのは、この時間で決まる）。
    pub outing_secs: u64,
}

impl Companion {
    /// 観測する。`now_unix` は現在、`idle_secs` は最後の操作からの秒数、`local_hour` はローカル時刻の「時」。
    /// ペットはずっと起きているものとして観測する（`observe_with` の簡単な版）。
    pub fn observe(&mut self, now_unix: u64, idle_secs: u64, local_hour: u8) -> Observation {
        self.observe_with(now_unix, idle_secs, local_hour, Awake::ALWAYS)
    }

    /// `observe` に、ペットが起きているかを加えたもの。席を離れている間にペットが眠っていた時間は、おでかけに数えない。
    pub fn observe_with(&mut self, now_unix: u64, idle_secs: u64, local_hour: u8, awake: Awake) -> Observation {
        let (step, gap) = self.step_since_last(now_unix);
        if let (Some(gap), Some(_)) = (gap, self.away_since) {
            self.away_awake_secs += awake.during_gap.unwrap_or(gap).min(gap);
        }
        let mut obs = Observation::default();
        if idle_secs < ACTIVE_IDLE_MAX_SECS {
            self.work(step, local_hour, &mut obs);
            if let Some(since) = self.away_since.take() {
                obs.returned_after = Some(now_unix.saturating_sub(since));
                obs.outing_secs = std::mem::take(&mut self.away_awake_secs);
                self.back_until = now_unix + WELCOME_BACK_SECS;
            }
            return obs;
        }
        if idle_secs >= BREAK_RESET_IDLE_SECS {
            self.session_secs = 0;
            self.break_notified = false;
        }
        if idle_secs >= AWAY_AFTER_SECS && self.away_since.is_none() {
            self.away_since = Some(now_unix.saturating_sub(idle_secs));
            self.away_awake_secs = if awake.now { idle_secs } else { 0 };
        } else if self.away_since.is_some() && awake.now {
            self.away_awake_secs += step;
        }
        obs
    }

    /// 最後に観測した時刻（まだなら None）。
    pub fn last_observed(&self) -> Option<u64> {
        self.last_observed
    }

    /// 前回の観測からの経過（上限つき）と、長く空いたときのその長さを返し、観測時刻を更新する。
    /// 前回からとても長く空いていたら（スリープなど）、その間は席を離れていたことにする。
    fn step_since_last(&mut self, now_unix: u64) -> (u64, Option<u64>) {
        let previous = self.last_observed.replace(now_unix);
        let Some(prev) = previous.filter(|&p| now_unix > p) else {
            return (0, None);
        };
        let gap = now_unix - prev;
        if gap >= AWAY_AFTER_SECS && self.away_since.is_none() {
            self.away_since = Some(prev);
        }
        if gap <= MAX_STEP_SECS { (gap, None) } else { (0, Some(gap)) }
    }

    fn work(&mut self, step: u64, local_hour: u8, obs: &mut Observation) {
        self.total_secs += step;
        self.session_secs += step;
        let slot = &mut self.rhythm[usize::from(local_hour.min(23))];
        *slot = slot.saturating_add(u32::try_from(step).unwrap_or(u32::MAX));
        self.affinity_acc += step;
        while self.affinity_acc >= AFFINITY_EVERY_SECS {
            self.affinity_acc -= AFFINITY_EVERY_SECS;
            obs.affinity_gain = obs.affinity_gain.saturating_add(1);
        }
        if self.session_secs >= BREAK_AFTER_SECS && !self.break_notified {
            self.break_notified = true;
            obs.break_due = true;
        }
    }

    /// 休憩なしで長く作業しているか（吹き出し「ちょっと休もうよ」）。
    pub fn needs_a_break(&self) -> bool {
        self.session_secs >= BREAK_AFTER_SECS
    }

    /// 戻ってきたばかりか（吹き出し「おかえり」）。
    pub fn is_welcoming_back(&self, now_unix: u64) -> bool {
        now_unix < self.back_until
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: u64 = 1_000_000;

    /// 10 秒おきに、操作している状態で `secs` 秒ぶん観測する。
    fn work_for(c: &mut Companion, start: u64, secs: u64, hour: u8) -> Vec<Observation> {
        (1..=secs / 10).map(|i| c.observe(start + i * 10, 0, hour)).collect()
    }

    #[test]
    fn working_time_is_counted_while_the_user_is_active() {
        let mut c = Companion::default();
        c.observe(T0, 0, 10); // 最初の観測は起点（数えない）
        work_for(&mut c, T0, 300, 10);
        assert_eq!((c.total_secs, c.session_secs), (300, 300));
        assert_eq!(c.rhythm[10], 300);
    }

    #[test]
    fn idle_time_is_not_counted() {
        let mut c = Companion::default();
        c.observe(T0, 0, 10);
        for i in 1..=30 {
            c.observe(T0 + i * 10, ACTIVE_IDLE_MAX_SECS, 10);
        }
        assert_eq!(c.total_secs, 0);
        assert_eq!(c.rhythm, [0; 24]);
    }

    #[test]
    fn half_an_hour_of_work_raises_the_affinity_by_one() {
        let mut c = Companion::default();
        c.observe(T0, 0, 9);
        let gains: u32 = work_for(&mut c, T0, AFFINITY_EVERY_SECS - 10, 9).iter().map(|o| u32::from(o.affinity_gain)).sum();
        assert_eq!(gains, 0);
        let gains: u32 = work_for(&mut c, T0 + AFFINITY_EVERY_SECS - 10, 20, 9).iter().map(|o| u32::from(o.affinity_gain)).sum();
        assert_eq!(gains, 1);
    }

    #[test]
    fn a_long_gap_between_observations_is_not_counted_as_work() {
        let mut c = Companion::default();
        c.observe(T0, 0, 9);
        c.observe(T0 + 3 * 3600, 0, 9); // Mac がスリープしていた
        assert_eq!(c.total_secs, 0);
    }

    #[test]
    fn a_break_is_suggested_once_after_long_continuous_work() {
        let mut c = Companion::default();
        c.observe(T0, 0, 14);
        let obs = work_for(&mut c, T0, BREAK_AFTER_SECS + 600, 14);
        assert_eq!(obs.iter().filter(|o| o.break_due).count(), 1);
        assert!(c.needs_a_break());
    }

    #[test]
    fn resting_for_a_while_resets_the_session_and_allows_another_suggestion() {
        let mut c = Companion::default();
        c.observe(T0, 0, 14);
        work_for(&mut c, T0, BREAK_AFTER_SECS, 14);
        assert!(c.needs_a_break());
        let t = T0 + BREAK_AFTER_SECS;
        c.observe(t + 10, BREAK_RESET_IDLE_SECS, 14);
        assert!(!c.needs_a_break());
        let again = work_for(&mut c, t + 10, BREAK_AFTER_SECS + 20, 14);
        assert_eq!(again.iter().filter(|o| o.break_due).count(), 1);
    }

    #[test]
    fn a_short_pause_does_not_count_as_a_break() {
        let mut c = Companion::default();
        c.observe(T0, 0, 14);
        work_for(&mut c, T0, 1000, 14);
        c.observe(T0 + 1010, BREAK_RESET_IDLE_SECS - 1, 14);
        assert_eq!(c.session_secs, 1000);
    }

    #[test]
    fn coming_back_after_being_away_is_noticed_and_welcomed_for_a_while() {
        let mut c = Companion::default();
        c.observe(T0, 0, 9);
        c.observe(T0 + 10, AWAY_AFTER_SECS, 9); // 離席（最後の操作は 30 分前）
        let back = T0 + 3600;
        let obs = c.observe(back, 0, 9);
        assert_eq!(obs.returned_after, Some(back - (T0 + 10 - AWAY_AFTER_SECS)));
        assert!(c.is_welcoming_back(back + WELCOME_BACK_SECS - 1));
        assert!(!c.is_welcoming_back(back + WELCOME_BACK_SECS));
        assert_eq!(c.observe(back + 10, 0, 9).returned_after, None, "only once");
    }

    const ASLEEP: Awake = Awake { now: false, during_gap: Some(0) };

    /// 10 秒おきに、操作していない状態を `secs` 秒ぶん観測する。
    fn idle_for(c: &mut Companion, start: u64, from_idle: u64, secs: u64, awake: Awake) {
        for i in 1..=secs / 10 {
            c.observe_with(start + i * 10, from_idle + i * 10, 9, awake);
        }
    }

    #[test]
    fn time_the_pet_spends_asleep_while_the_owner_is_away_is_not_an_outing() {
        let mut c = Companion::default();
        c.observe(T0, 0, 22);
        // 22:00 に席を離れる。30 分後（まだ起きている）に離席と分かる
        idle_for(&mut c, T0, 0, AWAY_AFTER_SECS, Awake::ALWAYS);
        // そのあとペットが寝て、7 時間、戻らない
        let slept_from = T0 + AWAY_AFTER_SECS;
        idle_for(&mut c, slept_from, AWAY_AFTER_SECS, 7 * 3600, ASLEEP);
        let back = slept_from + 7 * 3600 + 10;
        let obs = c.observe_with(back, 0, 6, Awake::ALWAYS);
        assert_eq!(obs.returned_after, Some(back - T0));
        assert_eq!(obs.outing_secs, AWAY_AFTER_SECS, "only the half hour before it fell asleep");
    }

    #[test]
    fn an_awake_pet_is_out_for_the_whole_time() {
        let mut c = Companion::default();
        c.observe(T0, 0, 10);
        idle_for(&mut c, T0, 0, 3 * 3600, Awake::ALWAYS);
        let back = T0 + 3 * 3600 + 10;
        let obs = c.observe_with(back, 0, 13, Awake::ALWAYS);
        assert!(obs.outing_secs >= 3 * 3600 - 20 && obs.outing_secs <= 3 * 3600 + 10, "{}", obs.outing_secs);
        // 次の離席は、数え直し
        let mut again = c.clone();
        idle_for(&mut again, back, 0, AWAY_AFTER_SECS + 100, Awake::ALWAYS);
        let obs = again.observe_with(back + AWAY_AFTER_SECS + 200, 0, 13, Awake::ALWAYS);
        assert!(obs.outing_secs <= AWAY_AFTER_SECS + 200, "{}", obs.outing_secs);
    }

    #[test]
    fn a_pet_that_was_asleep_when_the_owner_left_has_no_outing_at_all() {
        let mut c = Companion::default();
        c.observe_with(T0, 0, 2, ASLEEP);
        idle_for(&mut c, T0, 0, 4 * 3600, ASLEEP);
        let obs = c.observe_with(T0 + 4 * 3600 + 10, 0, 6, Awake::ALWAYS);
        assert!(obs.returned_after.is_some());
        assert_eq!(obs.outing_secs, 0);
    }

    #[test]
    fn when_the_mac_slept_only_the_awake_part_of_the_gap_counts() {
        let mut c = Companion::default();
        c.observe(T0, 0, 22);
        let wake = T0 + 9 * 3600;
        let obs = c.observe_with(wake, 5, 7, Awake { now: true, during_gap: Some(2 * 3600) });
        assert_eq!(obs.returned_after, Some(9 * 3600));
        assert_eq!(obs.outing_secs, 2 * 3600);
    }

    #[test]
    fn a_gap_without_the_pets_sleep_schedule_counts_in_full() {
        let mut c = Companion::default();
        c.observe(T0, 0, 22);
        let obs = c.observe_with(T0 + 5 * 3600, 5, 3, Awake { now: true, during_gap: None });
        assert_eq!(obs.outing_secs, 5 * 3600);
    }

    #[test]
    fn a_short_gap_is_not_an_outing_and_adds_nothing() {
        let mut c = Companion::default();
        c.observe(T0, 0, 12);
        c.observe_with(T0 + 600, 5, 12, Awake::ALWAYS);
        assert_eq!(c.observe_with(T0 + 610, 0, 12, Awake::ALWAYS).outing_secs, 0);
    }

    #[test]
    fn sleeping_the_mac_counts_as_being_away() {
        let mut c = Companion::default();
        c.observe(T0, 0, 9);
        let obs = c.observe(T0 + 2 * 3600, 5, 9);
        assert_eq!(obs.returned_after, Some(2 * 3600));
    }

    #[test]
    fn a_short_absence_is_not_an_outing() {
        let mut c = Companion::default();
        c.observe(T0, 0, 9);
        c.observe(T0 + 10, ACTIVE_IDLE_MAX_SECS + 5, 9);
        assert_eq!(c.observe(T0 + 20, 0, 9).returned_after, None);
    }

    #[test]
    fn the_hour_is_kept_in_range() {
        let mut c = Companion::default();
        c.observe(T0, 0, 99);
        c.observe(T0 + 10, 0, 99);
        assert_eq!(c.rhythm[23], 10);
    }

    #[test]
    fn old_saves_load_with_nothing_counted() {
        let c: Companion = serde_json::from_str("{}").unwrap();
        assert_eq!(c, Companion::default());
    }
}
