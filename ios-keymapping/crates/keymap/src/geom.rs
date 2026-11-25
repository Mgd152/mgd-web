//! Picture <-> phone panel geometry.
//!
//! "Picture" is the phone screen as the user sees it in the window (possibly
//! turned for landscape). "Panel" is the phone's native portrait touchscreen,
//! which is what HID touch reports address. Both are normalized to 0..1.
//!
//! [`Rotation`] uses exactly the convention of Mirrorly's
//! `crates/input/src/geom.rs` (clockwise turn of the picture relative to the
//! panel), so a profile laid out on a Mirrorly screenshot lands where the
//! user sees it.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Rotation {
    #[default]
    R0,
    R90,
    R180,
    R270,
}

impl Rotation {
    /// Whether the picture is wider than tall (the panel is portrait).
    pub fn is_landscape(self) -> bool {
        matches!(self, Rotation::R90 | Rotation::R270)
    }

    /// Point in the displayed picture (u, v in 0..1) -> panel (x, y in 0..1).
    pub fn picture_to_panel(self, u: f64, v: f64) -> (f64, f64) {
        match self {
            Rotation::R0 => (u, v),
            Rotation::R90 => (v, 1.0 - u),
            Rotation::R180 => (1.0 - u, 1.0 - v),
            Rotation::R270 => (1.0 - v, u),
        }
    }

    /// Inverse of [`picture_to_panel`](Self::picture_to_panel).
    pub fn panel_to_picture(self, x: f64, y: f64) -> (f64, f64) {
        match self {
            Rotation::R0 => (x, y),
            Rotation::R90 => (1.0 - y, x),
            Rotation::R180 => (1.0 - x, 1.0 - y),
            Rotation::R270 => (y, 1.0 - x),
        }
    }
}

/// What the window currently shows: how the picture is turned and its
/// width / height in pixels (only the ratio matters).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct View {
    pub rotation: Rotation,
    /// Picture width / height as displayed (> 1 in landscape).
    pub aspect: f64,
}

impl View {
    /// Portrait phone panel `panel_w` x `panel_h` shown with `rotation`.
    pub fn from_panel(panel_w: f64, panel_h: f64, rotation: Rotation) -> View {
        let a = if panel_h > 0.0 { panel_w / panel_h } else { 1.0 };
        let aspect = if rotation.is_landscape() { 1.0 / a } else { a };
        View { rotation, aspect }
    }

    /// Lengths in a profile (joystick radius, look reach, sensitivity) are
    /// fractions of the picture's *shorter* side so circles stay circles at
    /// any aspect ratio. Returns that length as (fraction of width, fraction
    /// of height).
    pub fn short_side_units(&self, len: f64) -> (f64, f64) {
        let aspect = if self.aspect.is_finite() && self.aspect > 0.0 { self.aspect } else { 1.0 };
        if aspect >= 1.0 {
            // Wide: height is the short side.
            (len / aspect, len)
        } else {
            (len, len * aspect)
        }
    }

    /// Picture point -> HID touchscreen coordinates (0..=65535, panel space).
    pub fn to_hid(&self, u: f64, v: f64) -> (u16, u16) {
        let (x, y) = self.rotation.picture_to_panel(u.clamp(0.0, 1.0), v.clamp(0.0, 1.0));
        (to_u16(x), to_u16(y))
    }
}

/// 0..1 -> the 0..=65535 range HID touch reports use.
pub fn to_u16(v: f64) -> u16 {
    (v.clamp(0.0, 1.0) * 65535.0).round() as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: (f64, f64), b: (f64, f64)) -> bool {
        (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9
    }

    #[test]
    fn rotations_round_trip() {
        for rot in [Rotation::R0, Rotation::R90, Rotation::R180, Rotation::R270] {
            for p in [(0.1, 0.2), (0.0, 1.0), (0.75, 0.33)] {
                let q = rot.picture_to_panel(p.0, p.1);
                assert!(close(rot.panel_to_picture(q.0, q.1), p), "{rot:?} {p:?}");
            }
        }
    }

    #[test]
    fn matches_mirrorly_r90_convention() {
        // Same assertion as Mirrorly's geom test: the panel's top-left corner
        // is the picture's top-right when turned 90 degrees clockwise.
        assert!(close(Rotation::R90.panel_to_picture(0.0, 0.0), (1.0, 0.0)));
        assert!(close(Rotation::R90.picture_to_panel(1.0, 0.0), (0.0, 0.0)));
        // R270: picture's bottom-left is the panel's top-left... and back.
        assert!(close(Rotation::R270.picture_to_panel(0.0, 1.0), (0.0, 0.0)));
    }

    #[test]
    fn view_aspect_follows_rotation() {
        let p = View::from_panel(828.0, 1792.0, Rotation::R0);
        assert!(p.aspect < 1.0);
        let l = View::from_panel(828.0, 1792.0, Rotation::R90);
        assert!((l.aspect - 1792.0 / 828.0).abs() < 1e-9);
    }

    #[test]
    fn short_side_units_keep_circles_round() {
        let v = View { rotation: Rotation::R90, aspect: 2.0 };
        let (w, h) = v.short_side_units(0.1);
        // In pixels: 0.05 * width(2) == 0.1 * height(1).
        assert!((w * 2.0 - h * 1.0).abs() < 1e-12);
        let v = View { rotation: Rotation::R0, aspect: 0.5 };
        let (w, h) = v.short_side_units(0.1);
        assert!((w * 0.5 - h * 1.0).abs() < 1e-12);
        // Degenerate aspect falls back to square.
        let v = View { rotation: Rotation::R0, aspect: f64::NAN };
        assert_eq!(v.short_side_units(0.2), (0.2, 0.2));
    }

    #[test]
    fn to_hid_clamps_and_rotates() {
        let v = View { rotation: Rotation::R0, aspect: 0.5 };
        assert_eq!(v.to_hid(0.0, 0.0), (0, 0));
        assert_eq!(v.to_hid(1.5, -1.0), (65535, 0));
        let v = View { rotation: Rotation::R90, aspect: 2.0 };
        // Picture top-right -> panel top-left.
        assert_eq!(v.to_hid(1.0, 0.0), (0, 0));
        assert_eq!(to_u16(0.5), 32768);
    }
}
