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
pub fn snap_move(target: Pt, others: &[Pt], view: &EditView, enabled: bool) -> (Pt, Vec<Guide>) {
    if !enabled {
        return (target, Vec::new());
    }
    snap_axes(target, others, view, true, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::hit::testing::{pt, view};

    // test 29
    #[test]
    fn snap_at_6_px_not_at_7_and_guides_only_when_snapped() {
        let v = view();
        let others = [pt(300.0, 100.0)];
        let (p, g) = snap_move(pt(306.0, 400.0), &others, &v, true);
        assert_eq!((p.x, p.y), (300.0, 400.0));
        assert_eq!(g.len(), 1);
        assert_eq!((g[0].a.x, g[0].b.x), (300.0, 300.0));
        let (p, g) = snap_move(pt(307.0, 400.0), &others, &v, true);
        assert_eq!((p.x, p.y), (307.0, 400.0));
        assert!(g.is_empty());
        // Axes snap independently; both at once give two guides.
        let (p, g) = snap_move(pt(294.0, 105.0), &others, &v, true);
        assert_eq!((p.x, p.y), (300.0, 100.0));
        assert_eq!(g.len(), 2);
        // Disabled: untouched.
        let (p, g) = snap_move(pt(301.0, 101.0), &others, &v, false);
        assert_eq!((p.x, p.y), (301.0, 101.0));
        assert!(g.is_empty());
        // The threshold is in logical px: 12 physical at scale 2.
        let mut v2 = v;
        v2.scale = 2.0;
        assert_eq!(snap_move(pt(312.0, 400.0), &others, &v2, true).0.x, 300.0);
        assert_eq!(snap_move(pt(313.0, 400.0), &others, &v2, true).0.x, 313.0);
    }

    #[test]
    fn nearest_other_wins_and_guide_spans_aligned_centres() {
        let v = view();
        let others = [pt(300.0, 50.0), pt(304.0, 200.0), pt(304.0, 450.0)];
        let (p, g) = snap_move(pt(303.0, 300.0), &others, &v, true);
        assert_eq!(p.x, 304.0);
        assert_eq!(g.len(), 1);
        assert_eq!((g[0].a.y, g[0].b.y), (200.0, 450.0));
    }

    #[test]
    fn edge_band_is_six_percent() {
        assert!(in_edge_band(pt(0.05, 0.5)));
        assert!(in_edge_band(pt(0.5, 0.95)));
        assert!(!in_edge_band(pt(0.06, 0.5)));
        assert!(!in_edge_band(pt(0.5, 0.94)));
        assert!(!in_edge_band(pt(0.5, 0.5)));
    }

    #[test]
    fn clamp_to_picture_all_edges() {
        let v = view();
        let c = clamp_to_picture(pt(-50.0, 900.0), &v);
        assert_eq!((c.x, c.y), (0.0, 500.0));
        let c = clamp_to_picture(pt(2000.0, -1.0), &v);
        assert_eq!((c.x, c.y), (1000.0, 0.0));
    }
}
