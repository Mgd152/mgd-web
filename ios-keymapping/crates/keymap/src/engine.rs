//! The keymap engine: keyboard/mouse input in, multi-touch frames out.
//!
//! Pure logic. The caller supplies the clock (`now_ms`) and sends the frames.
//! One key press creates at most one contact going down; the only timed
//! behaviours are the fixed tap lift and the optional idle lift of the look
//! finger.

use crate::geom::View;
use crate::profile::{ButtonMode, Control, Profile, ProfileError};
use std::collections::{HashMap, HashSet};
use std::fmt;

/// Simultaneous contacts the phone's report can carry.
pub const MAX_CONTACTS: usize = 5;
/// How long a `tap` button stays down.
pub const TAP_HOLD_MS: u64 = 40;
/// Pause between a joystick's touch-down and its slide. Games with a floating
/// stick (Free Fire) put the stick where the finger lands; a slide in the same
/// instant lands first and the stick sees no movement (measured 2026-11-26).
pub const JOY_SETTLE_MS: u64 = 50;
/// The look finger does not land within this long after any lift. A lift and
/// a touch-down in the same game frame let Free Fire swap fingers (the camera
/// snapped back or drove the joystick, seen 2026-11-26).
pub const LOOK_REST_MS: u64 = 35;

/// One finger in HID panel space (0..=65535, native portrait panel).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Contact {
    pub id: u8,
    pub touching: bool,
    pub x: u16,
    pub y: u16,
}

/// A full snapshot: every contact that is down, plus contacts lifting now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frame {
    contacts: [Contact; MAX_CONTACTS],
    len: u8,
    transition: bool,
}

impl Frame {
    /// `None` if empty, more than `MAX_CONTACTS`, an id >= 5, or duplicate
    /// ids. Contacts are stored sorted by id; unused entries are
    /// `Contact::default()`.
    pub fn from_contacts(contacts: &[Contact], transition: bool) -> Option<Frame> {
        if contacts.is_empty() || contacts.len() > MAX_CONTACTS {
            return None;
        }
        let mut sorted = [Contact::default(); MAX_CONTACTS];
        sorted[..contacts.len()].copy_from_slice(contacts);
        sorted[..contacts.len()].sort_by_key(|c| c.id);
        for (i, c) in sorted[..contacts.len()].iter().enumerate() {
            if usize::from(c.id) >= MAX_CONTACTS || (i > 0 && sorted[i - 1].id == c.id) {
                return None;
            }
        }
        Some(Frame {
            contacts: sorted,
            len: contacts.len() as u8,
            transition,
        })
    }

    /// Ascending id, 1..=5 entries.
    pub fn contacts(&self) -> &[Contact] {
        &self.contacts[..usize::from(self.len)]
    }

    /// Some contact goes down or up in this frame.
    pub fn has_transition(&self) -> bool {
        self.transition
    }

    /// Pure-move frame: a sender may drop it if a later frame is already
    /// queued. Transition frames are never droppable.
    pub fn droppable(&self) -> bool {
        !self.transition
    }
}

