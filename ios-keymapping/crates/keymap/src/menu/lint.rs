//! Warnings that never block saving or playing, and plain-language
//! explanations of validation problems (UI_SPEC 6.9).

use super::doc::{EditDoc, Kind};
use super::hit::{ANCHOR_RADIUS_LP, EditView, button_hit_radius};
use super::keynames::key_advice;
use super::msg::{Advice, Lang, Msg, ProblemMsg};
use super::snap::in_edge_band;
use super::{CtrlId, Pt};
use crate::geom::Rotation;
use crate::profile::{Control, Problem, Profile, ProfileError};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Link {
    None,
    Usb,
    Wifi,
}

/// Facts only the app knows, passed with every event (UI_SPEC 6.2). Defined
/// here because `lints` needs it; the shell re-exports it.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Env {
    pub link: Link,
    pub display_mode: bool,
    pub scroll_busy: bool,
    pub rotation: Rotation,
    pub panel: (f64, f64),
    pub has_picture: bool,
    pub view: EditView,
    pub can_grow_window: bool,
}

/// One piece of advice, about a control or (target `None`) the whole profile.
#[derive(Clone, PartialEq, Debug)]
pub struct Lint {
    pub target: Option<CtrlId>,
    pub advice: Advice,
}

fn centre(c: &Control) -> Pt {
    match c {
        Control::Joystick { x, y, .. }
        | Control::Look { x, y, .. }
        | Control::Button { x, y, .. } => Pt { x: *x, y: *y },
    }
}

/// Every key a control uses, in a fixed order.
fn keys_of(c: &Control) -> Vec<&str> {
    match c {
        Control::Joystick {
            up,
            down,
            left,
            right,
            ..
        } => vec![up, down, left, right],
        Control::Button { key, .. } => vec![key],
        Control::Look { .. } => Vec::new(),
    }
    .into_iter()
    .map(String::as_str)
    .collect()
}

/// Warnings for the whole document. Profile-wide ones come first, then per
/// control in array order. Advice for a key appears once per key.
pub fn lints(doc: &EditDoc, env: &Env) -> Vec<Lint> {
    let p = doc.profile();
    let mut out = Vec::new();
    if env.has_picture && !p.orientation.matches(env.rotation) {
        out.push(Lint {
            target: None,
            advice: Advice::OrientationMismatch {
                profile: p.orientation,
            },
        });
    }
    if !p.has_look() {
        out.push(Lint {
            target: None,
            advice: Advice::NoLook,
        });
    }
    if p.controls.len() > 5 {
        out.push(Lint {
            target: None,
            advice: Advice::MoreThanFive {
                count: p.controls.len(),
            },
        });
    }
    for k in [&p.toggle_key, &p.panic_key] {
        if let Some(advice) = key_advice(k) {
            out.push(Lint {
                target: None,
                advice,
            });
        }
    }
    let anchor = p.controls.iter().zip(doc.ids()).find_map(|(c, id)| {
        matches!(c, Control::Look { .. }).then(|| (*id, env.view.to_px(centre(c))))
    });
    for (c, id) in p.controls.iter().zip(doc.ids()) {
        if in_edge_band(centre(c)) {
            out.push(Lint {
                target: Some(*id),
                advice: Advice::NearEdge,
            });
        }
        for k in keys_of(c) {
            if let Some(advice) = key_advice(k) {
                out.push(Lint {
                    target: Some(*id),
                    advice,
                });
            }
        }
        if let Some((look_id, a)) = anchor
            && look_id != *id
        {
            let o = env.view.to_px(centre(c));
            let d = ((o.x - a.x).powi(2) + (o.y - a.y).powi(2)).sqrt();
            let reach = match c {
                Control::Button { .. } => button_hit_radius(&env.view),
                Control::Joystick { radius, .. } => radius * env.view.short_px(),
                Control::Look { .. } => 0.0,
            };
            if d <= env.view.lp(ANCHOR_RADIUS_LP) + reach {
                out.push(Lint {
                    target: Some(*id),
                    advice: Advice::OverlapsLookAnchor,
                });
            }
        }
    }
    out
}

