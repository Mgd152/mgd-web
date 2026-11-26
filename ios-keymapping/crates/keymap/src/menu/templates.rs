//! Built-in layouts: generic by genre, no game name, positions are guesses
//! (section 2.2).

use crate::keys::{DEFAULT_PANIC_KEY, DEFAULT_TOGGLE_KEY};
use crate::profile::{ButtonMode, Control, Orientation, Profile};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Template {
    ShooterLandscape,
    BasicLandscape,
    BasicPortrait,
    Empty(Orientation),
}

fn joystick(x: f64, y: f64) -> Control {
    Control::Joystick {
        label: "Di chuyển".into(),
        x,
        y,
        radius: 0.11,
        up: "KeyW".into(),
        down: "KeyS".into(),
        left: "KeyA".into(),
        right: "KeyD".into(),
        sprint_key: None,
        sprint_radius: None,
    }
}

fn look(x: f64, y: f64) -> Control {
    Control::Look {
        label: "Xoay camera".into(),
        x,
        y,
        sensitivity: 1.0,
        invert_y: false,
        reach: 0.30,
        release_after_ms: 0,
    }
}

fn button(label: &str, x: f64, y: f64, key: &str, mode: ButtonMode) -> Control {
    Control::Button {
        label: label.into(),
        x,
        y,
        key: key.into(),
        mode,
    }
}

