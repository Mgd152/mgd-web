//! Undo/redo over whole-document snapshots, with gesture coalescing.

use super::doc::EditDoc;
use super::CtrlId;

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
        if let Some(o) = self.open.take() {
            if self.undo.last().is_some_and(|s| s.doc == *doc) {
                self.undo.pop();
                self.redo = o.redo_backup;
            }
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
