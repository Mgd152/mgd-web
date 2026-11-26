//! Key names accepted in profiles.
//!
//! Keyboard names are exactly the `Debug` names of winit 0.30 `KeyCode`
//! variants (a test in Mirrorly's `input` crate proves this when the crate is
//! built there). Mouse buttons are extra names used by this project.

/// Keyboard key names: exactly these winit 0.30 `KeyCode` Debug names.
pub const KEY_NAMES: &[&str] = &[
    // Letters
    "KeyA", "KeyB", "KeyC", "KeyD", "KeyE", "KeyF", "KeyG", "KeyH", "KeyI", "KeyJ", "KeyK", "KeyL",
    "KeyM", "KeyN", "KeyO", "KeyP", "KeyQ", "KeyR", "KeyS", "KeyT", "KeyU", "KeyV", "KeyW", "KeyX",
    "KeyY", "KeyZ",
    // Digits
    "Digit0", "Digit1", "Digit2", "Digit3", "Digit4", "Digit5", "Digit6", "Digit7", "Digit8",
    "Digit9",
    // Function keys
    "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12", "F13", "F14", "F15",
    "F16", "F17", "F18", "F19", "F20", "F21", "F22", "F23", "F24",
    // Editing / whitespace
    "Space", "Tab", "Enter", "Escape", "Backspace", "CapsLock", "ContextMenu",
    // Modifiers
    "ShiftLeft", "ShiftRight", "ControlLeft", "ControlRight", "AltLeft", "AltRight", "SuperLeft",
    "SuperRight",
    // Punctuation
    "Backquote", "Minus", "Equal", "BracketLeft", "BracketRight", "Backslash", "Semicolon",
    "Quote", "Comma", "Period", "Slash", "IntlBackslash",
    // Navigation
    "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "Insert", "Delete", "Home", "End", "PageUp",
    "PageDown",
    // Numpad
    "NumLock", "Numpad0", "Numpad1", "Numpad2", "Numpad3", "Numpad4", "Numpad5", "Numpad6",
    "Numpad7", "Numpad8", "Numpad9", "NumpadAdd", "NumpadSubtract", "NumpadMultiply",
    "NumpadDivide", "NumpadDecimal", "NumpadEnter",
    // Misc
    "ScrollLock", "Pause",
];

/// Mouse button names.
pub const MOUSE_NAMES: &[&str] = &["MouseLeft", "MouseRight", "MouseMiddle", "MouseBack", "MouseForward"];

pub const DEFAULT_TOGGLE_KEY: &str = "Backquote";
pub const DEFAULT_PANIC_KEY: &str = "F8";

/// Keyboard or mouse name, case-sensitive.
pub fn is_known_key(name: &str) -> bool {
    KEY_NAMES.contains(&name) || MOUSE_NAMES.contains(&name)
}

pub fn is_mouse_button(name: &str) -> bool {
    MOUSE_NAMES.contains(&name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn count_and_uniqueness() {
        assert_eq!(KEY_NAMES.len(), 116);
        let set: HashSet<_> = KEY_NAMES.iter().collect();
        assert_eq!(set.len(), KEY_NAMES.len());
        let mouse: HashSet<_> = MOUSE_NAMES.iter().collect();
        assert_eq!(mouse.len(), MOUSE_NAMES.len());
    }

    #[test]
    fn mouse_names_are_disjoint_from_keys() {
        for m in MOUSE_NAMES {
            assert!(!KEY_NAMES.contains(m));
            assert!(is_mouse_button(m));
            assert!(is_known_key(m));
        }
        assert!(!is_mouse_button("KeyA"));
    }

    #[test]
    fn defaults_are_known_keyboard_keys() {
        assert!(is_known_key(DEFAULT_TOGGLE_KEY));
        assert!(is_known_key(DEFAULT_PANIC_KEY));
        assert!(!is_mouse_button(DEFAULT_TOGGLE_KEY));
        assert!(!is_mouse_button(DEFAULT_PANIC_KEY));
    }

    #[test]
    fn lookup_is_case_sensitive() {
        assert!(is_known_key("KeyA"));
        assert!(!is_known_key("keya"));
        assert!(!is_known_key(""));
    }
}
