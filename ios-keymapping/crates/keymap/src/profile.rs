//! JSON profile v1: parsing and validation.

use crate::geom::Rotation;
use crate::keys::{self, DEFAULT_PANIC_KEY, DEFAULT_TOGGLE_KEY};
use serde::{Deserialize, Serialize};
use std::fmt;

const MAX_CONTROLS: usize = 64;
const MAX_NAME_CHARS: usize = 100;
const MAX_LABEL_CHARS: usize = 40;
const MAX_RELEASE_MS: u32 = 60_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Orientation {
    Portrait,
    Landscape,
}

impl Orientation {
    /// Landscape <=> the rotation shows the picture wider than tall.
    pub fn matches(self, rotation: Rotation) -> bool {
        (self == Orientation::Landscape) == rotation.is_landscape()
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Orientation::Portrait => "portrait",
            Orientation::Landscape => "landscape",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ButtonMode {
    #[default]
    Hold,
    Tap,
}

fn default_sensitivity() -> f64 {
    1.0
}
fn default_reach() -> f64 {
    0.3
}
fn default_toggle() -> String {
    DEFAULT_TOGGLE_KEY.to_string()
}
fn default_panic() -> String {
    DEFAULT_PANIC_KEY.to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
pub enum Control {
    Joystick {
        #[serde(default)]
        label: String,
        x: f64,
        y: f64,
        radius: f64,
        up: String,
        down: String,
        left: String,
        right: String,
    },
    Look {
        #[serde(default)]
        label: String,
        x: f64,
        y: f64,
        #[serde(default = "default_sensitivity")]
        sensitivity: f64,
        #[serde(default)]
        invert_y: bool,
        #[serde(default = "default_reach")]
        reach: f64,
        #[serde(default)]
        release_after_ms: u32,
    },
    Button {
        #[serde(default)]
        label: String,
        x: f64,
        y: f64,
        key: String,
        #[serde(default)]
        mode: ButtonMode,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub version: u32,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub app: String,
    pub orientation: Orientation,
    #[serde(default = "default_toggle")]
    pub toggle_key: String,
    #[serde(default = "default_panic")]
    pub panic_key: String,
    #[serde(default)]
    pub controls: Vec<Control>,
}

/// One validation finding. `path` is e.g. "version", "panic_key",
/// "controls[2].radius", "controls[3].up", "controls".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileError {
    /// Not JSON / wrong shape; the serde_json message (with line and column).
    Json(String),
    /// Well-formed but breaks a rule.
    Invalid(Problem),
}

impl fmt::Display for ProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProfileError::Json(m) => write!(f, "invalid JSON: {m}"),
            ProfileError::Invalid(p) => write!(f, "{}: {}", p.path, p.message),
        }
    }
}

impl std::error::Error for ProfileError {}

fn problem(path: impl Into<String>, message: impl Into<String>) -> Problem {
    Problem {
        path: path.into(),
        message: message.into(),
    }
}

fn check_len(out: &mut Vec<Problem>, path: &str, s: &str, max: usize) {
    let n = s.chars().count();
    if n > max {
        out.push(problem(
            path,
            format!("must be at most {max} characters, got {n}"),
        ));
    }
}

fn check_unit(out: &mut Vec<Problem>, path: String, v: f64) {
    if !(v.is_finite() && (0.0..=1.0).contains(&v)) {
        out.push(problem(path, format!("must be in [0, 1], got {v}")));
    }
}

fn check_range(out: &mut Vec<Problem>, path: String, v: f64, max: f64) {
    if !(v.is_finite() && v > 0.0 && v <= max) {
        out.push(problem(path, format!("must be in (0, {max}], got {v}")));
    }
}

fn check_key(out: &mut Vec<Problem>, path: String, key: &str) {
    if !keys::is_known_key(key) {
        out.push(problem(path, format!("unknown key \"{key}\"")));
    }
}

fn check_kbd_key(out: &mut Vec<Problem>, path: &str, key: &str) {
    if !keys::is_known_key(key) {
        out.push(problem(path, format!("unknown key \"{key}\"")));
    } else if keys::is_mouse_button(key) {
        out.push(problem(
            path,
            format!("must be a keyboard key, got \"{key}\""),
        ));
    }
}

impl Profile {
    /// Parse and validate. A leading UTF-8 BOM is accepted.
    pub fn from_json(text: &str) -> Result<Profile, ProfileError> {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let p: Profile =
            serde_json::from_str(text).map_err(|e| ProfileError::Json(e.to_string()))?;
        p.validate()?;
        Ok(p)
    }

