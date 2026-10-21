//! Tauri からは取れない macOS の情報（画面に出ている窓の一覧）を CoreGraphics から読む。
//! 画面収録やアクセシビリティなどの権限は要らない（窓の位置・大きさ・階層・持ち主のプロセスだけを読む。窓の名前や中身は見ない）。

use core_foundation::array::{CFArray, CFArrayRef};
use core_foundation::base::{CFType, TCFType};
use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;

use crate::desk::{Rect, WindowInfo};

#[repr(C)]
#[derive(Default)]
struct CGRect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

const K_CG_WINDOW_LIST_ON_SCREEN_ONLY: u32 = 1 << 0;
const K_CG_WINDOW_LIST_EXCLUDE_DESKTOP_ELEMENTS: u32 = 1 << 4;
const K_CG_NULL_WINDOW_ID: u32 = 0;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGWindowListCopyWindowInfo(option: u32, relative_to_window: u32) -> CFArrayRef;
    fn CGRectMakeWithDictionaryRepresentation(dict: CFDictionaryRef, rect: *mut CGRect) -> bool;
}

/// 画面に出ている窓を、手前から順に返す。
pub fn on_screen_windows() -> Vec<WindowInfo> {
    let own_pid = i64::from(std::process::id());
    let id_key = CFString::from_static_string("kCGWindowNumber");
    let layer_key = CFString::from_static_string("kCGWindowLayer");
    let pid_key = CFString::from_static_string("kCGWindowOwnerPID");
    let bounds_key = CFString::from_static_string("kCGWindowBounds");

    let windows: CFArray<CFDictionary<CFString, CFType>> = unsafe {
        let raw = CGWindowListCopyWindowInfo(K_CG_WINDOW_LIST_ON_SCREEN_ONLY | K_CG_WINDOW_LIST_EXCLUDE_DESKTOP_ELEMENTS, K_CG_NULL_WINDOW_ID);
        if raw.is_null() {
            return Vec::new();
        }
        CFArray::wrap_under_create_rule(raw)
    };
    let number = |dict: &CFDictionary<CFString, CFType>, key: &CFString| dict.find(key).and_then(|v| v.downcast::<CFNumber>()).and_then(|n| n.to_i64());

    windows
        .iter()
        .filter_map(|dict| {
            let bounds = dict.find(&bounds_key).and_then(|v| v.downcast::<CFDictionary>())?;
            let mut r = CGRect::default();
            if !unsafe { CGRectMakeWithDictionaryRepresentation(bounds.as_concrete_TypeRef(), &mut r) } {
                return None;
            }
            Some(WindowInfo { id: number(&dict, &id_key)?, layer: number(&dict, &layer_key)?, bounds: Rect::new(r.x, r.y, r.width, r.height), own: number(&dict, &pid_key) == Some(own_pid) })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 窓の一覧が読める（画面のない環境では空でもよい）。読んだ窓は、大きさが正しい。
    #[test]
    fn the_window_list_can_be_read() {
        for w in on_screen_windows() {
            assert!(w.bounds.w >= 0.0 && w.bounds.h >= 0.0);
        }
    }
}
