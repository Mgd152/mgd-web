//! First-run guide: six steps on top of Edit (UI_SPEC 3.7).
//!
//! The state only watches the profile (compared with the profile the editor
//! opened with) and two Test facts; the editor draws the card and the ring.

use super::msg::Msg;
use crate::profile::{Control, Profile};

/// Movement / resize that counts as "the user did it".
const MOVED: f64 = 0.01;
pub const STEPS: u8 = 6;

#[derive(Clone, PartialEq, Debug)]
pub struct GuideState {
    step: u8,
    baseline: Profile,
    /// Opened from the `Trống` template: steps 2-4 ask to drag items in.
    empty: bool,
    test_entered: bool,
    contact_seen: bool,
}

fn first_joystick(p: &Profile) -> Option<(f64, f64, f64)> {
    p.controls.iter().find_map(|c| match c {
        Control::Joystick { x, y, radius, .. } => Some((*x, *y, *radius)),
        _ => None,
    })
}

fn look(p: &Profile) -> Option<(f64, f64, f64)> {
    p.controls.iter().find_map(|c| match c {
        Control::Look { x, y, reach, .. } => Some((*x, *y, *reach)),
        _ => None,
    })
}

/// Index of the button step 4 talks about: the one labelled `Bắn`, else the
/// first button.
fn target_button(p: &Profile, label: Option<&str>) -> Option<usize> {
    let buttons = || {
        p.controls
            .iter()
            .enumerate()
            .filter(|(_, c)| matches!(c, Control::Button { .. }))
    };
    match label {
        Some(l) if !l.is_empty() => buttons()
            .find(|(_, c)| matches!(c, Control::Button { label, .. } if label == l))
            .map(|(i, _)| i),
        _ => buttons().next().map(|(i, _)| i),
    }
}

fn differs(a: (f64, f64, f64), b: (f64, f64, f64)) -> bool {
    (a.0 - b.0).abs() > MOVED || (a.1 - b.1).abs() > MOVED || (a.2 - b.2).abs() > MOVED
}

impl GuideState {
    /// Starts at step 1. `baseline` is the profile the editor opened with.
    pub fn new(baseline: &Profile) -> GuideState {
        GuideState {
            step: 1,
            baseline: baseline.clone(),
            empty: baseline.controls.is_empty(),
            test_entered: false,
            contact_seen: false,
        }
    }

    pub fn step(&self) -> u8 {
        self.step
    }

    pub fn is_empty_variant(&self) -> bool {
        self.empty
    }

    /// The card text of the current step.
    pub fn msg(&self) -> Msg {
        if self.empty && (2..=4).contains(&self.step) {
            Msg::GuideStepEmpty(self.step)
        } else {
            Msg::GuideStep(self.step)
        }
    }

    pub fn on_test_entered(&mut self) {
        self.test_entered = true;
    }

    pub fn on_contact_seen(&mut self) {
        if self.test_entered {
            self.contact_seen = true;
        }
    }

    /// Whether step `n` is satisfied right now (`picture_ok`: a picture
    /// exists and its rotation matches the profile orientation).
    pub fn done(&self, n: u8, p: &Profile, picture_ok: bool) -> bool {
        match n {
            1 => picture_ok,
            2 => {
                if self.empty {
                    first_joystick(p).is_some()
                } else {
                    match (first_joystick(&self.baseline), first_joystick(p)) {
                        (Some(a), Some(b)) => differs(a, b),
                        _ => false,
                    }
                }
            }
            3 => {
                if self.empty {
                    look(p).is_some()
                } else {
                    match (look(&self.baseline), look(p)) {
                        (Some(a), Some(b)) => differs(a, b),
                        _ => false,
                    }
                }
            }
            4 => {
                if self.empty {
                    p.controls
                        .iter()
                        .any(|c| matches!(c, Control::Button { .. }))
                } else {
                    let label = self.baseline_label();
                    let (Some(bi), Some(ci)) = (
                        target_button(&self.baseline, label.as_deref()),
                        target_button(p, label.as_deref()),
                    ) else {
                        return false;
                    };
                    match (&self.baseline.controls[bi], &p.controls[ci]) {
                        (
                            Control::Button {
                                x: ax,
                                y: ay,
                                key: ak,
                                ..
                            },
                            Control::Button {
                                x: bx,
                                y: by,
                                key: bk,
                                ..
                            },
                        ) => (ax - bx).abs() > MOVED || (ay - by).abs() > MOVED || ak != bk,
                        _ => false,
                    }
                }
            }
            // Step 5 only ends with `Tiếp`.
            5 => false,
            _ => self.test_entered && self.contact_seen,
        }
    }

    fn baseline_label(&self) -> Option<String> {
        let i = target_button(&self.baseline, Some("Bắn"))
            .or_else(|| target_button(&self.baseline, None))?;
        match &self.baseline.controls[i] {
            Control::Button { label, .. } => Some(label.clone()),
            _ => None,
        }
    }

    /// Advance while the current step is satisfied (steps 1-4). Returns
    /// whether the step changed.
    pub fn check(&mut self, p: &Profile, picture_ok: bool) -> bool {
        let before = self.step;
        while self.step <= 4 && self.done(self.step, p, picture_ok) {
            self.step += 1;
        }
        self.step != before
    }

    /// `Tiếp`. Returns true when this ended the guide (pressed on step 6).
    pub fn advance(&mut self) -> bool {
        if self.step >= STEPS {
            return true;
        }
        self.step += 1;
        false
    }

