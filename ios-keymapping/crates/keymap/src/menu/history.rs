//! Undo/redo over whole-document snapshots, with gesture coalescing.

use super::CtrlId;
use super::doc::EditDoc;

const CAP: usize = 100;

/// What kind of edit is being made. Edits of the same gesture coalesce into
/// one undo step.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Gesture {
    /// One undo step from `begin` until `end`.
    Drag,
    /// Wheel events less than 400 ms apart.
    Wheel,
    /// One undo step from `begin` until `end`; the number tells sliders apart.
    Slider(u32),
    /// Arrow-key nudges less than 500 ms apart.
    Nudge,
    /// Typing into text field `n`, events less than 1000 ms apart.
    Typing(u32),
    /// Never coalesces.
    Single,
}

impl Gesture {
    /// `Some(window)` for time-coalesced gestures.
    fn window_ms(self) -> Option<u64> {
        match self {
            Gesture::Wheel => Some(400),
            Gesture::Nudge => Some(500),
            Gesture::Typing(_) => Some(1000),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
struct Snap {
    doc: EditDoc,
    sel: Option<CtrlId>,
}

#[derive(Clone, Debug)]
struct Open {
    g: Gesture,
    last_ms: u64,
    /// Between `begin` and `end` (Drag/Slider coalesce only while held).
    held: bool,
    /// The redo stack as it was before this step cleared it.
    redo_backup: Vec<Snap>,
}

#[derive(Clone, Debug)]
pub struct History {
    undo: Vec<Snap>,
    redo: Vec<Snap>,
    open: Option<Open>,
    /// Undo-stack depth at the last save; `None` when that state is gone.
    saved: Option<usize>,
}

impl History {
    pub fn new(_doc: &EditDoc) -> History {
        History {
            undo: Vec::new(),
            redo: Vec::new(),
            open: None,
            saved: Some(0),
        }
    }

    /// Finish the open step; drop it if the document did not change.
    fn close(&mut self, doc: &EditDoc) {
        let Some(o) = self.open.take() else { return };
        if self.undo.last().is_some_and(|s| s.doc == *doc) {
            self.undo.pop();
            self.redo = o.redo_backup;
        }
    }

    /// Call before a change. Same-gesture calls within the coalescing window
    /// (Wheel 400 ms, Nudge 500 ms, Typing 1000 ms; Drag/Slider until `end`)
    /// belong to the step opened by the first one.
    pub fn begin(&mut self, doc: &EditDoc, sel: Option<CtrlId>, g: Gesture, now_ms: u64) {
        if let Some(o) = &mut self.open {
            let same = o.g == g
                && match g.window_ms() {
                    Some(w) => now_ms.saturating_sub(o.last_ms) <= w,
                    None => o.held && g != Gesture::Single,
                };
            if same {
                o.last_ms = now_ms;
                o.held = true;
                return;
            }
        }
        self.close(doc);
        let redo_backup = std::mem::take(&mut self.redo);
        if self.saved.is_some_and(|s| s > self.undo.len()) {
            self.saved = None; // the saved state lived in the redo stack
        }
        self.undo.push(Snap {
            doc: doc.clone(),
            sel,
        });
        if self.undo.len() > CAP {
            self.undo.remove(0);
            self.saved = match self.saved {
                Some(0) | None => None,
                Some(n) => Some(n - 1),
            };
        }
        self.open = Some(Open {
            g,
            last_ms: now_ms,
            held: true,
            redo_backup,
        });
    }

    /// Call after the change; drops the step if nothing changed.
    pub fn end(&mut self, doc: &EditDoc) {
        let unchanged = self.undo.last().is_some_and(|s| s.doc == *doc);
        if unchanged && self.open.is_some() {
            self.close(doc);
        } else if let Some(o) = &mut self.open {
            o.held = false;
        }
    }

    /// Restores the previous document; returns the selection to restore.
    pub fn undo(&mut self, doc: &mut EditDoc) -> Option<Option<CtrlId>> {
        self.close(doc);
        let snap = self.undo.pop()?;
        self.redo.push(Snap {
            doc: doc.clone(),
            sel: snap.sel,
        });
        *doc = snap.doc;
        Some(sanitize(doc, snap.sel))
    }

    pub fn redo(&mut self, doc: &mut EditDoc) -> Option<Option<CtrlId>> {
        self.close(doc);
        let snap = self.redo.pop()?;
        self.undo.push(Snap {
            doc: doc.clone(),
            sel: snap.sel,
        });
        *doc = snap.doc;
        Some(sanitize(doc, snap.sel))
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// The current document is what is on disk now.
    pub fn mark_saved(&mut self) {
        match &self.open {
            Some(o) if o.held => self.saved = Some(self.undo.len().saturating_sub(1)),
            _ => {
                self.open = None; // later edits must not merge into a saved step
                self.saved = Some(self.undo.len());
            }
        }
    }

    pub fn dirty(&self) -> bool {
        self.saved != Some(self.undo.len())
    }
}

fn sanitize(doc: &EditDoc, sel: Option<CtrlId>) -> Option<CtrlId> {
    sel.filter(|id| doc.index_of(*id).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::templates::{Template, template};
    use crate::menu::{Kind, Pt};
    use crate::profile::Orientation;

    fn doc() -> EditDoc {
        EditDoc::from_profile(template(Template::Empty(Orientation::Landscape))).unwrap()
    }

    fn pt(x: f64, y: f64) -> Pt {
        Pt { x, y }
    }

    /// One whole edit: begin, change, end.
    fn edit(h: &mut History, d: &mut EditDoc, g: Gesture, t: u64, f: impl FnOnce(&mut EditDoc)) {
        h.begin(d, None, g, t);
        f(d);
        h.end(d);
    }

    // test 12
    #[test]
    fn undo_redo_restore_profile_and_ids_exactly() {
        let mut d = doc();
        let mut h = History::new(&d);
        let s0 = d.clone();
        edit(&mut h, &mut d, Gesture::Single, 0, |d| {
            d.add(Kind::Button, pt(0.5, 0.5)).unwrap();
        });
        let s1 = d.clone();
        edit(&mut h, &mut d, Gesture::Single, 1, |d| {
            d.add(Kind::Joystick, pt(0.2, 0.5)).unwrap();
        });
        let s2 = d.clone();
        assert!(h.can_undo() && !h.can_redo());
        h.undo(&mut d).unwrap();
        assert_eq!(d, s1);
        h.undo(&mut d).unwrap();
        assert_eq!(d, s0);
        assert_eq!(h.undo(&mut d), None);
        assert!(!h.can_undo() && h.can_redo());
        h.redo(&mut d).unwrap();
        assert_eq!(d, s1);
        h.redo(&mut d).unwrap();
        assert_eq!(d, s2);
        assert_eq!(h.redo(&mut d), None);
        assert_eq!(d.ids(), s2.ids());
    }

    // test 13
    #[test]
    fn a_new_edit_clears_redo() {
        let mut d = doc();
        let mut h = History::new(&d);
        edit(&mut h, &mut d, Gesture::Single, 0, |d| {
            d.add(Kind::Button, pt(0.5, 0.5)).unwrap();
        });
        h.undo(&mut d).unwrap();
        assert!(h.can_redo());
        edit(&mut h, &mut d, Gesture::Single, 5, |d| {
            d.add(Kind::Look, pt(0.5, 0.5)).unwrap();
        });
        assert!(!h.can_redo());
        assert_eq!(h.redo(&mut d), None);
    }

    // test 14
    #[test]
    fn capped_at_100_steps() {
        let mut d = doc();
        let mut h = History::new(&d);
        for i in 0..130u64 {
            edit(&mut h, &mut d, Gesture::Single, i * 10_000, |d| {
                d.set_name(&format!("n{i}"));
            });
        }
        let mut n = 0;
        while h.undo(&mut d).is_some() {
            n += 1;
        }
        assert_eq!(n, 100);
        assert_eq!(d.profile().name, "n29");
    }

    // test 15
    #[test]
    fn drag_is_one_step_and_no_change_is_no_step() {
        let mut d = doc();
        let b = d.add(Kind::Button, pt(0.5, 0.5)).unwrap();
        let mut h = History::new(&d);
        let start = d.clone();
        h.begin(&d, Some(b), Gesture::Drag, 0);
        for i in 1..=20 {
            d.move_to(b, pt(0.5 + i as f64 * 0.01, 0.5)).unwrap();
            // the app calls begin on every move event of the same drag
            h.begin(&d, Some(b), Gesture::Drag, i * 16);
        }
        h.end(&d);
        h.undo(&mut d).unwrap();
        assert_eq!(d, start);
        assert!(!h.can_undo(), "the whole drag was one step");

        // a click that moved nothing leaves no step and keeps redo
        let mut h = History::new(&d);
        edit(&mut h, &mut d, Gesture::Single, 0, |d| {
            d.set_name("x");
        });
        h.undo(&mut d).unwrap();
        assert!(h.can_redo());
        h.begin(&d, Some(b), Gesture::Drag, 100);
        h.end(&d);
        assert!(!h.can_undo());
        assert!(h.can_redo(), "a no-op gesture must not clear redo");
        // two separate drags are two steps
        edit(&mut h, &mut d, Gesture::Drag, 200, |d| {
            d.move_to(b, pt(0.1, 0.1)).unwrap();
        });
        edit(&mut h, &mut d, Gesture::Drag, 201, |d| {
            d.move_to(b, pt(0.2, 0.2)).unwrap();
        });
        h.undo(&mut d).unwrap();
        assert!(h.can_undo());
    }

    // test 16
    #[test]
    fn wheel_and_nudge_windows() {
        let steps = |g: Gesture, gap: u64| {
            let mut d = doc();
            let j = d.add(Kind::Joystick, pt(0.5, 0.5)).unwrap();
            let mut h = History::new(&d);
            for (i, t) in [0, gap].iter().enumerate() {
                edit(&mut h, &mut d, g, *t, |d| {
                    d.set_radius(j, 0.2 + i as f64 * 0.01).unwrap();
                });
            }
            let mut n = 0;
            while h.undo(&mut d).is_some() {
                n += 1;
            }
            n
        };
        assert_eq!(steps(Gesture::Wheel, 399), 1);
        assert_eq!(steps(Gesture::Wheel, 400), 1);
        assert_eq!(steps(Gesture::Wheel, 401), 2);
        assert_eq!(steps(Gesture::Nudge, 499), 1);
        assert_eq!(steps(Gesture::Nudge, 500), 1);
        assert_eq!(steps(Gesture::Nudge, 501), 2);
        assert_eq!(steps(Gesture::Typing(1), 1000), 1);
        assert_eq!(steps(Gesture::Typing(1), 1001), 2);
        assert_eq!(steps(Gesture::Single, 1), 2);
        // different gestures never merge
        let mut d = doc();
        let j = d.add(Kind::Joystick, pt(0.5, 0.5)).unwrap();
        let mut h = History::new(&d);
        edit(&mut h, &mut d, Gesture::Wheel, 0, |d| {
            d.set_radius(j, 0.2).unwrap()
        });
        edit(&mut h, &mut d, Gesture::Nudge, 10, |d| {
            d.set_radius(j, 0.3).unwrap()
        });
        edit(&mut h, &mut d, Gesture::Slider(1), 20, |d| {
            d.set_radius(j, 0.25).unwrap()
        });
        edit(&mut h, &mut d, Gesture::Slider(2), 21, |d| {
            d.set_radius(j, 0.26).unwrap()
        });
        let mut n = 0;
        while h.undo(&mut d).is_some() {
            n += 1;
        }
        assert_eq!(n, 4);
    }

    // test 17
    #[test]
    fn selection_is_restored() {
        let mut d = doc();
        let a = d.add(Kind::Button, pt(0.2, 0.2)).unwrap();
        let b = d.add(Kind::Button, pt(0.8, 0.8)).unwrap();
        let mut h = History::new(&d);
        h.begin(&d, Some(a), Gesture::Single, 0);
        d.remove(a).unwrap();
        h.end(&d);
        h.begin(&d, Some(b), Gesture::Single, 10);
        d.move_to(b, pt(0.5, 0.5)).unwrap();
        h.end(&d);
        assert_eq!(h.undo(&mut d), Some(Some(b)));
        assert_eq!(h.undo(&mut d), Some(Some(a)));
        assert!(d.index_of(a).is_some());
        // a selection that no longer exists is dropped
        let mut h = History::new(&d);
        let c = d.add(Kind::Button, pt(0.3, 0.3)).unwrap();
        let mut h2 = History::new(&d);
        h2.begin(&d, Some(c), Gesture::Single, 0);
        d.remove(c).unwrap();
        h2.end(&d);
        h.begin(&d, None, Gesture::Single, 0);
        d.set_name("z");
        h.end(&d);
        assert_eq!(h.undo(&mut d), Some(None));
        assert_eq!(h2.undo(&mut d), Some(Some(c)));
    }

    // test 18
    #[test]
    fn dirty_and_mark_saved() {
        let mut d = doc();
        let mut h = History::new(&d);
        assert!(!h.dirty());
        edit(&mut h, &mut d, Gesture::Single, 0, |d| d.set_name("a"));
        assert!(h.dirty());
        h.mark_saved();
        assert!(!h.dirty());
        edit(&mut h, &mut d, Gesture::Single, 5, |d| d.set_name("b"));
        assert!(h.dirty());
        h.undo(&mut d).unwrap();
        assert!(!h.dirty(), "undo back to the saved state is clean");
        h.undo(&mut d).unwrap();
        assert!(h.dirty());
        h.redo(&mut d).unwrap();
        assert!(!h.dirty());
        // a new edit after undoing past the saved point loses it for good
        h.undo(&mut d).unwrap();
        edit(&mut h, &mut d, Gesture::Single, 9, |d| d.set_name("c"));
        h.undo(&mut d).unwrap();
        assert!(h.dirty());
        // edits in the coalescing window after a save are not merged into it
        let mut d = doc();
        let j = d.add(Kind::Joystick, pt(0.5, 0.5)).unwrap();
        let mut h = History::new(&d);
        edit(&mut h, &mut d, Gesture::Wheel, 0, |d| {
            d.set_radius(j, 0.2).unwrap()
        });
        h.mark_saved();
        assert!(!h.dirty());
        edit(&mut h, &mut d, Gesture::Wheel, 100, |d| {
            d.set_radius(j, 0.25).unwrap()
        });
        assert!(h.dirty());
        // saving after the cap dropped the saved state
        let mut d = doc();
        let mut h = History::new(&d);
        for i in 0..105u64 {
            edit(&mut h, &mut d, Gesture::Single, i, |d| {
                d.set_name(&format!("{i}"))
            });
        }
        h.mark_saved();
        for i in 0..100u64 {
            h.undo(&mut d).unwrap();
            assert!(h.dirty(), "{i}");
        }
        assert!(!h.can_undo());
    }
}
