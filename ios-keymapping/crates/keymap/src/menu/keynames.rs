//! Key labels, free-key choice and per-key advice.

use super::doc::EditDoc;
use super::msg::{Advice, Lang, MirrorlyAction};
use crate::keys::{KEY_NAMES, is_known_key, is_mouse_button};

/// Suggested button labels (Vietnamese).
pub const LABEL_PRESETS_VI: &[&str] = &[
    "Bắn",
    "Ngắm",
    "Nhảy",
    "Ngồi",
    "Nằm",
    "Nạp đạn",
    "Đổi súng",
    "Hồi máu",
    "Lựu đạn",
    "Nhặt đồ",
    "Lên xe",
];

/// Default helper keys of the in-play sensitivity adjustment (section 4.7).
pub const HELPER_KEYS: [&str; 2] = ["BracketLeft", "BracketRight"];

/// Order in which `free_key` proposes keys.
pub const FREE_KEY_ORDER: &[&str] = &[
    "Space",
    "KeyR",
    "KeyC",
    "KeyF",
    "KeyG",
    "KeyQ",
    "KeyE",
    "KeyX",
    "KeyZ",
    "KeyV",
    "KeyB",
    "Digit1",
    "Digit2",
    "Digit3",
    "Digit4",
    "Digit5",
    "Digit6",
    "Digit7",
    "Digit8",
    "Digit9",
    "Digit0",
    "ShiftLeft",
    "ControlLeft",
    "Tab",
    "KeyT",
    "KeyY",
    "KeyH",
    "KeyN",
    "KeyM",
    "KeyU",
    "KeyI",
    "KeyO",
    "KeyP",
    "KeyJ",
    "KeyK",
    "KeyL",
];

/// Keys `free_key` never proposes: F-keys, Escape, Super, lock keys, helper keys.
pub(crate) fn excluded_from_auto(name: &str) -> bool {
    is_f_key(name)
        || name == "Escape"
        || name.starts_with("Super")
        || is_lock_key(name)
        || HELPER_KEYS.contains(&name)
}

fn is_f_key(name: &str) -> bool {
    name.strip_prefix('F')
        .is_some_and(|d| !d.is_empty() && d.chars().all(|c| c.is_ascii_digit()))
}

fn is_lock_key(name: &str) -> bool {
    matches!(name, "CapsLock" | "NumLock" | "ScrollLock")
}

/// First key (in the documented order) that nothing in the doc uses.
pub fn free_key(doc: &EditDoc) -> Option<&'static str> {
    free_key_excluding(doc, &[])
}

/// Like [`free_key`], also skipping `skip` (keys already picked for a
/// control that is not in the doc yet).
pub fn free_key_excluding(doc: &EditDoc, skip: &[&str]) -> Option<&'static str> {
    let free = |k: &str| doc.owner_of(k).is_none() && !skip.contains(&k);
    FREE_KEY_ORDER
        .iter()
        .copied()
        .find(|k| free(k))
        .or_else(|| {
            KEY_NAMES
                .iter()
                .copied()
                .find(|k| !excluded_from_auto(k) && !FREE_KEY_ORDER.contains(k) && free(k))
        })
}