pub enum Input<'a> {
    Press(&'a str),
    Release(&'a str),
    MouseMotion { dx: f64, dy: f64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineEvent {
    Activated,
    Deactivated,
    Panic,
    ActivateFailed(String),
}

#[derive(Debug, Default)]
pub struct Output {
    pub frames: Vec<Frame>,
    pub consumed: bool,
    pub event: Option<EngineEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivateError {
    NoView,
    OrientationMismatch {
        profile: crate::profile::Orientation,
        rotation: crate::geom::Rotation,
    },
}

impl fmt::Display for ActivateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ActivateError::NoView => write!(f, "no phone screen size yet (not connected?)"),
            ActivateError::OrientationMismatch { profile, rotation } => write!(
                f,
                "profile is {} but the picture is turned {:?}; rotate the view (F12) or use another profile",
                profile.as_str(),
                rotation
            ),
        }
    }
}

impl std::error::Error for ActivateError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Stats {
    pub frames: u64,
    pub dropped_presses: u64,
    pub look_recenters: u64,
}

#[derive(Debug, Clone)]
enum Ctl {
    Joy {
        keys: [bool; 4],
        id: Option<u8>,
        suppressed: bool,
        /// When the slide is due after a fresh touch-down.
        settle: Option<u64>,
    },
    Look {
        id: Option<u8>,
        offset: (f64, f64),
        last_ms: u64,
    },
    Btn {
        id: Option<u8>,
        deadline: Option<u64>,
    },
}

impl Ctl {
    fn fresh(c: &Control) -> Ctl {
        match c {
            Control::Joystick { .. } => Ctl::Joy {
                keys: [false; 4],
                id: None,
                suppressed: false,
                settle: None,
            },
            Control::Look { .. } => Ctl::Look {
                id: None,
                offset: (0.0, 0.0),
                last_ms: 0,
            },
            Control::Button { .. } => Ctl::Btn {
                id: None,
                deadline: None,
            },
        }
    }
}

pub struct Engine {
    profile: Profile,
    view: Option<View>,
    active: bool,
    now: u64,
    slots: [Option<(u16, u16)>; MAX_CONTACTS],
    ctl: Vec<Ctl>,
    /// key name -> (control index, role); role is 0..4 (up, down, left, right)
    /// for a joystick and 0 for a button.
    bindings: HashMap<String, (usize, usize)>,
    held: HashSet<String>,
    toggle_held: bool,
    look_idx: Option<usize>,
    /// Time of the last lift, for [`LOOK_REST_MS`].
    last_lift: Option<u64>,
    stats: Stats,
}

impl Engine {
    pub fn new(profile: Profile) -> Result<Engine, ProfileError> {
        profile.validate()?;
        let mut bindings = HashMap::new();
        let mut look_idx = None;
        for (i, c) in profile.controls.iter().enumerate() {
            match c {
                Control::Joystick {
                    up,
                    down,
                    left,
                    right,
                    ..
                } => {
                    for (role, k) in [up, down, left, right].into_iter().enumerate() {
                        bindings.insert(k.clone(), (i, role));
                    }
                }
                Control::Look { .. } => look_idx = Some(i),
                Control::Button { key, .. } => {
                    bindings.insert(key.clone(), (i, 0));
                }
            }
        }
        let ctl = profile.controls.iter().map(Ctl::fresh).collect();
        Ok(Engine {
            profile,
            view: None,
            active: false,
            now: 0,
            slots: [None; MAX_CONTACTS],
            ctl,
            bindings,
            held: HashSet::new(),
            toggle_held: false,
            look_idx,
            last_lift: None,
            stats: Stats::default(),
        })
    }

    pub fn profile(&self) -> &Profile {
        &self.profile
    }

    pub fn view(&self) -> Option<View> {
        self.view
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn stats(&self) -> Stats {
        self.stats
    }

    pub fn contacts_down(&self) -> usize {
        self.slots.iter().filter(|s| s.is_some()).count()
    }

    /// Change what the window shows. A changed view lifts every contact (their
    /// panel positions would be stale); if the engine is active and the
    /// profile orientation no longer matches, it also deactivates.
    pub fn set_view(&mut self, view: View) -> Vec<Frame> {
        if self.view == Some(view) {
            return Vec::new();
        }
        self.view = Some(view);
        let frame = self.lift_all();
        self.reset_controls();
        if self.active && !self.profile.orientation.matches(view.rotation) {
            self.active = false;
        }
        frame.into_iter().collect()
    }

    pub fn activate(&mut self) -> Result<(), ActivateError> {
        let view = self.view.ok_or(ActivateError::NoView)?;
        if !self.profile.orientation.matches(view.rotation) {
            return Err(ActivateError::OrientationMismatch {
                profile: self.profile.orientation,
                rotation: view.rotation,
            });
        }
        if !self.active {
            self.reset_controls();
        }
        self.active = true;
        Ok(())
    }

    /// Lift everything, forget held keys and deadlines, become inactive.
    pub fn deactivate(&mut self) -> Option<Frame> {
        let f = self.lift_all();
        self.reset_controls();
        self.active = false;
        f
    }

    /// Lift everything and forget held keys; the active state is unchanged.
    pub fn release_all(&mut self) -> Option<Frame> {
        let f = self.lift_all();
        self.reset_controls();
        f
    }

    /// Earliest pending deadline (tap lift or look idle lift).
    pub fn next_deadline(&self) -> Option<u64> {
        if !self.active {
            return None;
        }
        let settles = self.ctl.iter().filter_map(|c| match c {
            Ctl::Joy { settle, .. } => *settle,
            _ => None,
        });
        self.deadlines().into_iter().map(|(d, _)| d).chain(settles).min()
    }

    /// Perform every action whose deadline is `<= now_ms`. Lifts falling due
    /// together are merged into one frame.
    pub fn tick(&mut self, now_ms: u64) -> Vec<Frame> {
        self.now = self.now.max(now_ms);
        if !self.active {
            return Vec::new();
        }
        let now = self.now;
        let mut frames = self.settle_joysticks(now);
        let mut due: Vec<(u64, u8)> = self
            .deadlines()
            .into_iter()
            .filter(|(d, _)| *d <= now)
            .collect();
        if due.is_empty() {
            return frames;
        }
        due.sort_unstable();
        let ids: Vec<u8> = due.iter().map(|(_, id)| *id).collect();
        for c in &mut self.ctl {
            match c {
                Ctl::Btn { id, deadline }
                    if id.is_some_and(|n| ids.contains(&n)) && deadline.is_some() =>
                {
                    *id = None;
                    *deadline = None;
                }
                Ctl::Look { id, offset, .. } if id.is_some_and(|n| ids.contains(&n)) => {
                    *id = None;
                    *offset = (0.0, 0.0);
                }
                _ => {}
            }
        }
        frames.extend(self.lift(&ids));
        frames
    }

    pub fn handle(&mut self, input: Input<'_>, now_ms: u64) -> Output {
        let mut out = Output {
            frames: self.tick(now_ms),
            ..Output::default()
        };
        match input {
            Input::Press(k) => self.press(k, &mut out),
            Input::Release(k) => self.release(k, &mut out),
            Input::MouseMotion { dx, dy } => {
                if self.active {
                    out.consumed = true;
                    self.look_motion(dx, dy, &mut out.frames);
                }
            }
        }
        out
    }

    // ---- key handling -------------------------------------------------

    fn press(&mut self, k: &str, out: &mut Output) {
        if k == self.profile.panic_key {
            out.frames.extend(self.deactivate());
            out.consumed = true;
            out.event = Some(EngineEvent::Panic);
            return;
        }
        if k == self.profile.toggle_key {
            out.consumed = true;
            if self.toggle_held {
                return;
            }
            let was_active = self.active;
            if was_active {
                out.frames.extend(self.deactivate());
                out.event = Some(EngineEvent::Deactivated);
            } else {
                out.event = Some(match self.activate() {
                    Ok(()) => EngineEvent::Activated,
                    Err(e) => EngineEvent::ActivateFailed(e.to_string()),
                });
            }
            // Latch after (de)activation: those reset the latch, this press stays held.
            self.toggle_held = true;
            return;
        }
        if !self.active {
            return;
        }
        let Some(&(ci, role)) = self.bindings.get(k) else {
            return;
        };
        out.consumed = true;
        if !self.held.insert(k.to_string()) {
            return;
        }
        self.dispatch(ci, role, true, &mut out.frames);
    }

    fn release(&mut self, k: &str, out: &mut Output) {
        if k == self.profile.panic_key {
            out.consumed = true;
            return;
        }
        if k == self.profile.toggle_key {
            out.consumed = true;
            self.toggle_held = false;
            return;
        }
        if !self.active {
            return;
        }
        let Some(&(ci, role)) = self.bindings.get(k) else {
            return;
        };
        out.consumed = true;
        if !self.held.remove(k) {
            return;
        }
        self.dispatch(ci, role, false, &mut out.frames);
    }

    fn dispatch(&mut self, ci: usize, role: usize, pressed: bool, out: &mut Vec<Frame>) {
        match self.ctl[ci] {
            Ctl::Joy { .. } => self.joystick_key(ci, role, pressed, out),
            Ctl::Btn { .. } => self.button_key(ci, pressed, out),
            Ctl::Look { .. } => {}
        }
    }

    // ---- controls -----------------------------------------------------

    fn joystick_key(&mut self, ci: usize, role: usize, pressed: bool, out: &mut Vec<Frame>) {
        let (cx, cy, radius) = match &self.profile.controls[ci] {
            Control::Joystick { x, y, radius, .. } => (*x, *y, *radius),
            _ => return,
        };
        let (keys, id, suppressed, settle) = match &mut self.ctl[ci] {
            Ctl::Joy {
                keys,
                id,
                suppressed,
                settle,
            } => {
                keys[role] = pressed;
                (*keys, *id, *suppressed, *settle)
            }
            _ => return,
        };
        if !keys.iter().any(|k| *k) {
            if let Some(n) = id {
                out.extend(self.lift(&[n]));
            }
            if let Ctl::Joy {
                id,
                suppressed,
                settle,
                ..
            } = &mut self.ctl[ci]
            {
                *id = None;
                *suppressed = false;
                *settle = None;
            }
            return;
        }
        let n = match id {
            // The slide waits for the settle deadline; `tick` aims it at the keys held then.
            Some(_) if settle.is_some() => return,
            Some(n) => n,
            None => {
                if suppressed {
                    return;
                }
                let Some(n) = self.alloc() else {
                    self.stats.dropped_presses += 1;
                    if let Ctl::Joy { suppressed, .. } = &mut self.ctl[ci] {
                        *suppressed = true;
                    }
                    return;
                };
                let center = self.to_hid(cx, cy);
                out.push(self.down(n, center));
                let due = self.now.saturating_add(JOY_SETTLE_MS);
                if let Ctl::Joy { id, settle, .. } = &mut self.ctl[ci] {
                    *id = Some(n);
                    *settle = Some(due);
                }
                return;
            }
        };
        out.extend(self.joy_slide(n, keys, cx, cy, radius));
    }

    fn joy_slide(
        &mut self,
        n: u8,
        keys: [bool; 4],
        cx: f64,
        cy: f64,
        radius: f64,
    ) -> Option<Frame> {
        let (dx, dy) = (
            f64::from(u8::from(keys[3])) - f64::from(u8::from(keys[2])),
            f64::from(u8::from(keys[1])) - f64::from(u8::from(keys[0])),
        );
        let len = dx.hypot(dy);
        let (ux, uy) = if len > 0.0 {
            (dx / len, dy / len)
        } else {
            (0.0, 0.0)
        };
        let (fx, fy) = self.view_or_default().short_side_units(radius);
        let target = self.to_hid(cx + ux * fx, cy + uy * fy);
        self.move_to(n, target)
    }

    /// Slides of joysticks whose settle deadline is `<= now`.
    fn settle_joysticks(&mut self, now: u64) -> Vec<Frame> {
        let mut out = Vec::new();
        for ci in 0..self.ctl.len() {
            let (n, keys) = match &mut self.ctl[ci] {
                Ctl::Joy {
                    id: Some(n),
                    keys,
                    settle,
                    ..
                } if settle.is_some_and(|d| d <= now) => {
                    *settle = None;
                    (*n, *keys)
                }
                _ => continue,
            };
            if let Control::Joystick { x, y, radius, .. } = &self.profile.controls[ci] {
                let (x, y, radius) = (*x, *y, *radius);
                out.extend(self.joy_slide(n, keys, x, y, radius));
            }
        }
        out
    }

    fn button_key(&mut self, ci: usize, pressed: bool, out: &mut Vec<Frame>) {
        let (x, y, mode) = match &self.profile.controls[ci] {
            Control::Button { x, y, mode, .. } => (*x, *y, *mode),
            _ => return,
        };
        let id = match &self.ctl[ci] {
            Ctl::Btn { id, .. } => *id,
            _ => return,
        };
        if pressed {
            if id.is_some() {
                return; // tap contact still down: ignore
            }
            let Some(n) = self.alloc() else {
                self.stats.dropped_presses += 1;
                return;
            };
            let pos = self.to_hid(x, y);
            out.push(self.down(n, pos));
            let now = self.now;
            if let Ctl::Btn { id, deadline } = &mut self.ctl[ci] {
                *id = Some(n);
                *deadline = (mode == ButtonMode::Tap).then(|| now.saturating_add(TAP_HOLD_MS));
            }
        } else if let (ButtonMode::Hold, Some(n)) = (mode, id) {
            out.extend(self.lift(&[n]));
            if let Ctl::Btn { id, .. } = &mut self.ctl[ci] {
                *id = None;
            }
        }
    }

    fn look_motion(&mut self, dx: f64, dy: f64, out: &mut Vec<Frame>) {
        let Some(ci) = self.look_idx else { return };
        // Zero and non-finite deltas are not accepted motions: they emit
        // nothing, create no contact and do not postpone the look idle lift
        // (SPEC 5.6, Look rule 3).
        if !dx.is_finite() || !dy.is_finite() || (dx == 0.0 && dy == 0.0) {
            return;
        }
        let (ax, ay, sens, invert, reach) = match &self.profile.controls[ci] {
            Control::Look {
                x,
                y,
                sensitivity,
                invert_y,
                reach,
                ..
            } => (*x, *y, *sensitivity, *invert_y, *reach),
            _ => return,
        };
        let (cur_id, cur_off) = match &self.ctl[ci] {
            Ctl::Look { id, offset, .. } => (*id, *offset),
            _ => return,
        };
        let anchor = self.to_hid(ax, ay);
        let fresh = cur_id.is_none();
        // Resting after a lift: this motion is dropped (a few ms of mouse travel).
        if fresh
            && self
                .last_lift
                .is_some_and(|t| self.now < t.saturating_add(LOOK_REST_MS))
        {
            return;
        }
        let (n, offset) = match cur_id {
            Some(n) => (n, cur_off),
            None => {
                let Some(n) = self.alloc() else {
                    self.stats.dropped_presses += 1;
                    return;
                };
                out.push(self.down(n, anchor));
                (n, (0.0, 0.0))
            }
        };
        let sign = if invert { -1.0 } else { 1.0 };
        let step = (dx * sens / 1000.0, dy * sign * sens / 1000.0);
        let next = (offset.0 + step.0, offset.1 + step.1);
        let (id, new_offset) = if next.0.abs() <= reach && next.1.abs() <= reach {
            (n, next)
        } else if fresh {
            // Contact created by this very event: clamp, no phantom lift/re-down.
            (
                n,
                (next.0.clamp(-reach, reach), next.1.clamp(-reach, reach)),
            )
        } else {
            // Lift only; the next motion after LOOK_REST_MS lands at the anchor.
            out.extend(self.lift(&[n]));
            self.stats.look_recenters += 1;
            if let Ctl::Look { id, offset, .. } = &mut self.ctl[ci] {
                *id = None;
                *offset = (0.0, 0.0);
            }
            return;
        };
        let now = self.now;
        if let Ctl::Look {
            id: sid,
            offset,
            last_ms,
        } = &mut self.ctl[ci]
        {
            *sid = Some(id);
            *offset = new_offset;
            *last_ms = now;
        }
        let (ux, uy) = self.view_or_default().short_side_units(1.0);
        let pos = self.to_hid(ax + new_offset.0 * ux, ay + new_offset.1 * uy);
        out.extend(self.move_to(id, pos));
    }

    // ---- helpers ------------------------------------------------------

    fn view_or_default(&self) -> View {
        self.view.unwrap_or(View {
            rotation: crate::geom::Rotation::R0,
            aspect: 1.0,
        })
    }

    fn to_hid(&self, u: f64, v: f64) -> (u16, u16) {
        self.view_or_default().to_hid(u, v)
    }

    fn reset_controls(&mut self) {
        for (c, spec) in self.ctl.iter_mut().zip(&self.profile.controls) {
            *c = Ctl::fresh(spec);
        }
        self.held.clear();
        self.toggle_held = false;
    }

    fn deadlines(&self) -> Vec<(u64, u8)> {
        let mut v = Vec::new();
        for (c, spec) in self.ctl.iter().zip(&self.profile.controls) {
            match (c, spec) {
                (
                    Ctl::Btn {
                        id: Some(n),
                        deadline: Some(d),
                    },
                    _,
                ) => v.push((*d, *n)),
                (
                    Ctl::Look {
                        id: Some(n),
                        last_ms,
                        ..
                    },
                    Control::Look {
                        release_after_ms, ..
                    },
                ) if *release_after_ms > 0 => {
                    v.push((last_ms.saturating_add(u64::from(*release_after_ms)), *n));
                }
                _ => {}
            }
        }
        v
    }

    fn alloc(&self) -> Option<u8> {
        self.slots.iter().position(|s| s.is_none()).map(|i| i as u8)
    }

    fn down(&mut self, id: u8, pos: (u16, u16)) -> Frame {
        self.slots[usize::from(id)] = Some(pos);
        self.build(&[], true)
    }

    fn move_to(&mut self, id: u8, pos: (u16, u16)) -> Option<Frame> {
        if self.slots[usize::from(id)] == Some(pos) {
            return None;
        }
        self.slots[usize::from(id)] = Some(pos);
        Some(self.build(&[], false))
    }

    fn lift(&mut self, ids: &[u8]) -> Option<Frame> {
        let ids: Vec<u8> = ids
            .iter()
            .copied()
            .filter(|i| self.slots[usize::from(*i)].is_some())
            .collect();
        if ids.is_empty() {
            return None;
        }
        self.last_lift = Some(self.now);
        Some(self.build(&ids, true))
    }

    fn lift_all(&mut self) -> Option<Frame> {
        let ids: Vec<u8> = (0..MAX_CONTACTS as u8)
            .filter(|i| self.slots[usize::from(*i)].is_some())
            .collect();
        self.lift(&ids)
    }

    /// Snapshot of all down contacts plus `lifting` ones (then freed).
    fn build(&mut self, lifting: &[u8], transition: bool) -> Frame {
        let mut frame = Frame {
            contacts: [Contact::default(); MAX_CONTACTS],
            len: 0,
            transition,
        };
        for (i, slot) in self.slots.iter().enumerate() {
            if let Some((x, y)) = *slot {
                let touching = !lifting.contains(&(i as u8));
                frame.contacts[usize::from(frame.len)] = Contact {
                    id: i as u8,
                    touching,
                    x,
                    y,
                };
                frame.len += 1;
            }
        }
        for id in lifting {
            self.slots[usize::from(*id)] = None;
        }
        self.stats.frames += 1;
        frame
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::{Rotation, to_u16};
    use crate::profile::Orientation;

    const PANEL: (f64, f64) = (828.0, 1792.0);

    fn view(r: Rotation) -> View {
        View::from_panel(PANEL.0, PANEL.1, r)
    }

    fn engine(json: &str, rot: Rotation) -> Engine {
        let mut e = Engine::new(Profile::from_json(json).unwrap()).unwrap();
        e.set_view(view(rot));
        e
    }

    fn active(json: &str, rot: Rotation) -> Engine {
        let mut e = engine(json, rot);
        e.activate().unwrap();
        e
    }

    fn profile_json(orientation: &str, controls: &str) -> String {
        format!(r#"{{"version":1,"orientation":"{orientation}","controls":[{controls}]}}"#)
    }

    const JOY: &str = r#"{"type":"joystick","x":0.2,"y":0.75,"radius":0.12,"up":"KeyW","down":"KeyS","left":"KeyA","right":"KeyD"}"#;
    const LOOK: &str = r#"{"type":"look","x":0.7,"y":0.5,"sensitivity":1.0,"reach":0.1}"#;
    const BTN_HOLD: &str = r#"{"type":"button","x":0.85,"y":0.8,"key":"Space"}"#;
    const BTN_TAP: &str = r#"{"type":"button","x":0.85,"y":0.6,"key":"KeyJ","mode":"tap"}"#;

    fn press(e: &mut Engine, k: &str, t: u64) -> Output {
        e.handle(Input::Press(k), t)
    }
    fn release(e: &mut Engine, k: &str, t: u64) -> Output {
        e.handle(Input::Release(k), t)
    }
    fn mv(e: &mut Engine, dx: f64, dy: f64, t: u64) -> Output {
        e.handle(Input::MouseMotion { dx, dy }, t)
    }

    fn c(id: u8, touching: bool, p: (u16, u16)) -> Contact {
        Contact {
            id,
            touching,
            x: p.0,
            y: p.1,
        }
    }

    fn pic_hid(rot: Rotation, u: f64, v: f64) -> (u16, u16) {
        let (x, y) = rot.picture_to_panel(u, v);
        (to_u16(x), to_u16(y))
    }

    #[test]
    fn frame_from_contacts_rules() {
        assert!(Frame::from_contacts(&[], false).is_none());
        assert!(Frame::from_contacts(&[c(5, true, (0, 0))], false).is_none());
        assert!(Frame::from_contacts(&[c(1, true, (0, 0)), c(1, false, (0, 0))], true).is_none());
        let six: Vec<Contact> = (0..6).map(|i| c(i, true, (0, 0))).collect();
        assert!(Frame::from_contacts(&six, false).is_none());
        let f = Frame::from_contacts(&[c(3, true, (1, 2)), c(0, true, (3, 4))], false).unwrap();
        assert_eq!(f.contacts()[0].id, 0);
        assert_eq!(f.contacts()[1].id, 3);
        assert!(f.droppable() && !f.has_transition());
        let g = Frame::from_contacts(&[c(0, false, (0, 0))], true).unwrap();
        assert!(!g.droppable() && g.has_transition());
        // Copy + PartialEq are well defined.
        let h = f;
        assert_eq!(f, h);
    }

    #[test]
    fn activation_errors() {
        let json = profile_json("portrait", JOY);
        let mut e = Engine::new(Profile::from_json(&json).unwrap()).unwrap();
        assert_eq!(e.activate(), Err(ActivateError::NoView));
        let o = press(&mut e, "Backquote", 0);
        assert!(o.consumed);
        assert!(matches!(o.event, Some(EngineEvent::ActivateFailed(_))));
        assert!(!e.is_active());
        release(&mut e, "Backquote", 1);
        e.set_view(view(Rotation::R90));
        assert!(matches!(
            e.activate(),
            Err(ActivateError::OrientationMismatch { .. })
        ));
        let msg = e.activate().unwrap_err().to_string();
        assert!(msg.contains("portrait"));
        e.set_view(view(Rotation::R0));
        assert!(e.activate().is_ok());
        assert!(e.activate().is_ok()); // idempotent
    }

    #[test]
    fn engine_new_validates() {
        let mut p = Profile::from_json(&profile_json("portrait", JOY)).unwrap();
        p.version = 9;
        assert!(Engine::new(p).is_err());
    }

    #[test]
    fn toggle_on_off() {
        let mut e = engine(&profile_json("portrait", BTN_HOLD), Rotation::R0);
        let o = press(&mut e, "Backquote", 0);
        assert_eq!(o.event, Some(EngineEvent::Activated));
        assert!(o.consumed && o.frames.is_empty() && e.is_active());
        release(&mut e, "Backquote", 5);
        press(&mut e, "Space", 10);
        assert_eq!(e.contacts_down(), 1);
        release(&mut e, "Backquote", 15);
        let o = press(&mut e, "Backquote", 20);
        assert_eq!(o.event, Some(EngineEvent::Deactivated));
        assert_eq!(o.frames.len(), 1);
        assert!(!o.frames[0].contacts()[0].touching);
        assert!(!e.is_active());
        assert_eq!(e.contacts_down(), 0);
    }

    #[test]
    fn toggle_auto_repeat_is_ignored() {
        let mut e = engine(&profile_json("portrait", BTN_HOLD), Rotation::R0);
        press(&mut e, "Backquote", 0);
        let o = press(&mut e, "Backquote", 10);
        assert!(o.consumed && o.event.is_none());
        assert!(e.is_active());
    }

    #[test]
    fn inactive_and_unbound_input_not_consumed() {
        let mut e = engine(
            &profile_json("portrait", &format!("{JOY},{LOOK},{BTN_HOLD}")),
            Rotation::R0,
        );
        for i in [
            Input::Press("KeyW"),
            Input::Release("KeyW"),
            Input::MouseMotion { dx: 5.0, dy: 5.0 },
        ] {
            let o = e.handle(i, 0);
            assert!(!o.consumed && o.frames.is_empty() && o.event.is_none());
        }
        e.activate().unwrap();
        let o = press(&mut e, "KeyQ", 1);
        assert!(!o.consumed && o.frames.is_empty());
        let o = release(&mut e, "KeyQ", 2);
        assert!(!o.consumed);
        let o = press(&mut e, "NotAKey", 3);
        assert!(!o.consumed);
    }

    #[test]
    fn joystick_w_goes_down_then_moves() {
        let mut e = active(&profile_json("portrait", JOY), Rotation::R0);
        let o = press(&mut e, "KeyW", 0);
        assert!(o.consumed);
        assert_eq!(o.frames.len(), 1);
        // Center: (0.2, 0.75) of 65535, rounded.
        assert_eq!(o.frames[0].contacts(), &[c(0, true, (13107, 49151))]);
        assert!(o.frames[0].has_transition());
        // The slide waits JOY_SETTLE_MS.
        assert_eq!(e.next_deadline(), Some(JOY_SETTLE_MS));
        assert!(e.tick(JOY_SETTLE_MS - 1).is_empty());
        let f = e.tick(JOY_SETTLE_MS);
        assert_eq!(f.len(), 1);
        // Up by radius 0.12 of the short (width) side: 0.12 * 828 px up.
        let dy = 0.12 * 828.0 / 1792.0;
        let want = (13107u16, to_u16(0.75 - dy));
        assert_eq!(f[0].contacts(), &[c(0, true, want)]);
        assert!(f[0].droppable());
        assert_eq!(want, (13107, 45518));
        assert_eq!(e.next_deadline(), None);
    }

    #[test]
    fn joystick_settle_aims_at_keys_held_then_and_release_lifts_at_center() {
        let mut e = active(&profile_json("portrait", JOY), Rotation::R0);
        press(&mut e, "KeyW", 0);
        // Keys changed while settling: no slide yet, the slide aims at S.
        assert!(press(&mut e, "KeyS", 10).frames.is_empty());
        assert!(release(&mut e, "KeyW", 20).frames.is_empty());
        let f = e.tick(JOY_SETTLE_MS);
        assert!(f[0].contacts()[0].y > 49151);
        // Released before the slide: lifted at the center, no late slide.
        let mut e = active(&profile_json("portrait", JOY), Rotation::R0);
        press(&mut e, "KeyW", 0);
        let o = release(&mut e, "KeyW", 5);
        assert_eq!(o.frames[0].contacts(), &[c(0, false, (13107, 49151))]);
        assert!(e.tick(JOY_SETTLE_MS).is_empty());
        assert_eq!(e.next_deadline(), None);
    }

    #[test]
    fn joystick_diagonal_length_equals_radius() {
        let mut e = active(&profile_json("portrait", JOY), Rotation::R0);
        press(&mut e, "KeyW", 0);
        e.tick(JOY_SETTLE_MS);
        let o = press(&mut e, "KeyD", JOY_SETTLE_MS + 1);
        assert_eq!(o.frames.len(), 1);
        let p = o.frames[0].contacts()[0];
        let dx = (f64::from(p.x) - 13107.0) / 65535.0 * PANEL.0;
        let dy = (f64::from(p.y) - 49151.0) / 65535.0 * PANEL.1;
        assert!(dx > 0.0 && dy < 0.0);
        assert!(
            (dx.hypot(dy) - 0.12 * PANEL.0).abs() < 1.0,
            "len {}",
            dx.hypot(dy)
        );
    }

    #[test]
    fn joystick_opposite_keys_return_to_center_without_lift() {
        let mut e = active(&profile_json("portrait", JOY), Rotation::R0);
        press(&mut e, "KeyW", 0);
        e.tick(JOY_SETTLE_MS);
        let o = press(&mut e, "KeyS", JOY_SETTLE_MS + 1);
        assert_eq!(o.frames.len(), 1);
        assert_eq!(o.frames[0].contacts(), &[c(0, true, (13107, 49151))]);
        assert_eq!(e.contacts_down(), 1);
        // Releasing one of the cancelling pair moves the other way.
        let o = release(&mut e, "KeyW", JOY_SETTLE_MS + 2);
        assert_eq!(o.frames.len(), 1);
        assert!(o.frames[0].contacts()[0].y > 49151);
        let o = release(&mut e, "KeyS", JOY_SETTLE_MS + 3);
        assert_eq!(o.frames.len(), 1);
        assert!(o.frames[0].has_transition());
        assert!(!o.frames[0].contacts()[0].touching);
        assert_eq!(e.contacts_down(), 0);
    }

    #[test]
    fn joystick_last_release_lifts_at_last_position() {
        let mut e = active(&profile_json("portrait", JOY), Rotation::R0);
        press(&mut e, "KeyA", 0);
        e.tick(JOY_SETTLE_MS);
        let o = release(&mut e, "KeyA", JOY_SETTLE_MS + 1);
        assert_eq!(o.frames.len(), 1);
        let p = o.frames[0].contacts()[0];
        assert!(!p.touching);
        assert!(p.x < 13107);
        assert_eq!(p.y, 49151);
    }

    #[test]
    fn joystick_key_repeat_and_stale_release_ignored() {
        let mut e = active(&profile_json("portrait", JOY), Rotation::R0);
        press(&mut e, "KeyW", 0);
        let o = press(&mut e, "KeyW", 1);
        assert!(o.consumed && o.frames.is_empty());
        let o = release(&mut e, "KeyD", 2);
        assert!(o.consumed && o.frames.is_empty());
    }

    #[test]
    fn joystick_under_all_rotations() {
        for (rot, orient) in [
            (Rotation::R0, "portrait"),
            (Rotation::R180, "portrait"),
            (Rotation::R90, "landscape"),
            (Rotation::R270, "landscape"),
        ] {
            let mut e = active(&profile_json(orient, JOY), rot);
            let o = press(&mut e, "KeyW", 0);
            assert_eq!(
                o.frames[0].contacts()[0].x,
                pic_hid(rot, 0.2, 0.75).0,
                "{rot:?}"
            );
            assert_eq!(
                (o.frames[0].contacts()[0].x, o.frames[0].contacts()[0].y),
                pic_hid(rot, 0.2, 0.75),
                "{rot:?} center"
            );
            let v = view(rot);
            let (_, fy) = v.short_side_units(0.12);
            let s = e.tick(JOY_SETTLE_MS);
            assert_eq!(
                (s[0].contacts()[0].x, s[0].contacts()[0].y),
                pic_hid(rot, 0.2, 0.75 - fy),
                "{rot:?} up"
            );
        }
    }

    #[test]
    fn joystick_radius_is_round_in_landscape() {
        // Landscape R90: short side is the screen's panel width.
        let json = profile_json("landscape", JOY);
        let mut e = active(&json, Rotation::R90);
        press(&mut e, "KeyD", 0);
        e.tick(JOY_SETTLE_MS);
        let o = e.handle(Input::Press("KeyD"), JOY_SETTLE_MS + 1); // repeat, ignored
        assert!(o.frames.is_empty());
        let center = pic_hid(Rotation::R90, 0.2, 0.75);
        let right = e.slots[0].unwrap();
        // Picture right is panel -y direction under R90 (x_panel = v, y_panel = 1-u).
        let moved_px = (f64::from(center.1) - f64::from(right.1)) / 65535.0 * PANEL.1;
        // Short side of the landscape picture is 828 px.
        assert!((moved_px - 0.12 * PANEL.0).abs() < 1.0, "{moved_px}");
        assert_eq!(right.0, center.0);
    }

    #[test]
    fn look_first_motion_down_then_move() {
        let mut e = active(&profile_json("portrait", LOOK), Rotation::R0);
        let o = mv(&mut e, 20.0, -10.0, 0);
        assert!(o.consumed);
        assert_eq!(o.frames.len(), 2);
        assert_eq!(
            o.frames[0].contacts(),
            &[c(0, true, pic_hid(Rotation::R0, 0.7, 0.5))]
        );
        assert!(o.frames[0].has_transition());
        // sensitivity 1.0 -> 20/1000 = 0.02 short-side units.
        let (ux, uy) = view(Rotation::R0).short_side_units(1.0);
        let want = pic_hid(Rotation::R0, 0.7 + 0.02 * ux, 0.5 - 0.01 * uy);
        assert_eq!(o.frames[1].contacts(), &[c(0, true, want)]);
        assert!(o.frames[1].droppable());
    }

    #[test]
    fn look_sensitivity_and_invert() {
        let j = r#"{"type":"look","x":0.5,"y":0.5,"sensitivity":2.0,"reach":0.5,"invert_y":true}"#;
        let mut e = active(&profile_json("portrait", j), Rotation::R0);
        let o = mv(&mut e, 10.0, 10.0, 0);
        let (ux, uy) = view(Rotation::R0).short_side_units(1.0);
        let want = pic_hid(Rotation::R0, 0.5 + 0.02 * ux, 0.5 - 0.02 * uy);
        assert_eq!(o.frames[1].contacts()[0], c(0, true, want));
        // Accumulates.
        let o = mv(&mut e, 10.0, 10.0, 1);
        assert_eq!(o.frames.len(), 1);
        let want = pic_hid(Rotation::R0, 0.5 + 0.04 * ux, 0.5 - 0.04 * uy);
        assert_eq!(o.frames[0].contacts()[0], c(0, true, want));
    }

    #[test]
    fn look_recenter_order_and_counter() {
        let mut e = active(&profile_json("portrait", LOOK), Rotation::R0); // reach 0.1
        mv(&mut e, 80.0, 0.0, 0);
        assert_eq!(e.stats().look_recenters, 0);
        let o = mv(&mut e, 40.0, 0.0, 1); // 0.08 + 0.04 > 0.1
        let anchor = pic_hid(Rotation::R0, 0.7, 0.5);
        // Lift at the last position only; no touch-down in the same instant.
        assert_eq!(o.frames.len(), 1);
        assert!(!o.frames[0].contacts()[0].touching);
        assert_ne!(
            (o.frames[0].contacts()[0].x, o.frames[0].contacts()[0].y),
            anchor
        );
        assert_eq!(e.stats().look_recenters, 1);
        assert_eq!(e.contacts_down(), 0);
        // Resting: motion is dropped until LOOK_REST_MS after the lift.
        assert!(mv(&mut e, 40.0, 0.0, LOOK_REST_MS).frames.is_empty());
        // Then down at the anchor and move by the step.
        let o = mv(&mut e, 40.0, 0.0, 1 + LOOK_REST_MS);
        assert_eq!(o.frames.len(), 2);
        assert_eq!(o.frames[0].contacts(), &[c(0, true, anchor)]);
        assert!(o.frames[0].has_transition() && o.frames[1].droppable());
        let (ux, _) = view(Rotation::R0).short_side_units(1.0);
        assert_eq!(
            o.frames[1].contacts()[0].x,
            pic_hid(Rotation::R0, 0.7 + 0.04 * ux, 0.5).0
        );
        assert_eq!(e.contacts_down(), 1);
    }

    #[test]
    fn look_rests_after_another_controls_lift() {
        let all = format!("{LOOK},{BTN_TAP}");
        let mut e = active(&profile_json("portrait", &all), Rotation::R0);
        press(&mut e, "KeyJ", 0);
        release(&mut e, "KeyJ", 1);
        assert_eq!(e.tick(TAP_HOLD_MS).len(), 1); // tap lifts at 40
        assert!(mv(&mut e, 5.0, 0.0, TAP_HOLD_MS + 1).frames.is_empty());
        assert_eq!(mv(&mut e, 5.0, 0.0, TAP_HOLD_MS + LOOK_REST_MS).frames.len(), 2);
    }

    #[test]
    fn look_huge_step_is_clamped_to_reach() {
        let mut e = active(&profile_json("portrait", LOOK), Rotation::R0);
        let o = mv(&mut e, 100_000.0, 0.0, 0);
        // down at anchor, then move to the clamped edge; no phantom re-center.
        assert_eq!(o.frames.len(), 2);
        assert_eq!(e.stats().look_recenters, 0);
        let (ux, _) = view(Rotation::R0).short_side_units(1.0);
        let last = o.frames.last().unwrap().contacts()[0];
        assert_eq!(last.x, pic_hid(Rotation::R0, 0.7 + 0.1 * ux, 0.5).0);
    }

    const EDGE_LOOK: &str = r#"{"type":"look","x":1.0,"y":1.0,"sensitivity":1.0,"reach":0.1}"#;

    #[test]
    fn no_move_frame_when_hid_position_unchanged() {
        // (a) sub-unit motion after the contact is down.
        let mut e = active(&profile_json("portrait", LOOK), Rotation::R0);
        mv(&mut e, 10.0, 0.0, 0);
        assert!(mv(&mut e, 1e-6, 0.0, 1).frames.is_empty());
        // (b) look anchored at the right edge pushed further right (clamped).
        let mut e = active(&profile_json("portrait", EDGE_LOOK), Rotation::R0);
        mv(&mut e, 0.0, 1.0, 0); // contact down
        mv(&mut e, 50.0, 0.0, 1);
        assert!(mv(&mut e, 20.0, 0.0, 2).frames.is_empty());
        // (c) joystick at the left edge: `left` clamps to the center, so only the
        // touch-down is emitted; adding `right` (cancel) changes nothing either.
        let edge_joy = r#"{"type":"joystick","x":0.0,"y":0.5,"radius":0.1,"up":"KeyW","down":"KeyS","left":"KeyA","right":"KeyD"}"#;
        let mut e = active(&profile_json("portrait", edge_joy), Rotation::R0);
        assert_eq!(press(&mut e, "KeyA", 0).frames.len(), 1);
        assert!(e.tick(JOY_SETTLE_MS).is_empty());
        assert!(press(&mut e, "KeyD", JOY_SETTLE_MS + 1).frames.is_empty());
        assert!(release(&mut e, "KeyD", JOY_SETTLE_MS + 2).frames.is_empty());
        assert_eq!(press(&mut e, "KeyW", JOY_SETTLE_MS + 3).frames.len(), 1); // up-left: y moves
    }

    #[test]
    fn extreme_now_ms_does_not_overflow() {
        let mut e = active(&profile_json("portrait", BTN_TAP), Rotation::R0);
        let o = press(&mut e, "KeyJ", u64::MAX);
        assert_eq!(o.frames.len(), 1);
        assert_eq!(e.next_deadline(), Some(u64::MAX));
        let look = r#"{"type":"look","x":0.5,"y":0.5,"reach":0.5,"release_after_ms":50}"#;
        let mut e = active(&profile_json("portrait", look), Rotation::R0);
        mv(&mut e, 0.1, 0.0, u64::MAX - 10);
        assert_eq!(e.next_deadline(), Some(u64::MAX));
        assert_eq!(e.tick(u64::MAX).len(), 1);
    }

    #[test]
    fn toggle_latch_is_cleared_by_deactivate_and_focus_loss() {
        let mut e = engine(&profile_json("portrait", BTN_HOLD), Rotation::R0);
        let o = press(&mut e, "Backquote", 0);
        assert_eq!(o.event, Some(EngineEvent::Activated));
        // The release never arrives (focus lost), then the app deactivates.
        e.deactivate();
        let o = press(&mut e, "Backquote", 10);
        assert_eq!(o.event, Some(EngineEvent::Activated));
        // Same for release_all: the next press toggles (active -> inactive).
        e.release_all();
        let o = press(&mut e, "Backquote", 20);
        assert_eq!(o.event, Some(EngineEvent::Deactivated));
    }

    #[test]
    fn look_idle_release() {
        let j = r#"{"type":"look","x":0.5,"y":0.5,"reach":0.5,"release_after_ms":100}"#;
        let mut e = active(&profile_json("portrait", j), Rotation::R0);
        assert_eq!(e.next_deadline(), None);
        mv(&mut e, 10.0, 0.0, 1000);
        assert_eq!(e.next_deadline(), Some(1100));
        mv(&mut e, 10.0, 0.0, 1050); // refreshes
        assert_eq!(e.next_deadline(), Some(1150));
        assert!(e.tick(1149).is_empty());
        // Zero and non-finite deltas are not accepted motions: they do not
        // postpone the idle lift.
        mv(&mut e, 0.0, 0.0, 1100);
        mv(&mut e, f64::NAN, 1.0, 1120);
        mv(&mut e, 1.0, f64::INFINITY, 1140);
        assert_eq!(e.next_deadline(), Some(1150));
        let f = e.tick(1150);
        assert_eq!(f.len(), 1);
        assert!(!f[0].contacts()[0].touching && f[0].has_transition());
        assert_eq!(e.next_deadline(), None);
        assert_eq!(e.contacts_down(), 0);
        // A new motion starts a fresh contact.
        let o = mv(&mut e, 10.0, 0.0, 2000);
        assert_eq!(o.frames.len(), 2);
    }

    #[test]
    fn look_without_idle_release_never_lifts() {
        let mut e = active(&profile_json("portrait", LOOK), Rotation::R0);
        mv(&mut e, 10.0, 0.0, 0);
        assert!(e.tick(10_000_000).is_empty());
        assert_eq!(e.contacts_down(), 1);
    }

    #[test]
    fn look_zero_nan_and_no_look_control() {
        let mut e = active(&profile_json("portrait", LOOK), Rotation::R0);
        let o = mv(&mut e, 0.0, 0.0, 0);
        assert!(o.consumed && o.frames.is_empty() && e.contacts_down() == 0);
        let o = mv(&mut e, f64::NAN, 3.0, 1);
        assert!(o.consumed && o.frames.is_empty() && e.contacts_down() == 0);
        let o = mv(&mut e, 1.0, f64::INFINITY, 2);
        assert!(o.consumed && o.frames.is_empty());
        mv(&mut e, 10.0, 0.0, 3);
        let o = mv(&mut e, 0.0, 0.0, 4);
        assert!(o.frames.is_empty());
        let mut e = active(&profile_json("portrait", BTN_HOLD), Rotation::R0);
        let o = mv(&mut e, 10.0, 10.0, 0);
        assert!(o.consumed && o.frames.is_empty());
    }

    #[test]
    fn button_hold() {
        let mut e = active(&profile_json("portrait", BTN_HOLD), Rotation::R0);
        let o = press(&mut e, "Space", 0);
        assert_eq!(o.frames.len(), 1);
        assert_eq!(
            o.frames[0].contacts(),
            &[c(0, true, pic_hid(Rotation::R0, 0.85, 0.8))]
        );
        assert!(o.frames[0].has_transition());
        assert_eq!(e.next_deadline(), None);
        assert!(e.tick(5000).is_empty());
        let o = release(&mut e, "Space", 5001);
        assert_eq!(o.frames.len(), 1);
        assert!(!o.frames[0].contacts()[0].touching);
        assert_eq!(e.contacts_down(), 0);
    }

    #[test]
    fn button_tap_lifts_at_exactly_40ms() {
        let mut e = active(&profile_json("portrait", BTN_TAP), Rotation::R0);
        let o = press(&mut e, "KeyJ", 100);
        assert_eq!(o.frames.len(), 1);
        assert_eq!(e.next_deadline(), Some(140));
        assert!(e.tick(139).is_empty());
        let f = e.tick(140);
        assert_eq!(f.len(), 1);
        assert!(!f[0].contacts()[0].touching);
        assert_eq!(e.next_deadline(), None);
        assert_eq!(e.contacts_down(), 0);
    }

    #[test]
    fn tap_early_release_changes_nothing_and_held_key_never_repeats() {
        let mut e = active(&profile_json("portrait", BTN_TAP), Rotation::R0);
        press(&mut e, "KeyJ", 0);
        let o = release(&mut e, "KeyJ", 10);
        assert!(o.consumed && o.frames.is_empty());
        assert_eq!(e.contacts_down(), 1);
        assert_eq!(e.next_deadline(), Some(40));
        // Press again while the tap contact is still down: ignored, not dropped.
        let o = press(&mut e, "KeyJ", 20);
        assert!(o.frames.is_empty());
        assert_eq!(e.stats().dropped_presses, 0);
        release(&mut e, "KeyJ", 25);
        assert_eq!(e.tick(40).len(), 1);
        // Held: lifts at +40 even though the key is down, no second tap.
        press(&mut e, "KeyJ", 100);
        assert_eq!(e.tick(140).len(), 1);
        let o = press(&mut e, "KeyJ", 200); // auto-repeat, still held
        assert!(o.frames.is_empty());
        assert_eq!(e.contacts_down(), 0);
        assert_eq!(e.next_deadline(), None);
    }

    #[test]
    fn handle_at_later_time_lifts_first() {
        let mut e = active(
            &profile_json("portrait", &format!("{BTN_TAP},{BTN_HOLD}")),
            Rotation::R0,
        );
        press(&mut e, "KeyJ", 0);
        let o = press(&mut e, "Space", 500);
        assert_eq!(o.frames.len(), 2);
        assert!(!o.frames[0].contacts()[0].touching); // tap lift came first
        assert_eq!(o.frames[1].contacts().len(), 1);
        assert!(o.frames[1].contacts()[0].touching);
        assert_eq!(o.frames[1].contacts()[0].id, 0); // id 0 reused after lift frame
    }

    #[test]
    fn two_taps_due_together_merge() {
        let b2 = r#"{"type":"button","x":0.5,"y":0.5,"key":"KeyK","mode":"tap"}"#;
        let mut e = active(
            &profile_json("portrait", &format!("{BTN_TAP},{b2}")),
            Rotation::R0,
        );
        press(&mut e, "KeyJ", 0);
        press(&mut e, "KeyK", 10);
        let f = e.tick(1000);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].contacts().len(), 2);
        assert!(f[0].contacts().iter().all(|k| !k.touching));
        // Due at different times but ticked in one go -> still one frame.
    }

    #[test]
    fn slot_exhaustion_with_six_buttons() {
        let keys = ["KeyJ", "KeyK", "KeyL", "KeyU", "KeyI", "KeyO"];
        let ctl: Vec<String> = keys
            .iter()
            .enumerate()
            .map(|(i, k)| {
                format!(
                    r#"{{"type":"button","x":{},"y":0.5,"key":"{k}"}}"#,
                    0.1 + 0.1 * i as f64
                )
            })
            .collect();
        let mut e = active(&profile_json("portrait", &ctl.join(",")), Rotation::R0);
        for (i, k) in keys.iter().take(5).enumerate() {
            let o = press(&mut e, k, i as u64);
            assert_eq!(o.frames.len(), 1);
        }
        assert_eq!(e.contacts_down(), 5);
        let o = press(&mut e, "KeyO", 10);
        assert!(o.consumed && o.frames.is_empty());
        assert_eq!(e.stats().dropped_presses, 1);
        let o = release(&mut e, "KeyO", 11);
        assert!(o.consumed && o.frames.is_empty());
        assert_eq!(e.contacts_down(), 5);
        // Others still release normally and the ids are 0..5.
        let o = release(&mut e, "KeyK", 12);
        assert_eq!(o.frames.len(), 1);
        assert_eq!(o.frames[0].contacts().len(), 5);
        assert_eq!(
            o.frames[0]
                .contacts()
                .iter()
                .filter(|k| !k.touching)
                .count(),
            1
        );
        // The dropped key can go down after its fresh press.
        let o = press(&mut e, "KeyO", 13);
        assert_eq!(o.frames.len(), 1);
        assert_eq!(e.contacts_down(), 5);
    }

    #[test]
    fn slot_exhaustion_joystick_and_look() {
        let keys = ["KeyJ", "KeyK", "KeyL", "KeyU", "KeyI"];
        let btns: Vec<String> = keys
            .iter()
            .map(|k| format!(r#"{{"type":"button","x":0.5,"y":0.5,"key":"{k}"}}"#))
            .collect();
        let all = format!("{JOY},{LOOK},{}", btns.join(","));
        let mut e = active(&profile_json("portrait", &all), Rotation::R0);
        for (i, k) in keys.iter().enumerate() {
            press(&mut e, k, i as u64);
        }
        let o = press(&mut e, "KeyW", 10);
        assert!(o.frames.is_empty());
        assert_eq!(e.stats().dropped_presses, 1);
        // Free a slot; the joystick must not start mid-hold.
        release(&mut e, "KeyJ", 11);
        let o = press(&mut e, "KeyD", 12);
        assert!(o.frames.is_empty());
        assert_eq!(e.stats().dropped_presses, 1);
        release(&mut e, "KeyW", 13);
        release(&mut e, "KeyD", 14);
        // New hold episode works.
        let o = press(&mut e, "KeyW", 15);
        assert_eq!(o.frames.len(), 1);
        assert_eq!(e.tick(15 + JOY_SETTLE_MS).len(), 1);
        // Look with no free slot is dropped per motion event.
        let o = mv(&mut e, 5.0, 5.0, 16);
        assert!(o.frames.is_empty());
        assert_eq!(e.stats().dropped_presses, 2);
        mv(&mut e, 5.0, 5.0, 17);
        assert_eq!(e.stats().dropped_presses, 3);
    }

    #[test]
    fn id_reuse_takes_lowest_free_id() {
        let b2 = r#"{"type":"button","x":0.5,"y":0.5,"key":"KeyK"}"#;
        let b3 = r#"{"type":"button","x":0.4,"y":0.5,"key":"KeyL"}"#;
        let mut e = active(
            &profile_json("portrait", &format!("{BTN_HOLD},{b2},{b3}")),
            Rotation::R0,
        );
        press(&mut e, "Space", 0);
        press(&mut e, "KeyK", 1);
        press(&mut e, "KeyL", 2);
        release(&mut e, "KeyK", 3); // frees id 1
        let o = release(&mut e, "Space", 4); // frees id 0
        assert_eq!(o.frames[0].contacts().len(), 2);
        let o = press(&mut e, "KeyK", 5);
        let ids: Vec<u8> = o.frames[0].contacts().iter().map(|k| k.id).collect();
        assert_eq!(ids, vec![0, 2]);
    }

    #[test]
    fn panic_from_active_and_inactive() {
        let mut e = active(
            &profile_json("portrait", &format!("{JOY},{BTN_HOLD}")),
            Rotation::R0,
        );
        press(&mut e, "KeyW", 0);
        press(&mut e, "Space", 1);
        let o = press(&mut e, "F8", 2);
        assert_eq!(o.event, Some(EngineEvent::Panic));
        assert!(o.consumed && !e.is_active() && e.contacts_down() == 0);
        assert_eq!(o.frames.len(), 1);
        assert_eq!(o.frames[0].contacts().len(), 2);
        assert!(o.frames[0].contacts().iter().all(|k| !k.touching));
        let o = press(&mut e, "F8", 3);
        assert_eq!(o.event, Some(EngineEvent::Panic));
        assert!(o.frames.is_empty() && o.consumed);
        let o = release(&mut e, "F8", 4);
        assert!(o.consumed && o.event.is_none());
    }

    #[test]
    fn release_all_keeps_active_and_ignores_stale_releases() {
        let mut e = active(
            &profile_json("portrait", &format!("{JOY},{BTN_HOLD}")),
            Rotation::R0,
        );
        press(&mut e, "KeyW", 0);
        press(&mut e, "Space", 1);
        let f = e.release_all().unwrap();
        assert_eq!(f.contacts().len(), 2);
        assert!(e.is_active() && e.contacts_down() == 0);
        assert!(e.release_all().is_none());
        // The keys are still physically held: stale releases do nothing,
        // repeats do nothing, a fresh press works.
        assert!(release(&mut e, "KeyW", 2).frames.is_empty());
        assert!(release(&mut e, "Space", 3).frames.is_empty());
        let o = press(&mut e, "Space", 4);
        assert_eq!(o.frames.len(), 1);
    }

    #[test]
    fn deactivate_is_idempotent() {
        let mut e = active(&profile_json("portrait", BTN_HOLD), Rotation::R0);
        press(&mut e, "Space", 0);
        assert!(e.deactivate().is_some());
        assert!(e.deactivate().is_none());
        assert!(!e.is_active());
        // Inactive again: no frames from input, no deadlines.
        assert!(!press(&mut e, "Space", 1).consumed);
        assert_eq!(e.next_deadline(), None);
        assert!(e.tick(100).is_empty());
    }

    #[test]
    fn set_view_cases() {
        let mut e = active(
            &profile_json("portrait", &format!("{JOY},{BTN_HOLD}")),
            Rotation::R0,
        );
        assert!(e.set_view(view(Rotation::R0)).is_empty()); // same view
        press(&mut e, "KeyW", 0);
        // Rotated, same orientation: lift only.
        let f = e.set_view(view(Rotation::R180));
        assert_eq!(f.len(), 1);
        assert!(f[0].contacts().iter().all(|k| !k.touching));
        assert!(e.is_active() && e.contacts_down() == 0);
        // Stale release of the old hold is harmless.
        assert!(release(&mut e, "KeyW", 1).frames.is_empty());
        // Other orientation: lift + inactive.
        press(&mut e, "Space", 2);
        let f = e.set_view(view(Rotation::R90));
        assert_eq!(f.len(), 1);
        assert!(!e.is_active());
        assert_eq!(e.contacts_down(), 0);
        assert!(matches!(
            e.activate(),
            Err(ActivateError::OrientationMismatch { .. })
        ));
        // Changed aspect with same rotation also counts as changed.
        e.set_view(view(Rotation::R0));
        e.activate().unwrap();
        press(&mut e, "Space", 3);
        let f = e.set_view(View::from_panel(750.0, 1334.0, Rotation::R0));
        assert_eq!(f.len(), 1);
        assert_eq!(e.view().unwrap().rotation, Rotation::R0);
    }

    #[test]
    fn set_view_inactive_with_nothing_down() {
        let mut e = engine(&profile_json("portrait", BTN_HOLD), Rotation::R0);
        assert!(e.set_view(view(Rotation::R180)).is_empty());
        assert!(!e.is_active());
    }

    #[test]
    fn repeat_press_is_ignored() {
        let mut e = active(&profile_json("portrait", BTN_HOLD), Rotation::R0);
        press(&mut e, "Space", 0);
        let o = press(&mut e, "Space", 1);
        assert!(o.consumed && o.frames.is_empty());
        assert_eq!(e.contacts_down(), 1);
        assert_eq!(e.stats().dropped_presses, 0);
    }

    #[test]
    fn time_going_backwards_is_clamped() {
        let mut e = active(&profile_json("portrait", BTN_TAP), Rotation::R0);
        press(&mut e, "KeyJ", 1000);
        assert_eq!(e.next_deadline(), Some(1040));
        // An earlier time does not move the clock back or lift early.
        let o = e.handle(Input::Release("KeyJ"), 500);
        assert!(o.frames.is_empty());
        assert!(e.tick(10).is_empty());
        assert_eq!(e.tick(1040).len(), 1);
        // A tap pressed "in the past" uses the largest time seen.
        press(&mut e, "KeyJ", 100);
        assert_eq!(e.next_deadline(), Some(1080));
    }

    #[test]
    fn stats_count_frames() {
        let mut e = active(&profile_json("portrait", BTN_HOLD), Rotation::R0);
        press(&mut e, "Space", 0);
        release(&mut e, "Space", 1);
        assert_eq!(e.stats().frames, 2);
    }

    #[test]
    fn orientation_type_is_reexported() {
        let e = engine(&profile_json("landscape", BTN_HOLD), Rotation::R90);
        assert_eq!(e.profile().orientation, Orientation::Landscape);
    }

    // ---- fuzz ---------------------------------------------------------

    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            self.0 >> 33
        }
        fn below(&mut self, n: u64) -> u64 {
            self.next() % n
        }
        fn f(&mut self, span: f64) -> f64 {
            (self.next() % 2001) as f64 / 1000.0 * span - span
        }
    }

    /// Replays emitted frames and checks invariants I1-I6.
    struct Model {
        down: [Option<(u16, u16)>; MAX_CONTACTS],
    }

    impl Model {
        /// Checks the frame against the model, applies it and returns how
        /// many contacts went down in it.
        fn apply(&mut self, f: &Frame) -> usize {
            let cs = f.contacts();
            let mut new_down = 0;
            assert!(!cs.is_empty() && cs.len() <= MAX_CONTACTS, "I1 len");
            for w in cs.windows(2) {
                assert!(w[0].id < w[1].id, "I1 sorted/unique {cs:?}");
            }
            assert!(
                cs.iter().all(|k| usize::from(k.id) < MAX_CONTACTS),
                "I1 id range"
            );
            // I2: every previously-down contact appears.
            for (i, d) in self.down.iter().enumerate() {
                if d.is_some() {
                    assert!(
                        cs.iter().any(|k| usize::from(k.id) == i),
                        "I2 vanished id {i}: {cs:?}"
                    );
                }
            }
            let mut transition = false;
            let mut moved = false;
            for k in cs {
                let prev = self.down[usize::from(k.id)];
                if k.touching {
                    match prev {
                        None => {
                            transition = true;
                            new_down += 1;
                            // A newly-down contact takes the lowest free id.
                            assert!(
                                self.down[..usize::from(k.id)].iter().all(Option::is_some),
                                "new contact {} is not the lowest free id: {cs:?}",
                                k.id
                            );
                        }
                        Some(p) => moved |= p != (k.x, k.y),
                    }
                } else {
                    let p = prev.expect("I3: lift of an id that was not down");
                    assert_eq!(p, (k.x, k.y), "I2 lift position");
                    transition = true;
                }
            }
            assert_eq!(f.has_transition(), transition, "I5 flag {cs:?}");
            if !transition {
                assert!(moved, "I6 frame without change {cs:?}");
            }
            for k in cs {
                self.down[usize::from(k.id)] = k.touching.then_some((k.x, k.y));
            }
            new_down
        }

        fn count(&self) -> usize {
            self.down.iter().filter(|d| d.is_some()).count()
        }
    }

    const FUZZ_KEYS: &[&str] = &[
        "KeyW",
        "KeyA",
        "KeyS",
        "KeyD",
        "Space",
        "KeyJ",
        "KeyK",
        "KeyL",
        "KeyU",
        "KeyI",
        "MouseLeft",
        "KeyQ",
        "KeyZ",
    ];

    /// Controls pinned to screen corners so clamping makes moves no-ops.
    fn fuzz_edge_profile() -> Profile {
        let json = r#"{"version":1,"orientation":"portrait","controls":[
            {"type":"joystick","x":0.0,"y":0.0,"radius":0.12,"up":"KeyW","down":"KeyS","left":"KeyA","right":"KeyD"},
            {"type":"look","x":1.0,"y":1.0,"sensitivity":3.0,"reach":0.05,"release_after_ms":120},
            {"type":"button","x":1.0,"y":0.8,"key":"Space"},
            {"type":"button","x":0.85,"y":0.6,"key":"KeyJ","mode":"tap"},
            {"type":"button","x":0.75,"y":0.7,"key":"KeyK"},
            {"type":"button","x":0.65,"y":0.8,"key":"KeyL","mode":"tap"},
            {"type":"button","x":0.55,"y":0.8,"key":"KeyU"},
            {"type":"button","x":0.95,"y":0.5,"key":"KeyI"},
            {"type":"button","x":0.95,"y":0.4,"key":"MouseLeft"}]}"#;
        Profile::from_json(json).unwrap()
    }

    fn fuzz_profile() -> Profile {
        let json = r#"{"version":1,"orientation":"portrait","controls":[
            {"type":"joystick","x":0.2,"y":0.75,"radius":0.12,"up":"KeyW","down":"KeyS","left":"KeyA","right":"KeyD"},
            {"type":"look","x":0.7,"y":0.5,"sensitivity":3.0,"reach":0.05,"release_after_ms":120},
            {"type":"button","x":0.85,"y":0.8,"key":"Space"},
            {"type":"button","x":0.85,"y":0.6,"key":"KeyJ","mode":"tap"},
            {"type":"button","x":0.75,"y":0.7,"key":"KeyK"},
            {"type":"button","x":0.65,"y":0.8,"key":"KeyL","mode":"tap"},
            {"type":"button","x":0.55,"y":0.8,"key":"KeyU"},
            {"type":"button","x":0.95,"y":0.5,"key":"KeyI"},
            {"type":"button","x":0.95,"y":0.4,"key":"MouseLeft"}]}"#;
        Profile::from_json(json).unwrap()
    }

    #[test]
    fn fuzz_invariants() {
        let mut total_dropped = 0u64;
        let mut total_recenters = 0u64;
        for seed in 0..24u64 {
            let mut rng = Lcg(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xABCD);
            // Odd seeds use corner-anchored controls; every 4th seed is press-heavy
            // so that all contact slots fill up and presses get dropped.
            let edge = seed % 2 == 1;
            let press_heavy = seed % 4 >= 2;
            let mut e = Engine::new(if edge {
                fuzz_edge_profile()
            } else {
                fuzz_profile()
            })
            .unwrap();
            e.set_view(view(Rotation::R0));
            let mut model = Model {
                down: [None; MAX_CONTACTS],
            };
            let mut now = 0u64;
            for step in 0..5000 {
                now = match rng.below(10) {
                    0 => now.saturating_sub(rng.below(50)),
                    1..=5 => now + rng.below(60),
                    _ => now,
                };
                let was_active = e.is_active();
                let mut frames: Vec<Frame> = Vec::new();
                let mut explicit_inactive = false;
                // Number of handle() calls in this step; each may put at most
                // one new contact down (tick, release_all, deactivate and
                // set_view never put one down).
                let mut handle_calls = 0usize;
                match rng.below(100) {
                    0..=44 => {
                        let k = FUZZ_KEYS[rng.below(FUZZ_KEYS.len() as u64) as usize];
                        handle_calls = 1;
                        let press_p = if press_heavy { 9 } else { 5 };
                        let o = if rng.below(10) < press_p {
                            e.handle(Input::Press(k), now)
                        } else {
                            e.handle(Input::Release(k), now)
                        };
                        frames = o.frames;
                    }
                    45..=64 => {
                        let (dx, dy) = match rng.below(40) {
                            0 => (f64::NAN, 1.0),
                            1 => (0.0, 0.0),
                            2 => (rng.f(5000.0), rng.f(5000.0)),
                            3..=9 => (rng.f(1e-3), rng.f(1e-3)),
                            10..=14 => (rng.f(30.0).abs(), rng.f(30.0).abs()),
                            _ => (rng.f(30.0), rng.f(30.0)),
                        };
                        handle_calls = 1;
                        frames = e.handle(Input::MouseMotion { dx, dy }, now).frames;
                    }
                    65..=74 => frames = e.tick(now),
                    75..=79 => {
                        handle_calls = 2;
                        let o = e.handle(Input::Press("Backquote"), now);
                        frames = o.frames;
                        frames.extend(e.handle(Input::Release("Backquote"), now).frames);
                    }
                    80 => {
                        handle_calls = 1;
                        let o = e.handle(Input::Press("F8"), now);
                        frames = o.frames;
                        explicit_inactive = true;
                    }
                    81..=83 => {
                        frames.extend(e.release_all());
                        assert_eq!(e.contacts_down(), 0, "I7 release_all seed {seed}");
                        assert_eq!(e.is_active(), was_active, "release_all keeps state");
                    }
                    84..=85 => {
                        frames.extend(e.deactivate());
                        explicit_inactive = true;
                    }
                    86..=87 => {
                        let r = [Rotation::R0, Rotation::R180, Rotation::R90, Rotation::R270]
                            [rng.below(4) as usize];
                        frames = e.set_view(view(r));
                    }
                    _ => {
                        handle_calls = 1;
                        let o = e.handle(Input::Press("KeyJ"), now);
                        frames = o.frames;
                    }
                }
                let new_down: usize = frames.iter().map(|f| model.apply(f)).sum();
                assert!(
                    new_down <= handle_calls,
                    "more than one new contact per handle() call, seed {seed} step {step}"
                );
                assert_eq!(model.count(), e.contacts_down(), "seed {seed} step {step}");
                if !was_active && !e.is_active() {
                    assert!(frames.is_empty(), "I8 seed {seed} step {step}");
                }
                if explicit_inactive {
                    assert_eq!(e.contacts_down(), 0, "I7 seed {seed} step {step}");
                }
                if !e.is_active() {
                    assert_eq!(e.contacts_down(), 0, "inactive with contacts, seed {seed}");
                    assert_eq!(e.next_deadline(), None);
                }
            }
            assert!(
                e.stats().frames > 300,
                "seed {seed}: fuzz barely emitted frames"
            );
            total_dropped += e.stats().dropped_presses;
            total_recenters += e.stats().look_recenters;
            frames_after_release(&mut e, &mut model);
        }
        assert!(total_dropped > 0, "fuzz never exhausted slots");
        assert!(total_recenters > 0, "fuzz never re-centered the look");
    }

    fn frames_after_release(e: &mut Engine, model: &mut Model) {
        if let Some(f) = e.release_all() {
            let _ = model.apply(&f);
        }
        assert_eq!(e.contacts_down(), 0);
        assert_eq!(model.count(), 0);
        assert!(e.release_all().is_none());
    }
}
