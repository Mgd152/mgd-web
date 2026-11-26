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
    pub fn mouse(
        &mut self,
        doc: &mut EditDoc,
        button: &str,
        on_cancel_chip: bool,
    ) -> CaptureResult {
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
        let CaptureState::Listening {
            esc_until, note, ..
        } = &mut self.state
        else {
            return;
        };
        if esc_until.is_some_and(|t| now_ms > t) {
            *esc_until = None;
            if *note == Some(Msg::EscAgain) {
                *note = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::{KEY_NAMES, MOUSE_NAMES};
    use crate::menu::Dir;
    use crate::menu::msg::{Advice, MirrorlyAction};
    use crate::menu::templates::{Template, template};

    fn shooter() -> EditDoc {
        EditDoc::from_profile(template(Template::ShooterLandscape)).unwrap()
    }

    fn slot_of(doc: &EditDoc, key: &str) -> Slot {
        doc.owner_of(key).unwrap()
    }

    fn bound(r: &CaptureResult) -> bool {
        matches!(r, CaptureResult::Bound { .. })
    }

    fn note(c: &Capture) -> Option<Msg> {
        match &c.state {
            CaptureState::Listening { note, .. } => note.clone(),
            _ => None,
        }
    }

    // test 33
    #[test]
    fn press_binds() {
        let mut doc = shooter();
        let slot = slot_of(&doc, "KeyR");
        let mut c = Capture::start(slot);
        assert_eq!(c.slot(), slot);
        assert_eq!(c.message(&doc, Lang::Vi), Msg::Listening);
        let r = c.key(&mut doc, "KeyZ", 0);
        assert_eq!(r, CaptureResult::Bound { advice: None });
        assert_eq!(doc.key_of(slot), Some("KeyZ"));
        // binding the same key again is Bound with no change
        let before = doc.clone();
        let mut c = Capture::start(slot);
        assert!(bound(&c.key(&mut doc, "KeyZ", 0)));
        assert_eq!(doc, before);
    }

    // test 34
    #[test]
    fn unknown_name_keeps_listening_with_message() {
        let mut doc = shooter();
        let before = doc.clone();
        let mut c = Capture::start(slot_of(&doc, "KeyR"));
        assert_eq!(c.key(&mut doc, "KeyÀ", 0), CaptureResult::Pending);
        assert_eq!(note(&c), Some(Msg::KeyUnsupported));
        assert_eq!(
            c.message(&doc, Lang::Vi).text(Lang::Vi),
            "Phím này chưa dùng được. Hãy chọn phím khác."
        );
        assert_eq!(c.mouse(&mut doc, "MouseX", false), CaptureResult::Pending);
        assert_eq!(doc, before);
        assert!(bound(&c.key(&mut doc, "KeyZ", 1)), "still listening");
    }

    // test 35
    #[test]
    fn mouse_binds_buttons_and_directions_but_not_toggle_or_panic() {
        let mut doc = shooter();
        let b = slot_of(&doc, "KeyR");
        let mut c = Capture::start(b);
        // MouseLeft is taken by "Bắn": conflict, then swap
        assert_eq!(
            c.mouse(&mut doc, "MouseLeft", false),
            CaptureResult::Pending
        );
        assert!(matches!(c.state, CaptureState::Conflict { .. }));
        assert!(matches!(
            c.confirm_swap(&mut doc),
            CaptureResult::Swapped { .. }
        ));
        assert_eq!(doc.key_of(b), Some("MouseLeft"));
        assert_eq!(doc.key_of(slot_of(&doc, "KeyR")), Some("KeyR"));
        // free mouse button on a direction slot
        let Slot::Dir(j, _) = slot_of(&doc, "KeyW") else {
            panic!()
        };
        let mut c = Capture::start(Slot::Dir(j, Dir::Up));
        assert!(bound(&c.mouse(&mut doc, "MouseMiddle", false)));
        assert_eq!(doc.key_of(Slot::Dir(j, Dir::Up)), Some("MouseMiddle"));
        // toggle / panic refuse, keep listening
        for (slot, free) in [(Slot::Toggle, "KeyY"), (Slot::Panic, "KeyU")] {
            let before = doc.clone();
            let mut c = Capture::start(slot);
            assert_eq!(
                c.mouse(&mut doc, "MouseBack", false),
                CaptureResult::Pending
            );
            assert_eq!(note(&c), Some(Msg::MouseNotForToggle));
            assert_eq!(c.key(&mut doc, "MouseForward", 0), CaptureResult::Pending);
            assert_eq!(doc, before);
            assert!(
                bound(&c.key(&mut doc, free, 0)),
                "keyboard works after refusal"
            );
        }
    }

    // test 36
    #[test]
    fn cancel_chip_cancels() {
        let mut doc = shooter();
        let before = doc.clone();
        let mut c = Capture::start(slot_of(&doc, "KeyR"));
        assert_eq!(
            c.mouse(&mut doc, "MouseLeft", true),
            CaptureResult::Cancelled
        );
        assert_eq!(doc, before);
        // also while a conflict is showing
        let mut c = Capture::start(slot_of(&doc, "KeyR"));
        c.key(&mut doc, "KeyC", 0);
        assert!(matches!(c.state, CaptureState::Conflict { .. }));
        assert_eq!(
            c.mouse(&mut doc, "MouseLeft", true),
            CaptureResult::Cancelled
        );
        assert_eq!(c.cancel(), CaptureResult::Cancelled);
        assert_eq!(doc, before);
    }

    // test 37
    #[test]
    fn esc_arm_window_is_1500_ms() {
        let mut doc = shooter();
        let slot = slot_of(&doc, "KeyR");
        let before = doc.clone();
        let mut c = Capture::start(slot);
        assert_eq!(c.key(&mut doc, "Escape", 1000), CaptureResult::Pending);
        assert_eq!(note(&c), Some(Msg::EscAgain));
        assert_eq!(doc, before, "the first Esc binds nothing");
        // second Esc at 1499 ms later binds Escape
        assert!(bound(&c.key(&mut doc, "Escape", 1000 + 1499)));
        assert_eq!(doc.key_of(slot), Some("Escape"));

        // second Esc 1501 ms later only re-arms
        let mut doc = shooter();
        let mut c = Capture::start(slot);
        c.key(&mut doc, "Escape", 1000);
        assert_eq!(
            c.key(&mut doc, "Escape", 1000 + 1501),
            CaptureResult::Pending
        );
        assert_eq!(doc, before);
        // and the new arm is good for another 1500 ms
        assert!(bound(&c.key(&mut doc, "Escape", 1000 + 1501 + 1500)));

        // tick clears an expired arm and its hint; a fresh Esc re-arms
        let mut doc = shooter();
        let mut c = Capture::start(slot);
        c.key(&mut doc, "Escape", 0);
        c.tick(1500);
        assert_eq!(note(&c), Some(Msg::EscAgain));
        c.tick(1501);
        assert_eq!(note(&c), None);
        assert_eq!(c.key(&mut doc, "Escape", 1502), CaptureResult::Pending);
        // another key between the two Esc presses does not matter (it binds)
        let mut c = Capture::start(slot);
        c.key(&mut doc, "Escape", 0);
        assert!(bound(&c.key(&mut doc, "KeyV", 10)));
    }

    // test 38
    #[test]
    fn conflict_swap_and_conflict_cancel() {
        let mut doc = shooter();
        let r = slot_of(&doc, "KeyR");
        let c_slot = slot_of(&doc, "KeyC");
        let mut cap = Capture::start(r);
        assert_eq!(cap.key(&mut doc, "KeyC", 0), CaptureResult::Pending);
        assert_eq!(
            cap.state,
            CaptureState::Conflict {
                slot: r,
                key: "KeyC".into(),
                owner: c_slot
            }
        );
        assert_eq!(
            cap.message(&doc, Lang::Vi).text(Lang::Vi),
            "Phím C đang dùng cho “Ngồi”."
        );
        // other keys are ignored while the prompt is up
        assert_eq!(cap.key(&mut doc, "KeyZ", 1), CaptureResult::Pending);
        assert_eq!(
            cap.mouse(&mut doc, "MouseMiddle", false),
            CaptureResult::Pending
        );
        assert!(matches!(cap.state, CaptureState::Conflict { .. }));
        // Enter swaps
        assert!(matches!(
            cap.key(&mut doc, "Enter", 2),
            CaptureResult::Swapped { .. }
        ));
        assert_eq!(doc.key_of(r), Some("KeyC"));
        assert_eq!(doc.key_of(c_slot), Some("KeyR"));
        assert_eq!(doc.profile().problems(), vec![]);

        // Escape cancels, unchanged
        let mut doc = shooter();
        let before = doc.clone();
        let mut cap = Capture::start(r);
        cap.key(&mut doc, "KeyC", 0);
        assert_eq!(cap.key(&mut doc, "Escape", 1), CaptureResult::Cancelled);
        assert_eq!(doc, before);
        // confirm_swap outside a conflict does nothing
        let mut cap = Capture::start(r);
        assert_eq!(cap.confirm_swap(&mut doc), CaptureResult::Pending);
        // a conflicting swap that would put a mouse name on Toggle returns to listening
        let mut doc = shooter();
        let fire = slot_of(&doc, "MouseLeft");
        let mut cap = Capture::start(fire);
        assert_eq!(cap.key(&mut doc, "Backquote", 0), CaptureResult::Pending);
        assert_eq!(cap.confirm_swap(&mut doc), CaptureResult::Pending);
        assert_eq!(note(&cap), Some(Msg::MouseNotForToggle));
        assert_eq!(doc, shooter());
    }

    // test 39
    #[test]
    fn tab_space_f_keys_and_backquote_are_bindable() {
        for k in [
            "Tab", "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F9", "F10", "F11", "F12",
        ] {
            let mut doc = shooter();
            let slot = slot_of(&doc, "KeyR");
            let mut c = Capture::start(slot);
            let r = c.key(&mut doc, k, 0);
            match doc.owner_of(k) {
                Some(s) if s == slot => assert!(bound(&r), "{k}"),
                _ => panic!("{k} not bound: {r:?}"),
            }
        }
    }

    // test 39 (continued): Space is taken in the shooter, Backquote is the toggle key
    #[test]
    fn taken_keys_conflict_and_backquote_swaps_with_toggle() {
        let mut doc = shooter();
        let slot = slot_of(&doc, "KeyR");
        let mut c = Capture::start(slot);
        assert_eq!(c.key(&mut doc, "Space", 0), CaptureResult::Pending);
        assert!(matches!(c.state, CaptureState::Conflict { .. }));

        let mut c = Capture::start(slot);
        assert_eq!(c.key(&mut doc, "Backquote", 0), CaptureResult::Pending);
        assert_eq!(
            c.state,
            CaptureState::Conflict {
                slot,
                key: "Backquote".into(),
                owner: Slot::Toggle
            }
        );
        assert_eq!(
            c.message(&doc, Lang::Vi).text(Lang::Vi),
            "Phím ` đang dùng cho “Phím bật/tắt”."
        );
        assert!(matches!(
            c.confirm_swap(&mut doc),
            CaptureResult::Swapped { .. }
        ));
        assert_eq!(doc.key_of(Slot::Toggle), Some("KeyR"));
        assert_eq!(doc.key_of(slot), Some("Backquote"));
        assert_eq!(doc.profile().problems(), vec![]);
    }

    // test 40
    #[test]
    fn advice_comes_back_with_the_bind() {
        let mut doc = shooter();
        let slot = slot_of(&doc, "KeyR");
        let advice_of = |doc: &mut EditDoc, k: &str| match Capture::start(slot).key(doc, k, 0) {
            CaptureResult::Bound { advice } => advice,
            r => panic!("{k}: {r:?}"),
        };
        assert_eq!(
            advice_of(&mut doc, "F4"),
            Some(Msg::Advice(Advice::ShadowsMirrorly {
                key: "F4".into(),
                action: MirrorlyAction::VolumeDown
            }))
        );
        assert_eq!(
            advice_of(&mut doc, "SuperLeft"),
            Some(Msg::Advice(Advice::WindowsKey))
        );
        assert_eq!(
            advice_of(&mut doc, "AltLeft"),
            Some(Msg::Advice(Advice::AltKey))
        );
        assert_eq!(
            advice_of(&mut doc, "CapsLock"),
            Some(Msg::Advice(Advice::LockKey))
        );
        assert_eq!(
            advice_of(&mut doc, "BracketLeft"),
            Some(Msg::Advice(Advice::HelperKey))
        );
        assert_eq!(advice_of(&mut doc, "KeyZ"), None);
        // swapping also reports advice for the key that was pressed
        let mut c = Capture::start(slot);
        c.key(&mut doc, "KeyC", 0);
        match c.confirm_swap(&mut doc) {
            CaptureResult::Swapped { advice } => assert_eq!(advice, None),
            r => panic!("{r:?}"),
        }
    }

    #[test]
    fn capture_never_breaks_the_profile() {
        let mut doc = shooter();
        let slots = [
            Slot::Toggle,
            Slot::Panic,
            slot_of(&doc, "KeyR"),
            slot_of(&doc, "KeyW"),
            slot_of(&doc, "KeyD"),
        ];
        let mut t = 0;
        for slot in slots {
            for k in KEY_NAMES.iter().chain(MOUSE_NAMES).chain(&["Nope"]) {
                for step in 0..3 {
                    let mut c = Capture::start(slot);
                    t += 100;
                    let _ = c.key(&mut doc, k, t);
                    if step == 1 {
                        let _ = c.confirm_swap(&mut doc);
                    }
                    if step == 2 {
                        let _ = c.mouse(&mut doc, k, false);
                    }
                    assert_eq!(doc.profile().problems(), vec![], "{k}");
                }
            }
        }
    }
}
