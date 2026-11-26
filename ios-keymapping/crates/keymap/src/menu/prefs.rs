//! `keymap-ui.json`: small UI preferences (section 6.12, 7.3).
//!
//! Forward compatible (unknown fields are ignored). A file that cannot be
//! read or parsed gives the defaults, including `notice_ack = 0`, so the
//! first-run notice shows again; the file is rewritten on the next change.

use super::msg::Lang;
use super::store::{Fs, FsError};
use crate::keys::{is_known_key, is_mouse_button};
use serde::{Deserialize, Serialize};

pub const PREFS_FILE: &str = "keymap-ui.json";
pub const DEFAULT_SENS_DOWN_KEY: &str = "BracketLeft";
pub const DEFAULT_SENS_UP_KEY: &str = "BracketRight";

/// Play overlay: `Auto` hides itself after 5 s.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Overlay {
    #[default]
    Auto,
    Always,
    Off,
}

fn d_version() -> u32 {
    1
}
fn d_down() -> String {
    DEFAULT_SENS_DOWN_KEY.to_string()
}
fn d_up() -> String {
    DEFAULT_SENS_UP_KEY.to_string()
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)] // NOT deny_unknown_fields
pub struct UiPrefs {
    #[serde(default = "d_version")]
    pub version: u32,
    /// File name of the chosen profile.
    #[serde(default)]
    pub active: Option<String>,
    #[serde(default)]
    pub overlay: Overlay,
    #[serde(default = "d_down")]
    pub sens_down_key: String,
    #[serde(default = "d_up")]
    pub sens_up_key: String,
    #[serde(default)]
    pub notice_ack: u32,
    #[serde(default)]
    pub guide_done: bool,
    #[serde(default)]
    pub intro_toast_shown: bool,
    #[serde(default)]
    pub lang: Lang,
    /// Profile file, old and new sensitivity of the last change made in Play.
    #[serde(default)]
    pub last_sensitivity_change: Option<(String, f64, f64)>,
}

impl Default for UiPrefs {
    fn default() -> Self {
        UiPrefs {
            version: 1,
            active: None,
            overlay: Overlay::Auto,
            sens_down_key: d_down(),
            sens_up_key: d_up(),
            notice_ack: 0,
            guide_done: false,
            intro_toast_shown: false,
            lang: Lang::Vi,
            last_sensitivity_change: None,
        }
    }
}

fn usable_key(k: &str) -> bool {
    is_known_key(k) && !is_mouse_button(k)
}

impl UiPrefs {
    /// Parse; anything unreadable gives `UiPrefs::default()`. The two
    /// sensitivity keys are reset to their defaults when either is not a
    /// known keyboard key or both are the same key.
    pub fn from_text(text: &str) -> UiPrefs {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let mut p = match serde_json::from_str::<UiPrefs>(text) {
            Ok(p) => p,
            Err(_) => return UiPrefs::default(),
        };
        if !usable_key(&p.sens_down_key)
            || !usable_key(&p.sens_up_key)
            || p.sens_down_key == p.sens_up_key
        {
            p.sens_down_key = d_down();
            p.sens_up_key = d_up();
        }
        if let Some((_, a, b)) = &p.last_sensitivity_change {
            if !(a.is_finite() && b.is_finite()) {
                p.last_sensitivity_change = None;
            }
        }
        p
    }

    pub fn to_text(&self) -> String {
        let mut s = serde_json::to_string_pretty(self).unwrap_or_default();
        s.push('\n');
        s
    }

    /// Read `path` through `fs`; a missing or broken file gives defaults.
    pub fn load<F: Fs>(fs: &F, path: &str) -> UiPrefs {
        match fs.read(path) {
            Ok(t) => UiPrefs::from_text(&t),
            Err(_) => UiPrefs::default(),
        }
    }

    /// Write `path` atomically.
    pub fn save<F: Fs>(&self, fs: &mut F, path: &str) -> Result<(), FsError> {
        fs.write_atomic(path, &self.to_text())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::store::{FsOp, MemFs};

    // test 69
    #[test]
    fn defaults() {
        let p = UiPrefs::default();
        assert_eq!(p.version, 1);
        assert_eq!(p.active, None);
        assert_eq!(p.overlay, Overlay::Auto);
        assert_eq!(p.sens_down_key, "BracketLeft");
        assert_eq!(p.sens_up_key, "BracketRight");
        assert_eq!(p.notice_ack, 0);
        assert!(!p.guide_done && !p.intro_toast_shown);
        assert_eq!(p.lang, Lang::Vi);
        assert_eq!(p.last_sensitivity_change, None);
        // an empty object is the defaults too
        assert_eq!(UiPrefs::from_text("{}"), p);
    }

    #[test]
    fn unknown_fields_tolerated_and_partial_files_fill_defaults() {
        let p = UiPrefs::from_text(
            r#"{"notice_ack":1,"overlay":"always","lang":"en","future_thing":{"a":[1,2]},"active":"ho-so-1.json"}"#,
        );
        assert_eq!(p.notice_ack, 1);
        assert_eq!(p.overlay, Overlay::Always);
        assert_eq!(p.lang, Lang::En);
        assert_eq!(p.active.as_deref(), Some("ho-so-1.json"));
        assert_eq!(p.sens_down_key, "BracketLeft");
    }

    #[test]
    fn garbage_gives_defaults() {
        for t in [
            "",
            "not json",
            "[]",
            "null",
            "{\"overlay\":\"loud\"}",
            "{\"notice_ack\":-3}",
            "{\"lang\":\"fr\"}",
            "{\"notice_ack\":",
        ] {
            assert_eq!(UiPrefs::from_text(t), UiPrefs::default(), "{t:?}");
        }
    }

    #[test]
    fn bad_sensitivity_keys_reset() {
        for (d, u) in [
            ("Nope", "BracketRight"),
            ("MouseLeft", "BracketRight"),
            ("KeyQ", "KeyQ"),
        ] {
            let p = UiPrefs::from_text(&format!(r#"{{"sens_down_key":"{d}","sens_up_key":"{u}"}}"#));
            assert_eq!(p.sens_down_key, "BracketLeft");
            assert_eq!(p.sens_up_key, "BracketRight");
        }
        let p = UiPrefs::from_text(r#"{"sens_down_key":"KeyN","sens_up_key":"KeyM"}"#);
        assert_eq!((p.sens_down_key.as_str(), p.sens_up_key.as_str()), ("KeyN", "KeyM"));
    }

    #[test]
    fn round_trip_and_fs() {
        let mut p = UiPrefs::default();
        p.active = Some("a.json".into());
        p.overlay = Overlay::Off;
        p.lang = Lang::En;
        p.last_sensitivity_change = Some(("a.json".into(), 1.0, 1.21));
        assert_eq!(UiPrefs::from_text(&p.to_text()), p);
        assert!(p.to_text().contains("\"lang\": \"en\""));
        let mut fs = MemFs::new();
        assert_eq!(UiPrefs::load(&fs, PREFS_FILE), UiPrefs::default());
        p.save(&mut fs, PREFS_FILE).unwrap();
        assert_eq!(UiPrefs::load(&fs, PREFS_FILE), p);
        fs.fail_next(FsOp::WriteAtomic);
        assert!(UiPrefs::default().save(&mut fs, PREFS_FILE).is_err());
        assert_eq!(UiPrefs::load(&fs, PREFS_FILE), p);
        fs.write_atomic(PREFS_FILE, "garbage").unwrap();
        assert_eq!(UiPrefs::load(&fs, PREFS_FILE).notice_ack, 0);
    }
}