    /// Pretty JSON, 2-space indent, trailing newline.
    pub fn to_json_pretty(&self) -> String {
        let mut s = serde_json::to_string_pretty(self).unwrap_or_default();
        s.push('\n');
        s
    }

    /// First finding of [`problems`](Self::problems).
    pub fn validate(&self) -> Result<(), ProfileError> {
        match self.problems().into_iter().next() {
            Some(p) => Err(ProfileError::Invalid(p)),
            None => Ok(()),
        }
    }

    pub fn has_look(&self) -> bool {
        self.controls
            .iter()
            .any(|c| matches!(c, Control::Look { .. }))
    }

    /// Every bound key with its path: toggle, panic, then controls in order.
    pub fn bound_keys(&self) -> Vec<(String, &str)> {
        let mut v: Vec<(String, &str)> = vec![
            ("toggle_key".to_string(), self.toggle_key.as_str()),
            ("panic_key".to_string(), self.panic_key.as_str()),
        ];
        for (i, c) in self.controls.iter().enumerate() {
            match c {
                Control::Joystick {
                    up,
                    down,
                    left,
                    right,
                    ..
                } => {
                    v.push((format!("controls[{i}].up"), up.as_str()));
                    v.push((format!("controls[{i}].down"), down.as_str()));
                    v.push((format!("controls[{i}].left"), left.as_str()));
                    v.push((format!("controls[{i}].right"), right.as_str()));
                }
                Control::Look { .. } => {}
                Control::Button { key, .. } => v.push((format!("controls[{i}].key"), key.as_str())),
            }
        }
        v
    }

