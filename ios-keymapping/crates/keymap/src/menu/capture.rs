//! Key-capture state machine (sections 2.4 and 4.4).

use super::doc::{BindOutcome, EditDoc, EditError, Slot};
use super::keynames::key_advice;
use super::msg::{Lang, Msg};
use crate::keys::{is_known_key, is_mouse_button};

/// A second Esc within this long binds Escape.
pub const ESC_WINDOW_MS: u64 = 1500;

#[derive(Clone, PartialEq, Debug)]
pub enum CaptureState {
    Listening {
        slot: Slot,
        /// The first Esc armed Escape binding until this time.
        esc_until: Option<u64>,
        /// Message shown under the cap (unsupported key, Esc hint, ...).
        note: Option<Msg>,
    },
    Conflict {
        slot: Slot,
        key: String,
        owner: Slot,
    },
}

#[derive(Clone, PartialEq, Debug)]
pub enum CaptureResult {
    Pending,
    Bound { advice: Option<Msg> },
    Swapped { advice: Option<Msg> },
    Cancelled,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Capture {
    pub state: CaptureState,
}

fn advice(name: &str) -> Option<Msg> {
    key_advice(name).map(Msg::Advice)
}

impl Capture {
    pub fn start(slot: Slot) -> Capture {
        Capture {
            state: CaptureState::Listening {
                slot,
                esc_until: None,
                note: None,
            },
        }
    }

    pub fn slot(&self) -> Slot {
        match &self.state {
            CaptureState::Listening { slot, .. } | CaptureState::Conflict { slot, .. } => *slot,
        }
    }

    fn listen_with(&mut self, note: Msg) -> CaptureResult {
        let slot = self.slot();
        self.state = CaptureState::Listening {
            slot,
            esc_until: None,
            note: Some(note),
        };
        CaptureResult::Pending
    }

    /// What to show under the cap now.
    pub fn message(&self, doc: &EditDoc, lang: Lang) -> Msg {
        match &self.state {
            CaptureState::Listening { note, .. } => note.clone().unwrap_or(Msg::Listening),
            CaptureState::Conflict { key, owner, .. } => Msg::KeyTaken {
                key: key.clone(),
                owner: doc.slot_name(*owner, lang),
            },
        }
    }

    fn bind(&mut self, doc: &mut EditDoc, name: &str) -> CaptureResult {
        let slot = self.slot();
        match doc.bind(slot, name) {
            Ok(BindOutcome::Bound | BindOutcome::Unchanged) => CaptureResult::Bound {
                advice: advice(name),
            },
            Ok(BindOutcome::Conflict { owner }) => {
                self.state = CaptureState::Conflict {
                    slot,
                    key: name.to_string(),
                    owner,
                };
                CaptureResult::Pending
            }
            Err(EditError::MouseNotAllowed) => self.listen_with(Msg::MouseNotForToggle),
            Err(EditError::UnknownKey) => self.listen_with(Msg::KeyUnsupported),
            Err(_) => CaptureResult::Cancelled, // the slot vanished
        }
    }

    /// A key press (not a release, not an auto-repeat).
    pub fn key(&mut self, doc: &mut EditDoc, name: &str, now_ms: u64) -> CaptureResult {
        if let CaptureState::Conflict { .. } = self.state {
            return match name {
                "Enter" | "NumpadEnter" => self.confirm_swap(doc),
                "Escape" => self.cancel(),
                _ => CaptureResult::Pending,
            };
        }
        if is_mouse_button(name) {
            return self.mouse(doc, name, false);
        }
        if !is_known_key(name) {
            return self.listen_with(Msg::KeyUnsupported);
        }
        if name == "Escape" {
            let slot = self.slot();
            let armed = matches!(
                self.state,
                CaptureState::Listening { esc_until: Some(t), .. } if now_ms <= t
            );
            if !armed {
                self.state = CaptureState::Listening {
                    slot,
                    esc_until: Some(now_ms + ESC_WINDOW_MS),
                    note: Some(Msg::EscAgain),
                };
                return CaptureResult::Pending;
            }
        }
        self.bind(doc, name)
    }

    /// A mouse button press. `on_cancel_chip`: the pointer is on `Huỷ`.
    pub fn mouse(&mut self, doc: &mut EditDoc, button: &str, on_cancel_chip: bool) -> CaptureResult {
        if on_cancel_chip {
            return self.cancel();
        }
        if let CaptureState::Conflict { .. } = self.state {
            return CaptureResult::Pending;
        }
        if !is_mouse_button(button) {
            return self.listen_with(Msg::KeyUnsupported);
        }
        self.bind(doc, button)
    }

    /// `Đổi chỗ`: exchange the two keys. Only meaningful in a conflict.
    pub fn confirm_swap(&mut self, doc: &mut EditDoc) -> CaptureResult {
        let CaptureState::Conflict { slot, key, owner } = self.state.clone() else {
            return CaptureResult::Pending;
        };
        match doc.swap(slot, owner) {
            Ok(()) => CaptureResult::Swapped {
                advice: advice(&key),
            },
            Err(EditError::MouseNotAllowed) => self.listen_with(Msg::MouseNotForToggle),
            Err(_) => CaptureResult::Cancelled,
        }
    }

    pub fn cancel(&mut self) -> CaptureResult {
        CaptureResult::Cancelled
    }

    /// Clears an expired Esc arm.
    pub fn tick(&mut self, now_ms: u64) {
        if let CaptureState::Listening {
            esc_until, note, ..
        } = &mut self.state
        {
            if esc_until.is_some_and(|t| now_ms > t) {
                *esc_until = None;
                if *note == Some(Msg::EscAgain) {
                    *note = None;
                }
            }
        }
    }
}
