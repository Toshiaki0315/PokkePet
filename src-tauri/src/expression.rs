//! ステータスを「見た目の表現」（背景・吹き出し）に変換する。
//! 閾値などの判断はここ（Rust）に置き、フロントは受け取った値を描画するだけにする。

use serde::Serialize;

use crate::status::Status;

/// 値が低い／ふつう／高い。
pub const LOW_MAX: u8 = 30;
pub const HIGH_MIN: u8 = 70;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Low,
    Mid,
    High,
}

/// 吹き出しで伝える内容。`express` が返すのは Hungry > Dirty > Gloomy > Happy の優先度順で、
/// Welcome（初回オープニング）と Sleeping（おやすみ中）は呼び出し側が差し込む（Welcome > Sleeping > 他）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Bubble {
    Hungry,
    Dirty,
    Gloomy,
    Happy,
    Welcome,
    Sleeping,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Expression {
    pub mood: Level,
    /// 満腹側が大きい値なので Low = おなかがすいている。
    pub hunger: Level,
    /// High = 部屋がとても汚れている。
    pub dirtiness: Level,
    pub bubble: Option<Bubble>,
}

pub fn level(value: u8) -> Level {
    if value <= LOW_MAX {
        return Level::Low;
    }
    if value >= HIGH_MIN {
        return Level::High;
    }
    Level::Mid
}

pub fn express(status: &Status) -> Expression {
    let mood = level(status.mood);
    let hunger = level(status.hunger);
    let dirtiness = level(status.dirtiness);
    Expression { mood, hunger, dirtiness, bubble: pick_bubble(mood, hunger, dirtiness) }
}

fn pick_bubble(mood: Level, hunger: Level, dirtiness: Level) -> Option<Bubble> {
    if hunger == Level::Low {
        return Some(Bubble::Hungry);
    }
    if dirtiness == Level::High {
        return Some(Bubble::Dirty);
    }
    match mood {
        Level::Low => Some(Bubble::Gloomy),
        Level::High => Some(Bubble::Happy),
        Level::Mid => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(mood: u8, hunger: u8, dirtiness: u8) -> Status {
        Status::new(mood, 50, hunger, dirtiness)
    }

    #[test]
    fn level_boundaries() {
        assert_eq!(level(0), Level::Low);
        assert_eq!(level(LOW_MAX), Level::Low);
        assert_eq!(level(LOW_MAX + 1), Level::Mid);
        assert_eq!(level(HIGH_MIN - 1), Level::Mid);
        assert_eq!(level(HIGH_MIN), Level::High);
        assert_eq!(level(100), Level::High);
    }

    #[test]
    fn express_maps_each_parameter_to_a_level() {
        let e = express(&st(10, 50, 90));
        assert_eq!((e.mood, e.hunger, e.dirtiness), (Level::Low, Level::Mid, Level::High));
    }

    #[test]
    fn no_bubble_when_everything_is_ordinary() {
        assert_eq!(express(&st(50, 60, 20)).bubble, None);
    }

    #[test]
    fn hungry_bubble_when_hunger_is_low() {
        assert_eq!(express(&st(50, 20, 20)).bubble, Some(Bubble::Hungry));
    }

    #[test]
    fn dirty_bubble_when_room_is_filthy() {
        assert_eq!(express(&st(50, 60, 80)).bubble, Some(Bubble::Dirty));
    }

    #[test]
    fn gloomy_bubble_when_mood_is_low() {
        assert_eq!(express(&st(10, 60, 20)).bubble, Some(Bubble::Gloomy));
    }

    #[test]
    fn happy_bubble_when_mood_is_high_and_nothing_is_wrong() {
        assert_eq!(express(&st(90, 60, 20)).bubble, Some(Bubble::Happy));
    }

    #[test]
    fn bubble_priority_is_hungry_then_dirty_then_gloomy_then_happy() {
        assert_eq!(express(&st(0, 0, 100)).bubble, Some(Bubble::Hungry));
        assert_eq!(express(&st(0, 60, 100)).bubble, Some(Bubble::Dirty));
        assert_eq!(express(&st(0, 60, 20)).bubble, Some(Bubble::Gloomy));
        // 機嫌が高くても空腹・汚れが優先される。
        assert_eq!(express(&st(100, 10, 20)).bubble, Some(Bubble::Hungry));
        assert_eq!(express(&st(100, 60, 90)).bubble, Some(Bubble::Dirty));
    }

    #[test]
    fn serializes_with_lowercase_names_and_null_bubble() {
        let json = serde_json::to_value(express(&st(50, 60, 20))).unwrap();
        assert_eq!(json["mood"], "mid");
        assert_eq!(json["bubble"], serde_json::Value::Null);
        let json = serde_json::to_value(express(&st(50, 10, 20))).unwrap();
        assert_eq!(json["hunger"], "low");
        assert_eq!(json["bubble"], "hungry");
    }
}
