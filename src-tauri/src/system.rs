//! Mac の状態を読むための、出力の解釈（純粋な関数）。実際のコマンド実行は shell.rs 側。

/// `ioreg -c IOHIDSystem` の出力から、最後のキーボード・マウス操作からの秒数を取り出す。
/// 出力には `"HIDIdleTime" = 1234567890`（ナノ秒）の形で入っている。
pub fn parse_hid_idle_secs(ioreg_output: &str) -> Option<u64> {
    let rest = ioreg_output.split("\"HIDIdleTime\"").nth(1)?;
    let digits: String = rest.trim_start_matches([' ', '=', '\t']).chars().take_while(char::is_ascii_digit).collect();
    let nanos: u64 = digits.parse().ok()?;
    Some(nanos / 1_000_000_000)
}

/// 電池の状態。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Battery {
    pub percent: u8,
    /// 電源につながっている（充電中、または満充電）。
    pub on_ac: bool,
}

/// いまの Mac の状態。ペットが反応するために使う。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SystemInfo {
    /// 電池（電池のない Mac では None）。
    pub battery: Option<Battery>,
    /// インターネットにつながっているか。
    pub online: bool,
    /// ダークモードか。
    pub dark_mode: bool,
}

impl Default for SystemInfo {
    /// 何も分かっていないときは、「何も起きていない」状態（つながっていて、電池の心配もない）。
    fn default() -> Self {
        Self { battery: None, online: true, dark_mode: false }
    }
}

/// 電池が「少ない」とみなす残量（%）。電源につながっていないときだけ。
pub const LOW_BATTERY_PERCENT: u8 = 20;

impl SystemInfo {
    pub fn battery_low(&self) -> bool {
        self.battery.is_some_and(|b| !b.on_ac && b.percent <= LOW_BATTERY_PERCENT)
    }

    /// 充電中（電源につながっていて、まだ満タンでない）。
    pub fn charging(&self) -> bool {
        self.battery.is_some_and(|b| b.on_ac && b.percent < 100)
    }
}

/// `pmset -g batt` の出力から、電池の残量と電源の状態を取り出す。電池のない Mac では None。
pub fn parse_battery(pmset_output: &str) -> Option<Battery> {
    let line = pmset_output.lines().find(|l| l.contains("InternalBattery"))?;
    let percent_text = line.split('%').next()?;
    let digits: String = percent_text.chars().rev().take_while(char::is_ascii_digit).collect::<Vec<_>>().into_iter().rev().collect();
    let percent: u8 = digits.parse::<u16>().ok()?.min(100) as u8;
    let on_ac = pmset_output.contains("AC Power");
    Some(Battery { percent, on_ac })
}

/// `defaults read -g AppleInterfaceStyle` の出力（ダークモードのときだけ "Dark" と出る）。
pub fn parse_dark_mode(defaults_output: &str) -> bool {
    defaults_output.trim() == "Dark"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_idle_time_is_read_in_seconds_from_nanoseconds() {
        let out = "+-o IOHIDSystem  <class IOHIDSystem>\n    | {\n    |   \"HIDIdleTime\" = 4296731458\n    | }\n";
        assert_eq!(parse_hid_idle_secs(out), Some(4));
        assert_eq!(parse_hid_idle_secs("\"HIDIdleTime\" = 0"), Some(0));
        assert_eq!(parse_hid_idle_secs("\"HIDIdleTime\" = 1800000000000"), Some(1800));
    }

    #[test]
    fn the_battery_is_read_from_pmset() {
        let on_battery = "Now drawing from 'Battery Power'\n -InternalBattery-0 (id=4653155)\t85%; discharging; 4:23 remaining present: true\n";
        assert_eq!(parse_battery(on_battery), Some(Battery { percent: 85, on_ac: false }));
        let charging = "Now drawing from 'AC Power'\n -InternalBattery-0 (id=4653155)\t42%; charging; 1:10 remaining present: true\n";
        assert_eq!(parse_battery(charging), Some(Battery { percent: 42, on_ac: true }));
        let full = "Now drawing from 'AC Power'\n -InternalBattery-0 (id=1)\t100%; charged; 0:00 remaining present: true\n";
        assert_eq!(parse_battery(full), Some(Battery { percent: 100, on_ac: true }));
    }

    #[test]
    fn a_mac_without_a_battery_reports_none() {
        assert_eq!(parse_battery("Now drawing from 'AC Power'\n"), None);
        assert_eq!(parse_battery(""), None);
        assert_eq!(parse_battery("-InternalBattery-0\tno percent here"), None);
    }

    #[test]
    fn only_the_word_dark_means_dark_mode() {
        assert!(parse_dark_mode("Dark\n"));
        assert!(!parse_dark_mode(""));
        assert!(!parse_dark_mode("Light"));
    }

    #[test]
    fn a_low_battery_matters_only_when_not_plugged_in() {
        let info = |percent, on_ac| SystemInfo { battery: Some(Battery { percent, on_ac }), ..SystemInfo::default() };
        assert!(info(20, false).battery_low());
        assert!(!info(21, false).battery_low());
        assert!(!info(5, true).battery_low());
        assert!(!SystemInfo::default().battery_low());
    }

    #[test]
    fn charging_means_plugged_in_and_not_yet_full() {
        let info = |percent, on_ac| SystemInfo { battery: Some(Battery { percent, on_ac }), ..SystemInfo::default() };
        assert!(info(50, true).charging());
        assert!(!info(100, true).charging());
        assert!(!info(50, false).charging());
        assert!(!SystemInfo::default().charging());
    }

    #[test]
    fn by_default_nothing_is_going_on() {
        let d = SystemInfo::default();
        assert_eq!((d.battery, d.online, d.dark_mode), (None, true, false));
    }

    #[test]
    fn output_without_an_idle_time_is_not_understood() {
        assert_eq!(parse_hid_idle_secs(""), None);
        assert_eq!(parse_hid_idle_secs("nothing here"), None);
        assert_eq!(parse_hid_idle_secs("\"HIDIdleTime\" = abc"), None);
    }
}
