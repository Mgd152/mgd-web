//! Pure model of the in-app keymapping menu (see `docs/UI_SPEC.md`).
//!
//! No I/O, clocks, threads or randomness: time comes in as `now_ms`, files go
//! through the `Fs` trait (later work packages). This first slice holds the
//! always-valid document, undo history, key capture, text fields, messages
//! and the built-in templates.

pub mod capture;
pub mod doc;
pub mod drag;
pub mod history;
pub mod hit;
pub mod keynames;
pub mod lint;
pub mod msg;
pub mod prefs;
pub mod snap;
pub mod store;
pub mod templates;
pub mod text;
pub mod tracker;

pub use capture::{Capture, CaptureResult, CaptureState};
pub use doc::{BindOutcome, EditDoc, EditError, KeyPreset, Kind, Slot};
pub use drag::{Axis, Drag, DragEnd, DragUpdate, resize_value};
pub use history::{Gesture, History};
pub use hit::{EditView, Hit, cap_rects, handle_pos, hit_test};
pub use keynames::{LABEL_PRESETS_VI, free_key, key_advice, key_label};
pub use lint::{Env, Link, Lint, explain, explain_in, explain_json_error, lints};
pub use msg::{
    Advice, Fix, Lang, MirrorlyAction, Msg, PlayRefusal, ProblemMsg, RepairField, fmt_num,
};
pub use prefs::{Overlay, UiPrefs};
pub use snap::{Guide, in_edge_band, snap_move};
pub use store::{
    Entry, Fs, FsError, FsOp, Health, MemFs, ProfileStore, StoreError, TrashToken, repair_ranges,
    slugify,
};
pub use templates::{Template, template};
pub use text::TextEdit;
pub use tracker::{HeldTracker, TouchDots, dots_from_frame};

/// Stable id of a control during one editing session; never serialized.
pub type CtrlId = u32;

/// A point (picture fraction or window pixels, depending on context).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Pt {
    pub x: f64,
    pub y: f64,
}

/// A rectangle in window pixels.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PxRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dir {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Mods {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

pub const RADIUS_MIN: f64 = 0.03;
pub const RADIUS_MAX: f64 = 0.5;
pub const REACH_MIN: f64 = 0.05;
pub const REACH_MAX: f64 = 1.0;
pub const SENS_SLIDER_MIN: f64 = 0.1;
pub const SENS_SLIDER_MAX: f64 = 10.0;
pub const EDGE_BAND: f64 = 0.06;
pub const NOTICE_VERSION: u32 = 1;
