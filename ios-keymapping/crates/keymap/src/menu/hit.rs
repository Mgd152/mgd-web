//! Hit-testing of the editing canvas (UI_SPEC 6.1, 6.5).
//!
//! Everything here works in window pixels; a point in "picture fractions" is
//! what the profile stores. `EditView` is the one place that converts.

use super::doc::{EditDoc, Slot};
use super::{CtrlId, Dir, Pt, PxRect};
use crate::profile::Control;

/// Where the picture is inside the window (physical px) and the DPI scale.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct EditView {
    pub window: (f64, f64),
    pub picture: PxRect,
    pub scale: f64,
}

impl EditView {
    /// Window px -> picture fraction (not clamped).
    pub fn to_norm(&self, p: Pt) -> Pt {
        let w = if self.picture.w > 0.0 {
            self.picture.w
        } else {
            1.0
        };
        let h = if self.picture.h > 0.0 {
            self.picture.h
        } else {
            1.0
        };
        Pt {
            x: (p.x - self.picture.x) / w,
            y: (p.y - self.picture.y) / h,
        }
    }

    /// Picture fraction -> window px.
    pub fn to_px(&self, n: Pt) -> Pt {
        Pt {
            x: self.picture.x + n.x * self.picture.w,
            y: self.picture.y + n.y * self.picture.h,
        }
    }

    /// The shorter side of the picture in px (lengths in a profile are
    /// fractions of it).
    pub fn short_px(&self) -> f64 {
        self.picture.w.min(self.picture.h)
    }

    /// Logical px -> physical px.
    pub fn lp(&self, logical: f64) -> f64 {
        logical * self.scale
    }

    /// Whether a window pixel is inside the picture.
    pub fn contains(&self, p: Pt) -> bool {
        p.x >= self.picture.x
            && p.x <= self.picture.x + self.picture.w
            && p.y >= self.picture.y
            && p.y <= self.picture.y + self.picture.h
    }
}

/// What a pointer is over on the canvas.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Hit {
    Handle(CtrlId),
    Cap(Slot),
    Body(CtrlId),
    None,
}

/// Visual radius of a button, logical px (not stored in the profile).
pub const BUTTON_RADIUS_LP: f64 = 22.0;
/// A button is never harder to hit than this many physical px.
pub const BUTTON_MIN_HIT_PX: f64 = 18.0;
/// Radius of the look anchor disc, logical px.
pub const ANCHOR_RADIUS_LP: f64 = 14.0;
/// Width of the grab band along the look border (total, both sides), logical px.
pub const BORDER_BAND_LP: f64 = 8.0;
/// Extra reach around the resize handle, logical px.
pub const HANDLE_SLOP_LP: f64 = 10.0;
/// Key cap size, logical px.
pub const CAP_W_LP: f64 = 36.0;
pub const CAP_H_LP: f64 = 28.0;

/// Hit radius of a button disc in physical px.
pub fn button_hit_radius(view: &EditView) -> f64 {
    view.lp(BUTTON_RADIUS_LP).max(BUTTON_MIN_HIT_PX)
}

fn centre_px(c: &Control, view: &EditView) -> Pt {
    let (x, y) = match c {
        Control::Joystick { x, y, .. }
        | Control::Look { x, y, .. }
        | Control::Button { x, y, .. } => (*x, *y),
    };
    view.to_px(Pt { x, y })
}

fn dist(a: Pt, b: Pt) -> f64 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}

/// Position of the resize handle in window px: on the joystick ring at 45
/// degrees (down-right), at the bottom-right corner of the look square.
/// Buttons have none.
pub fn handle_pos(doc: &EditDoc, view: &EditView, id: CtrlId) -> Option<Pt> {
    let c = doc.control(id)?;
    let o = centre_px(c, view);
    match c {
        Control::Joystick { radius, .. } => {
            let d = radius * view.short_px() * std::f64::consts::FRAC_1_SQRT_2;
            Some(Pt {
                x: o.x + d,
                y: o.y + d,
            })
        }
        Control::Look { reach, .. } => {
            let h = reach * view.short_px();
            Some(Pt {
                x: o.x + h,
                y: o.y + h,
            })
        }
        Control::Button { .. } => None,
    }
}