/// Human label of a key or mouse button name. Unknown names come back as is.
pub fn key_label(name: &str, lang: Lang) -> String {
    let vi = lang == Lang::Vi;
    let pick = |v: &str, e: &str| if vi { v } else { e }.to_string();
    if let Some(c) = name.strip_prefix("Key").filter(|c| c.len() == 1) {
        return c.to_string();
    }
    if let Some(c) = name.strip_prefix("Digit").filter(|c| c.len() == 1) {
        return c.to_string();
    }
    if let Some(n) = name.strip_prefix("Numpad") {
        let n = match n {
            "Add" => "+",
            "Subtract" => "-",
            "Multiply" => "*",
            "Divide" => "/",
            "Decimal" => ".",
            other => other,
        };
        return format!("Num {n}");
    }
    match name {
        "Space" | "Tab" | "Enter" | "Backspace" | "Insert" | "Delete" | "Home" | "End"
        | "Pause" => name.to_string(),
        "Escape" => "Esc".into(),
        "CapsLock" => "Caps Lock".into(),
        "NumLock" => "Num Lock".into(),
        "ScrollLock" => "Scroll Lock".into(),
        "ContextMenu" => "Menu".into(),
        "PageUp" => "Page Up".into(),
        "PageDown" => "Page Down".into(),
        "ShiftLeft" => pick("Shift trái", "Left Shift"),
        "ShiftRight" => pick("Shift phải", "Right Shift"),
        "ControlLeft" => pick("Ctrl trái", "Left Ctrl"),
        "ControlRight" => pick("Ctrl phải", "Right Ctrl"),
        "AltLeft" => pick("Alt trái", "Left Alt"),
        "AltRight" => pick("Alt phải", "Right Alt"),
        "SuperLeft" => pick("Windows trái", "Left Windows"),
        "SuperRight" => pick("Windows phải", "Right Windows"),
        "Backquote" => "`".into(),
        "Minus" => "-".into(),
        "Equal" => "=".into(),
        "BracketLeft" => "[".into(),
        "BracketRight" => "]".into(),
        "Backslash" => "\\".into(),
        "IntlBackslash" => pick("\\ phụ", "Extra \\"),
        "Semicolon" => ";".into(),
        "Quote" => "'".into(),
        "Comma" => ",".into(),
        "Period" => ".".into(),
        "Slash" => "/".into(),
        "ArrowUp" => "↑".into(),
        "ArrowDown" => "↓".into(),
        "ArrowLeft" => "←".into(),
        "ArrowRight" => "→".into(),
        "MouseLeft" => pick("Chuột trái", "Left mouse"),
        "MouseRight" => pick("Chuột phải", "Right mouse"),
        "MouseMiddle" => pick("Chuột giữa", "Middle mouse"),
        "MouseBack" => pick("Chuột bên (lùi)", "Mouse side (back)"),
        "MouseForward" => pick("Chuột bên (tới)", "Mouse side (forward)"),
        other => other.to_string(), // F1..F24 and anything unknown
    }
}

