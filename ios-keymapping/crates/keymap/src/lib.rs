//! Keyboard/mouse -> multi-touch keymapping logic for a mirrored iPhone.
//!
//! Pure logic: no I/O, clocks, threads or randomness. See `docs/SPEC.md`.
#![forbid(unsafe_code)]

pub mod engine;
pub mod geom;
pub mod keys;
pub mod profile;

pub use engine::{
    ActivateError, Contact, Engine, EngineEvent, Frame, Input, MAX_CONTACTS, Output, Stats,
    TAP_HOLD_MS,
};
pub use geom::{Rotation, View};
pub use profile::{ButtonMode, Control, Orientation, Problem, Profile, ProfileError};