const MAX_NAME: usize = 100;
const MAX_LABEL: usize = 40;

/// How many characters of an over-long text are quoted back to the user.
const QUOTE_CHARS: usize = 20;

/// `controls[3].radius` -> (3, "radius"); `controls[3]` -> (3, "").
fn parse_control_path(path: &str) -> Option<(usize, &str)> {
    let rest = path.strip_prefix("controls[")?;
    let (n, tail) = rest.split_once(']')?;
    let i = n.parse().ok()?;
    Some((i, tail.strip_prefix('.').unwrap_or(tail)))
}

/// Name of control `i`: its label, else kind and number (as `EditDoc`).
pub(crate) fn control_name(p: &Profile, i: usize, lang: Lang) -> String {
    let Some(c) = p.controls.get(i) else {
        return String::new();
    };
    let (label, kind) = match c {
        Control::Joystick { label, .. } => (label, Kind::Joystick),
        Control::Look { label, .. } => (label, Kind::Look),
        Control::Button { label, .. } => (label, Kind::Button),
    };
    if !label.trim().is_empty() {
        return label.clone();
    }
    let of_kind = |c: &Control| {
        matches!(
            (c, kind),
            (Control::Joystick { .. }, Kind::Joystick)
                | (Control::Look { .. }, Kind::Look)
                | (Control::Button { .. }, Kind::Button)
        )
    };
    let total = p.controls.iter().filter(|c| of_kind(c)).count();
    let pos = p.controls[..i].iter().filter(|c| of_kind(c)).count() as u32 + 1;
    let n = match kind {
        Kind::Button => pos,
        Kind::Joystick if total > 1 => pos,
        _ => 0,
    };
    Msg::ControlName { kind, n }.text(lang)
}

/// Display name of whoever a profile path points at (a key slot or control).
fn owner_name(p: &Profile, path: &str, lang: Lang) -> String {
    match path {
        "toggle_key" => Msg::SlotToggle.text(lang),
        "panic_key" => Msg::SlotPanic.text(lang),
        _ => match parse_control_path(path) {
            Some((i, _)) => control_name(p, i, lang),
            None => path.to_string(),
        },
    }
}

fn quote(s: &str) -> String {
    let mut t: String = s.chars().take(QUOTE_CHARS).collect();
    if s.chars().count() > QUOTE_CHARS {
        t.push('…');
    }
    t
}

/// The key named in a problem message (`unknown key "X"`), else the key the
/// profile holds at `path`.
fn key_at(p: &Profile, path: &str) -> String {
    match path {
        "toggle_key" => return p.toggle_key.clone(),
        "panic_key" => return p.panic_key.clone(),
        _ => {}
    }
    let Some((i, field)) = parse_control_path(path) else {
        return String::new();
    };
    match p.controls.get(i) {
        Some(Control::Button { key, .. }) if field == "key" => key.clone(),
        Some(Control::Joystick {
            up,
            down,
            left,
            right,
            ..
        }) => match field {
            "up" => up.clone(),
            "down" => down.clone(),
            "left" => left.clone(),
            "right" => right.clone(),
            _ => String::new(),
        },
        _ => String::new(),
    }
}

/// Friendly sentence for a SPEC 4.3 finding, naming unlabeled controls in
/// Vietnamese. See [`explain_in`].
pub fn explain(problem: &Problem, profile: &Profile) -> ProblemMsg {
    explain_in(problem, profile, Lang::Vi)
}

/// Like [`explain`], with the default names of unlabeled controls (`Nút 3`)
/// written in `lang`.
pub fn explain_in(problem: &Problem, profile: &Profile, lang: Lang) -> ProblemMsg {
    // Labels and keys come from a possibly hand-edited file: every user
    // string is cut to the quoting budget (6.10).
    match explain_raw(problem, profile, lang) {
        ProblemMsg::NameTooLong { text, max } => ProblemMsg::NameTooLong {
            text: quote(&text),
            max,
        },
        ProblemMsg::UnknownKey { key, label } => ProblemMsg::UnknownKey {
            key: quote(&key),
            label: quote(&label),
        },
        ProblemMsg::DuplicateKey { key, a, b } => ProblemMsg::DuplicateKey {
            key: quote(&key),
            a: quote(&a),
            b: quote(&b),
        },
        ProblemMsg::OutOfScreen { label } => ProblemMsg::OutOfScreen {
            label: quote(&label),
        },
        ProblemMsg::RadiusRange { label } => ProblemMsg::RadiusRange {
            label: quote(&label),
        },
        other => other,
    }
}

