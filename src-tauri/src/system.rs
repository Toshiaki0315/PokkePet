//! Mac の状態を読むための、出力の解釈（純粋な関数）。実際のコマンド実行は shell.rs 側。

/// `ioreg -c IOHIDSystem` の出力から、最後のキーボード・マウス操作からの秒数を取り出す。
/// 出力には `"HIDIdleTime" = 1234567890`（ナノ秒）の形で入っている。
pub fn parse_hid_idle_secs(ioreg_output: &str) -> Option<u64> {
    let rest = ioreg_output.split("\"HIDIdleTime\"").nth(1)?;
    let digits: String = rest.trim_start_matches([' ', '=', '\t']).chars().take_while(char::is_ascii_digit).collect();
    let nanos: u64 = digits.parse().ok()?;
    Some(nanos / 1_000_000_000)
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
    fn output_without_an_idle_time_is_not_understood() {
        assert_eq!(parse_hid_idle_secs(""), None);
        assert_eq!(parse_hid_idle_secs("nothing here"), None);
        assert_eq!(parse_hid_idle_secs("\"HIDIdleTime\" = abc"), None);
    }
}
