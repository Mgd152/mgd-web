//! Which controls have a held key (Test / Play highlights) and where the
//! dry-run fingers are (UI_SPEC 6.11).

use crate::engine::{Frame, MAX_CONTACTS};
use crate::geom::View;
use crate::profile::{Control, Profile};

/// Maps profile keys to controls and remembers which are held.
#[derive(Clone, Debug)]
pub struct HeldTracker {
    /// (key name, control index, held)
    keys: Vec<(String, usize, bool)>,
}

impl HeldTracker {
    pub fn new(profile: &Profile) -> HeldTracker {
        let mut keys = Vec::new();
        for (i, c) in profile.controls.iter().enumerate() {
            match c {
                Control::Button { key, .. } => keys.push((key.clone(), i, false)),
                Control::Joystick {
                    up,
                    down,
                    left,
                    right,
                    ..
                } => {
                    for k in [up, down, left, right] {
                        keys.push((k.clone(), i, false));
                    }
                }
                Control::Look { .. } => {}
            }
        }
        HeldTracker { keys }
    }

    /// Feed a key or mouse-button edge (auto-repeat is harmless). Returns
    /// whether `mask()` changed.
    pub fn on_key(&mut self, name: &str, pressed: bool) -> bool {
        let before = self.mask();
        for k in self.keys.iter_mut().filter(|k| k.0 == name) {
            k.2 = pressed;
        }
        self.mask() != before
    }

    /// Forget every held key (focus loss, leaving Test).
    pub fn clear(&mut self) {
        for k in &mut self.keys {
            k.2 = false;
        }
    }

    /// Bit `i` is set when control `i` has a held key (controls beyond 64
    /// cannot exist in a valid profile and are ignored).
    pub fn mask(&self) -> u64 {
        self.keys
            .iter()
            .filter(|k| k.2 && k.1 < 64)
            .fold(0u64, |m, k| m | (1u64 << k.1))
    }
}

/// Dry-run fingers as picture fractions, indexed by contact id.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct TouchDots {
    pub dots: [Option<(f64, f64)>; MAX_CONTACTS],
}

/// The dots after an engine frame. A frame is a full snapshot, so a contact
/// that is absent or lifting has no dot; `_prev` is kept for the API of the
/// spec and is not needed with snapshot frames.
pub fn dots_from_frame(_prev: TouchDots, frame: &Frame, view: &View) -> TouchDots {
    let mut out = TouchDots::default();
    for c in frame.contacts() {
        if c.touching && usize::from(c.id) < MAX_CONTACTS {
            out.dots[usize::from(c.id)] = Some(
                view.rotation
                    .panel_to_picture(f64::from(c.x) / 65535.0, f64::from(c.y) / 65535.0),
            );
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{Engine, Input};
    use crate::geom::Rotation;
    use crate::profile::Orientation;

    fn profile(orientation: Orientation) -> Profile {
        let o = orientation.as_str();
        Profile::from_json(&format!(
            r#"{{"version":1,"orientation":"{o}","controls":[
              {{"type":"button","x":0.8,"y":0.7,"key":"Space"}},
              {{"type":"joystick","x":0.2,"y":0.7,"radius":0.1,"up":"KeyW","down":"KeyS","left":"KeyA","right":"KeyD"}},
              {{"type":"look","x":0.6,"y":0.5}},
              {{"type":"button","x":0.9,"y":0.4,"key":"MouseLeft"}}]}}"#
        ))
        .unwrap()
    }

    // test 57
    #[test]
    fn mask_bits_for_button_joystick_repeat_and_clear() {
        let mut t = HeldTracker::new(&profile(Orientation::Landscape));
        assert_eq!(t.mask(), 0);
        assert!(t.on_key("Space", true));
        assert_eq!(t.mask(), 0b0001);
        assert!(!t.on_key("Space", true)); // repeat
        assert!(t.on_key("KeyW", true));
        assert_eq!(t.mask(), 0b0011);
        assert!(!t.on_key("KeyD", true)); // joystick already counted
        assert!(!t.on_key("KeyW", false)); // D still holds the stick
        assert_eq!(t.mask(), 0b0011);
        assert!(t.on_key("KeyD", false));
        assert_eq!(t.mask(), 0b0001);
        assert!(t.on_key("MouseLeft", true));
        assert_eq!(t.mask(), 0b1001);
        // Keys that no control uses, and the look control, never light anything.
        assert!(!t.on_key("KeyZ", true));
        assert!(!t.on_key("Backquote", true));
        t.on_key("KeyA", true);
        t.clear();
        assert_eq!(t.mask(), 0);
        assert!(!t.on_key("Space", false)); // release after clear
    }

    // test 58
    #[test]
    fn dots_equal_engine_contacts_mapped_to_the_picture() {
        for (rot, orientation) in [
            (Rotation::R0, Orientation::Portrait),
            (Rotation::R90, Orientation::Landscape),
            (Rotation::R180, Orientation::Portrait),
            (Rotation::R270, Orientation::Landscape),
        ] {
            let view = View::from_panel(828.0, 1792.0, rot);
            let mut e = Engine::new(profile(orientation)).unwrap();
            e.set_view(view);
            e.activate().unwrap();
            let o = e.handle(Input::Press("Space"), 0);
            let f = o.frames.last().unwrap();
            let dots = dots_from_frame(TouchDots::default(), f, &view);
            let c = f.contacts()[0];
            let want = rot.panel_to_picture(f64::from(c.x) / 65535.0, f64::from(c.y) / 65535.0);
            assert_eq!(dots.dots[usize::from(c.id)], Some(want));
            // ... which is where the profile put the button (0.8, 0.7).
            assert!((want.0 - 0.8).abs() < 1e-4 && (want.1 - 0.7).abs() < 1e-4);
            assert_eq!(dots.dots.iter().flatten().count(), 1);
            // Second finger: both dots.
            let o2 = e.handle(Input::Press("KeyW"), 1);
            let f2 = o2.frames.last().unwrap();
            let dots2 = dots_from_frame(dots, f2, &view);
            assert_eq!(dots2.dots.iter().flatten().count(), 2);
            // Lifting the button clears only its dot.
            let o3 = e.handle(Input::Release("Space"), 2);
            let f3 = o3.frames.last().unwrap();
            let dots3 = dots_from_frame(dots2, f3, &view);
            assert_eq!(dots3.dots.iter().flatten().count(), 1);
            assert_eq!(dots3.dots[usize::from(c.id)], None);
            // Lifting the last finger clears everything.
            let o4 = e.handle(Input::Release("KeyW"), 3);
            let f4 = o4.frames.last().unwrap();
            assert_eq!(dots_from_frame(dots3, f4, &view), TouchDots::default());
        }
    }
}
