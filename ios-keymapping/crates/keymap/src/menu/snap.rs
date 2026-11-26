//! Alignment guides and clamping while dragging (UI_SPEC 6.6).

use super::hit::EditView;
use super::{EDGE_BAND, Pt};

/// Snap threshold per axis, logical px.
pub const SNAP_LP: f64 = 6.0;

/// A pink dashed alignment line, in window px.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Guide {
    pub a: Pt,
    pub b: Pt,
}

/// Whether a picture fraction lies in the hatched band along an edge.
pub fn in_edge_band(n: Pt) -> bool {
    n.x < EDGE_BAND || n.x > 1.0 - EDGE_BAND || n.y < EDGE_BAND || n.y > 1.0 - EDGE_BAND
}

/// Clamp a window pixel to the picture rectangle.
pub fn clamp_to_picture(p: Pt, view: &EditView) -> Pt {
    let r = view.picture;
    Pt {
        x: p.x.clamp(r.x, r.x + r.w.max(0.0)),
        y: p.y.clamp(r.y, r.y + r.h.max(0.0)),
    }
}

/// Closest value within `thr` of `v`; ties keep the first.
fn nearest(v: f64, others: impl Iterator<Item = f64>, thr: f64) -> Option<f64> {
    let mut best: Option<(f64, f64)> = None;
    for o in others {
        let d = (o - v).abs();
        if d <= thr && best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, o));
        }
    }
    best.map(|(_, o)| o)
}

/// Snap `target` (window px) to the x / y of other centres, each axis on its
/// own, within 6 logical px. Guides are returned only for axes that snapped.
/// `snap_x` / `snap_y` switch an axis off (axis lock).
pub fn snap_axes(
    target: Pt,
    others: &[Pt],
    view: &EditView,
    snap_x: bool,
    snap_y: bool,
) -> (Pt, Vec<Guide>) {
    let thr = view.lp(SNAP_LP);
    let mut out = target;
    let sx = if snap_x {
        nearest(target.x, others.iter().map(|o| o.x), thr)
    } else {
        None
    };
    let sy = if snap_y {
        nearest(target.y, others.iter().map(|o| o.y), thr)
    } else {
        None
    };
    if let Some(x) = sx {
        out.x = x;
    }
    if let Some(y) = sy {
        out.y = y;
    }
    let mut guides = Vec::new();
    if let Some(x) = sx {
        // From the snapped control to the farthest other centre on that line.
        let (mut lo, mut hi) = (out.y, out.y);
        for o in others.iter().filter(|o| o.x == x) {
            lo = lo.min(o.y);
            hi = hi.max(o.y);
        }
        guides.push(Guide {
            a: Pt { x, y: lo },
            b: Pt { x, y: hi },
        });
    }
    if let Some(y) = sy {
        let (mut lo, mut hi) = (out.x, out.x);
        for o in others.iter().filter(|o| o.y == y) {
            lo = lo.min(o.x);
            hi = hi.max(o.x);
        }
        guides.push(Guide {
            a: Pt { x: lo, y },
            b: Pt { x: hi, y },
        });
    }
    (out, guides)
}

/// `snap_axes` on both axes; `enabled == false` returns the target untouched.
pub fn snap_move(
    target: Pt,
    others: &[Pt],
    view: &EditView,
    enabled: bool,
) -> (Pt, Vec<Guide>) {
    if !enabled {
        return (target, Vec::new());
    }
    snap_axes(target, others, view, true, true)
}