fn explain_raw(problem: &Problem, profile: &Profile, lang: Lang) -> ProblemMsg {
    let path = problem.path.as_str();
    let msg = problem.message.as_str();
    match path {
        "version" => {
            if profile.version > 1 {
                ProblemMsg::NewerVersion
            } else {
                ProblemMsg::BadVersion
            }
        }
        "name" => ProblemMsg::NameTooLong {
            text: quote(&profile.name),
            max: MAX_NAME,
        },
        "app" => ProblemMsg::NameTooLong {
            text: quote(&profile.app),
            max: MAX_NAME,
        },
        "toggle_key" | "panic_key" => {
            if msg.starts_with("must be a keyboard key") {
                ProblemMsg::MouseOnToggle
            } else if msg.contains("is already used by") {
                ProblemMsg::TogglePanicSame
            } else {
                ProblemMsg::UnknownKey {
                    key: key_at(profile, path),
                    label: owner_name(profile, path, lang),
                }
            }
        }
        "controls" => ProblemMsg::TooManyControls { max: 64 },
        _ => {
            let Some((i, field)) = parse_control_path(path) else {
                return ProblemMsg::NotUnderstood;
            };
            let label = control_name(profile, i, lang);
            if msg.contains("is already used by") {
                // "key "X" is already used by controls[1].key"
                let first = msg.rsplit_once("used by ").map_or("", |(_, f)| f);
                return ProblemMsg::DuplicateKey {
                    key: key_at(profile, path),
                    a: owner_name(profile, first, lang),
                    b: label,
                };
            }
            match field {
                "" => ProblemMsg::SecondLook,
                "x" | "y" => ProblemMsg::OutOfScreen { label },
                "radius" => ProblemMsg::RadiusRange { label },
                "reach" => ProblemMsg::ReachRange,
                "sensitivity" => ProblemMsg::SensitivityRange,
                "release_after_ms" => ProblemMsg::ReleaseRange,
                "label" => ProblemMsg::NameTooLong {
                    text: match profile.controls.get(i) {
                        Some(
                            Control::Joystick { label, .. }
                            | Control::Look { label, .. }
                            | Control::Button { label, .. },
                        ) => quote(label),
                        None => String::new(),
                    },
                    max: MAX_LABEL,
                },
                "key" | "up" | "down" | "left" | "right" => ProblemMsg::UnknownKey {
                    key: key_at(profile, path),
                    label,
                },
                _ => ProblemMsg::NotUnderstood,
            }
        }
    }
}