fn cap_at(view: &EditView, c: Pt) -> PxRect {
    let w = view.lp(CAP_W_LP);
    let h = view.lp(CAP_H_LP);
    PxRect {
        x: c.x - w / 2.0,
        y: c.y - h / 2.0,
        w,
        h,
    }
}

/// Key caps of one control, in window px. Joystick: four caps centred on the
/// ring (up, down, left, right). Button: one cap directly below its disc.
/// Look: none.
pub fn cap_rects(doc: &EditDoc, view: &EditView, id: CtrlId) -> Vec<(Slot, PxRect)> {
    let Some(c) = doc.control(id) else {
        return Vec::new();
    };
    let o = centre_px(c, view);
    match c {
        Control::Joystick { radius, .. } => {
            let r = radius * view.short_px();
            [
                (Dir::Up, 0.0, -r),
                (Dir::Down, 0.0, r),
                (Dir::Left, -r, 0.0),
                (Dir::Right, r, 0.0),
            ]
            .into_iter()
            .map(|(d, dx, dy)| {
                (
                    Slot::Dir(id, d),
                    cap_at(
                        view,
                        Pt {
                            x: o.x + dx,
                            y: o.y + dy,
                        },
                    ),
                )
            })
            .collect()
        }
        Control::Button { .. } => {
            let below = view.lp(BUTTON_RADIUS_LP) + view.lp(CAP_H_LP) / 2.0;
            vec![(
                Slot::Button(id),
                cap_at(
                    view,
                    Pt {
                        x: o.x,
                        y: o.y + below,
                    },
                ),
            )]
        }
        Control::Look { .. } => Vec::new(),
    }
}

fn in_rect(r: &PxRect, p: Pt) -> bool {
    p.x >= r.x && p.x <= r.x + r.w && p.y >= r.y && p.y <= r.y + r.h
}

/// The body of a control under `p`, with the area used to rank overlaps
/// (smaller wins).
fn body_hit(c: &Control, view: &EditView, p: Pt) -> Option<f64> {
    let o = centre_px(c, view);
    let d = dist(o, p);
    match c {
        Control::Button { .. } => {
            let r = button_hit_radius(view);
            (d <= r).then_some(std::f64::consts::PI * r * r)
        }
        Control::Joystick { radius, .. } => {
            let r = radius * view.short_px();
            (d <= r).then_some(std::f64::consts::PI * r * r)
        }
        Control::Look { reach, .. } => {
            let half = reach * view.short_px();
            let ar = view.lp(ANCHOR_RADIUS_LP);
            if d <= ar {
                return Some(std::f64::consts::PI * ar * ar);
            }
            let band = view.lp(BORDER_BAND_LP) / 2.0;
            let m = (p.x - o.x).abs().max((p.y - o.y).abs());
            // On the dashed border only; the interior is click-through.
            ((m - half).abs() <= band).then_some(4.0 * half * half)
        }
    }
}

/// What is under `p` (window px). Order: resize handle of the selection, key
/// caps (later controls on top), bodies (smallest area first).
pub fn hit_test(doc: &EditDoc, view: &EditView, selected: Option<CtrlId>, p: Pt) -> Hit {
    if let Some(id) = selected
        && let Some(h) = handle_pos(doc, view, id)
        && dist(h, p) <= view.lp(HANDLE_SLOP_LP)
    {
        return Hit::Handle(id);
    }
    for id in doc.ids().iter().rev() {
        for (slot, r) in cap_rects(doc, view, *id).into_iter().rev() {
            if in_rect(&r, p) {
                return Hit::Cap(slot);
            }
        }
    }
    let mut best: Option<(f64, CtrlId)> = None;
    for id in doc.ids() {
        let Some(c) = doc.control(*id) else { continue };
        if let Some(area) = body_hit(c, view, p) {
            // `<=`: on equal area the later (topmost) control wins.
            if best.is_none_or(|(a, _)| area <= a) {
                best = Some((area, *id));
            }
        }
    }
    match best {
        Some((_, id)) => Hit::Body(id),
        None => Hit::None,
    }
}

