//! Pointer drags in Edit mode: move, resize, place from the palette
//! (UI_SPEC 4.4, 6.6).
//!
//! The state machine owns the history gesture: a move or resize is exactly one
//! undo step, a click that never passes the threshold records nothing, and
//! `cancel` (Esc / F8) puts the document back byte for byte.
//!
//! Precondition: while a `Drag` is alive nothing else may edit the document or
//! the history (undo, redo, capture, wheel, dock edits). The editor finishes or
//! cancels the drag first; `cancel` restores the document wholesale.

use super::doc::{EditDoc, EditError, Kind};
use super::history::{Gesture, History};
use super::hit::{EditView, Hit};
use super::snap::{Guide, clamp_to_picture, snap_axes};
use super::{CtrlId, Mods, Pt, RADIUS_MAX, RADIUS_MIN, REACH_MAX, REACH_MIN};

/// A press becomes a move after this many logical px.
pub const DRAG_THRESHOLD_LP: f64 = 4.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Axis {
    X,
    Y,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Drag {
    /// Pressed on a body; becomes `Move` after 4 logical px, else a select.
    Pending { id: CtrlId, down: Pt },
    Move {
        id: CtrlId,
        /// Window-px offset centre - pointer, taken when the drag started.
        grab: Pt,
        /// Picture fraction of the centre when the drag started.
        start: Pt,
        axis: Option<Axis>,
        /// The document as it was before the first change (for `cancel`).
        before: Box<EditDoc>,
    },
    /// Pressed on the resize handle. Like a body press it only becomes a
    /// resize after 4 logical px (`offset` is `None` until then), and the size
    /// keeps the offset between the grab point and the current value, so the
    /// first move never jumps.
    Resize {
        id: CtrlId,
        down: Pt,
        offset: Option<f64>,
        before: Box<EditDoc>,
    },
    /// Palette ghost following the pointer.
    Place { kind: Kind },
}

/// What the drawer needs after a pointer move.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct DragUpdate {
    pub guides: Vec<Guide>,
    /// Joystick radius / look reach while resizing (shown beside the pointer).
    pub value: Option<f64>,
}

/// How a drag ended.
#[derive(Clone, PartialEq, Debug)]
pub enum DragEnd {
    /// The pointer never left the threshold: select this control.
    Select(CtrlId),
    Moved(CtrlId),
    Resized(CtrlId),
    Placed(CtrlId),
    /// Palette item released outside the picture, or nothing happened.
    Nothing,
    Refused(EditError),
}

/// Unclamped size for a pointer position; `None` for a non-finite pointer.
fn raw_value(kind: Kind, centre: Pt, pointer: Pt, view: &EditView) -> Option<f64> {
    let short = view.short_px();
    let (dx, dy) = ((pointer.x - centre.x).abs(), (pointer.y - centre.y).abs());
    if !(dx.is_finite() && dy.is_finite()) || short <= 0.0 {
        return None;
    }
    match kind {
        Kind::Joystick => Some((dx * dx + dy * dy).sqrt() / short),
        Kind::Look => Some(dx.max(dy) / short),
        Kind::Button => None,
    }
}

fn range_of(kind: Kind) -> (f64, f64) {
    match kind {
        Kind::Joystick => (RADIUS_MIN, RADIUS_MAX),
        Kind::Look => (REACH_MIN, REACH_MAX),
        Kind::Button => (0.0, 0.0),
    }
}

/// Joystick: distance / short side; look: max(|dx|, |dy|) / short side.
/// Clamped to the profile's range (the range minimum for a non-finite
/// pointer). A button has no size: 0.
pub fn resize_value(kind: Kind, centre: Pt, pointer: Pt, view: &EditView) -> f64 {
    let (lo, hi) = range_of(kind);
    match raw_value(kind, centre, pointer, view) {
        Some(v) => v.clamp(lo, hi),
        None => lo,
    }
}

