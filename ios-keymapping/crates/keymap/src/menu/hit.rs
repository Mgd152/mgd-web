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
        let w = if self.picture.w > 0.0 { self.picture.w } else { 1.0 };
        let h = if self.picture.h > 0.0 { self.picture.h } else { 1.0 };
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
    if let Some(id) = selected {
        if let Some(h) = handle_pos(doc, view, id) {
            if dist(h, p) <= view.lp(HANDLE_SLOP_LP) {
                return Hit::Handle(id);
            }
        }
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
