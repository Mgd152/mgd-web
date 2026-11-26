//! Warnings that never block saving or playing, and plain-language
//! explanations of validation problems (UI_SPEC 6.9).

use super::doc::{EditDoc, Kind};
use super::hit::{ANCHOR_RADIUS_LP, BUTTON_RADIUS_LP, EditView, button_hit_radius};
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
        if let Some((look_id, a)) = anchor {
            if look_id != *id {
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
fn control_name(p: &Profile, i: usize, lang: Lang) -> String {
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
        ProfileError::Invalid(p) => ProblemMsg::TooManyControls { max: 0 }.or_path(p),
    }
}

impl ProblemMsg {
    /// Placeholder used only by `explain_json_error` for the (unexpected)
    /// case of a validation problem without its profile.
    fn or_path(self, _p: &Problem) -> ProblemMsg {
        ProblemMsg::NotUnderstood
    }
}

/// `BUTTON_RADIUS_LP` is part of the overlap rule documented in UI_SPEC
/// 6.9; keep the import used when the rule changes.
const _: f64 = BUTTON_RADIUS_LP;