    /// Index of the control that gets the pulsing ring on this step.
    pub fn target(&self, p: &Profile) -> Option<usize> {
        match self.step {
            2 => p
                .controls
                .iter()
                .position(|c| matches!(c, Control::Joystick { .. })),
            3 => p
                .controls
                .iter()
                .position(|c| matches!(c, Control::Look { .. })),
            4 => {
                let label = if self.empty {
                    None
                } else {
                    self.baseline_label()
                };
                target_button(p, label.as_deref())
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::msg::Lang;
    use crate::menu::templates::{Template, template};
    use crate::profile::Orientation;

    fn shooter() -> Profile {
        template(Template::ShooterLandscape)
    }

    // test 71
    #[test]
    fn step_one_needs_a_matching_picture() {
        let p = shooter();
        let mut g = GuideState::new(&p);
        assert!(!g.check(&p, false));
        assert_eq!(g.step(), 1);
        assert!(g.check(&p, true));
        assert_eq!(g.step(), 2);
    }

    #[test]
    fn joystick_look_and_button_steps_complete_on_real_changes() {
        let base = shooter();
        let mut g = GuideState::new(&base);
        g.check(&base, true);
        assert_eq!(g.step(), 2);

        let mut p = base.clone();
        if let Control::Joystick { x, .. } = &mut p.controls[0] {
            *x += 0.005; // below the threshold
        }
        assert!(!g.check(&p, true));
        if let Control::Joystick { x, .. } = &mut p.controls[0] {
            *x += 0.02;
        }
        assert!(g.check(&p, true));
        assert_eq!(g.step(), 3);

        if let Some(Control::Look { reach, .. }) = p
            .controls
            .iter_mut()
            .find(|c| matches!(c, Control::Look { .. }))
        {
            *reach += 0.05;
        }
        assert!(g.check(&p, true));
        assert_eq!(g.step(), 4);

        // The highlighted button is the one labelled Bắn: rebinding counts.
        let mut q = p.clone();
        for c in &mut q.controls {
            if let Control::Button { label, key, .. } = c
                && label == "Bắn"
            {
                *key = "KeyZ".into();
            }
        }
        // A different button moving does not count.
        let mut other = p.clone();
        if let Some(Control::Button { x, label, .. }) = other
            .controls
            .iter_mut()
            .find(|c| matches!(c, Control::Button { label, .. } if label != "Bắn"))
        {
            *x += 0.1;
            let _ = label;
        }
        assert!(!g.clone().check(&other, true));
        assert!(g.check(&q, true));
        assert_eq!(g.step(), 5);
    }

    #[test]
    fn step_five_only_ends_with_next_and_six_needs_a_contact() {
        let p = shooter();
        let mut g = GuideState::new(&p);
        g.step = 5;
        assert!(!g.check(&p, true));
        assert!(!g.advance());
        assert_eq!(g.step(), 6);
        assert!(!g.done(6, &p, true));
        g.on_contact_seen(); // no Test yet: ignored
        assert!(!g.done(6, &p, true));
        g.on_test_entered();
        assert!(!g.done(6, &p, true));
        g.on_contact_seen();
        assert!(g.done(6, &p, true));
        assert!(g.advance(), "Tiếp on the last step ends the guide");
    }

    #[test]
    fn next_always_advances_and_restart_is_a_new_state() {
        let p = shooter();
        let mut g = GuideState::new(&p);
        for want in 2..=6 {
            assert!(!g.advance());
            assert_eq!(g.step(), want);
        }
        let g2 = GuideState::new(&p);
        assert_eq!(g2.step(), 1);
    }

    #[test]
    fn empty_template_uses_the_drag_in_variant() {
        let base = template(Template::Empty(Orientation::Landscape));
        let mut g = GuideState::new(&base);
        assert!(g.is_empty_variant());
        g.check(&base, true);
        assert_eq!(g.step(), 2);
        assert_eq!(g.msg(), Msg::GuideStepEmpty(2));
        assert!(g.msg().text(Lang::Vi).contains("Cần di chuyển"));

        let mut doc = crate::menu::EditDoc::from_profile(base.clone()).unwrap();
        doc.add(
            crate::menu::Kind::Joystick,
            crate::menu::Pt { x: 0.2, y: 0.6 },
        )
        .unwrap();
        assert!(g.check(doc.profile(), true));
        assert_eq!(g.step(), 3);
        doc.add(crate::menu::Kind::Look, crate::menu::Pt { x: 0.7, y: 0.4 })
            .unwrap();
        assert!(g.check(doc.profile(), true));
        assert_eq!(g.step(), 4);
        doc.add(
            crate::menu::Kind::Button,
            crate::menu::Pt { x: 0.8, y: 0.7 },
        )
        .unwrap();
        assert!(g.check(doc.profile(), true));
        assert_eq!(g.step(), 5);
        assert_eq!(g.msg(), Msg::GuideStep(5));
    }

    #[test]
    fn target_points_at_the_control_of_the_step() {
        let p = shooter();
        let mut g = GuideState::new(&p);
        assert_eq!(g.target(&p), None);
        g.step = 2;
        assert!(matches!(
            p.controls[g.target(&p).unwrap()],
            Control::Joystick { .. }
        ));
        g.step = 3;
        assert!(matches!(
            p.controls[g.target(&p).unwrap()],
            Control::Look { .. }
        ));
        g.step = 4;
        assert!(
            matches!(&p.controls[g.target(&p).unwrap()], Control::Button { label, .. } if label == "Bắn")
        );
    }
}
