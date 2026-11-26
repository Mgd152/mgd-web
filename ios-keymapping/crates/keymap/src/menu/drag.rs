//! Pointer drags in Edit mode: move, resize, place from the palette
//! (UI_SPEC 4.4, 6.6).
//!
//! The state machine owns the history gesture: a move or resize is exactly one
//! undo step, a click that never passes the threshold records nothing, and
//! `cancel` (Esc / F8) puts the document back byte for byte.

use super::doc::{EditDoc, EditError, Kind};
use super::hit::{EditView, Hit};
use super::history::{Gesture, History};
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
    Resize {
        id: CtrlId,
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

/// Joystick: distance / short side; look: max(|dx|, |dy|) / short side.
/// Clamped to the profile's range. A button has no size: 0.
pub fn resize_value(kind: Kind, centre: Pt, pointer: Pt, view: &EditView) -> f64 {
    let short = view.short_px();
    let (dx, dy) = ((pointer.x - centre.x).abs(), (pointer.y - centre.y).abs());
    match kind {
        Kind::Joystick => {
            let v = if short > 0.0 {
                (dx * dx + dy * dy).sqrt() / short
            } else {
                0.0
            };
            v.clamp(RADIUS_MIN, RADIUS_MAX)
        }
        Kind::Look => {
            let v = if short > 0.0 { dx.max(dy) / short } else { 0.0 };
            v.clamp(REACH_MIN, REACH_MAX)
        }
        Kind::Button => 0.0,
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
        hist: &mut History,
        now_ms: u64,
    ) -> Option<Drag> {
        match hit {
            Hit::Body(id) => Some(Drag::Pending { id, down }),
            Hit::Handle(id) => {
                hist.begin(doc, Some(id), Gesture::Drag, now_ms);
                Some(Drag::Resize {
                    id,
                    before: Box::new(doc.clone()),
                })
            }
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
                        let (dx, dy) = ((target.x - start_px.x).abs(), (target.y - start_px.y).abs());
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
            Drag::Resize { id, .. } => {
                let id = *id;
                let (Some(kind), Some(c)) = (doc.kind(id), centre_of(doc, view, id)) else {
                    return DragUpdate::default();
                };
                let v = resize_value(kind, c, p, view);
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
            Drag::Move { before, .. } | Drag::Resize { before, .. } => {
                *doc = *before;
                hist.end(doc);
            }
            Drag::Pending { .. } | Drag::Place { .. } => {}
        }
    }
}