    /// All findings in the order of the spec.
    pub fn problems(&self) -> Vec<Problem> {
        let mut out = Vec::new();
        if self.version != 1 {
            out.push(problem(
                "version",
                format!("must be 1, got {}", self.version),
            ));
        }
        check_len(&mut out, "name", &self.name, MAX_NAME_CHARS);
        check_len(&mut out, "app", &self.app, MAX_NAME_CHARS);
        check_kbd_key(&mut out, "toggle_key", &self.toggle_key);
        check_kbd_key(&mut out, "panic_key", &self.panic_key);
        if self.controls.len() > MAX_CONTROLS {
            out.push(problem(
                "controls",
                format!(
                    "at most {MAX_CONTROLS} controls are allowed, got {}",
                    self.controls.len()
                ),
            ));
        }
        let mut first_look: Option<usize> = None;
        for (i, c) in self.controls.iter().enumerate() {
            let p = |f: &str| format!("controls[{i}].{f}");
            match c {
                Control::Joystick {
                    label,
                    x,
                    y,
                    radius,
                    up,
                    down,
                    left,
                    right,
                } => {
                    check_len(&mut out, &p("label"), label, MAX_LABEL_CHARS);
                    check_unit(&mut out, p("x"), *x);
                    check_unit(&mut out, p("y"), *y);
                    check_range(&mut out, p("radius"), *radius, 0.5);
                    check_key(&mut out, p("up"), up);
                    check_key(&mut out, p("down"), down);
                    check_key(&mut out, p("left"), left);
                    check_key(&mut out, p("right"), right);
                }
                Control::Look {
                    label,
                    x,
                    y,
                    sensitivity,
                    reach,
                    release_after_ms,
                    ..
                } => {
                    check_len(&mut out, &p("label"), label, MAX_LABEL_CHARS);
                    check_unit(&mut out, p("x"), *x);
                    check_unit(&mut out, p("y"), *y);
                    check_range(&mut out, p("sensitivity"), *sensitivity, 100.0);
                    check_range(&mut out, p("reach"), *reach, 1.0);
                    if *release_after_ms > MAX_RELEASE_MS {
                        out.push(problem(
                            p("release_after_ms"),
                            format!("must be in 0..={MAX_RELEASE_MS}, got {release_after_ms}"),
                        ));
                    }
                    if first_look.is_some() {
                        out.push(problem(
                            format!("controls[{i}]"),
                            "only one look control is allowed",
                        ));
                    } else {
                        first_look = Some(i);
                    }
                }
                Control::Button {
                    label, x, y, key, ..
                } => {
                    check_len(&mut out, &p("label"), label, MAX_LABEL_CHARS);
                    check_unit(&mut out, p("x"), *x);
                    check_unit(&mut out, p("y"), *y);
                    check_key(&mut out, p("key"), key);
                }
            }
        }
        let mut seen: Vec<(String, &str)> = Vec::new();
        for (path, key) in self.bound_keys() {
            if !keys::is_known_key(key) {
                continue;
            }
            if let Some((first, _)) = seen.iter().find(|(_, k)| *k == key) {
                out.push(problem(
                    path,
                    format!("key \"{key}\" is already used by {first}"),
                ));
            } else {
                seen.push((path, key));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn first_path(json: &str) -> String {
        match Profile::from_json(json) {
            Err(ProfileError::Invalid(p)) => p.path,
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    fn with_controls(controls: &str) -> String {
        format!(r#"{{"version":1,"orientation":"portrait","controls":[{controls}]}}"#)
    }

    const JOY: &str = r#"{"type":"joystick","x":0.2,"y":0.7,"radius":0.1,"up":"KeyW","down":"KeyS","left":"KeyA","right":"KeyD"}"#;

    #[test]
    fn minimal_profile_gets_defaults() {
        let p = Profile::from_json(r#"{"version":1,"orientation":"landscape"}"#).unwrap();
        assert_eq!(p.name, "");
        assert_eq!(p.app, "");
        assert_eq!(p.toggle_key, "Backquote");
        assert_eq!(p.panic_key, "F8");
        assert!(p.controls.is_empty());
        assert_eq!(p.orientation, Orientation::Landscape);
        assert!(!p.has_look());
    }

    #[test]
    fn control_defaults() {
        let p = Profile::from_json(&with_controls(
            r#"{"type":"look","x":0.5,"y":0.5},{"type":"button","x":0.9,"y":0.9,"key":"Space"}"#,
        ))
        .unwrap();
        match &p.controls[0] {
            Control::Look {
                label,
                sensitivity,
                invert_y,
                reach,
                release_after_ms,
                ..
            } => {
                assert_eq!(label, "");
                assert_eq!(*sensitivity, 1.0);
                assert!(!*invert_y);
                assert_eq!(*reach, 0.3);
                assert_eq!(*release_after_ms, 0);
            }
            c => panic!("{c:?}"),
        }
        match &p.controls[1] {
            Control::Button { mode, .. } => assert_eq!(*mode, ButtonMode::Hold),
            c => panic!("{c:?}"),
        }
        assert!(p.has_look());
    }

    #[test]
    fn round_trip() {
        let src = format!(
            r#"{{"version":1,"name":"T","app":"a","orientation":"landscape","toggle_key":"F2","panic_key":"F3",
            "controls":[{JOY},{{"type":"look","label":"Cam","x":0.7,"y":0.5,"sensitivity":2.5,"invert_y":true,"reach":0.25,"release_after_ms":300}},
            {{"type":"button","label":"A","x":0.8,"y":0.8,"key":"MouseLeft","mode":"tap"}}]}}"#
        );
        let p = Profile::from_json(&src).unwrap();
        let text = p.to_json_pretty();
        assert!(text.ends_with("}\n"));
        assert!(text.contains("\n  \"version\": 1"));
        assert_eq!(Profile::from_json(&text).unwrap(), p);
    }

    #[test]
    fn bom_is_accepted() {
        let src = "\u{feff}{\"version\":1,\"orientation\":\"portrait\"}";
        assert!(Profile::from_json(src).is_ok());
    }

    #[test]
    fn wrong_version() {
        assert_eq!(
            first_path(r#"{"version":2,"orientation":"portrait"}"#),
            "version"
        );
        assert_eq!(
            first_path(r#"{"version":0,"orientation":"portrait"}"#),
            "version"
        );
    }

    #[test]
    fn name_and_label_length() {
        let long = "x".repeat(101);
        let j = format!(r#"{{"version":1,"name":"{long}","orientation":"portrait"}}"#);
        assert_eq!(first_path(&j), "name");
        let j = format!(r#"{{"version":1,"app":"{long}","orientation":"portrait"}}"#);
        assert_eq!(first_path(&j), "app");
        let l = "y".repeat(41);
        let c = format!(r#"{{"type":"button","label":"{l}","x":0.5,"y":0.5,"key":"Space"}}"#);
        assert_eq!(first_path(&with_controls(&c)), "controls[0].label");
        // Counted in characters, not bytes.
        let ok = "é".repeat(40);
        let c = format!(r#"{{"type":"button","label":"{ok}","x":0.5,"y":0.5,"key":"Space"}}"#);
        assert!(Profile::from_json(&with_controls(&c)).is_ok());
    }

    #[test]
    fn coordinates_bounds() {
        let b = |x: &str, y: &str| {
            with_controls(&format!(
                r#"{{"type":"button","x":{x},"y":{y},"key":"Space"}}"#
            ))
        };
        assert!(Profile::from_json(&b("0", "1")).is_ok());
        assert!(Profile::from_json(&b("1.0", "0.0")).is_ok());
        assert_eq!(first_path(&b("1.01", "0.5")), "controls[0].x");
        assert_eq!(first_path(&b("0.5", "-0.01")), "controls[0].y");
    }

    #[test]
    fn non_finite_coordinates_are_rejected_for_built_profiles() {
        let mut p = Profile::from_json(&with_controls(
            r#"{"type":"button","x":0.5,"y":0.5,"key":"Space"}"#,
        ))
        .unwrap();
        if let Control::Button { x, .. } = &mut p.controls[0] {
            *x = f64::NAN;
        }
        assert_eq!(
            p.validate()
                .unwrap_err()
                .to_string()
                .split(':')
                .next()
                .unwrap(),
            "controls[0].x"
        );
    }

    #[test]
    fn radius_bounds() {
        let j = |r: &str| {
            with_controls(&format!(
                r#"{{"type":"joystick","x":0.2,"y":0.7,"radius":{r},"up":"KeyW","down":"KeyS","left":"KeyA","right":"KeyD"}}"#
            ))
        };
        assert_eq!(first_path(&j("0")), "controls[0].radius");
        assert!(Profile::from_json(&j("0.5")).is_ok());
        assert_eq!(first_path(&j("0.7")), "controls[0].radius");
        match Profile::from_json(&j("0.7")) {
            Err(ProfileError::Invalid(p)) => assert_eq!(p.message, "must be in (0, 0.5], got 0.7"),
            o => panic!("{o:?}"),
        }
    }

    #[test]
    fn look_numeric_bounds() {
        let l =
            |extra: &str| with_controls(&format!(r#"{{"type":"look","x":0.5,"y":0.5,{extra}}}"#));
        assert_eq!(first_path(&l(r#""reach":0"#)), "controls[0].reach");
        assert_eq!(first_path(&l(r#""reach":1.5"#)), "controls[0].reach");
        assert!(Profile::from_json(&l(r#""reach":1"#)).is_ok());
        assert_eq!(
            first_path(&l(r#""sensitivity":0"#)),
            "controls[0].sensitivity"
        );
        assert_eq!(
            first_path(&l(r#""sensitivity":100.5"#)),
            "controls[0].sensitivity"
        );
        assert!(Profile::from_json(&l(r#""sensitivity":100"#)).is_ok());
        assert_eq!(
            first_path(&l(r#""release_after_ms":60001"#)),
            "controls[0].release_after_ms"
        );
        assert!(Profile::from_json(&l(r#""release_after_ms":60000"#)).is_ok());
    }

    #[test]
    fn unknown_keys_and_mouse_toggle() {
        let b = |k: &str| {
            with_controls(&format!(
                r#"{{"type":"button","x":0.5,"y":0.5,"key":"{k}"}}"#
            ))
        };
        assert_eq!(first_path(&b("Foo")), "controls[0].key");
        assert_eq!(first_path(&b("keya")), "controls[0].key");
        assert!(Profile::from_json(&b("MouseBack")).is_ok());
        assert_eq!(
            first_path(r#"{"version":1,"orientation":"portrait","toggle_key":"MouseLeft"}"#),
            "toggle_key"
        );
        assert_eq!(
            first_path(r#"{"version":1,"orientation":"portrait","panic_key":"MouseRight"}"#),
            "panic_key"
        );
        assert_eq!(
            first_path(r#"{"version":1,"orientation":"portrait","toggle_key":"Nope"}"#),
            "toggle_key"
        );
    }

    #[test]
    fn toggle_equals_panic() {
        assert_eq!(
            first_path(
                r#"{"version":1,"orientation":"portrait","toggle_key":"F2","panic_key":"F2"}"#
            ),
            "panic_key"
        );
    }

    #[test]
    fn duplicate_keys() {
        let two = with_controls(
            r#"{"type":"button","x":0.5,"y":0.5,"key":"Space"},{"type":"button","x":0.6,"y":0.5,"key":"Space"}"#,
        );
        assert_eq!(first_path(&two), "controls[1].key");
        match Profile::from_json(&two) {
            Err(ProfileError::Invalid(p)) => {
                assert_eq!(
                    p.message,
                    "key \"Space\" is already used by controls[0].key"
                )
            }
            o => panic!("{o:?}"),
        }
        let with_toggle = with_controls(r#"{"type":"button","x":0.5,"y":0.5,"key":"Backquote"}"#);
        assert_eq!(first_path(&with_toggle), "controls[0].key");
        let in_joy = with_controls(&format!(
            "{JOY},{}",
            r#"{"type":"button","x":0.5,"y":0.5,"key":"KeyA"}"#
        ));
        assert_eq!(first_path(&in_joy), "controls[1].key");
        // Same key inside one joystick.
        let joy_dup = with_controls(
            r#"{"type":"joystick","x":0.2,"y":0.7,"radius":0.1,"up":"KeyW","down":"KeyW","left":"KeyA","right":"KeyD"}"#,
        );
        assert_eq!(first_path(&joy_dup), "controls[0].down");
    }

    #[test]
    fn second_look_rejected() {
        let two =
            with_controls(r#"{"type":"look","x":0.5,"y":0.5},{"type":"look","x":0.6,"y":0.5}"#);
        assert_eq!(first_path(&two), "controls[1]");
    }

    #[test]
    fn unknown_field_type_and_bad_json() {
        for j in [
            r#"{"version":1,"orientation":"portrait","extra":1}"#,
            r#"{"version":1,"orientation":"portrait","controls":[{"type":"button","x":0.5,"y":0.5,"key":"Space","foo":1}]}"#,
            r#"{"version":1,"orientation":"portrait","controls":[{"type":"slider","x":0.5,"y":0.5}]}"#,
            r#"{"version":1}"#,
            r#"{"version":1,"orientation":"sideways"}"#,
            r#"{"version":1,"orientation":"portrait","controls":[{"type":"button","x":0.5,"y":0.5}]}"#,
            "not json",
            "",
        ] {
            assert!(
                matches!(Profile::from_json(j), Err(ProfileError::Json(_))),
                "{j}"
            );
        }
    }

    #[test]
    fn control_count() {
        let one = r#"{"type":"button","x":0.5,"y":0.5,"key":"Space"}"#;
        let many = vec![one; 65].join(",");
        assert_eq!(first_path(&with_controls(&many)), "controls");
        // 6+ distinct controls are valid.
        let keys = ["KeyJ", "KeyK", "KeyL", "KeyU", "KeyI", "KeyO", "KeyP"];
        let c: Vec<String> = keys
            .iter()
            .map(|k| format!(r#"{{"type":"button","x":0.5,"y":0.5,"key":"{k}"}}"#))
            .collect();
        let p = Profile::from_json(&with_controls(&c.join(","))).unwrap();
        assert_eq!(p.controls.len(), 7);
    }

    #[test]
    fn problems_come_in_spec_order() {
        let j = r#"{"version":3,"orientation":"portrait","toggle_key":"Zzz","panic_key":"F8",
            "controls":[{"type":"look","x":2,"y":0.5,"reach":0},{"type":"look","x":0.5,"y":0.5},
            {"type":"button","x":0.5,"y":0.5,"key":"F8"}]}"#;
        let p: Profile = serde_json::from_str(j).unwrap();
        let paths: Vec<String> = p.problems().into_iter().map(|p| p.path).collect();
        assert_eq!(
            paths,
            [
                "version",
                "toggle_key",
                "controls[0].x",
                "controls[0].reach",
                "controls[1]",
                "controls[2].key"
            ]
        );
    }

    #[test]
    fn bound_keys_order() {
        let p = Profile::from_json(&with_controls(&format!(
            "{JOY},{}",
            r#"{"type":"button","x":0.5,"y":0.5,"key":"Space"}"#
        )))
        .unwrap();
        let paths: Vec<String> = p.bound_keys().into_iter().map(|(p, _)| p).collect();
        assert_eq!(
            paths,
            [
                "toggle_key",
                "panic_key",
                "controls[0].up",
                "controls[0].down",
                "controls[0].left",
                "controls[0].right",
                "controls[1].key"
            ]
        );
    }

    #[test]
    fn orientation_matches_rotation() {
        assert!(Orientation::Portrait.matches(Rotation::R0));
        assert!(Orientation::Portrait.matches(Rotation::R180));
        assert!(!Orientation::Portrait.matches(Rotation::R90));
        assert!(Orientation::Landscape.matches(Rotation::R270));
        assert!(!Orientation::Landscape.matches(Rotation::R0));
    }
}
