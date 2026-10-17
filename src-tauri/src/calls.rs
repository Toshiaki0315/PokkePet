//! 「呼びかけ」通知の判断。おなかがすいた・部屋が汚れた・つまらないを、しつこくならない範囲で知らせる。

use crate::expression::Bubble;

/// 同じ種類の呼びかけを繰り返す最短間隔（秒）。
pub const CALL_COOLDOWN_SECS: u64 = 60 * 60;
/// 世話をしないまま呼びかけを続ける上限。超えたら、次に世話をするまで黙る。
pub const MAX_UNANSWERED: u8 = 3;

/// 呼びかけの種類（吹き出しのうち「困りごと」だけ）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Call {
    Hungry,
    Dirty,
    Gloomy,
    Selfish,
}

impl Call {
    pub fn from_bubble(bubble: Option<Bubble>) -> Option<Call> {
        match bubble? {
            Bubble::Hungry => Some(Call::Hungry),
            Bubble::Dirty => Some(Call::Dirty),
            Bubble::Gloomy => Some(Call::Gloomy),
            Bubble::Selfish => Some(Call::Selfish),
            Bubble::Happy | Bubble::Welcome | Bubble::Sleeping | Bubble::Morning | Bubble::Noon | Bubble::Evening | Bubble::Drowsy => None,
        }
    }

    /// 通知の本文。
    pub fn message(self) -> &'static str {
        match self {
            Call::Hungry => "おなかすいたよ…",
            Call::Dirty => "おへや、よごれてるよ…",
            Call::Gloomy => "つまんないな…",
            Call::Selfish => "ねえねえ、ちょっと来て！",
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

#[derive(Debug, Default)]
pub struct CallGate {
    last: [Option<u64>; 4],
    unanswered: u8,
}

impl CallGate {
    pub fn new() -> Self {
        Self::default()
    }

    /// いま呼びかけの通知を出すべきなら、その種類を返す（出したものとして記録する）。
    /// `quiet` は、おやすみ中・ポップアップを開いている・通知オフなど、鳴らしてはいけない状況。
    pub fn decide(&mut self, now_unix: u64, bubble: Option<Bubble>, quiet: bool) -> Option<Call> {
        let call = Call::from_bubble(bubble)?;
        if quiet || self.unanswered >= MAX_UNANSWERED {
            return None;
        }
        let slot = &mut self.last[call.index()];
        if slot.is_some_and(|last| now_unix.saturating_sub(last) < CALL_COOLDOWN_SECS) {
            return None;
        }
        *slot = Some(now_unix);
        self.unanswered += 1;
        Some(call)
    }

    /// 世話をした。次の困りごとからは、すぐに呼びかけられるようになる。
    pub fn answered(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: u64 = 1_000_000;

    fn gate() -> CallGate {
        CallGate::new()
    }

    #[test]
    fn only_needs_become_calls() {
        assert_eq!(Call::from_bubble(Some(Bubble::Hungry)), Some(Call::Hungry));
        assert_eq!(Call::from_bubble(Some(Bubble::Dirty)), Some(Call::Dirty));
        assert_eq!(Call::from_bubble(Some(Bubble::Gloomy)), Some(Call::Gloomy));
        assert_eq!(Call::from_bubble(Some(Bubble::Selfish)), Some(Call::Selfish));
        for other in [Bubble::Happy, Bubble::Welcome, Bubble::Sleeping, Bubble::Morning, Bubble::Noon, Bubble::Evening, Bubble::Drowsy] {
            assert_eq!(Call::from_bubble(Some(other)), None);
        }
        assert_eq!(Call::from_bubble(None), None);
    }

    #[test]
    fn every_call_has_a_message() {
        for call in [Call::Hungry, Call::Dirty, Call::Gloomy, Call::Selfish] {
            assert!(!call.message().is_empty());
        }
        assert_ne!(Call::Hungry.message(), Call::Dirty.message());
    }

    #[test]
    fn calls_as_soon_as_a_need_appears() {
        assert_eq!(gate().decide(T0, Some(Bubble::Hungry), false), Some(Call::Hungry));
    }

    #[test]
    fn says_nothing_without_a_need() {
        let mut g = gate();
        for b in [None, Some(Bubble::Happy), Some(Bubble::Welcome), Some(Bubble::Sleeping)] {
            assert_eq!(g.decide(T0, b, false), None);
        }
    }

    #[test]
    fn stays_quiet_when_told_to() {
        let mut g = gate();
        assert_eq!(g.decide(T0, Some(Bubble::Hungry), true), None);
        // 静かにしていた分は記録されない: 静かでなくなればすぐ呼ぶ。
        assert_eq!(g.decide(T0 + 1, Some(Bubble::Hungry), false), Some(Call::Hungry));
    }

    #[test]
    fn does_not_repeat_the_same_call_within_the_cooldown() {
        let mut g = gate();
        assert!(g.decide(T0, Some(Bubble::Hungry), false).is_some());
        assert_eq!(g.decide(T0 + CALL_COOLDOWN_SECS - 1, Some(Bubble::Hungry), false), None);
        assert_eq!(g.decide(T0 + CALL_COOLDOWN_SECS, Some(Bubble::Hungry), false), Some(Call::Hungry));
    }

    #[test]
    fn different_kinds_have_independent_cooldowns() {
        let mut g = gate();
        assert_eq!(g.decide(T0, Some(Bubble::Hungry), false), Some(Call::Hungry));
        assert_eq!(g.decide(T0 + 10, Some(Bubble::Dirty), false), Some(Call::Dirty));
        assert_eq!(g.decide(T0 + 20, Some(Bubble::Gloomy), false), Some(Call::Gloomy));
    }

    #[test]
    fn gives_up_after_three_unanswered_calls_until_cared_for() {
        let mut g = gate();
        for i in 0..u64::from(MAX_UNANSWERED) {
            assert!(g.decide(T0 + i * CALL_COOLDOWN_SECS, Some(Bubble::Hungry), false).is_some(), "call {i}");
        }
        let later = T0 + 10 * CALL_COOLDOWN_SECS;
        assert_eq!(g.decide(later, Some(Bubble::Hungry), false), None);
        assert_eq!(g.decide(later + 1, Some(Bubble::Dirty), false), None); // 種類が違っても黙る

        g.answered();
        assert_eq!(g.decide(later + 2, Some(Bubble::Dirty), false), Some(Call::Dirty));
    }

    #[test]
    fn caring_resets_the_cooldowns_too() {
        let mut g = gate();
        assert!(g.decide(T0, Some(Bubble::Hungry), false).is_some());
        g.answered();
        // ご飯をあげたあとまたおなかがすいたら、クールダウン前でも呼べる。
        assert_eq!(g.decide(T0 + 5, Some(Bubble::Hungry), false), Some(Call::Hungry));
    }

    #[test]
    fn a_clock_that_runs_backwards_does_not_panic() {
        let mut g = gate();
        assert!(g.decide(T0, Some(Bubble::Hungry), false).is_some());
        assert_eq!(g.decide(T0 - 500, Some(Bubble::Hungry), false), None);
    }
}