/// Friendly sentence for a file that did not parse. Unknown fields, types,
/// orientations and missing fields mean "made by another tool"; anything else
/// is not a profile at all.
pub fn explain_json_error(e: &ProfileError) -> ProblemMsg {
    match e {
        ProfileError::Json(m)
            if m.starts_with("unknown field")
                || m.starts_with("unknown variant")
                || m.starts_with("missing field")
                || m.starts_with("invalid type")
                || m.starts_with("invalid value") =>
        {
            ProblemMsg::NotUnderstood
        }
        ProfileError::Json(_) => ProblemMsg::NotReadable,
        // A validation finding needs its profile: use `explain`.
        ProfileError::Invalid(_) => ProblemMsg::NotUnderstood,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::hit::testing::{doc, pt, view};
    use crate::menu::msg::MirrorlyAction;
    use crate::profile::Orientation;
    use std::path::PathBuf;

    fn env() -> Env {
        Env {
            link: Link::Usb,
            display_mode: false,
            scroll_busy: false,
            rotation: Rotation::R90,
            panel: (828.0, 1792.0),
            has_picture: true,
            view: view(),
            can_grow_window: false,
        }
    }

    fn data(sub: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data")
            .join(sub)
    }

    #[test]
    fn user_text_in_sentences_is_cut() {
        let long = "x".repeat(50);
        let mut p =
            crate::menu::templates::template(crate::menu::Template::Empty(Orientation::Landscape));
        p.controls.push(Control::Button {
            label: long.clone(),
            x: 2.0,
            y: 0.5,
            key: "Nope".into(),
            mode: Default::default(),
        });
        for path in ["controls[0].x", "controls[0].key"] {
            let problem = Problem {
                path: path.into(),
                message: "bad".into(),
            };
            for lang in [Lang::Vi, Lang::En] {
                let t = explain_in(&problem, &p, lang).text(lang);
                assert!(!t.contains(&long), "{t}");
                assert!(t.contains('…'), "{t}");
            }
        }
    }

    // test 49
    #[test]
    fn explain_every_invalid_file() {
        let mut n = 0;
        for entry in std::fs::read_dir(data("invalid")).unwrap() {
            let path = entry.unwrap().path();
            if path.file_name().is_some_and(|f| f == "expected.json")
                || path.extension().is_none_or(|x| x != "json")
            {
                continue;
            }
            n += 1;
            let text = std::fs::read_to_string(&path).unwrap();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            let msg = match Profile::from_json(&text) {
                Ok(_) => panic!("{name} should be invalid"),
                Err(e @ ProfileError::Json(_)) => explain_json_error(&e),
                Err(ProfileError::Invalid(p)) => {
                    let profile: Profile = serde_json::from_str(&text).unwrap();
                    for lang in [Lang::Vi, Lang::En] {
                        assert!(!explain_in(&p, &profile, lang).text(lang).is_empty());
                    }
                    explain(&p, &profile)
                }
            };
            for lang in [Lang::Vi, Lang::En] {
                let t = msg.text(lang);
                assert!(!t.is_empty(), "{name}");
                assert!(!t.contains("{"), "{name}: {t}");
            }
            // "Not understood" is only right for files serde rejected.
            let expected: serde_json::Value = serde_json::from_str(
                &std::fs::read_to_string(data("invalid/expected.json")).unwrap(),
            )
            .unwrap();
            if expected[&name] != "<json>" {
                assert_ne!(msg, ProblemMsg::NotUnderstood, "{name}: {msg:?}");
            }
        }
        assert!(n >= 20, "found only {n} invalid files");
    }

    fn explain_file(name: &str) -> ProblemMsg {
        let text = std::fs::read_to_string(data("invalid").join(name)).unwrap();
        match Profile::from_json(&text) {
            Err(e @ ProfileError::Json(_)) => explain_json_error(&e),
            Err(ProfileError::Invalid(p)) => explain(&p, &serde_json::from_str(&text).unwrap()),
            Ok(_) => panic!("valid"),
        }
    }

    #[test]
    fn explain_picks_the_matching_sentence() {
        assert_eq!(explain_file("bad-version.json"), ProblemMsg::NewerVersion);
        assert_eq!(explain_file("mouse-toggle.json"), ProblemMsg::MouseOnToggle);
        assert_eq!(
            explain_file("toggle-equals-panic.json"),
            ProblemMsg::TogglePanicSame
        );
        assert_eq!(explain_file("second-look.json"), ProblemMsg::SecondLook);
        assert_eq!(
            explain_file("too-many-controls.json"),
            ProblemMsg::TooManyControls { max: 64 }
        );
        assert_eq!(explain_file("reach-zero.json"), ProblemMsg::ReachRange);
        assert_eq!(
            explain_file("sensitivity-zero.json"),
            ProblemMsg::SensitivityRange
        );
        assert_eq!(
            explain_file("release-too-long.json"),
            ProblemMsg::ReleaseRange
        );
        assert_eq!(explain_file("not-json.json"), ProblemMsg::NotReadable);
        assert_eq!(explain_file("array-root.json"), ProblemMsg::NotReadable);
        assert_eq!(
            explain_file("unknown-field.json"),
            ProblemMsg::NotUnderstood
        );
        assert_eq!(explain_file("unknown-type.json"), ProblemMsg::NotUnderstood);
        assert_eq!(
            explain_file("bad-orientation.json"),
            ProblemMsg::NotUnderstood
        );
        assert!(matches!(
            explain_file("name-too-long.json"),
            ProblemMsg::NameTooLong { max: 100, .. }
        ));
        assert!(matches!(
            explain_file("unknown-key.json"),
            ProblemMsg::UnknownKey { .. }
        ));
        assert!(matches!(
            explain_file("radius-zero.json"),
            ProblemMsg::RadiusRange { .. }
        ));
        assert!(matches!(
            explain_file("x-out-of-range.json"),
            ProblemMsg::OutOfScreen { .. }
        ));
        match explain_file("duplicate-keys.json") {
            ProblemMsg::DuplicateKey { key, a, b } => {
                assert!(!key.is_empty() && !a.is_empty() && !b.is_empty());
                assert_ne!(a, b);
            }
            other => panic!("{other:?}"),
        }
        // Unlabeled controls are named by kind and number.
        match explain_file("x-out-of-range.json") {
            ProblemMsg::OutOfScreen { label } => assert!(!label.is_empty()),
            other => panic!("{other:?}"),
        }
    }

    fn has(ls: &[Lint], a: &Advice) -> bool {
        ls.iter().any(|l| &l.advice == a)
    }

    // test 50
    #[test]
    fn lints_cover_the_listed_cases() {
        use crate::menu::doc::Kind;
        let mut d = doc();
        let e = env();
        // Empty landscape profile on a landscape picture: only "no look".
        let ls = lints(&d, &e);
        assert_eq!(ls.len(), 1);
        assert_eq!(
            ls[0],
            Lint {
                target: None,
                advice: Advice::NoLook
            }
        );

        let look = d.add(Kind::Look, pt(0.5, 0.5)).unwrap();
        assert!(lints(&d, &e).is_empty());

        // Edge band: centre inside 6 %; exactly 0.06 is outside.
        let b = d.add(Kind::Button, pt(0.8, 0.5)).unwrap();
        d.move_to(b, pt(0.05, 0.5)).unwrap();
        let ls = lints(&d, &e);
        assert_eq!(
            ls,
            vec![Lint {
                target: Some(b),
                advice: Advice::NearEdge
            }]
        );
        d.move_to(b, pt(0.06, 0.5)).unwrap();
        assert!(lints(&d, &e).is_empty());

        // F-key shadow.
        d.bind(crate::menu::doc::Slot::Button(b), "F4").unwrap();
        let ls = lints(&d, &e);
        assert_eq!(
            ls,
            vec![Lint {
                target: Some(b),
                advice: Advice::ShadowsMirrorly {
                    key: "F4".into(),
                    action: MirrorlyAction::VolumeDown
                }
            }]
        );
        d.bind(crate::menu::doc::Slot::Button(b), "KeyR").unwrap();

        // Overlap with the look anchor: button on it, then far away.
        d.move_to(b, pt(0.5, 0.5)).unwrap();
        let ls = lints(&d, &e);
        assert_eq!(
            ls,
            vec![Lint {
                target: Some(b),
                advice: Advice::OverlapsLookAnchor
            }]
        );
        d.move_to(b, pt(0.5, 0.7)).unwrap();
        assert!(lints(&d, &e).is_empty());
        let _ = look;

        // Orientation mismatch, only when a picture exists.
        let mut portrait = e;
        portrait.rotation = Rotation::R0;
        let ls = lints(&d, &portrait);
        assert!(has(
            &ls,
            &Advice::OrientationMismatch {
                profile: Orientation::Landscape
            }
        ));
        portrait.has_picture = false;
        assert!(lints(&d, &portrait).is_empty());

        // More than five controls (all in the middle, away from the anchor).
        for i in 0..5 {
            d.add(Kind::Button, pt(0.1 + 0.1 * i as f64, 0.8)).unwrap();
        }
        let ls = lints(&d, &e);
        assert!(has(&ls, &Advice::MoreThanFive { count: 7 }));
        let n = d.profile().controls.len();
        assert_eq!(n, 7);
    }
}
