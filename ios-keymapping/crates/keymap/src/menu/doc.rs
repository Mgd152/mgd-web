//! `EditDoc`: a profile that is valid after every call, with stable ids.

use super::keynames::{free_key, free_key_excluding};
use super::msg::{Lang, Msg};
use super::{CtrlId, Dir, Pt, RADIUS_MAX, RADIUS_MIN, REACH_MAX, REACH_MIN};
use crate::keys::{is_known_key, is_mouse_button};
use crate::profile::{ButtonMode, Control, Orientation, Profile, ProfileError};

const MAX_CONTROLS: usize = 64;
const MAX_NAME_CHARS: usize = 100;
const MAX_LABEL_CHARS: usize = 40;
const MAX_RELEASE_MS: u32 = 60_000;
const SENS_MIN: f64 = 0.05;
const SENS_MAX: f64 = 100.0;
const DUP_OFFSET: f64 = 0.03;

/// A place a key can be bound.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Slot {
    Toggle,
    Panic,
    Button(CtrlId),
    Dir(CtrlId, Dir),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Joystick,
    Look,
    Button,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum KeyPreset {
    Wasd,
    Arrows,
}

#[derive(Clone, PartialEq, Debug)]
pub enum BindOutcome {
    Bound,
    Unchanged,
    /// Nothing changed.
    Conflict {
        owner: Slot,
    },
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum EditError {
    NoSuchControl,
    UnknownKey,
    MouseNotAllowed,
    TooManyControls,
    SecondLook,
    NoFreeKey,
    WrongKind,
}

/// Auto-assignment sets for new joysticks: up, down, left, right.
const KEY_SETS: [[&str; 4]; 4] = [
    ["KeyW", "KeyS", "KeyA", "KeyD"],
    ["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"],
    ["KeyI", "KeyK", "KeyJ", "KeyL"],
    ["KeyT", "KeyG", "KeyF", "KeyH"],
];

/// Round to 4 decimals (the browser editor's rule).
pub fn round4(v: f64) -> f64 {
    (v * 10_000.0).round() / 10_000.0
}

fn unit(v: f64) -> f64 {
    round4(v.clamp(0.0, 1.0))
}

fn truncate_chars(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

#[derive(Clone, PartialEq, Debug)]
pub struct EditDoc {
    profile: Profile,
    ids: Vec<CtrlId>,
    next: CtrlId,
}

impl EditDoc {
    /// Validates the profile; ids are assigned 1.. in array order.
    pub fn from_profile(p: Profile) -> Result<EditDoc, ProfileError> {
        p.validate()?;
        let n = p.controls.len() as CtrlId;
        Ok(EditDoc {
            profile: p,
            ids: (1..=n).collect(),
            next: n + 1,
        })
    }

    pub fn profile(&self) -> &Profile {
        &self.profile
    }

    pub fn ids(&self) -> &[CtrlId] {
        &self.ids
    }

    pub fn index_of(&self, id: CtrlId) -> Option<usize> {
        self.ids.iter().position(|i| *i == id)
    }

    pub fn control(&self, id: CtrlId) -> Option<&Control> {
        self.index_of(id).map(|i| &self.profile.controls[i])
    }

    pub fn kind(&self, id: CtrlId) -> Option<Kind> {
        self.control(id).map(|c| match c {
            Control::Joystick { .. } => Kind::Joystick,
            Control::Look { .. } => Kind::Look,
            Control::Button { .. } => Kind::Button,
        })
    }

    /// The key bound to a slot (`None` when the slot does not exist).
    pub fn key_of(&self, slot: Slot) -> Option<&str> {
        match slot {
            Slot::Toggle => Some(&self.profile.toggle_key),
            Slot::Panic => Some(&self.profile.panic_key),
            Slot::Button(id) => match self.control(id)? {
                Control::Button { key, .. } => Some(key),
                _ => None,
            },
            Slot::Dir(id, d) => match self.control(id)? {
                Control::Joystick {
                    up,
                    down,
                    left,
                    right,
                    ..
                } => Some(match d {
                    Dir::Up => up,
                    Dir::Down => down,
                    Dir::Left => left,
                    Dir::Right => right,
                }),
                _ => None,
            },
        }
    }

    /// Who uses `key` (toggle, panic, then controls in order).
    pub fn owner_of(&self, key: &str) -> Option<Slot> {
        if self.profile.toggle_key == key {
            return Some(Slot::Toggle);
        }
        if self.profile.panic_key == key {
            return Some(Slot::Panic);
        }
        for (c, id) in self.profile.controls.iter().zip(&self.ids) {
            match c {
                Control::Button { key: k, .. } if k == key => return Some(Slot::Button(*id)),
                Control::Joystick {
                    up,
                    down,
                    left,
                    right,
                    ..
                } => {
                    for (k, d) in [
                        (up, Dir::Up),
                        (down, Dir::Down),
                        (left, Dir::Left),
                        (right, Dir::Right),
                    ] {
                        if k == key {
                            return Some(Slot::Dir(*id, d));
                        }
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// Name shown to the user: its label, else kind and number
    /// (`Nút 3`, `Cần di chuyển`, `Xoay camera`).
    pub fn display_name(&self, id: CtrlId, lang: Lang) -> Option<String> {
        let i = self.index_of(id)?;
        let kind = self.kind(id)?;
        let label = match &self.profile.controls[i] {
            Control::Joystick { label, .. }
            | Control::Look { label, .. }
            | Control::Button { label, .. } => label,
        };
        if !label.trim().is_empty() {
            return Some(label.clone());
        }
        let same: Vec<usize> = (0..self.ids.len())
            .filter(|j| self.kind(self.ids[*j]) == Some(kind))
            .collect();
        let pos = same.iter().position(|j| *j == i).unwrap_or(0) as u32 + 1;
        let n = match kind {
            Kind::Button => pos,
            Kind::Joystick if same.len() > 1 => pos,
            _ => 0,
        };
        Some(Msg::ControlName { kind, n }.text(lang))
    }

    /// Display name of whoever owns a slot.
    pub fn slot_name(&self, slot: Slot, lang: Lang) -> String {
        match slot {
            Slot::Toggle => Msg::SlotToggle.text(lang),
            Slot::Panic => Msg::SlotPanic.text(lang),
            Slot::Button(id) | Slot::Dir(id, _) => self.display_name(id, lang).unwrap_or_default(),
        }
    }

    fn idx(&self, id: CtrlId) -> Result<usize, EditError> {
        self.index_of(id).ok_or(EditError::NoSuchControl)
    }

    fn has_look(&self) -> bool {
        self.profile.has_look()
    }

    fn fresh_id(&mut self) -> CtrlId {
        let id = self.next;
        self.next += 1;
        id
    }

    /// Four free keys for a joystick (up, down, left, right).
    fn free_joystick_keys(&self) -> Result<[String; 4], EditError> {
        for set in KEY_SETS {
            if set.iter().all(|k| self.owner_of(k).is_none()) {
                return Ok(set.map(String::from));
            }
        }
        let mut picked: Vec<&'static str> = Vec::new();
        for _ in 0..4 {
            let k = free_key_excluding(self, &picked).ok_or(EditError::NoFreeKey)?;
            picked.push(k);
        }
        Ok([
            picked[0].to_string(),
            picked[1].to_string(),
            picked[2].to_string(),
            picked[3].to_string(),
        ])
    }

    fn check_room(&self, kind: Kind) -> Result<(), EditError> {
        if kind == Kind::Look && self.has_look() {
            return Err(EditError::SecondLook);
        }
        if self.profile.controls.len() >= MAX_CONTROLS {
            return Err(EditError::TooManyControls);
        }
        Ok(())
    }

    fn push(&mut self, c: Control) -> CtrlId {
        let id = self.fresh_id();
        self.profile.controls.push(c);
        self.ids.push(id);
        id
    }

    /// Adds a control of `kind` at a picture fraction with default size and
    /// automatically chosen free keys.
    pub fn add(&mut self, kind: Kind, at: Pt) -> Result<CtrlId, EditError> {
        self.check_room(kind)?;
        let (x, y) = (unit_or(at.x, 0.5), unit_or(at.y, 0.5));
        let c = match kind {
            Kind::Joystick => {
                let [up, down, left, right] = self.free_joystick_keys()?;
                Control::Joystick {
                    label: String::new(),
                    x,
                    y,
                    radius: 0.11,
                    up,
                    down,
                    left,
                    right,
                }
            }
            Kind::Look => Control::Look {
                label: String::new(),
                x,
                y,
                sensitivity: 1.0,
                invert_y: false,
                reach: 0.30,
                release_after_ms: 0,
            },
            Kind::Button => Control::Button {
                label: String::new(),
                x,
                y,
                key: free_key(self).ok_or(EditError::NoFreeKey)?.to_string(),
                mode: ButtonMode::Hold,
            },
        };
        Ok(self.push(c))
    }

    pub fn remove(&mut self, id: CtrlId) -> Result<(), EditError> {
        let i = self.idx(id)?;
        self.profile.controls.remove(i);
        self.ids.remove(i);
        Ok(())
    }

    /// Copy shifted by +0.03/+0.03 (clamped) with fresh free keys.
    pub fn duplicate(&mut self, id: CtrlId) -> Result<CtrlId, EditError> {
        let i = self.idx(id)?;
        let kind = self.kind(id).ok_or(EditError::NoSuchControl)?;
        self.check_room(kind)?;
        let mut c = self.profile.controls[i].clone();
        match &mut c {
            Control::Joystick {
                x,
                y,
                up,
                down,
                left,
                right,
                ..
            } => {
                *x = unit(*x + DUP_OFFSET);
                *y = unit(*y + DUP_OFFSET);
                [*up, *down, *left, *right] = self.free_joystick_keys()?;
            }
            Control::Look { x, y, .. } => {
                *x = unit(*x + DUP_OFFSET);
                *y = unit(*y + DUP_OFFSET);
            }
            Control::Button { x, y, key, .. } => {
                *x = unit(*x + DUP_OFFSET);
                *y = unit(*y + DUP_OFFSET);
                *key = free_key(self).ok_or(EditError::NoFreeKey)?.to_string();
            }
        }
        Ok(self.push(c))
    }

    /// Clamps to [0, 1] and rounds; a non-finite coordinate is ignored.
    pub fn move_to(&mut self, id: CtrlId, at: Pt) -> Result<(), EditError> {
        let i = self.idx(id)?;
        let (nx, ny) = (at.x, at.y);
        let (Control::Joystick { x, y, .. }
        | Control::Look { x, y, .. }
        | Control::Button { x, y, .. }) = &mut self.profile.controls[i];
        if nx.is_finite() {
            *x = unit(nx);
        }
        if ny.is_finite() {
            *y = unit(ny);
        }
        Ok(())
    }

    pub fn set_radius(&mut self, id: CtrlId, r: f64) -> Result<(), EditError> {
        let i = self.idx(id)?;
        match &mut self.profile.controls[i] {
            Control::Joystick { radius, .. } => {
                if r.is_finite() {
                    *radius = round4(r.clamp(RADIUS_MIN, RADIUS_MAX));
                }
                Ok(())
            }
            _ => Err(EditError::WrongKind),
        }
    }

    pub fn set_reach(&mut self, id: CtrlId, r: f64) -> Result<(), EditError> {
        let i = self.idx(id)?;
        match &mut self.profile.controls[i] {
            Control::Look { reach, .. } => {
                if r.is_finite() {
                    *reach = round4(r.clamp(REACH_MIN, REACH_MAX));
                }
                Ok(())
            }
            _ => Err(EditError::WrongKind),
        }
    }

    pub fn set_sensitivity(&mut self, id: CtrlId, s: f64) -> Result<(), EditError> {
        let i = self.idx(id)?;
        match &mut self.profile.controls[i] {
            Control::Look { sensitivity, .. } => {
                if s.is_finite() {
                    // 4 decimals would round 0.05 fine, but keep the floor exact.
                    *sensitivity = round4(s.clamp(SENS_MIN, SENS_MAX)).max(SENS_MIN);
                }
                Ok(())
            }
            _ => Err(EditError::WrongKind),
        }
    }

    pub fn set_invert_y(&mut self, id: CtrlId, on: bool) -> Result<(), EditError> {
        let i = self.idx(id)?;
        match &mut self.profile.controls[i] {
            Control::Look { invert_y, .. } => {
                *invert_y = on;
                Ok(())
            }
            _ => Err(EditError::WrongKind),
        }
    }

    pub fn set_release_after(&mut self, id: CtrlId, ms: u32) -> Result<(), EditError> {
        let i = self.idx(id)?;
        match &mut self.profile.controls[i] {
            Control::Look {
                release_after_ms, ..
            } => {
                *release_after_ms = ms.min(MAX_RELEASE_MS);
                Ok(())
            }
            _ => Err(EditError::WrongKind),
        }
    }

    pub fn set_mode(&mut self, id: CtrlId, m: ButtonMode) -> Result<(), EditError> {
        let i = self.idx(id)?;
        match &mut self.profile.controls[i] {
            Control::Button { mode, .. } => {
                *mode = m;
                Ok(())
            }
            _ => Err(EditError::WrongKind),
        }
    }

    /// Truncates to 40 characters.
    pub fn set_label(&mut self, id: CtrlId, s: &str) -> Result<(), EditError> {
        let i = self.idx(id)?;
        let (Control::Joystick { label, .. }
        | Control::Look { label, .. }
        | Control::Button { label, .. }) = &mut self.profile.controls[i];
        *label = truncate_chars(s, MAX_LABEL_CHARS);
        Ok(())
    }

    /// Truncates to 100 characters.
    pub fn set_name(&mut self, s: &str) {
        self.profile.name = truncate_chars(s, MAX_NAME_CHARS);
    }

    pub fn set_orientation(&mut self, o: Orientation) {
        self.profile.orientation = o;
    }

    fn slot_ref(&mut self, slot: Slot) -> Result<&mut String, EditError> {
        match slot {
            Slot::Toggle => Ok(&mut self.profile.toggle_key),
            Slot::Panic => Ok(&mut self.profile.panic_key),
            Slot::Button(id) => {
                let i = self.idx(id)?;
                match &mut self.profile.controls[i] {
                    Control::Button { key, .. } => Ok(key),
                    _ => Err(EditError::WrongKind),
                }
            }
            Slot::Dir(id, d) => {
                let i = self.idx(id)?;
                match &mut self.profile.controls[i] {
                    Control::Joystick {
                        up,
                        down,
                        left,
                        right,
                        ..
                    } => Ok(match d {
                        Dir::Up => up,
                        Dir::Down => down,
                        Dir::Left => left,
                        Dir::Right => right,
                    }),
                    _ => Err(EditError::WrongKind),
                }
            }
        }
    }

    fn keyboard_only(slot: Slot) -> bool {
        matches!(slot, Slot::Toggle | Slot::Panic)
    }

    /// Bind `key` to `slot`. A key used elsewhere is a `Conflict` and nothing
    /// changes; the caller may then `swap`.
    pub fn bind(&mut self, slot: Slot, key: &str) -> Result<BindOutcome, EditError> {
        if !is_known_key(key) {
            return Err(EditError::UnknownKey);
        }
        if Self::keyboard_only(slot) && is_mouse_button(key) {
            return Err(EditError::MouseNotAllowed);
        }
        let current = self.key_of(slot).ok_or_else(|| match slot {
            Slot::Button(id) | Slot::Dir(id, _) if self.index_of(id).is_some() => {
                EditError::WrongKind
            }
            _ => EditError::NoSuchControl,
        })?;
        if current == key {
            return Ok(BindOutcome::Unchanged);
        }
        if let Some(owner) = self.owner_of(key) {
            return Ok(BindOutcome::Conflict { owner });
        }
        *self.slot_ref(slot)? = key.to_string();
        Ok(BindOutcome::Bound)
    }

    /// Exchange the keys of two slots; the profile stays valid.
    pub fn swap(&mut self, a: Slot, b: Slot) -> Result<(), EditError> {
        let ka = self.key_of(a).ok_or(EditError::NoSuchControl)?.to_string();
        let kb = self.key_of(b).ok_or(EditError::NoSuchControl)?.to_string();
        if (Self::keyboard_only(a) && is_mouse_button(&kb))
            || (Self::keyboard_only(b) && is_mouse_button(&ka))
        {
            return Err(EditError::MouseNotAllowed);
        }
        *self.slot_ref(a)? = kb;
        *self.slot_ref(b)? = ka;
        Ok(())
    }

    /// Give a joystick W A S D or the arrow keys. `Err(NoFreeKey)` when one of
    /// them is bound to something other than this joystick.
    pub fn set_joystick_keys(&mut self, id: CtrlId, preset: KeyPreset) -> Result<(), EditError> {
        let i = self.idx(id)?;
        if self.kind(id) != Some(Kind::Joystick) {
            return Err(EditError::WrongKind);
        }
        let set = match preset {
            KeyPreset::Wasd => KEY_SETS[0],
            KeyPreset::Arrows => KEY_SETS[1],
        };
        let own = |k: &str| matches!(self.owner_of(k), Some(Slot::Dir(j, _)) if j == id);
        if set.iter().any(|k| self.owner_of(k).is_some() && !own(k)) {
            return Err(EditError::NoFreeKey);
        }
        if let Control::Joystick {
            up,
            down,
            left,
            right,
            ..
        } = &mut self.profile.controls[i]
        {
            *up = set[0].into();
            *down = set[1].into();
            *left = set[2].into();
            *right = set[3].into();
        }
        Ok(())
    }
}

fn unit_or(v: f64, fallback: f64) -> f64 {
    if v.is_finite() { unit(v) } else { fallback }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::{DEFAULT_TOGGLE_KEY, KEY_NAMES, MOUSE_NAMES};
    use crate::menu::templates::{Template, template};

    pub(crate) fn empty() -> EditDoc {
        EditDoc::from_profile(template(Template::Empty(Orientation::Landscape))).unwrap()
    }

    pub(crate) fn shooter() -> EditDoc {
        EditDoc::from_profile(template(Template::ShooterLandscape)).unwrap()
    }

    fn pt(x: f64, y: f64) -> Pt {
        Pt { x, y }
    }

    fn check(doc: &EditDoc) {
        assert_eq!(doc.profile().problems(), vec![]);
        assert_eq!(doc.ids().len(), doc.profile().controls.len());
        let mut ids = doc.ids().to_vec();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), doc.ids().len());
    }

    fn id_of(doc: &EditDoc, kind: Kind) -> CtrlId {
        *doc.ids()
            .iter()
            .find(|i| doc.kind(**i) == Some(kind))
            .unwrap()
    }

    // test 1
    #[test]
    fn every_mutator_keeps_the_profile_valid_on_every_kind() {
        let mut doc = empty();
        let j = doc.add(Kind::Joystick, pt(0.2, 0.7)).unwrap();
        let l = doc.add(Kind::Look, pt(0.7, 0.4)).unwrap();
        let b = doc.add(Kind::Button, pt(0.9, 0.8)).unwrap();
        let nasty = [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            -5.0,
            0.0,
            1e9,
            0.123456789,
        ];
        for id in [j, l, b, 9999] {
            for v in nasty {
                let _ = doc.move_to(id, pt(v, v));
                check(&doc);
                let _ = doc.set_radius(id, v);
                check(&doc);
                let _ = doc.set_reach(id, v);
                check(&doc);
                let _ = doc.set_sensitivity(id, v);
                check(&doc);
            }
            let _ = doc.set_invert_y(id, true);
            let _ = doc.set_release_after(id, u32::MAX);
            let _ = doc.set_mode(id, ButtonMode::Tap);
            let _ = doc.set_label(id, &"ệ".repeat(200));
            let _ = doc.set_joystick_keys(id, KeyPreset::Arrows);
            check(&doc);
        }
        doc.set_name(&"Ô".repeat(300));
        doc.set_orientation(Orientation::Portrait);
        for k in KEY_NAMES.iter().chain(MOUSE_NAMES) {
            for slot in [
                Slot::Toggle,
                Slot::Panic,
                Slot::Button(b),
                Slot::Button(j),
                Slot::Dir(j, Dir::Up),
                Slot::Dir(j, Dir::Right),
                Slot::Dir(l, Dir::Left),
                Slot::Dir(42, Dir::Down),
            ] {
                let _ = doc.bind(slot, k);
                check(&doc);
                let _ = doc.swap(slot, Slot::Toggle);
                check(&doc);
            }
        }
        assert!(doc.bind(Slot::Toggle, "nope").is_err());
        let d = doc.duplicate(j).unwrap();
        let _ = doc.duplicate(l);
        doc.remove(d).unwrap();
        check(&doc);
    }

    // test 2
    #[test]
    fn add_picks_wasd_then_arrows_then_ijkl() {
        let mut doc = empty();
        let keys = |doc: &EditDoc, id| -> Vec<String> {
            [Dir::Up, Dir::Down, Dir::Left, Dir::Right]
                .iter()
                .map(|d| doc.key_of(Slot::Dir(id, *d)).unwrap().to_string())
                .collect()
        };
        let a = doc.add(Kind::Joystick, pt(0.2, 0.7)).unwrap();
        let b = doc.add(Kind::Joystick, pt(0.3, 0.7)).unwrap();
        let c = doc.add(Kind::Joystick, pt(0.4, 0.7)).unwrap();
        let d = doc.add(Kind::Joystick, pt(0.5, 0.7)).unwrap();
        assert_eq!(keys(&doc, a), ["KeyW", "KeyS", "KeyA", "KeyD"]);
        assert_eq!(
            keys(&doc, b),
            ["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"]
        );
        assert_eq!(keys(&doc, c), ["KeyI", "KeyK", "KeyJ", "KeyL"]);
        assert_eq!(keys(&doc, d), ["KeyT", "KeyG", "KeyF", "KeyH"]);
        let e = doc.add(Kind::Joystick, pt(0.6, 0.7)).unwrap();
        let ks = keys(&doc, e);
        let mut u = ks.clone();
        u.sort();
        u.dedup();
        assert_eq!(u.len(), 4, "{ks:?}");
        check(&doc);
        // ijkl taken by buttons -> falls through to the next set
        let mut doc = empty();
        let b = doc.add(Kind::Button, pt(0.5, 0.5)).unwrap();
        doc.bind(Slot::Button(b), "KeyJ").unwrap();
        let a = doc.add(Kind::Joystick, pt(0.2, 0.7)).unwrap();
        let b = doc.add(Kind::Joystick, pt(0.2, 0.7)).unwrap();
        let c = doc.add(Kind::Joystick, pt(0.2, 0.7)).unwrap();
        assert_eq!(keys(&doc, a)[0], "KeyW");
        assert_eq!(keys(&doc, b)[0], "ArrowUp");
        assert_eq!(keys(&doc, c)[0], "KeyT", "IJKL is blocked by a button on J");
    }

    #[test]
    fn add_defaults() {
        let mut doc = empty();
        let j = doc.add(Kind::Joystick, pt(0.2, 0.7)).unwrap();
        let l = doc.add(Kind::Look, pt(0.7, 0.4)).unwrap();
        let b = doc.add(Kind::Button, pt(0.9, 0.8)).unwrap();
        match &doc.profile().controls[0] {
            Control::Joystick { radius, .. } => assert_eq!(*radius, 0.11),
            c => panic!("{c:?}"),
        }
        match &doc.profile().controls[1] {
            Control::Look {
                sensitivity,
                reach,
                invert_y,
                release_after_ms,
                ..
            } => assert_eq!(
                (*sensitivity, *reach, *invert_y, *release_after_ms),
                (1.0, 0.30, false, 0)
            ),
            c => panic!("{c:?}"),
        }
        match &doc.profile().controls[2] {
            Control::Button { key, mode, .. } => {
                assert_eq!(key, "Space");
                assert_eq!(*mode, ButtonMode::Hold);
            }
            c => panic!("{c:?}"),
        }
        assert_eq!(doc.ids(), &[j, l, b]);
        // position is clamped and rounded
        let b2 = doc.add(Kind::Button, pt(1.7, -0.3)).unwrap();
        assert_eq!(
            doc.control(b2)
                .map(|c| matches!(c, Control::Button { x, y, .. } if *x == 1.0 && *y == 0.0)),
            Some(true)
        );
    }

    // test 3
    #[test]
    fn second_look_and_65th_control_are_refused() {
        let mut doc = empty();
        doc.add(Kind::Look, pt(0.5, 0.5)).unwrap();
        assert_eq!(
            doc.add(Kind::Look, pt(0.5, 0.5)),
            Err(EditError::SecondLook)
        );
        for _ in 0..63 {
            doc.add(Kind::Button, pt(0.5, 0.5)).unwrap();
        }
        assert_eq!(doc.profile().controls.len(), 64);
        assert_eq!(
            doc.add(Kind::Button, pt(0.5, 0.5)),
            Err(EditError::TooManyControls)
        );
        assert_eq!(
            doc.add(Kind::Joystick, pt(0.5, 0.5)),
            Err(EditError::TooManyControls)
        );
        let b = doc.ids()[5];
        assert_eq!(doc.duplicate(b), Err(EditError::TooManyControls));
        check(&doc);
    }

    // test 4
    #[test]
    fn bind_free_same_and_conflict() {
        let mut doc = shooter();
        let reload = doc.owner_of("KeyR").unwrap();
        let Slot::Button(rid) = reload else { panic!() };
        let jump = doc.owner_of("Space").unwrap();
        let Slot::Button(jid) = jump else { panic!() };
        assert_eq!(
            doc.bind(Slot::Button(rid), "KeyR"),
            Ok(BindOutcome::Unchanged)
        );
        let before = doc.clone();
        assert_eq!(
            doc.bind(Slot::Button(jid), "KeyR"),
            Ok(BindOutcome::Conflict {
                owner: Slot::Button(rid)
            })
        );
        assert_eq!(doc, before, "a conflict changes nothing");
        assert_eq!(doc.bind(Slot::Button(jid), "KeyZ"), Ok(BindOutcome::Bound));
        assert_eq!(doc.key_of(Slot::Button(jid)), Some("KeyZ"));
        assert_eq!(doc.owner_of("Space"), None);
        assert_eq!(
            doc.bind(Slot::Button(jid), "KeyQQ"),
            Err(EditError::UnknownKey)
        );
        assert_eq!(
            doc.bind(Slot::Button(777), "KeyZ"),
            Err(EditError::NoSuchControl)
        );
        let lid = doc
            .ids()
            .iter()
            .copied()
            .find(|i| doc.kind(*i) == Some(Kind::Look))
            .unwrap();
        assert_eq!(
            doc.bind(Slot::Button(lid), "KeyZ"),
            Err(EditError::WrongKind)
        );
        assert_eq!(
            doc.bind(Slot::Toggle, "MouseLeft"),
            Err(EditError::MouseNotAllowed)
        );
        assert_eq!(doc.bind(Slot::Panic, "KeyX"), Ok(BindOutcome::Bound));
        // mouse names are fine on buttons and directions
        let Some(Slot::Dir(jy, _)) = doc.owner_of("KeyW") else {
            panic!()
        };
        assert!(matches!(
            doc.bind(Slot::Dir(jy, Dir::Up), "MouseMiddle"),
            Ok(BindOutcome::Bound)
        ));
        check(&doc);
    }

    // test 5
    #[test]
    fn swap_cases() {
        let mut doc = shooter();
        let Some(Slot::Button(r)) = doc.owner_of("KeyR") else {
            panic!()
        };
        let Some(Slot::Button(c)) = doc.owner_of("KeyC") else {
            panic!()
        };
        let Some(Slot::Dir(j, _)) = doc.owner_of("KeyW") else {
            panic!()
        };
        // button-button
        doc.swap(Slot::Button(r), Slot::Button(c)).unwrap();
        assert_eq!(doc.key_of(Slot::Button(r)), Some("KeyC"));
        assert_eq!(doc.key_of(Slot::Button(c)), Some("KeyR"));
        // button-joystick direction
        doc.swap(Slot::Button(r), Slot::Dir(j, Dir::Up)).unwrap();
        assert_eq!(doc.key_of(Slot::Button(r)), Some("KeyW"));
        assert_eq!(doc.key_of(Slot::Dir(j, Dir::Up)), Some("KeyC"));
        // control-toggle
        doc.swap(Slot::Button(c), Slot::Toggle).unwrap();
        assert_eq!(doc.key_of(Slot::Toggle), Some("KeyR"));
        assert_eq!(doc.key_of(Slot::Button(c)), Some("Backquote"));
        check(&doc);
        // a mouse name may not land on toggle / panic
        let Some(Slot::Button(fire)) = doc.owner_of("MouseLeft") else {
            panic!()
        };
        let before = doc.clone();
        assert_eq!(
            doc.swap(Slot::Button(fire), Slot::Toggle),
            Err(EditError::MouseNotAllowed)
        );
        assert_eq!(
            doc.swap(Slot::Panic, Slot::Button(fire)),
            Err(EditError::MouseNotAllowed)
        );
        assert_eq!(doc, before);
        assert_eq!(doc.swap(Slot::Button(c), Slot::Button(c)), Ok(()));
        assert_eq!(
            doc.swap(Slot::Button(999), Slot::Toggle),
            Err(EditError::NoSuchControl)
        );
        assert_eq!(doc, before);
    }

    // test 6
    #[test]
    fn clamping_and_non_finite() {
        let mut doc = empty();
        let j = doc.add(Kind::Joystick, pt(0.2, 0.7)).unwrap();
        let l = doc.add(Kind::Look, pt(0.7, 0.4)).unwrap();
        doc.move_to(j, pt(-1.0, 7.0)).unwrap();
        assert!(
            matches!(doc.control(j), Some(Control::Joystick { x, y, .. }) if *x == 0.0 && *y == 1.0)
        );
        doc.move_to(j, pt(0.123456, f64::NAN)).unwrap();
        assert!(
            matches!(doc.control(j), Some(Control::Joystick { x, y, .. }) if *x == 0.1235 && *y == 1.0)
        );
        doc.set_radius(j, 0.0).unwrap();
        assert!(
            matches!(doc.control(j), Some(Control::Joystick { radius, .. }) if *radius == RADIUS_MIN)
        );
        doc.set_radius(j, 9.0).unwrap();
        assert!(
            matches!(doc.control(j), Some(Control::Joystick { radius, .. }) if *radius == RADIUS_MAX)
        );
        doc.set_radius(j, f64::NAN).unwrap();
        assert!(
            matches!(doc.control(j), Some(Control::Joystick { radius, .. }) if *radius == RADIUS_MAX)
        );
        doc.set_reach(l, 0.0).unwrap();
        assert!(matches!(doc.control(l), Some(Control::Look { reach, .. }) if *reach == REACH_MIN));
        doc.set_reach(l, 5.0).unwrap();
        assert!(matches!(doc.control(l), Some(Control::Look { reach, .. }) if *reach == REACH_MAX));
        doc.set_sensitivity(l, 0.0).unwrap();
        assert!(
            matches!(doc.control(l), Some(Control::Look { sensitivity, .. }) if *sensitivity == 0.05)
        );
        doc.set_sensitivity(l, 1e6).unwrap();
        assert!(
            matches!(doc.control(l), Some(Control::Look { sensitivity, .. }) if *sensitivity == 100.0)
        );
        doc.set_sensitivity(l, f64::INFINITY).unwrap();
        doc.set_sensitivity(l, f64::NAN).unwrap();
        assert!(
            matches!(doc.control(l), Some(Control::Look { sensitivity, .. }) if *sensitivity == 100.0)
        );
        doc.set_release_after(l, 1_000_000).unwrap();
        assert!(
            matches!(doc.control(l), Some(Control::Look { release_after_ms, .. }) if *release_after_ms == 60_000)
        );
        assert_eq!(doc.set_radius(l, 0.1), Err(EditError::WrongKind));
        assert_eq!(doc.set_reach(j, 0.1), Err(EditError::WrongKind));
        assert_eq!(doc.set_release_after(j, 1), Err(EditError::WrongKind));
        assert_eq!(doc.set_invert_y(j, true), Err(EditError::WrongKind));
        assert_eq!(doc.set_mode(j, ButtonMode::Tap), Err(EditError::WrongKind));
        assert_eq!(doc.set_radius(55, 0.1), Err(EditError::NoSuchControl));
        check(&doc);
    }

    // test 7
    #[test]
    fn label_and_name_truncate_on_char_boundaries() {
        let mut doc = empty();
        let b = doc.add(Kind::Button, pt(0.5, 0.5)).unwrap();
        let long = "Nạp đạn ệ".repeat(10);
        doc.set_label(b, &long).unwrap();
        let Some(Control::Button { label, .. }) = doc.control(b) else {
            panic!()
        };
        assert_eq!(label.chars().count(), 40);
        assert!(long.starts_with(label.as_str()));
        doc.set_name(&"Đấu trường ".repeat(20));
        assert_eq!(doc.profile().name.chars().count(), 100);
        doc.set_name("Hồ sơ");
        assert_eq!(doc.profile().name, "Hồ sơ");
        check(&doc);
    }

    // test 8
    #[test]
    fn duplicate_offsets_clamps_and_gets_fresh_keys() {
        let mut doc = shooter();
        let Some(Slot::Button(r)) = doc.owner_of("KeyR") else {
            panic!()
        };
        let d = doc.duplicate(r).unwrap();
        let (
            Some(Control::Button { x: x0, y: y0, .. }),
            Some(Control::Button {
                x, y, key, label, ..
            }),
        ) = (doc.control(r), doc.control(d))
        else {
            panic!()
        };
        assert_eq!((*x, *y), (round4(x0 + 0.03), round4(y0 + 0.03)));
        assert_ne!(key, "KeyR");
        assert_eq!(label, "Nạp đạn");
        // clamps at the edge
        let Some(Slot::Button(n)) = doc.owner_of("Space") else {
            panic!()
        };
        doc.move_to(n, pt(1.0, 1.0)).unwrap();
        let d2 = doc.duplicate(n).unwrap();
        assert!(
            matches!(doc.control(d2), Some(Control::Button { x, y, .. }) if *x == 1.0 && *y == 1.0)
        );
        // joystick gets four fresh keys
        let j = id_of(&doc, Kind::Joystick);
        let dj = doc.duplicate(j).unwrap();
        let ks: Vec<_> = [Dir::Up, Dir::Down, Dir::Left, Dir::Right]
            .iter()
            .map(|d| doc.key_of(Slot::Dir(dj, *d)).unwrap().to_string())
            .collect();
        assert_eq!(ks, ["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"]);
        // look refused
        let l = id_of(&doc, Kind::Look);
        assert_eq!(doc.duplicate(l), Err(EditError::SecondLook));
        assert_eq!(doc.duplicate(12345), Err(EditError::NoSuchControl));
        check(&doc);
    }

    // test 9
    #[test]
    fn ids_are_stable_across_remove() {
        let mut doc = shooter();
        let ids = doc.ids().to_vec();
        let victim = ids[3];
        let kind_of_5 = doc.kind(ids[5]);
        doc.remove(victim).unwrap();
        assert_eq!(doc.index_of(victim), None);
        assert_eq!(doc.index_of(ids[5]), Some(4));
        assert_eq!(doc.kind(ids[5]), kind_of_5);
        assert_eq!(doc.remove(victim), Err(EditError::NoSuchControl));
        let fresh = doc.add(Kind::Button, pt(0.5, 0.5)).unwrap();
        assert!(!ids.contains(&fresh), "ids are never reused");
        check(&doc);
    }

    // test 10
    #[test]
    fn numbers_are_rounded_and_json_round_trips() {
        let mut doc = empty();
        let j = doc
            .add(Kind::Joystick, pt(0.123456789, 0.987654321))
            .unwrap();
        doc.set_radius(j, 0.123456789).unwrap();
        let l = doc.add(Kind::Look, pt(1.0 / 3.0, 2.0 / 3.0)).unwrap();
        doc.set_sensitivity(l, 1.234567).unwrap();
        doc.set_reach(l, 0.333333333).unwrap();
        let b = doc.add(Kind::Button, pt(0.55555555, 0.44444444)).unwrap();
        doc.set_label(b, "Bắn").unwrap();
        doc.set_name("Hồ sơ 1");
        let text = doc.profile().to_json_pretty();
        assert!(text.contains("0.1235"), "{text}");
        assert!(!text.contains("0.12345"), "{text}");
        let back = Profile::from_json(&text).unwrap();
        assert_eq!(&back, doc.profile());
        let again = EditDoc::from_profile(back).unwrap();
        assert_eq!(again.profile(), doc.profile());
        assert_eq!(round4(0.00005), 0.0001);
    }

    // test 11
    #[test]
    fn no_free_key_when_all_are_taken() {
        use crate::menu::keynames::excluded_from_auto;
        let pool: Vec<&str> = KEY_NAMES
            .iter()
            .copied()
            .filter(|k| !excluded_from_auto(k) && *k != DEFAULT_TOGGLE_KEY)
            .collect();
        assert!(pool.len() >= 80 && pool.len() < 84, "pool = {}", pool.len());
        let mut p = template(Template::Empty(Orientation::Landscape));
        for four in pool.as_chunks::<4>().0.iter().take(20) {
            p.controls.push(Control::Joystick {
                label: String::new(),
                x: 0.5,
                y: 0.5,
                radius: 0.1,
                up: four[0].into(),
                down: four[1].into(),
                left: four[2].into(),
                right: four[3].into(),
            });
        }
        let mut doc = EditDoc::from_profile(p).unwrap();
        let spare = pool.len() - 80;
        assert!(spare < 4);
        assert_eq!(
            doc.add(Kind::Joystick, pt(0.5, 0.5)),
            Err(EditError::NoFreeKey)
        );
        for _ in 0..spare {
            doc.add(Kind::Button, pt(0.5, 0.5)).unwrap();
        }
        let before = doc.clone();
        assert_eq!(
            doc.add(Kind::Button, pt(0.5, 0.5)),
            Err(EditError::NoFreeKey)
        );
        let j = doc.ids()[0];
        assert_eq!(doc.duplicate(j), Err(EditError::NoFreeKey));
        assert_eq!(doc, before, "a refused add leaves the doc untouched");
        check(&doc);
    }

    #[test]
    fn display_names() {
        let mut doc = empty();
        let j = doc.add(Kind::Joystick, pt(0.2, 0.7)).unwrap();
        let l = doc.add(Kind::Look, pt(0.7, 0.4)).unwrap();
        let b1 = doc.add(Kind::Button, pt(0.8, 0.4)).unwrap();
        let b2 = doc.add(Kind::Button, pt(0.8, 0.5)).unwrap();
        assert_eq!(doc.display_name(j, Lang::Vi).unwrap(), "Cần di chuyển");
        assert_eq!(doc.display_name(l, Lang::Vi).unwrap(), "Xoay camera");
        assert_eq!(doc.display_name(b1, Lang::Vi).unwrap(), "Nút 1");
        assert_eq!(doc.display_name(b2, Lang::En).unwrap(), "Button 2");
        doc.set_label(b2, "Bắn").unwrap();
        assert_eq!(doc.display_name(b2, Lang::Vi).unwrap(), "Bắn");
        assert_eq!(doc.slot_name(Slot::Button(b2), Lang::Vi), "Bắn");
        assert_eq!(
            doc.slot_name(Slot::Dir(j, Dir::Up), Lang::Vi),
            "Cần di chuyển"
        );
        assert_eq!(doc.slot_name(Slot::Toggle, Lang::Vi), "Phím bật/tắt");
        assert_eq!(doc.display_name(99, Lang::Vi), None);
    }

    #[test]
    fn set_joystick_keys_presets() {
        let mut doc = shooter();
        let j = id_of(&doc, Kind::Joystick);
        doc.set_joystick_keys(j, KeyPreset::Arrows).unwrap();
        assert_eq!(doc.key_of(Slot::Dir(j, Dir::Left)), Some("ArrowLeft"));
        doc.set_joystick_keys(j, KeyPreset::Wasd).unwrap();
        assert_eq!(doc.key_of(Slot::Dir(j, Dir::Left)), Some("KeyA"));
        // taken elsewhere -> refused, unchanged
        let b = doc
            .ids()
            .iter()
            .copied()
            .find(|i| doc.kind(*i) == Some(Kind::Button))
            .unwrap();
        doc.bind(Slot::Button(b), "ArrowUp").unwrap();
        let before = doc.clone();
        assert_eq!(
            doc.set_joystick_keys(j, KeyPreset::Arrows),
            Err(EditError::NoFreeKey)
        );
        assert_eq!(doc, before);
        // wasd again is a no-op, not an error (own keys)
        assert_eq!(doc.set_joystick_keys(j, KeyPreset::Wasd), Ok(()));
        let l = id_of(&doc, Kind::Look);
        assert_eq!(
            doc.set_joystick_keys(l, KeyPreset::Wasd),
            Err(EditError::WrongKind)
        );
    }
}