/// The joystick radius / look reach the control has now.
fn current_size(doc: &EditDoc, id: CtrlId) -> Option<f64> {
    use crate::profile::Control;
    match doc.control(id)? {
        Control::Joystick { radius, .. } => Some(*radius),
        Control::Look { reach, .. } => Some(*reach),
        Control::Button { .. } => None,
    }
}

fn centre_of(doc: &EditDoc, view: &EditView, id: CtrlId) -> Option<Pt> {
    use crate::profile::Control;
    match doc.control(id)? {
        Control::Joystick { x, y, .. }
        | Control::Look { x, y, .. }
        | Control::Button { x, y, .. } => Some(view.to_px(Pt { x: *x, y: *y })),
    }
}

impl Drag {
    /// Start a drag from what the pointer went down on. Caps and empty canvas
    /// start none (the editor selects, captures or deselects instead).
    pub fn from_hit(
        hit: Hit,
        down: Pt,
        doc: &EditDoc,
        _hist: &mut History,
        _now_ms: u64,
    ) -> Option<Drag> {
        match hit {
            Hit::Body(id) => Some(Drag::Pending { id, down }),
            Hit::Handle(id) => Some(Drag::Resize {
                id,
                down,
                offset: None,
                before: Box::new(doc.clone()),
            }),
            Hit::Cap(_) | Hit::None => None,
        }
    }

    /// A palette item is being carried to the picture.
    pub fn place(kind: Kind) -> Drag {
        Drag::Place { kind }
    }

    /// Pointer moved to `p` (window px).
    pub fn update(
        &mut self,
        doc: &mut EditDoc,
        hist: &mut History,
        view: &EditView,
        p: Pt,
        mods: Mods,
        now_ms: u64,
    ) -> DragUpdate {
        if let Drag::Pending { id, down } = self {
            let (id, down) = (*id, *down);
            let far = ((p.x - down.x).powi(2) + (p.y - down.y).powi(2)).sqrt()
                >= view.lp(DRAG_THRESHOLD_LP);
            let Some(c) = centre_of(doc, view, id) else {
                return DragUpdate::default();
            };
            if !far {
                return DragUpdate::default();
            }
            hist.begin(doc, Some(id), Gesture::Drag, now_ms);
            *self = Drag::Move {
                id,
                grab: Pt {
                    x: c.x - down.x,
                    y: c.y - down.y,
                },
                start: view.to_norm(c),
                axis: None,
                before: Box::new(doc.clone()),
            };
        }
        if let Drag::Resize {
            id,
            down,
            offset: offset @ None,
            ..
        } = self
        {
            let id = *id;
            let far = ((p.x - down.x).powi(2) + (p.y - down.y).powi(2)).sqrt()
                >= view.lp(DRAG_THRESHOLD_LP);
            if far
                && let (Some(kind), Some(c), Some(cur)) = (
                    doc.kind(id),
                    centre_of(doc, view, id),
                    current_size(doc, id),
                )
            {
                let off = raw_value(kind, c, *down, view).map_or(0.0, |r| cur - r);
                // Float noise from the handle's own geometry is not an offset.
                *offset = Some(if off.abs() < 1e-9 { 0.0 } else { off });
                hist.begin(doc, Some(id), Gesture::Drag, now_ms);
            }
        }
        match self {
            Drag::Move {
                id,
                grab,
                start,
                axis,
                ..
            } => {
                let id = *id;
                let start_px = view.to_px(*start);
                let mut target = Pt {
                    x: p.x + grab.x,
                    y: p.y + grab.y,
                };
                if mods.shift {
                    if axis.is_none() {
                        let (dx, dy) =
                            ((target.x - start_px.x).abs(), (target.y - start_px.y).abs());
                        if dx > 0.0 || dy > 0.0 {
                            *axis = Some(if dx >= dy { Axis::X } else { Axis::Y });
                        }
                    }
                } else {
                    *axis = None;
                }
                match axis {
                    Some(Axis::X) => target.y = start_px.y,
                    Some(Axis::Y) => target.x = start_px.x,
                    None => {}
                }
                let target = clamp_to_picture(target, view);
                let others: Vec<Pt> = doc
                    .ids()
                    .iter()
                    .filter(|o| **o != id)
                    .filter_map(|o| centre_of(doc, view, *o))
                    .collect();
                let (snapped, guides) = if mods.alt {
                    (target, Vec::new())
                } else {
                    snap_axes(
                        target,
                        &others,
                        view,
                        *axis != Some(Axis::Y),
                        *axis != Some(Axis::X),
                    )
                };
                let snapped = clamp_to_picture(snapped, view);
                let _ = doc.move_to(id, view.to_norm(snapped));
                DragUpdate {
                    guides,
                    value: None,
                }
            }
            Drag::Resize {
                id,
                offset: Some(off),
                ..
            } => {
                let id = *id;
                let off = *off;
                let (Some(kind), Some(c)) = (doc.kind(id), centre_of(doc, view, id)) else {
                    return DragUpdate::default();
                };
                let (lo, hi) = range_of(kind);
                let v = match raw_value(kind, c, p, view) {
                    Some(r) => (r + off).clamp(lo, hi),
                    None => lo,
                };
                let _ = match kind {
                    Kind::Joystick => doc.set_radius(id, v),
                    Kind::Look => doc.set_reach(id, v),
                    Kind::Button => Ok(()),
                };
                DragUpdate {
                    guides: Vec::new(),
                    value: Some(v),
                }
            }
            Drag::Resize { offset: None, .. } => DragUpdate::default(),
            Drag::Pending { .. } | Drag::Place { .. } => DragUpdate::default(),
        }
    }