/// Advice about a key that is not an error (section 4.7, 6.9).
pub fn key_advice(name: &str) -> Option<Advice> {
    if !is_known_key(name) || is_mouse_button(name) {
        return None;
    }
    let shadow = |action| {
        Some(Advice::ShadowsMirrorly {
            key: name.to_string(),
            action,
        })
    };
    match name {
        "F1" => shadow(MirrorlyAction::Home),
        "F2" => shadow(MirrorlyAction::AppSwitcher),
        "F3" => shadow(MirrorlyAction::Lock),
        "F4" => shadow(MirrorlyAction::VolumeDown),
        "F5" => shadow(MirrorlyAction::VolumeUp),
        "F6" => shadow(MirrorlyAction::Screenshot),
        "F7" => shadow(MirrorlyAction::Record),
        "F9" => shadow(MirrorlyAction::LatencyProbe),
        "F10" => shadow(MirrorlyAction::Stats),
        "F11" => shadow(MirrorlyAction::Fullscreen),
        "F12" => shadow(MirrorlyAction::Rotate),
        "SuperLeft" | "SuperRight" => Some(Advice::WindowsKey),
        "AltLeft" | "AltRight" => Some(Advice::AltKey),
        _ if is_lock_key(name) => Some(Advice::LockKey),
        _ if HELPER_KEYS.contains(&name) => Some(Advice::HelperKey),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::MOUSE_NAMES;
    use crate::menu::templates::{Template, template};
    use std::collections::HashSet;

    // test 47
    #[test]
    fn labels_unique_and_non_empty() {
        for lang in [Lang::Vi, Lang::En] {
            let mut seen = HashSet::new();
            for n in KEY_NAMES.iter().chain(MOUSE_NAMES) {
                let l = key_label(n, lang);
                assert!(!l.is_empty(), "{n}");
                assert!(
                    seen.insert(l.clone()),
                    "duplicate label {l} for {n} ({lang:?})"
                );
            }
        }
    }

    #[test]
    fn spec_label_examples() {
        let v = |n| key_label(n, Lang::Vi);
        assert_eq!(v("KeyW"), "W");
        assert_eq!(v("Digit1"), "1");
        assert_eq!(v("Space"), "Space");
        assert_eq!(v("Backquote"), "`");
        assert_eq!(v("BracketLeft"), "[");
        assert_eq!(v("ShiftLeft"), "Shift trái");
        assert_eq!(v("ControlLeft"), "Ctrl trái");
        assert_eq!(v("AltLeft"), "Alt trái");
        assert_eq!(v("ArrowUp"), "↑");
        assert_eq!(v("Enter"), "Enter");
        assert_eq!(v("Escape"), "Esc");
        assert_eq!(v("MouseLeft"), "Chuột trái");
        assert_eq!(v("MouseRight"), "Chuột phải");
        assert_eq!(v("MouseMiddle"), "Chuột giữa");
        assert_eq!(v("MouseBack"), "Chuột bên (lùi)");
        assert_eq!(v("MouseForward"), "Chuột bên (tới)");
        assert_eq!(v("Numpad7"), "Num 7");
        assert_eq!(v("F5"), "F5");
        assert_eq!(v("Whatever"), "Whatever");
        assert_eq!(key_label("MouseLeft", Lang::En), "Left mouse");
    }

    // test 48
    #[test]
    fn free_key_order_names_are_known_and_unique() {
        let mut seen = HashSet::new();
        for k in FREE_KEY_ORDER {
            assert!(KEY_NAMES.contains(k), "{k} is not in KEY_NAMES");
            assert!(seen.insert(*k), "{k} twice");
            assert!(!excluded_from_auto(k), "{k} is excluded");
        }
        for h in HELPER_KEYS {
            assert!(KEY_NAMES.contains(&h));
        }
    }

    #[test]
    fn free_key_skips_bound_helper_and_excluded_keys() {
        let mut doc = EditDoc::from_profile(template(Template::Empty(
            crate::profile::Orientation::Landscape,
        )))
        .unwrap();
        assert_eq!(free_key(&doc), Some("Space"));
        let mut got = Vec::new();
        while got.len() < 60 {
            let k = free_key(&doc).unwrap();
            got.push(k);
            doc.add(
                crate::menu::Kind::Button,
                crate::menu::Pt { x: 0.5, y: 0.5 },
            )
            .unwrap();
        }
        for k in &got {
            assert!(!excluded_from_auto(k));
            assert!(!is_mouse_button(k));
        }
        assert_eq!(&got[..3], &["Space", "KeyR", "KeyC"]);
        assert!(!got.contains(&"BracketLeft"));
        assert!(!got.contains(&"F5"));
        assert!(!got.contains(&"Escape"));
        assert!(!got.contains(&"CapsLock"));
        assert!(!got.contains(&"SuperLeft"));
        assert!(!got.contains(&"Backquote")); // toggle key is taken
    }

    // test 40 (key side)
    #[test]
    fn advice_cases() {
        assert!(matches!(
            key_advice("F4"),
            Some(Advice::ShadowsMirrorly {
                action: MirrorlyAction::VolumeDown,
                ..
            })
        ));
        for f in [
            "F1", "F2", "F3", "F5", "F6", "F7", "F9", "F10", "F11", "F12",
        ] {
            assert!(
                matches!(key_advice(f), Some(Advice::ShadowsMirrorly { .. })),
                "{f}"
            );
        }
        assert_eq!(key_advice("F8"), None);
        assert_eq!(key_advice("SuperLeft"), Some(Advice::WindowsKey));
        assert_eq!(key_advice("AltLeft"), Some(Advice::AltKey));
        assert_eq!(key_advice("CapsLock"), Some(Advice::LockKey));
        assert_eq!(key_advice("NumLock"), Some(Advice::LockKey));
        assert_eq!(key_advice("ScrollLock"), Some(Advice::LockKey));
        assert_eq!(key_advice("BracketLeft"), Some(Advice::HelperKey));
        assert_eq!(key_advice("BracketRight"), Some(Advice::HelperKey));
        assert_eq!(key_advice("KeyR"), None);
        assert_eq!(key_advice("MouseLeft"), None);
        assert_eq!(key_advice("Nope"), None);
    }

    #[test]
    fn label_presets() {
        assert_eq!(LABEL_PRESETS_VI.len(), 11);
        assert_eq!(LABEL_PRESETS_VI[0], "Bắn");
        assert!(LABEL_PRESETS_VI.iter().all(|s| s.chars().count() <= 40));
    }
}