/// Shared helpers for the tests of `hit`, `drag`, `snap` and `lint`.
#[cfg(test)]
pub(crate) mod testing {
    use super::*;
    use crate::menu::templates::{Template, template};
    use crate::profile::Orientation;

    pub fn pt(x: f64, y: f64) -> Pt {
        Pt { x, y }
    }

    /// 1000 x 500 picture filling a 1000 x 500 window, scale 1.
    pub fn view() -> EditView {
        EditView {
            window: (1000.0, 500.0),
            picture: PxRect {
                x: 0.0,
                y: 0.0,
                w: 1000.0,
                h: 500.0,
            },
            scale: 1.0,
        }
    }

    pub fn doc() -> EditDoc {
        EditDoc::from_profile(template(Template::Empty(Orientation::Landscape))).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;
    use crate::geom::{Rotation, View};
    use crate::menu::doc::Kind;

    fn with(doc: &mut EditDoc, kind: Kind, x: f64, y: f64) -> CtrlId {
        doc.add(kind, pt(x, y)).unwrap()
    }

    // test 19
    #[test]
    fn handle_beats_cap_beats_body() {
        let mut d = doc();
        let id = with(&mut d, Kind::Joystick, 0.5, 0.5);
        d.set_radius(id, 0.03).unwrap(); // ring radius 15 px
        let v = view();
        let h = handle_pos(&d, &v, id).unwrap();
        // A tiny ring: the handle overlaps the right and down caps.
        assert_eq!(hit_test(&d, &v, Some(id), h), Hit::Handle(id));
        assert!(matches!(hit_test(&d, &v, None, h), Hit::Cap(_)));
        // The down cap centre is inside the body disc: the cap wins.
        let down = pt(500.0, 250.0 + 15.0);
        assert_eq!(
            hit_test(&d, &v, None, down),
            Hit::Cap(Slot::Dir(id, Dir::Down))
        );
        // On a bigger ring only the body is left at the centre.
        d.set_radius(id, 0.2).unwrap();
        assert_eq!(hit_test(&d, &v, Some(id), pt(500.0, 250.0)), Hit::Body(id));
        assert_eq!(hit_test(&d, &v, Some(id), pt(900.0, 50.0)), Hit::None);
    }

    // test 20
    #[test]
    fn smallest_body_first() {
        let mut d = doc();
        let joy = with(&mut d, Kind::Joystick, 0.5, 0.5);
        d.set_radius(joy, 0.3).unwrap(); // 150 px
        let btn = with(&mut d, Kind::Button, 0.5, 0.5);
        let v = view();
        assert_eq!(hit_test(&d, &v, None, pt(510.0, 250.0)), Hit::Body(btn));
        assert_eq!(hit_test(&d, &v, None, pt(590.0, 250.0)), Hit::Body(joy));
        // Order of creation does not matter.
        let mut d2 = doc();
        let btn2 = with(&mut d2, Kind::Button, 0.5, 0.5);
        let joy2 = with(&mut d2, Kind::Joystick, 0.5, 0.5);
        d2.set_radius(joy2, 0.3).unwrap();
        assert_eq!(hit_test(&d2, &v, None, pt(510.0, 250.0)), Hit::Body(btn2));
    }

    // test 21
    #[test]
    fn look_interior_is_click_through() {
        let mut d = doc();
        let look = with(&mut d, Kind::Look, 0.5, 0.5);
        d.set_reach(look, 0.4).unwrap(); // half side 200 px
        let btn = with(&mut d, Kind::Button, 0.4, 0.4); // inside the box
        let v = view();
        assert_eq!(hit_test(&d, &v, None, pt(500.0, 250.0)), Hit::Body(look)); // anchor
        assert_eq!(hit_test(&d, &v, None, pt(510.0, 258.0)), Hit::Body(look));
        assert_eq!(hit_test(&d, &v, None, pt(450.0, 200.0)), Hit::None); // interior
        assert_eq!(hit_test(&d, &v, None, pt(700.0, 250.0)), Hit::Body(look)); // border
        assert_eq!(hit_test(&d, &v, None, pt(703.9, 250.0)), Hit::Body(look));
        assert_eq!(hit_test(&d, &v, None, pt(705.0, 250.0)), Hit::None);
        assert_eq!(hit_test(&d, &v, None, pt(400.0, 200.0)), Hit::Body(btn));
        // Handle at the bottom-right corner of the box.
        assert_eq!(handle_pos(&d, &v, look), Some(pt(700.0, 450.0)));
        assert_eq!(
            hit_test(&d, &v, Some(look), pt(705.0, 445.0)),
            Hit::Handle(look)
        );
    }

    // test 22
    #[test]
    fn button_min_hit_radius_at_every_scale() {
        for scale in [0.5, 1.0, 1.5, 2.0] {
            let mut d = doc();
            let id = with(&mut d, Kind::Button, 0.5, 0.5);
            let mut v = view();
            v.scale = scale;
            let r = (22.0 * scale).max(18.0);
            assert_eq!(button_hit_radius(&v), r);
            assert_eq!(
                hit_test(&d, &v, None, pt(500.0 + r - 0.1, 250.0)),
                Hit::Body(id)
            );
            assert_eq!(
                hit_test(&d, &v, None, pt(500.0 + r + 0.1, 250.0)),
                Hit::None
            );
            assert_eq!(
                hit_test(&d, &v, None, pt(500.0, 250.0 - r + 0.1)),
                Hit::Body(id)
            );
        }
    }

    // test 23
    #[test]
    fn norm_px_round_trip() {
        let views = [
            (1950.0, 900.0, 0.0, 0.0),    // 19.5:9
            (900.0, 1950.0, 0.0, 0.0),    // 9:19.5
            (1400.0, 700.0, 200.0, 20.0), // letterboxed
        ];
        for (w, h, ox, oy) in views {
            let (pw, ph) = if ox > 0.0 { (1000.0, 500.0) } else { (w, h) };
            let v = EditView {
                window: (w, h),
                picture: PxRect {
                    x: ox,
                    y: oy,
                    w: pw,
                    h: ph,
                },
                scale: 1.25,
            };
            for n in [pt(0.0, 0.0), pt(0.3, 0.9), pt(1.0, 1.0), pt(-0.1, 1.2)] {
                let back = v.to_norm(v.to_px(n));
                assert!((back.x - n.x).abs() < 1e-12 && (back.y - n.y).abs() < 1e-12);
            }
            let p = v.to_px(pt(0.0, 0.0));
            assert_eq!((p.x, p.y), (ox, oy));
            let p = v.to_px(pt(1.0, 1.0));
            assert_eq!((p.x, p.y), (ox + pw, oy + ph));
        }
    }

    // test 24
    #[test]
    fn ring_radius_agrees_with_the_engine_view() {
        for (w, h) in [(1950.0, 900.0), (900.0, 1950.0)] {
            let v = EditView {
                window: (w, h),
                picture: PxRect {
                    x: 0.0,
                    y: 0.0,
                    w,
                    h,
                },
                scale: 1.0,
            };
            assert_eq!(v.short_px(), w.min(h));
            let mut d = doc();
            let id = with(&mut d, Kind::Joystick, 0.5, 0.5);
            d.set_radius(id, 0.12).unwrap();
            let caps = cap_rects(&d, &v, id);
            let up = caps
                .iter()
                .find(|c| c.0 == Slot::Dir(id, Dir::Up))
                .unwrap()
                .1;
            let right = caps
                .iter()
                .find(|c| c.0 == Slot::Dir(id, Dir::Right))
                .unwrap()
                .1;
            let cx = right.x + right.w / 2.0;
            let cy = up.y + up.h / 2.0;
            let ring_px = (cx - w / 2.0, h / 2.0 - cy);
            assert!((ring_px.0 - 0.12 * w.min(h)).abs() < 1e-9);
            assert!((ring_px.1 - 0.12 * w.min(h)).abs() < 1e-9);
            // The engine's short-side units give the same pixel length.
            let ev = View {
                rotation: Rotation::R0,
                aspect: w / h,
            };
            let (fx, fy) = ev.short_side_units(0.12);
            assert!((fx * w - ring_px.0).abs() < 1e-9);
            assert!((fy * h - ring_px.1).abs() < 1e-9);
        }
    }
}
