//! `keymap-ui.json`: small UI preferences (section 6.12, 7.3).
//!
//! Forward compatible (unknown fields are ignored). A file that cannot be
//! read or parsed gives the defaults, including `notice_ack = 0`, so the
//! first-run notice shows again; the file is rewritten on the next change.

use super::msg::Lang;
use super::store::{Fs, FsError};
use crate::keys::{is_known_key, is_mouse_button};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

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

fn sens_change_ok(c: &(String, f64, f64)) -> bool {
    let ok = |v: f64| v.is_finite() && v > 0.0 && v <= 100.0;
    ok(c.1) && ok(c.2)
}

impl UiPrefs {
    /// Parse; anything unreadable gives `UiPrefs::default()`, and a field
    /// that is missing or has an unknown value gives that field's default. The two
    /// sensitivity keys are reset to their defaults when either is not a
    /// known keyboard key or both are the same key.
    pub fn from_text(text: &str) -> UiPrefs {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let Ok(Value::Object(m)) = serde_json::from_str::<Value>(text) else {
            return UiPrefs::default();
        };
        // each field on its own: one value this build does not know (say a
        // newer overlay mode) must not reset the others
        fn get<T: DeserializeOwned>(m: &Map<String, Value>, k: &str) -> Option<T> {
            serde_json::from_value(m.get(k)?.clone()).ok()
        }
        let d = UiPrefs::default();
        let mut p = UiPrefs {
            version: get(&m, "version").unwrap_or(d.version),
            active: get(&m, "active").unwrap_or(d.active),
            overlay: get(&m, "overlay").unwrap_or(d.overlay),
            sens_down_key: get(&m, "sens_down_key").unwrap_or(d.sens_down_key),
            sens_up_key: get(&m, "sens_up_key").unwrap_or(d.sens_up_key),
            notice_ack: get(&m, "notice_ack").unwrap_or(d.notice_ack),
            guide_done: get(&m, "guide_done").unwrap_or(d.guide_done),
            intro_toast_shown: get(&m, "intro_toast_shown").unwrap_or(d.intro_toast_shown),
            lang: get(&m, "lang").unwrap_or(d.lang),
            last_sensitivity_change: get(&m, "last_sensitivity_change")
                .unwrap_or(d.last_sensitivity_change),
        };
        if !usable_key(&p.sens_down_key)
            || !usable_key(&p.sens_up_key)
            || p.sens_down_key == p.sens_up_key
        {
            p.sens_down_key = d_down();
            p.sens_up_key = d_up();
        }
        p.last_sensitivity_change = p.last_sensitivity_change.filter(sens_change_ok);
        p
    }

    /// Text for the file; a sensitivity change that `from_text` would not
    /// accept is left out so the file always reads back as written.
    pub fn to_text(&self) -> String {
        let mut p = self.clone();
        p.last_sensitivity_change = p.last_sensitivity_change.filter(sens_change_ok);
        let mut s = serde_json::to_string_pretty(&p).unwrap_or_default();
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
            let p =
                UiPrefs::from_text(&format!(r#"{{"sens_down_key":"{d}","sens_up_key":"{u}"}}"#));
            assert_eq!(p.sens_down_key, "BracketLeft");
            assert_eq!(p.sens_up_key, "BracketRight");
        }
        let p = UiPrefs::from_text(r#"{"sens_down_key":"KeyN","sens_up_key":"KeyM"}"#);
        assert_eq!(
            (p.sens_down_key.as_str(), p.sens_up_key.as_str()),
            ("KeyN", "KeyM")
        );
    }

    #[test]
    fn nonsense_sensitivity_change_is_dropped_and_never_written() {
        let nan = UiPrefs {
            notice_ack: 1,
            lang: Lang::En,
            active: Some("a.json".into()),
            last_sensitivity_change: Some(("a.json".into(), f64::NAN, 1.0)),
            ..UiPrefs::default()
        };
        let back = UiPrefs::from_text(&nan.to_text());
        assert_eq!(back.last_sensitivity_change, None);
        assert_eq!(
            (back.notice_ack, back.lang, back.active.as_deref()),
            (1, Lang::En, Some("a.json"))
        );
        for t in [
            r#"{"last_sensitivity_change":["a",-5,1]}"#,
            r#"{"last_sensitivity_change":["a",1,1e300]}"#,
        ] {
            assert_eq!(UiPrefs::from_text(t).last_sensitivity_change, None, "{t}");
        }
    }

    #[test]
    fn one_unknown_value_does_not_reset_the_other_fields() {
        let p = UiPrefs::from_text(
            r#"{"notice_ack":1,"lang":"en","overlay":"minimal","active":"a.json","guide_done":"yes"}"#,
        );
        assert_eq!(p.overlay, Overlay::Auto);
        assert_eq!(p.notice_ack, 1);
        assert_eq!(p.lang, Lang::En);
        assert_eq!(p.active.as_deref(), Some("a.json"));
        assert!(!p.guide_done);
    }

    #[test]
    fn round_trip_and_fs() {
        let p = UiPrefs {
            active: Some("a.json".into()),
            overlay: Overlay::Off,
            lang: Lang::En,
            last_sensitivity_change: Some(("a.json".into(), 1.0, 1.21)),
            ..UiPrefs::default()
        };
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
