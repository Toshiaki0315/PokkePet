//! 季節（春・夏・秋・冬）。背景の「季節にまかせる」と、季節限定のおみやげで使う。

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Season {
    Spring,
    Summer,
    Autumn,
    Winter,
}

/// 月（1〜12）から季節を返す。春 3〜5 月・夏 6〜8 月・秋 9〜11 月・冬 12〜2 月。
pub fn season_of(month: u32) -> Season {
    match month {
        3..=5 => Season::Spring,
        6..=8 => Season::Summer,
        9..=11 => Season::Autumn,
        _ => Season::Winter,
    }
}

/// 季節に合う背景（`settings::BACKGROUNDS` のどれか）。
pub fn scene_for(season: Season) -> &'static str {
    match season {
        Season::Spring => "sakura",
        Season::Summer => "beach",
        Season::Autumn => "autumn",
        Season::Winter => "snow",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::BACKGROUNDS;

    #[test]
    fn every_month_belongs_to_one_of_the_four_seasons() {
        let seasons: Vec<Season> = (1..=12).map(season_of).collect();
        assert_eq!(
            seasons,
            [
                Season::Winter, Season::Winter, Season::Spring, Season::Spring, Season::Spring, Season::Summer,
                Season::Summer, Season::Summer, Season::Autumn, Season::Autumn, Season::Autumn, Season::Winter,
            ]
        );
    }

    #[test]
    fn each_season_has_a_background_that_can_be_chosen() {
        for season in [Season::Spring, Season::Summer, Season::Autumn, Season::Winter] {
            assert!(BACKGROUNDS.contains(&scene_for(season)), "{season:?}");
        }
        assert_eq!(scene_for(Season::Spring), "sakura");
        assert_eq!(scene_for(Season::Winter), "snow");
    }

    #[test]
    fn seasons_serialize_in_lowercase() {
        assert_eq!(serde_json::to_value(Season::Autumn).unwrap(), "autumn");
    }
}
