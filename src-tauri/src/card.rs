//! 成長カード（画面で描いた PNG 画像）を保存するための、受け取りと名前づけ。

use chrono::{DateTime, Datelike, Timelike};

/// 受け付ける画像の大きさの上限（バイト）。
pub const MAX_CARD_BYTES: usize = 8 * 1024 * 1024;

const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
const PREFIX: &str = "data:image/png;base64,";

fn sextet(c: u8) -> Option<u32> {
    match c {
        b'A'..=b'Z' => Some(u32::from(c - b'A')),
        b'a'..=b'z' => Some(u32::from(c - b'a') + 26),
        b'0'..=b'9' => Some(u32::from(c - b'0') + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

/// 標準の base64（`=` で埋める形）を戻す。
fn decode_base64(text: &str) -> Result<Vec<u8>, String> {
    let bytes: Vec<u8> = text.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if !bytes.len().is_multiple_of(4) {
        return Err("the image data is incomplete".to_string());
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for chunk in bytes.chunks(4) {
        let padding = chunk.iter().rev().take_while(|&&b| b == b'=').count();
        if padding > 2 {
            return Err("the image data is not valid".to_string());
        }
        let mut triple = 0u32;
        for &c in &chunk[..4 - padding] {
            triple = (triple << 6) | sextet(c).ok_or("the image data is not valid")?;
        }
        triple <<= 6 * padding as u32;
        let all = [(triple >> 16) as u8, (triple >> 8) as u8, triple as u8];
        out.extend_from_slice(&all[..3 - padding]);
    }
    Ok(out)
}

/// 画面から届いた `data:image/png;base64,...` を、PNG のバイト列にする。PNG でなければエラー。
pub fn decode_png_data_url(url: &str) -> Result<Vec<u8>, String> {
    let body = url.strip_prefix(PREFIX).ok_or("the card must be a PNG image")?;
    if body.len() > MAX_CARD_BYTES * 4 / 3 + 8 {
        return Err("the card image is too large".to_string());
    }
    let bytes = decode_base64(body)?;
    if !bytes.starts_with(&PNG_SIGNATURE) {
        return Err("the card must be a PNG image".to_string());
    }
    Ok(bytes)
}

/// 保存するファイル名。「PokkePet-ごま-20261017-153045.png」。名前に使えない文字は `_` にする。
pub fn card_file_name(pet_name: Option<&str>, unix_secs: u64, utc_offset_secs: i32) -> String {
    let local = i64::try_from(unix_secs).unwrap_or(0).saturating_add(i64::from(utc_offset_secs));
    let stamp = DateTime::from_timestamp(local, 0).map_or_else(
        || "00000000-000000".to_string(),
        |d| format!("{:04}{:02}{:02}-{:02}{:02}{:02}", d.year(), d.month(), d.day(), d.hour(), d.minute(), d.second()),
    );
    let name: String = pet_name
        .unwrap_or("card")
        .chars()
        .map(|c| if c.is_control() || "/\\:*?\"<>|. ".contains(c) { '_' } else { c })
        .collect();
    format!("PokkePet-{name}-{stamp}.png")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data_url(bytes: &[u8]) -> String {
        const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::from(PREFIX);
        for chunk in bytes.chunks(3) {
            let n = chunk.iter().enumerate().fold(0u32, |acc, (i, b)| acc | (u32::from(*b) << (16 - 8 * i)));
            for i in 0..4 {
                if i <= chunk.len() {
                    out.push(char::from(ALPHABET[((n >> (18 - 6 * i)) & 63) as usize]));
                } else {
                    out.push('=');
                }
            }
        }
        out
    }

    #[test]
    fn base64_is_decoded_with_and_without_padding() {
        assert_eq!(decode_base64("TWFu").unwrap(), b"Man");
        assert_eq!(decode_base64("TWE=").unwrap(), b"Ma");
        assert_eq!(decode_base64("TQ==").unwrap(), b"M");
        assert_eq!(decode_base64("").unwrap(), b"");
        assert_eq!(decode_base64("TW Fu\n").unwrap(), b"Man", "whitespace is ignored");
    }

    #[test]
    fn broken_base64_is_rejected() {
        assert!(decode_base64("TWF").unwrap_err().contains("incomplete"));
        assert!(decode_base64("TW!u").unwrap_err().contains("not valid"));
        assert!(decode_base64("T===").unwrap_err().contains("not valid"));
    }

    #[test]
    fn a_png_data_url_round_trips() {
        let mut png = PNG_SIGNATURE.to_vec();
        png.extend((0..=255u8).cycle().take(1000));
        assert_eq!(decode_png_data_url(&data_url(&png)).unwrap(), png);
    }

    #[test]
    fn only_png_images_are_accepted() {
        assert!(decode_png_data_url("data:image/jpeg;base64,AAAA").is_err());
        assert!(decode_png_data_url("hello").is_err());
        assert!(decode_png_data_url(&data_url(b"not a png at all")).unwrap_err().contains("PNG"));
    }

    #[test]
    fn a_huge_image_is_refused() {
        let url = format!("{PREFIX}{}", "A".repeat(MAX_CARD_BYTES * 2));
        assert!(decode_png_data_url(&url).unwrap_err().contains("too large"));
    }

    #[test]
    fn the_file_name_has_the_pet_and_the_local_time() {
        // 2026-10-17 06:30:45 UTC は、UTC+9 では 15:30:45
        let unix = 1_792_218_645;
        assert_eq!(card_file_name(Some("ごま"), unix, 9 * 3600), "PokkePet-ごま-20261017-153045.png");
        assert_eq!(card_file_name(None, unix, 0), "PokkePet-card-20261017-063045.png");
    }

    #[test]
    fn an_impossible_time_still_gives_a_usable_file_name() {
        assert_eq!(card_file_name(Some("ごま"), i64::MAX as u64, 0), "PokkePet-ごま-00000000-000000.png");
    }

    #[test]
    fn characters_that_are_not_safe_in_a_file_name_are_replaced() {
        let name = card_file_name(Some("a/b:c.d e"), 0, 0);
        assert_eq!(name, "PokkePet-a_b_c_d_e-19700101-000000.png");
        assert!(!card_file_name(Some("x\ny"), 0, 0).contains('\n'));
    }
}