/// The profile a template creates (version 1, `app` empty). The store names
/// it `Hồ sơ N`; the name set here is the template's label.
pub fn template(t: Template) -> Profile {
    use ButtonMode::{Hold, Tap};
    let (name, orientation, controls) = match t {
        Template::ShooterLandscape => (
            "Bắn súng – màn hình ngang",
            Orientation::Landscape,
            vec![
                joystick(0.15, 0.68),
                look(0.68, 0.40),
                button("Bắn", 0.87, 0.60, "MouseLeft", Hold),
                button("Ngắm", 0.78, 0.44, "MouseRight", Tap),
                button("Nhảy", 0.93, 0.76, "Space", Tap),
                button("Ngồi", 0.84, 0.82, "KeyC", Tap),
                button("Nạp đạn", 0.72, 0.84, "KeyR", Tap),
                button("Súng 1", 0.50, 0.90, "Digit1", Tap),
                button("Súng 2", 0.58, 0.90, "Digit2", Tap),
                button("Hồi máu", 0.40, 0.90, "KeyF", Tap),
            ],
        ),
        Template::BasicLandscape => (
            "Cơ bản – màn hình ngang",
            Orientation::Landscape,
            vec![
                joystick(0.15, 0.68),
                look(0.68, 0.40),
                button("", 0.88, 0.62, "Space", Tap),
                button("", 0.80, 0.78, "KeyJ", Tap),
                button("", 0.92, 0.82, "KeyK", Tap),
            ],
        ),
        Template::BasicPortrait => (
            "Cơ bản – màn hình dọc",
            Orientation::Portrait,
            vec![
                joystick(0.25, 0.78),
                look(0.62, 0.40),
                button("", 0.84, 0.80, "Space", Tap),
                button("", 0.70, 0.88, "KeyJ", Tap),
                button("", 0.88, 0.66, "KeyK", Tap),
            ],
        ),
        Template::Empty(o) => ("Trống", o, Vec::new()),
    };
    Profile {
        version: 1,
        name: name.to_string(),
        app: String::new(),
        orientation,
        toggle_key: DEFAULT_TOGGLE_KEY.to_string(),
        panic_key: DEFAULT_PANIC_KEY.to_string(),
        controls,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{Engine, Input};
    use crate::geom::{Rotation, View};

    fn all() -> Vec<Template> {
        vec![
            Template::ShooterLandscape,
            Template::BasicLandscape,
            Template::BasicPortrait,
            Template::Empty(Orientation::Landscape),
            Template::Empty(Orientation::Portrait),
        ]
    }

    fn rotation(o: Orientation) -> Rotation {
        match o {
            Orientation::Landscape => Rotation::R90,
            Orientation::Portrait => Rotation::R0,
        }
    }

    // test 70
    #[test]
    fn every_template_validates_and_round_trips() {
        for t in all() {
            let p = template(t);
            assert_eq!(p.problems(), vec![], "{t:?}");
            assert_eq!(p.app, "", "templates carry no game name");
            assert_eq!(Profile::from_json(&p.to_json_pretty()).unwrap(), p);
        }
    }

    #[test]
    fn controls_keep_away_from_the_edges() {
        for t in all() {
            let p = template(t);
            let view = View::from_panel(1320.0, 2868.0, rotation(p.orientation));
            for c in &p.controls {
                let (x, y) = match c {
                    Control::Joystick { x, y, .. }
                    | Control::Look { x, y, .. }
                    | Control::Button { x, y, .. } => (*x, *y),
                };
                for v in [x, y] {
                    assert!((0.06..=0.94).contains(&v), "{t:?}: centre {x},{y}");
                }
                // Extent of rings and camera boxes stays inside the picture.
                let len = match c {
                    Control::Joystick { radius, .. } => *radius,
                    Control::Look { reach, .. } => *reach,
                    Control::Button { .. } => 0.0,
                };
                let (dx, dy) = view.short_side_units(len);
                assert!(x - dx >= 0.0 && x + dx <= 1.0, "{t:?}: x extent");
                assert!(y - dy >= 0.0 && y + dy <= 1.0, "{t:?}: y extent");
            }
        }
    }

    #[test]
    fn shooter_matches_the_spec_table() {
        let p = template(Template::ShooterLandscape);
        assert_eq!(p.controls.len(), 10);
        match &p.controls[0] {
            Control::Joystick { x, y, radius, .. } => {
                assert_eq!((*x, *y, *radius), (0.15, 0.68, 0.11))
            }
            c => panic!("{c:?}"),
        }
        match &p.controls[1] {
            Control::Look {
                x,
                y,
                reach,
                sensitivity,
                ..
            } => assert_eq!((*x, *y, *reach, *sensitivity), (0.68, 0.40, 0.30, 1.0)),
            c => panic!("{c:?}"),
        }
        let fire = &p.controls[2];
        assert!(
            matches!(fire, Control::Button { key, mode: ButtonMode::Hold, .. } if key == "MouseLeft")
        );
        assert!(
            matches!(&p.controls[3], Control::Button { key, mode: ButtonMode::Tap, .. } if key == "MouseRight")
        );
        assert_eq!(template(Template::BasicLandscape).controls.len(), 5);
        assert_eq!(
            template(Template::BasicPortrait).orientation,
            Orientation::Portrait
        );
        assert!(
            template(Template::Empty(Orientation::Portrait))
                .controls
                .is_empty()
        );
    }

    #[test]
    fn scripted_engine_run_does_not_panic() {
        for t in all() {
            let p = template(t);
            let keys: Vec<String> = p
                .bound_keys()
                .into_iter()
                .map(|(_, k)| k.to_string())
                .collect();
            let mut e = Engine::new(p.clone()).unwrap();
            let view = View::from_panel(1320.0, 2868.0, rotation(p.orientation));
            e.set_view(view);
            e.activate().unwrap();
            let mut now = 0u64;
            for round in 0..3 {
                for k in &keys {
                    if k == &p.panic_key || k == &p.toggle_key {
                        continue;
                    }
                    e.handle(Input::Press(k), now);
                    e.handle(
                        Input::MouseMotion {
                            dx: 40.0 * (round as f64 + 1.0),
                            dy: -25.0,
                        },
                        now + 1,
                    );
                    now += 50;
                    e.tick(now);
                    e.handle(Input::Release(k), now);
                    now += 50;
                    e.tick(now);
                }
            }
            for _ in 0..200 {
                e.handle(
                    Input::MouseMotion {
                        dx: 500.0,
                        dy: 300.0,
                    },
                    now,
                );
                now += 5;
                e.tick(now);
            }
            e.handle(Input::Press(&p.panic_key), now);
            assert_eq!(e.contacts_down(), 0, "{t:?}");
        }
    }
}