    /// Button released at `p`.
    pub fn finish(
        mut self,
        doc: &mut EditDoc,
        hist: &mut History,
        view: &EditView,
        p: Pt,
        mods: Mods,
        now_ms: u64,
    ) -> DragEnd {
        if let Drag::Place { kind } = self {
            if !view.contains(p) {
                return DragEnd::Nothing;
            }
            let at = view.to_norm(p);
            hist.begin(doc, None, Gesture::Single, now_ms);
            let r = doc.add(kind, at);
            hist.end(doc);
            return match r {
                Ok(id) => DragEnd::Placed(id),
                Err(e) => DragEnd::Refused(e),
            };
        }
        self.update(doc, hist, view, p, mods, now_ms);
        match self {
            Drag::Pending { id, .. } => DragEnd::Select(id),
            Drag::Move { id, .. } => {
                hist.end(doc);
                DragEnd::Moved(id)
            }
            // A click on the handle changes nothing.
            Drag::Resize { offset: None, .. } => DragEnd::Nothing,
            Drag::Resize { id, .. } => {
                hist.end(doc);
                DragEnd::Resized(id)
            }
            Drag::Place { .. } => DragEnd::Nothing,
        }
    }

    /// Esc / F8: undo whatever the drag changed and record nothing.
    pub fn cancel(self, doc: &mut EditDoc, hist: &mut History) {
        match self {
            Drag::Move { before, .. }
            | Drag::Resize {
                before,
                offset: Some(_),
                ..
            } => {
                *doc = *before;
                hist.end(doc);
            }
            Drag::Resize { offset: None, .. } | Drag::Pending { .. } | Drag::Place { .. } => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{Contact, Engine, Input};
    use crate::geom::{Rotation, View};
    use crate::menu::Dir;
    use crate::menu::PxRect;
    use crate::menu::doc::Slot;
    use crate::menu::hit::testing::{doc, pt, view};
    use crate::menu::hit::{handle_pos, hit_test};
    use crate::profile::Orientation;

    const NONE: Mods = Mods {
        shift: false,
        ctrl: false,
        alt: false,
    };
    const ALT: Mods = Mods {
        shift: false,
        ctrl: false,
        alt: true,
    };
    const SHIFT: Mods = Mods {
        shift: true,
        ctrl: false,
        alt: false,
    };

    fn pos(d: &EditDoc, id: CtrlId) -> (f64, f64) {
        use crate::profile::Control;
        match d.control(id).unwrap() {
            Control::Joystick { x, y, .. }
            | Control::Look { x, y, .. }
            | Control::Button { x, y, .. } => (*x, *y),
        }
    }

    /// One button at (0.5, 0.5) = window px (500, 250) in a 1000 x 500 picture.
    fn one_button() -> (EditDoc, History, CtrlId) {
        let mut d = doc();
        let id = d.add(Kind::Button, pt(0.5, 0.5)).unwrap();
        let h = History::new(&d);
        (d, h, id)
    }

    fn grab(d: &EditDoc, h: &mut History, at: Pt) -> Drag {
        let hit = hit_test(d, &view(), None, at);
        Drag::from_hit(hit, at, d, h, 0).expect("a drag")
    }

    // test 25
    #[test]
    fn click_below_threshold_selects_only() {
        let (mut d, mut h, id) = one_button();
        let before = d.clone();
        let mut drag = grab(&d, &mut h, pt(500.0, 250.0));
        drag.update(&mut d, &mut h, &view(), pt(503.0, 252.0), NONE, 1);
        assert!(matches!(drag, Drag::Pending { .. }));
        let end = drag.finish(&mut d, &mut h, &view(), pt(503.0, 252.0), NONE, 2);
        assert_eq!(end, DragEnd::Select(id));
        assert_eq!(d, before);
        assert!(!h.can_undo());
        assert!(!h.dirty());
        // At 4 px it is a move.
        let mut drag = grab(&d, &mut h, pt(500.0, 250.0));
        drag.update(&mut d, &mut h, &view(), pt(504.0, 250.0), NONE, 3);
        assert!(matches!(drag, Drag::Move { .. }));
    }

    // test 26
    #[test]
    fn move_keeps_the_grab_offset() {
        let (mut d, mut h, id) = one_button();
        // Grab 10 px up-left of the centre.
        let mut drag = grab(&d, &mut h, pt(490.0, 240.0));
        drag.update(&mut d, &mut h, &view(), pt(600.0, 340.0), ALT, 1);
        let end = drag.finish(&mut d, &mut h, &view(), pt(710.0, 440.0), ALT, 2);
        assert_eq!(end, DragEnd::Moved(id));
        // centre = pointer + (10, 10) -> (720, 450) px -> (0.72, 0.9)
        assert_eq!(pos(&d, id), (0.72, 0.9));
        assert!(h.can_undo());
        h.undo(&mut d);
        assert_eq!(pos(&d, id), (0.5, 0.5));
        assert!(!h.can_undo()); // one step, not many
    }

    // test 27
    #[test]
    fn clamp_at_all_four_edges() {
        for (to, want) in [
            (pt(-300.0, 250.0), (0.0, 0.5)),
            (pt(1500.0, 250.0), (1.0, 0.5)),
            (pt(500.0, -300.0), (0.5, 0.0)),
            (pt(500.0, 900.0), (0.5, 1.0)),
            (pt(-9.0, -9.0), (0.0, 0.0)),
            (pt(9999.0, 9999.0), (1.0, 1.0)),
        ] {
            let (mut d, mut h, id) = one_button();
            let mut drag = grab(&d, &mut h, pt(500.0, 250.0));
            drag.update(&mut d, &mut h, &view(), to, NONE, 1);
            drag.finish(&mut d, &mut h, &view(), to, NONE, 2);
            assert_eq!(pos(&d, id), want, "{to:?}");
        }
    }

    // test 28
    #[test]
    fn shift_locks_an_axis_and_alt_disables_snapping() {
        let (mut d, mut h, id) = one_button();
        let mut drag = grab(&d, &mut h, pt(500.0, 250.0));
        drag.update(&mut d, &mut h, &view(), pt(560.0, 270.0), SHIFT, 1);
        assert_eq!(pos(&d, id), (0.56, 0.5)); // x dominates: y stays
        drag.update(&mut d, &mut h, &view(), pt(570.0, 400.0), SHIFT, 2);
        assert_eq!(pos(&d, id), (0.57, 0.5)); // the axis stays locked
        drag.update(&mut d, &mut h, &view(), pt(570.0, 400.0), NONE, 3);
        assert_eq!(pos(&d, id), (0.57, 0.8)); // released: free again
        drag.finish(&mut d, &mut h, &view(), pt(570.0, 400.0), NONE, 4);

        // Snapping: another control at x = 0.3 (300 px).
        for (mods, want_x) in [(NONE, 0.3), (ALT, 0.303)] {
            let mut d = doc();
            let a = d.add(Kind::Button, pt(0.3, 0.2)).unwrap();
            let b = d.add(Kind::Button, pt(0.6, 0.7)).unwrap();
            let mut h = History::new(&d);
            let mut drag = grab(&d, &mut h, pt(600.0, 350.0));
            let up = drag.update(&mut d, &mut h, &view(), pt(303.0, 350.0), mods, 1);
            assert_eq!(pos(&d, b).0, want_x);
            assert_eq!(up.guides.is_empty(), mods.alt);
            drag.finish(&mut d, &mut h, &view(), pt(303.0, 350.0), mods, 2);
            assert_eq!(pos(&d, a), (0.3, 0.2));
        }
    }

    // test 30
    #[test]
    fn resize_values_and_clamps() {
        let v = view(); // short side 500
        let c = pt(500.0, 250.0);
        assert_eq!(resize_value(Kind::Joystick, c, pt(600.0, 250.0), &v), 0.2);
        assert_eq!(resize_value(Kind::Joystick, c, pt(560.0, 330.0), &v), 0.2); // 3-4-5
        assert_eq!(resize_value(Kind::Joystick, c, c, &v), RADIUS_MIN);
        assert_eq!(
            resize_value(Kind::Joystick, c, pt(0.0, 0.0), &v),
            RADIUS_MAX
        );
        assert_eq!(resize_value(Kind::Look, c, pt(650.0, 300.0), &v), 0.3);
        assert_eq!(resize_value(Kind::Look, c, pt(520.0, 100.0), &v), 0.3);
        assert_eq!(resize_value(Kind::Look, c, c, &v), REACH_MIN);
        assert_eq!(resize_value(Kind::Look, c, pt(0.0, 0.0), &v), REACH_MAX);
        let nan = pt(f64::NAN, 3.0);
        assert_eq!(resize_value(Kind::Joystick, c, nan, &v), RADIUS_MIN);
        assert_eq!(resize_value(Kind::Look, c, nan, &v), REACH_MIN);

        // Through the handle, as one undo step, with the value for the label.
        let mut d = doc();
        let id = d.add(Kind::Joystick, pt(0.5, 0.5)).unwrap();
        let before = d.clone();
        let mut h = History::new(&d);
        let hp = handle_pos(&d, &v, id).unwrap();
        let hit = hit_test(&d, &v, Some(id), hp);
        assert_eq!(hit, Hit::Handle(id));
        let mut drag = Drag::from_hit(hit, hp, &d, &mut h, 0).unwrap();
        let up = drag.update(&mut d, &mut h, &v, pt(600.0, 250.0), NONE, 1);
        assert_eq!(up.value, Some(0.2));
        drag.update(&mut d, &mut h, &v, pt(0.0, 0.0), NONE, 2);
        assert_eq!(d.profile().controls.len(), 1);
        let end = drag.finish(&mut d, &mut h, &v, pt(560.0, 330.0), NONE, 3);
        assert_eq!(end, DragEnd::Resized(id));
        h.undo(&mut d);
        assert_eq!(d, before);
        assert!(!h.can_undo());
    }

    #[test]
    fn a_click_on_the_handle_off_centre_changes_nothing() {
        let v = view();
        for kind in [Kind::Joystick, Kind::Look] {
            let mut d = doc();
            let id = d.add(kind, pt(0.5, 0.5)).unwrap();
            let before = d.clone();
            let mut h = History::new(&d);
            let hp = handle_pos(&d, &v, id).unwrap();
            let at = pt(hp.x + 7.0, hp.y + 7.0);
            assert_eq!(hit_test(&d, &v, Some(id), at), Hit::Handle(id));
            let mut drag = Drag::from_hit(Hit::Handle(id), at, &d, &mut h, 0).unwrap();
            drag.update(&mut d, &mut h, &v, pt(at.x + 3.0, at.y + 1.0), NONE, 1);
            assert_eq!(d, before);
            assert_eq!(
                drag.finish(&mut d, &mut h, &v, at, NONE, 2),
                DragEnd::Nothing
            );
            assert_eq!(d, before);
            assert!(!h.can_undo() && !h.dirty());
        }
    }

    #[test]
    fn the_first_resize_move_does_not_jump() {
        let v = view();
        let mut d = doc();
        let id = d.add(Kind::Joystick, pt(0.5, 0.5)).unwrap();
        let mut h = History::new(&d);
        let r0 = 0.11 * 500.0;
        let hp = handle_pos(&d, &v, id).unwrap();
        let at = pt(hp.x + 7.0, hp.y + 7.0); // off the handle's centre
        let mut drag = Drag::from_hit(Hit::Handle(id), at, &d, &mut h, 0).unwrap();
        // 10 px further out along the pointer's own direction from the centre.
        let c = pt(500.0, 250.0);
        let (dx, dy) = (at.x - c.x, at.y - c.y);
        let len = (dx * dx + dy * dy).sqrt();
        let to = pt(at.x + dx / len * 10.0, at.y + dy / len * 10.0);
        let up = drag.update(&mut d, &mut h, &v, to, NONE, 1);
        let want = (r0 + 10.0) / 500.0;
        assert!((up.value.unwrap() - want).abs() < 1e-6, "{:?}", up.value);
        drag.finish(&mut d, &mut h, &v, to, NONE, 2);
        h.undo(&mut d).unwrap();
        assert!(!h.can_undo());
    }

    // test 31
    #[test]
    fn esc_restores_the_start() {
        // Unrounded start value: the restore must still be exact.
        let mut p = doc().profile().clone();
        p.controls.push(crate::profile::Control::Button {
            label: String::new(),
            x: 0.123_456,
            y: 0.654_321,
            key: "Space".into(),
            mode: Default::default(),
        });
        let mut d = EditDoc::from_profile(p).unwrap();
        let id = d.ids()[0];
        let before = d.clone();
        let mut h = History::new(&d);
        let v = view();
        let c = v.to_px(pt(0.123_456, 0.654_321));
        let mut drag = grab(&d, &mut h, c);
        drag.update(&mut d, &mut h, &v, pt(c.x + 200.0, c.y + 50.0), NONE, 1);
        assert_ne!(pos(&d, id), (0.123_456, 0.654_321));
        drag.cancel(&mut d, &mut h);
        assert_eq!(d, before);
        assert!(!h.can_undo() && !h.dirty());

        // Same for a resize.
        let mut d = doc();
        let j = d.add(Kind::Joystick, pt(0.5, 0.5)).unwrap();
        let before = d.clone();
        let mut h = History::new(&d);
        let hp = handle_pos(&d, &v, j).unwrap();
        let mut drag = Drag::from_hit(Hit::Handle(j), hp, &d, &mut h, 0).unwrap();
        drag.update(&mut d, &mut h, &v, pt(900.0, 250.0), NONE, 1);
        assert_ne!(d, before);
        drag.cancel(&mut d, &mut h);
        assert_eq!(d, before);
        assert!(!h.can_undo());
    }

    // test 32
    #[test]
    fn palette_placement_is_one_undo_step_and_esc_adds_nothing() {
        let mut d = doc();
        let before = d.clone();
        let mut h = History::new(&d);
        let v = view();
        let mut drag = Drag::place(Kind::Button);
        drag.update(&mut d, &mut h, &v, pt(700.0, 125.0), NONE, 1);
        assert_eq!(d, before); // the ghost is not in the document
        let end = drag.finish(&mut d, &mut h, &v, pt(700.0, 125.0), NONE, 2);
        let DragEnd::Placed(id) = end else {
            panic!("{end:?}")
        };
        assert_eq!(pos(&d, id), (0.7, 0.25));
        assert_eq!(d.profile().controls.len(), 1);
        h.undo(&mut d);
        assert_eq!(d, before);
        assert!(!h.can_undo());

        // Esc adds nothing and records nothing.
        let drag = Drag::place(Kind::Joystick);
        drag.cancel(&mut d, &mut h);
        assert_eq!(d, before);
        assert!(!h.can_undo());

        // Released outside the picture: nothing.
        let end = Drag::place(Kind::Look).finish(&mut d, &mut h, &v, pt(1200.0, 100.0), NONE, 3);
        assert_eq!(end, DragEnd::Nothing);
        assert_eq!(d, before);

        // A second look is refused and leaves no step.
        let mut d2 = doc();
        d2.add(Kind::Look, pt(0.5, 0.5)).unwrap();
        let mut h2 = History::new(&d2);
        let end = Drag::place(Kind::Look).finish(&mut d2, &mut h2, &v, pt(100.0, 100.0), NONE, 4);
        assert_eq!(end, DragEnd::Refused(EditError::SecondLook));
        assert!(!h2.can_undo());
    }

    // test 86
    #[test]
    fn pixel_placed_is_hid_sent_for_a_button() {
        // (rotation, picture rectangle, P): a letterboxed picture in the window.
        let cases = [
            (
                Rotation::R0,
                PxRect {
                    x: 300.0,
                    y: 10.0,
                    w: 500.0,
                    h: 1000.0,
                },
                pt(675.0, 760.0),
            ),
            (
                Rotation::R90,
                PxRect {
                    x: 200.0,
                    y: 20.0,
                    w: 1000.0,
                    h: 500.0,
                },
                pt(840.0, 395.0),
            ),
            (
                Rotation::R180,
                PxRect {
                    x: 300.0,
                    y: 10.0,
                    w: 500.0,
                    h: 1000.0,
                },
                pt(425.0, 260.0),
            ),
            (
                Rotation::R270,
                PxRect {
                    x: 200.0,
                    y: 20.0,
                    w: 1000.0,
                    h: 500.0,
                },
                pt(360.0, 145.0),
            ),
        ];
        for (rot, picture, p) in cases {
            let ev = EditView {
                window: (1400.0, 1100.0),
                picture,
                scale: 1.0,
            };
            let mut d = doc();
            d.set_orientation(if rot.is_landscape() {
                Orientation::Landscape
            } else {
                Orientation::Portrait
            });
            let id = d.add(Kind::Button, pt(0.5, 0.5)).unwrap();
            let mut h = History::new(&d);
            let start = ev.to_px(pt(0.5, 0.5));
            let mut drag = Drag::from_hit(Hit::Body(id), start, &d, &mut h, 0).unwrap();
            drag.update(&mut d, &mut h, &ev, p, ALT, 1);
            drag.finish(&mut d, &mut h, &ev, p, ALT, 2);

            let hv = View {
                rotation: rot,
                aspect: picture.w / picture.h,
            };
            let mut e = Engine::new(d.profile().clone()).unwrap();
            e.set_view(hv);
            e.activate().unwrap();
            let key = d.key_of(Slot::Button(id)).unwrap().to_string();
            let o = e.handle(Input::Press(&key), 0);
            let n = ev.to_norm(p);
            let (x, y) = hv.to_hid(n.x, n.y);
            assert_eq!(
                o.frames[0].contacts(),
                &[Contact {
                    id: 0,
                    touching: true,
                    x,
                    y
                }],
                "{rot:?}"
            );
        }
    }

    // test 87
    #[test]
    fn pixel_placed_is_hid_sent_for_a_joystick() {
        let cases = [
            (
                Rotation::R0,
                PxRect {
                    x: 300.0,
                    y: 10.0,
                    w: 500.0,
                    h: 1000.0,
                },
                pt(675.0, 760.0),
            ),
            (
                Rotation::R90,
                PxRect {
                    x: 200.0,
                    y: 20.0,
                    w: 1000.0,
                    h: 500.0,
                },
                pt(840.0, 395.0),
            ),
        ];
        for (rot, picture, p) in cases {
            let ev = EditView {
                window: (1400.0, 1100.0),
                picture,
                scale: 1.0,
            };
            let mut d = doc();
            d.set_orientation(if rot.is_landscape() {
                Orientation::Landscape
            } else {
                Orientation::Portrait
            });
            let id = d.add(Kind::Joystick, pt(0.5, 0.5)).unwrap();
            let mut h = History::new(&d);
            // Move the joystick, then size it by dragging the handle.
            let start = ev.to_px(pt(0.5, 0.5));
            let mut drag = Drag::from_hit(Hit::Body(id), start, &d, &mut h, 0).unwrap();
            drag.update(&mut d, &mut h, &ev, p, ALT, 1);
            drag.finish(&mut d, &mut h, &ev, p, ALT, 2);
            let r_px = 0.2 * ev.short_px();
            let hp = handle_pos(&d, &ev, id).unwrap();
            let mut drag = Drag::from_hit(Hit::Handle(id), hp, &d, &mut h, 3).unwrap();
            let target = pt(p.x + r_px, p.y);
            drag.update(&mut d, &mut h, &ev, target, NONE, 4);
            drag.finish(&mut d, &mut h, &ev, target, NONE, 5);

            let hv = View {
                rotation: rot,
                aspect: picture.w / picture.h,
            };
            let close =
                |a: (u16, u16), b: (u16, u16)| a.0.abs_diff(b.0) <= 1 && a.1.abs_diff(b.1) <= 1;
            for (dir, dx, dy) in [
                (Dir::Up, 0.0, -r_px),
                (Dir::Down, 0.0, r_px),
                (Dir::Left, -r_px, 0.0),
                (Dir::Right, r_px, 0.0),
            ] {
                let mut e = Engine::new(d.profile().clone()).unwrap();
                e.set_view(hv);
                e.activate().unwrap();
                let key = d.key_of(Slot::Dir(id, dir)).unwrap().to_string();
                let o = e.handle(Input::Press(&key), 0);
                assert_eq!(o.frames.len(), 1);
                let c0 = o.frames[0].contacts()[0];
                let c1 = e.tick(crate::engine::JOY_SETTLE_MS)[0].contacts()[0];
                let n = ev.to_norm(p);
                let want0 = hv.to_hid(n.x, n.y);
                let n1 = ev.to_norm(pt(p.x + dx, p.y + dy));
                let want1 = hv.to_hid(n1.x, n1.y);
                assert!(close((c0.x, c0.y), want0), "{rot:?} {dir:?} centre");
                assert!(close((c1.x, c1.y), want1), "{rot:?} {dir:?} edge");
            }
        }
    }
}
