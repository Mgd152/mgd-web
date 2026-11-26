//! The editor of one profile: Edit and Test modes (UI_SPEC 2.3-2.5, 3.2-3.4,
//! 4.4-4.5, 6.13).
//!
//! Structural guarantee: nothing here can produce a `Frame` for the app. The
//! dry-run engine of Test keeps its frames inside `TestRun`; the only things
//! that leave are `EditorOut` values and `Effect`s, and no `Effect` carries a
//! frame.

use super::capture::{Capture, CaptureResult, CaptureState};
use super::doc::{EditDoc, KeyPreset, Kind, Slot};
use super::drag::{Drag, DragEnd, DragUpdate};
use super::guide::GuideState;
use super::history::{Gesture, History};
use super::hit::{
    BUTTON_RADIUS_LP, EditView, Hit, button_hit_radius, cap_rects, handle_pos, hit_test,
};
use super::keynames::{LABEL_PRESETS_VI, key_label};
use super::layout::{
    Column, DOCK_W, DockPlacement, GAP, ROW_H, TITLE_H, Widget, WidgetId, WidgetKind, covers,
    dock_placement_with, dock_rect, hit_widget, line_count, slider_parts, slider_step,
    slider_value, split, widgets_to_prims,
};
use super::lint::{Env, lints};
use super::msg::{Lang, Msg, fmt_num};
use super::prefs::Overlay;
use super::scene::{Align, Font, Prim, Scene, Shape, Tone, ellipsize};
use super::shell::{CursorKind, Effect, MenuItem, MenuSpec, UiEvent};
use super::snap::Guide;
use super::text::TextEdit;
use super::tracker::{HeldTracker, TouchDots, dots_from_frame};
use super::ui_text::UiText;
use super::{
    CtrlId, Dir, Mods, Pt, PxRect, RADIUS_MAX, RADIUS_MIN, REACH_MAX, REACH_MIN, SENS_SLIDER_MAX,
    SENS_SLIDER_MIN,
};
use crate::engine::{Engine, Input, Stats};
use crate::geom::{Rotation, View};
use crate::profile::{ButtonMode, Control, Orientation, Profile, ProfileError};

fn ui(t: UiText) -> Msg {
    Msg::Ui(t)
}

fn rc(x: f64, y: f64, w: f64, h: f64) -> PxRect {
    PxRect { x, y, w, h }
}

/// What the editor asks of the shell.
#[derive(Clone, PartialEq, Debug)]
pub enum EditorOut {
    Effect(Effect),
    /// Write the profile now; the shell calls `Editor::mark_saved` on success.
    Save,
    /// Leave Edit (after a `Save` the shell may veto when saving failed).
    Leave,
    /// `Lưu & Chơi`: save, leave and ask for Play.
    SaveAndPlay,
    /// The profile changed (draft needs writing).
    Changed,
    /// The overlay choice of the profile section changed.
    SetOverlay(Overlay),
    /// The guide ended (finished or skipped).
    GuideDone,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum SliderTarget {
    Ring(CtrlId),
    Sens(CtrlId),
    Reach(CtrlId),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum FieldTarget {
    ProfileName,
    Label(CtrlId),
}

/// What a dock widget does when pressed.
#[derive(Clone, PartialEq, Debug)]
enum Act {
    None,
    Undo,
    Redo,
    Collapse,
    Expand,
    Arm(Kind),
    Test,
    Done,
    Revert,
    RevertYes,
    RevertNo,
    Field(FieldTarget),
    Orient(usize),
    OverlayPick(usize),
    AdviceToggle,
    InfoToggle,
    AdvancedToggle,
    Cap(Slot),
    CapCancel,
    CapSwap,
    Dup,
    Del,
    Mode(ButtonMode),
    Suggest,
    Keys(KeyPreset),
    Slider(SliderTarget),
    SensPreset(f64),
    InvertY,
    Idle(usize),
    GuideNext,
    GuideSkip,
    GuideSaveAndPlay,
    SaveYes,
    SaveNo,
    SaveKeep,
    TestBack,
    Rotate,
}

#[derive(Clone, PartialEq, Debug)]
enum MenuCtx {
    Control(CtrlId),
    Add(Pt),
}

#[derive(Clone, Copy, PartialEq, Debug)]
struct SliderDrag {
    target: SliderTarget,
    track: PxRect,
}

#[derive(Clone, PartialEq, Debug)]
struct FieldFocus {
    target: FieldTarget,
    edit: TextEdit,
}

struct TestRun {
    engine: Engine,
    view: View,
    tracker: HeldTracker,
    dots: TouchDots,
    held: Vec<String>,
    full_until: u64,
    flash_until: u64,
    last: Stats,
    has_look: bool,
}

/// Idle-lift choices (ms), in the order of the segmented control.
const IDLE_MS: [u32; 4] = [0, 300, 1000, 3000];
const SENS_PRESETS: [f64; 3] = [0.6, 1.0, 1.8];

pub struct Editor {
    file: String,
    doc: EditDoc,
    hist: History,
    sel: Option<CtrlId>,
    drag: Option<Drag>,
    drag_info: DragUpdate,
    pointer: Option<Pt>,
    capture: Option<Capture>,
    last_advice: Option<Msg>,
    armed: Option<Kind>,
    field: Option<FieldFocus>,
    test: Option<TestRun>,
    collapsed: bool,
    hide_held: bool,
    guide: Option<GuideState>,
    placement: Option<DockPlacement>,
    scroll: f64,
    info_open: bool,
    advice_open: bool,
    advanced_open: bool,
    save_prompt: bool,
    revert_ask: bool,
    menu: Option<MenuCtx>,
    slider: Option<SliderDrag>,
    overlay: Overlay,
    cursor: CursorKind,
    saved: Profile,
    next_slider: u32,
    lang: Lang,
}

/// Short text of a key for the small canvas caps (at most 5 characters).
pub fn cap_text(name: &str, lang: Lang) -> String {
    let mouse = match name {
        "MouseLeft" => Some("1"),
        "MouseRight" => Some("2"),
        "MouseMiddle" => Some("3"),
        "MouseBack" => Some("4"),
        "MouseForward" => Some("5"),
        _ => None,
    };
    if let Some(n) = mouse {
        return format!("M{n}");
    }
    let l = key_label(name, lang);
    let l = [" trái", " phải", " left", " right"]
        .iter()
        .find_map(|s| l.strip_suffix(s))
        .unwrap_or(&l)
        .to_string();
    ellipsize(&l, 5)
}

fn pt_of(c: &Control) -> Pt {
    match c {
        Control::Joystick { x, y, .. }
        | Control::Look { x, y, .. }
        | Control::Button { x, y, .. } => Pt { x: *x, y: *y },
    }
}

fn is_mouse_name(b: &str) -> bool {
    crate::keys::is_mouse_button(b)
}

impl Editor {
    /// Open one profile for editing. `guide`: show the first-run guide.
    pub fn open(file: &str, profile: Profile, guide: bool) -> Result<Editor, ProfileError> {
        let saved = profile.clone();
        let doc = EditDoc::from_profile(profile)?;
        let hist = History::new(&doc);
        let guide = guide.then(|| GuideState::new(doc.profile()));
        Ok(Editor {
            file: file.to_string(),
            doc,
            hist,
            sel: None,
            drag: None,
            drag_info: DragUpdate::default(),
            pointer: None,
            capture: None,
            last_advice: None,
            armed: None,
            field: None,
            test: None,
            collapsed: false,
            hide_held: false,
            guide,
            placement: None,
            scroll: 0.0,
            info_open: false,
            advice_open: false,
            advanced_open: false,
            save_prompt: false,
            revert_ask: false,
            menu: None,
            slider: None,
            overlay: Overlay::default(),
            cursor: CursorKind::Arrow,
            saved,
            next_slider: 0,
            lang: Lang::Vi,
        })
    }

    pub fn file(&self) -> &str {
        &self.file
    }
    pub fn profile(&self) -> &Profile {
        self.doc.profile()
    }
    pub fn dirty(&self) -> bool {
        self.hist.dirty()
    }
    pub fn in_test(&self) -> bool {
        self.test.is_some()
    }
    pub fn selection(&self) -> Option<CtrlId> {
        self.sel
    }
    pub fn capturing(&self) -> bool {
        self.capture.is_some()
    }
    pub fn field_focused(&self) -> bool {
        self.field.is_some()
    }
    pub fn guide_step(&self) -> Option<u8> {
        self.guide.as_ref().map(GuideState::step)
    }
    pub fn hidden(&self) -> bool {
        self.hide_held && self.test.is_none()
    }
    pub fn set_overlay(&mut self, o: Overlay) {
        self.overlay = o;
    }
    pub fn set_lang(&mut self, l: Lang) {
        self.lang = l;
    }
    /// The shell calls this after a successful save.
    pub fn mark_saved(&mut self) {
        self.hist.mark_saved();
        self.saved = self.doc.profile().clone();
    }
    /// Draft recovery: the opened profile is the saved one, the draft is a
    /// pending change.
    pub fn open_with_draft(
        file: &str,
        saved: Profile,
        draft: Profile,
    ) -> Result<Editor, ProfileError> {
        let mut e = Editor::open(file, saved, false)?;
        let mut doc = EditDoc::from_profile(draft)?;
        std::mem::swap(&mut e.doc, &mut doc);
        e.hist.begin(&doc, None, Gesture::Single, 0);
        e.hist.end(&e.doc);
        Ok(e)
    }

    pub fn dots(&self) -> TouchDots {
        self.test.as_ref().map(|t| t.dots).unwrap_or_default()
    }
    pub fn pressed_mask(&self) -> u64 {
        self.test.as_ref().map(|t| t.tracker.mask()).unwrap_or(0)
    }
    pub fn test_fingers(&self) -> usize {
        self.test
            .as_ref()
            .map(|t| t.engine.contacts_down())
            .unwrap_or(0)
    }
    pub fn next_deadline(&self) -> Option<u64> {
        let esc = match self.capture.as_ref().map(|c| &c.state) {
            Some(CaptureState::Listening {
                esc_until: Some(t), ..
            }) => Some(*t),
            _ => None,
        };
        let engine = self.test.as_ref().and_then(|t| t.engine.next_deadline());
        [esc, engine].into_iter().flatten().min()
    }

    /// F-keys the app should keep handling upstream (table 4.1).
    pub fn passes_to_upstream(&self, name: &str) -> bool {
        if self.capture.is_some() || self.field.is_some() {
            return false;
        }
        match name {
            "F8" | "F9" => false,
            "F1" | "F2" | "F3" | "F4" | "F5" => self.test.is_none(),
            "F6" | "F7" | "F10" | "F11" | "F12" => true,
            _ => false,
        }
    }

    fn picture_ok(&self, env: &Env) -> bool {
        env.has_picture && self.doc.profile().orientation.matches(env.rotation)
    }

    fn short_name(&self, id: CtrlId, lang: Lang) -> String {
        self.doc.display_name(id, lang).unwrap_or_default()
    }

    fn effect(out: &mut Vec<EditorOut>, e: Effect) {
        out.push(EditorOut::Effect(e));
    }

    // ------------------------------------------------------------ input ----

    pub fn event(&mut self, ev: &UiEvent<'_>, env: &Env, now_ms: u64) -> Vec<EditorOut> {
        let before = self.doc.clone();
        let mut out = Vec::new();
        if self.test.is_some() {
            self.test_event(ev, env, now_ms, &mut out);
        } else {
            self.edit_event(ev, env, now_ms, &mut out);
        }
        if self.doc != before {
            out.push(EditorOut::Changed);
            if let Some(s) = self.sel
                && self.doc.index_of(s).is_none()
            {
                self.sel = None;
            }
        }
        if self.test.is_none() {
            self.after_event(env, &mut out);
        }
        out
    }

    fn after_event(&mut self, env: &Env, out: &mut Vec<EditorOut>) {
        let focus = self.focus_point(env);
        let pl = dock_placement_with(&env.view, focus, self.grown(env), self.placement);
        self.placement = Some(pl);
        let ok = self.picture_ok(env);
        if let Some(g) = &mut self.guide {
            g.check(self.doc.profile(), ok);
        }
        let cur = self.cursor_for(env);
        if cur != self.cursor {
            self.cursor = cur;
            Self::effect(out, Effect::Cursor(cur));
        }
    }

    fn grown(&self, env: &Env) -> bool {
        let right = env.view.picture.x + env.view.picture.w;
        env.can_grow_window && env.view.window.0 - right >= env.view.lp(DOCK_W) - 0.5
    }

    /// The point the dock keeps away from: a drag, else the selection.
    fn focus_point(&self, env: &Env) -> Option<Pt> {
        if let Some(d) = &self.drag
            && let Some(p) = self.pointer
        {
            // Carrying a palette item that is still over the dock must not make
            // the dock jump away from under the pointer.
            let over_dock = matches!(d, Drag::Place { .. })
                && self
                    .placement
                    .is_some_and(|pl| contains(dock_rect(&env.view, pl, self.collapsed), p));
            if !over_dock {
                return Some(p);
            }
        }
        // With nothing selected the guide's target keeps the dock off the
        // control the card asks the user to drag (3.2, amended).
        let c = match self.sel {
            Some(id) => self.doc.control(id)?,
            None => {
                let g = self.guide.as_ref()?;
                let i = g.target(self.doc.profile())?;
                self.doc.profile().controls.get(i)?
            }
        };
        Some(env.view.to_px(pt_of(c)))
    }

    fn cursor_for(&self, env: &Env) -> CursorKind {
        let Some(p) = self.pointer else {
            return CursorKind::Arrow;
        };
        if matches!(self.drag, Some(Drag::Move { .. })) {
            return CursorKind::Move;
        }
        let built = self.build(env);
        if let Some((id, _)) = hit_widget(&widgets_of(&built), p) {
            return match built.ws[id.0 as usize].0.kind {
                WidgetKind::Field { .. } => CursorKind::Text,
                _ => CursorKind::Hand,
            };
        }
        if built.covers(p) {
            return CursorKind::Arrow;
        }
        match hit_test(&self.doc, &env.view, self.sel, p) {
            Hit::Handle(_) => CursorKind::ResizeNwse,
            Hit::Cap(_) => CursorKind::Hand,
            Hit::Body(_) => CursorKind::Move,
            Hit::None => CursorKind::Arrow,
        }
    }

    fn edit_event(&mut self, ev: &UiEvent<'_>, env: &Env, now: u64, out: &mut Vec<EditorOut>) {
        match ev {
            UiEvent::PointerMove { p, mods } => self.pointer_move(*p, *mods, env, now),
            UiEvent::PointerDown { p, button, mods } => {
                self.pointer_down(*p, button, *mods, env, now, out)
            }
            UiEvent::PointerUp { p, button, mods } => {
                self.pointer_up(*p, button, *mods, env, now, out)
            }
            UiEvent::Wheel { p, dy, mods } => self.wheel(*p, *dy, *mods, env, now),
            UiEvent::Key {
                name,
                pressed,
                mods,
            } => {
                if *pressed {
                    self.key_down(name, *mods, env, now, out);
                } else if *name == "KeyH" && self.field.is_none() && self.capture.is_none() {
                    self.hide_held = false;
                }
            }
            UiEvent::KeyRepeat { name, mods } => self.key_repeat(name, *mods, env, now, out),
            UiEvent::Text(s) => {
                if let Some(f) = &mut self.field {
                    f.edit.insert(s);
                }
            }
            UiEvent::Tick => {
                if let Some(c) = &mut self.capture {
                    c.tick(now);
                }
            }
            UiEvent::FocusLost => {
                self.hide_held = false;
                self.cancel_drag();
                if self.capture.take().is_some() {
                    self.last_advice = None;
                }
                self.slider_end();
                if self.field.is_some() {
                    self.field = None;
                    Self::effect(out, Effect::ImeAllowed(false));
                }
            }
            UiEvent::MenuPick(id) => self.menu_pick(*id, now, out),
            UiEvent::EnvChanged
            | UiEvent::RawMotion { .. }
            | UiEvent::Command(_)
            | UiEvent::Pasted(_)
            | UiEvent::Dropped { .. }
            | UiEvent::PlayEnded { .. }
            | UiEvent::PlayStarted => {}
        }
    }

    // ---- helpers over the document ----

    fn begin(&mut self, g: Gesture, now: u64) {
        self.hist.begin(&self.doc, self.sel, g, now);
    }
    fn end(&mut self) {
        self.hist.end(&self.doc);
    }

    /// One undo step around `f`.
    fn single(&mut self, now: u64, f: impl FnOnce(&mut EditDoc)) {
        self.begin(Gesture::Single, now);
        f(&mut self.doc);
        self.end();
    }

    fn cancel_drag(&mut self) {
        if let Some(d) = self.drag.take() {
            d.cancel(&mut self.doc, &mut self.hist);
        }
        self.drag_info = DragUpdate::default();
    }

    fn slider_end(&mut self) {
        if self.slider.take().is_some() {
            self.end();
        }
    }

    fn commit_field(&mut self, now: u64, out: &mut Vec<EditorOut>) {
        let Some(f) = self.field.take() else {
            return;
        };
        let text = f.edit.text().to_string();
        match f.target {
            FieldTarget::ProfileName => {
                if text != self.doc.profile().name {
                    self.single(now, |d| d.set_name(&text));
                }
            }
            FieldTarget::Label(id) => {
                self.single(now, |d| {
                    let _ = d.set_label(id, &text);
                });
            }
        }
        Self::effect(out, Effect::ImeAllowed(false));
    }

    fn start_capture(&mut self, slot: Slot) {
        self.capture = Some(Capture::start(slot));
        self.last_advice = None;
        self.collapsed = false;
        self.armed = None;
    }

    fn capture_step(
        &mut self,
        now: u64,
        f: impl FnOnce(&mut Capture, &mut EditDoc) -> CaptureResult,
    ) {
        let Some(mut c) = self.capture.take() else {
            return;
        };
        self.begin(Gesture::Single, now);
        let r = f(&mut c, &mut self.doc);
        self.end();
        match r {
            CaptureResult::Pending => self.capture = Some(c),
            CaptureResult::Bound { advice } | CaptureResult::Swapped { advice } => {
                self.last_advice = advice;
            }
            CaptureResult::Cancelled => self.last_advice = None,
        }
    }

    // ---- pointer ----

    fn pointer_move(&mut self, p: Pt, mods: Mods, env: &Env, now: u64) {
        self.pointer = Some(p);
        if self.slider.is_some() {
            self.slider_to(p, env);
            return;
        }
        if let Some(mut d) = self.drag.take() {
            self.drag_info = d.update(&mut self.doc, &mut self.hist, &env.view, p, mods, now);
            self.drag = Some(d);
        }
    }

    fn slider_value_of(&self, t: SliderTarget) -> Option<f64> {
        match (
            t,
            self.doc.control(match t {
                SliderTarget::Ring(i) | SliderTarget::Sens(i) | SliderTarget::Reach(i) => i,
            })?,
        ) {
            (SliderTarget::Ring(_), Control::Joystick { radius, .. }) => Some(*radius),
            (SliderTarget::Sens(_), Control::Look { sensitivity, .. }) => Some(*sensitivity),
            (SliderTarget::Reach(_), Control::Look { reach, .. }) => Some(*reach),
            _ => None,
        }
    }

    fn slider_range(t: SliderTarget) -> (f64, f64, bool) {
        match t {
            SliderTarget::Ring(_) => (RADIUS_MIN, RADIUS_MAX, false),
            SliderTarget::Sens(_) => (SENS_SLIDER_MIN, SENS_SLIDER_MAX, true),
            SliderTarget::Reach(_) => (REACH_MIN, REACH_MAX, false),
        }
    }

    fn set_slider(&mut self, t: SliderTarget, v: f64) {
        let _ = match t {
            SliderTarget::Ring(i) => self.doc.set_radius(i, v),
            SliderTarget::Sens(i) => self.doc.set_sensitivity(i, v),
            SliderTarget::Reach(i) => self.doc.set_reach(i, v),
        };
    }

    fn slider_to(&mut self, p: Pt, _env: &Env) {
        let Some(s) = self.slider else {
            return;
        };
        let t = ((p.x - s.track.x) / s.track.w).clamp(0.0, 1.0);
        let (lo, hi, log) = Self::slider_range(s.target);
        self.set_slider(s.target, slider_value(lo, hi, log, t));
    }

    fn pointer_down(
        &mut self,
        p: Pt,
        button: &str,
        mods: Mods,
        env: &Env,
        now: u64,
        out: &mut Vec<EditorOut>,
    ) {
        self.pointer = Some(p);
        let built = self.build(env);
        let ws = widgets_of(&built);
        let wid = hit_widget(&ws, p);

        // Key capture owns the pointer: chips first, any button binds.
        if self.capture.is_some() {
            let act = wid.map(|(id, _)| built.ws[id.0 as usize].1.clone());
            match (button, act) {
                ("MouseLeft", Some(Act::CapCancel)) => {
                    self.capture_step(now, |c, d| c.mouse(d, button, true))
                }
                ("MouseLeft", Some(Act::CapSwap)) => {
                    self.capture_step(now, |c, d| c.confirm_swap(d))
                }
                _ => self.capture_step(now, |c, d| c.mouse(d, button, false)),
            }
            return;
        }

        if button == "MouseRight" {
            self.right_click(p, env, &built, out);
            return;
        }
        if button != "MouseLeft" {
            return;
        }

        // A focused text field ends on any click (a click on the same field
        // keeps it).
        if self.field.is_some() {
            let keep = wid.is_some_and(|(id, _)| matches!(built.ws[id.0 as usize].1, Act::Field(t) if Some(t) == self.field.as_ref().map(|f| f.target)));
            if !keep {
                self.commit_field(now, out);
            }
        }

        if let Some((id, t)) = wid {
            let (w, act) = &built.ws[id.0 as usize];
            let act = act.clone();
            let rect = w.rect;
            let kind = w.kind.clone();
            self.widget_down(act, rect, &kind, t, p, env, now, out);
            return;
        }
        if built.covers(p) {
            return; // on the dock or a banner: swallowed
        }

        // Canvas.
        if let Some(kind) = self.armed.take() {
            if env.view.contains(p) {
                self.place(kind, p, env, now);
            }
            return;
        }
        let hit = hit_test(&self.doc, &env.view, self.sel, p);
        match hit {
            Hit::Cap(slot) => {
                let id = match slot {
                    Slot::Button(i) | Slot::Dir(i, _) => Some(i),
                    _ => None,
                };
                self.sel = id.or(self.sel);
                self.start_capture(slot);
            }
            Hit::Body(id) | Hit::Handle(id) => {
                if matches!(hit, Hit::Body(_)) {
                    self.sel = Some(id);
                    self.last_advice = None;
                }
                self.drag = Drag::from_hit(hit, p, &self.doc, &mut self.hist, now);
            }
            Hit::None => {
                self.sel = None;
                self.last_advice = None;
            }
        }
        let _ = mods;
    }

    fn place(&mut self, kind: Kind, p: Pt, env: &Env, now: u64) {
        let at = env.view.to_norm(p);
        self.begin(Gesture::Single, now);
        let r = self.doc.add(kind, at);
        self.end();
        if let Ok(id) = r {
            self.sel = Some(id);
            if kind == Kind::Button {
                self.start_capture(Slot::Button(id));
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn widget_down(
        &mut self,
        act: Act,
        rect: PxRect,
        kind: &WidgetKind,
        t: f64,
        p: Pt,
        env: &Env,
        now: u64,
        out: &mut Vec<EditorOut>,
    ) {
        let scale = env.view.scale;
        match act {
            Act::None => {}
            Act::Undo => self.undo(),
            Act::Redo => self.redo(),
            Act::Collapse => self.collapsed = true,
            Act::Expand => self.collapsed = false,
            Act::Arm(k) => {
                // Press = start carrying; release over the picture places it,
                // release on the button arms it for the next click.
                self.drag = Some(Drag::place(k));
            }
            Act::Test => self.enter_test(env, now, out),
            Act::Done => {
                self.commit_field(now, out);
                out.push(EditorOut::Save);
                out.push(EditorOut::Leave);
            }
            Act::Revert => self.revert_ask = true,
            Act::RevertNo => self.revert_ask = false,
            Act::RevertYes => {
                self.revert_ask = false;
                let saved = self.saved.clone();
                if let Ok(doc) = EditDoc::from_profile(saved) {
                    self.begin(Gesture::Single, now);
                    self.doc = doc;
                    self.end();
                    self.sel = None;
                }
            }
            Act::Field(target) => {
                let text = match target {
                    FieldTarget::ProfileName => self.doc.profile().name.clone(),
                    FieldTarget::Label(id) => match self.doc.control(id) {
                        Some(
                            Control::Joystick { label, .. }
                            | Control::Look { label, .. }
                            | Control::Button { label, .. },
                        ) => label.clone(),
                        None => String::new(),
                    },
                };
                let max = match target {
                    FieldTarget::ProfileName => 100,
                    FieldTarget::Label(_) => 40,
                };
                let mut edit = TextEdit::new(&text, max);
                edit.select_all();
                self.field = Some(FieldFocus { target, edit });
                Self::effect(out, Effect::ImeAllowed(true));
            }
            Act::Orient(_) | Act::OverlayPick(_) => {} // need the clicked cell: below
            Act::AdviceToggle => self.advice_open = !self.advice_open,
            Act::InfoToggle => self.info_open = !self.info_open,
            Act::AdvancedToggle => self.advanced_open = !self.advanced_open,
            Act::Cap(slot) => self.start_capture(slot),
            Act::CapCancel | Act::CapSwap => {}
            Act::Dup => {
                if let Some(id) = self.sel {
                    self.begin(Gesture::Single, now);
                    let r = self.doc.duplicate(id);
                    self.end();
                    if let Ok(n) = r {
                        self.sel = Some(n);
                    }
                }
            }
            Act::Del => self.delete_selected(now),
            Act::Mode(m) => {
                if let Some(id) = self.sel {
                    self.single(now, |d| {
                        let _ = d.set_mode(id, m);
                    });
                }
            }
            Act::Suggest => {
                if let Some(id) = self.sel {
                    let cur = match self.doc.control(id) {
                        Some(Control::Button { label, .. }) => label.clone(),
                        _ => String::new(),
                    };
                    let next = LABEL_PRESETS_VI
                        .iter()
                        .position(|l| *l == cur)
                        .map_or(0, |i| (i + 1) % LABEL_PRESETS_VI.len());
                    let text = LABEL_PRESETS_VI[next];
                    self.single(now, |d| {
                        let _ = d.set_label(id, text);
                    });
                }
            }
            Act::Keys(preset) => {
                if let Some(id) = self.sel {
                    self.single(now, |d| {
                        let _ = d.set_joystick_keys(id, preset);
                    });
                }
            }
            Act::Slider(target) => {
                let parts = slider_parts(rect, scale);
                let (lo, hi, log) = Self::slider_range(target);
                let cur = self.slider_value_of(target).unwrap_or(lo);
                self.next_slider = self.next_slider.wrapping_add(1);
                self.hist
                    .begin(&self.doc, self.sel, Gesture::Slider(self.next_slider), now);
                if contains(parts.minus, p) {
                    self.set_slider(target, slider_step(lo, hi, log, cur, false, false));
                    self.end();
                } else if contains(parts.plus, p) {
                    self.set_slider(target, slider_step(lo, hi, log, cur, true, false));
                    self.end();
                } else if contains(parts.track, p) {
                    self.slider = Some(SliderDrag {
                        target,
                        track: parts.track,
                    });
                    self.slider_to(p, env);
                } else {
                    self.end();
                }
            }
            Act::SensPreset(v) => {
                if let Some(id) = self.sel {
                    self.single(now, |d| {
                        let _ = d.set_sensitivity(id, v);
                    });
                }
            }
            Act::InvertY => {
                if let Some(id) = self.sel {
                    let on = !matches!(
                        self.doc.control(id),
                        Some(Control::Look { invert_y: true, .. })
                    );
                    self.single(now, |d| {
                        let _ = d.set_invert_y(id, on);
                    });
                }
            }
            Act::Idle(_) => {
                if let (Some(id), WidgetKind::Segmented { options, .. }) = (self.sel, kind) {
                    let i = ((t * options.len() as f64).floor() as usize).min(options.len() - 1);
                    let ms = IDLE_MS[i.min(IDLE_MS.len() - 1)];
                    self.single(now, |d| {
                        let _ = d.set_release_after(id, ms);
                    });
                }
            }
            Act::GuideNext => {
                let ended = self.guide.as_mut().is_some_and(GuideState::advance);
                if ended {
                    self.guide = None;
                    out.push(EditorOut::GuideDone);
                }
            }
            Act::GuideSkip => {
                self.guide = None;
                out.push(EditorOut::GuideDone);
            }
            Act::GuideSaveAndPlay => {
                self.guide = None;
                out.push(EditorOut::GuideDone);
                self.commit_field(now, out);
                out.push(EditorOut::SaveAndPlay);
            }
            Act::SaveYes => {
                self.save_prompt = false;
                out.push(EditorOut::Save);
                out.push(EditorOut::Leave);
            }
            Act::SaveNo => {
                self.save_prompt = false;
                out.push(EditorOut::Leave);
            }
            Act::SaveKeep => self.save_prompt = false,
            Act::TestBack => {}
            Act::Rotate => Self::effect(out, Effect::Rotate),
        }
        // A segmented orientation / overlay pick needs the clicked option.
        if let (Act::Orient(_), WidgetKind::Segmented { options, .. }) = (&act, kind) {
            let i = ((t * options.len() as f64).floor() as usize).min(options.len() - 1);
            let o = if i == 0 {
                Orientation::Landscape
            } else {
                Orientation::Portrait
            };
            if self.doc.profile().orientation != o {
                self.single(now, |d| d.set_orientation(o));
            }
        }
        if let (Act::OverlayPick(_), WidgetKind::Segmented { options, .. }) = (&act, kind) {
            let i = ((t * options.len() as f64).floor() as usize).min(options.len() - 1);
            let o = [Overlay::Auto, Overlay::Always, Overlay::Off][i.min(2)];
            self.overlay = o;
            out.push(EditorOut::SetOverlay(o));
        }
    }

    fn pointer_up(
        &mut self,
        p: Pt,
        button: &str,
        mods: Mods,
        env: &Env,
        now: u64,
        out: &mut Vec<EditorOut>,
    ) {
        let _ = out;
        if button != "MouseLeft" {
            return;
        }
        self.slider_end();
        let Some(d) = self.drag.take() else {
            return;
        };
        self.drag_info = DragUpdate::default();
        if let Drag::Place { kind } = d {
            // Released on its own palette button: a click arms the item.
            let built = self.build(env);
            let ws = widgets_of(&built);
            let over = hit_widget(&ws, p).map(|(id, _)| built.ws[id.0 as usize].1.clone());
            if over == Some(Act::Arm(kind)) {
                self.armed = Some(kind);
                return;
            }
            if built.covers(p) {
                return; // dropped on the dock: nothing
            }
            if let DragEnd::Placed(id) =
                d.finish(&mut self.doc, &mut self.hist, &env.view, p, mods, now)
            {
                self.sel = Some(id);
                if kind == Kind::Button {
                    self.start_capture(Slot::Button(id));
                }
            }
            return;
        }
        match d.finish(&mut self.doc, &mut self.hist, &env.view, p, mods, now) {
            DragEnd::Select(id) => self.sel = Some(id),
            DragEnd::Moved(_)
            | DragEnd::Resized(_)
            | DragEnd::Placed(_)
            | DragEnd::Nothing
            | DragEnd::Refused(_) => {}
        }
    }

    fn wheel(&mut self, p: Pt, dy: f64, mods: Mods, env: &Env, now: u64) {
        let built = self.build(env);
        if built.covers(p) {
            let step = 30.0 * env.view.scale;
            self.scroll = (self.scroll - dy.signum() * step).max(0.0);
            return;
        }
        if self.drag.is_some() || self.capture.is_some() {
            return;
        }
        let Some(id) = self.sel else {
            return;
        };
        if !matches!(
            hit_test(&self.doc, &env.view, self.sel, p),
            Hit::Body(i) | Hit::Handle(i) if i == id
        ) {
            return;
        }
        let d = if mods.shift { 0.002 } else { 0.01 } * dy.signum();
        if d == 0.0 || !d.is_finite() {
            return;
        }
        self.begin(Gesture::Wheel, now);
        match self.doc.control(id) {
            Some(Control::Joystick { radius, .. }) => {
                let v = radius + d;
                let _ = self.doc.set_radius(id, v);
            }
            Some(Control::Look { reach, .. }) => {
                let v = reach + d;
                let _ = self.doc.set_reach(id, v);
            }
            _ => {}
        }
        self.end();
    }

    fn right_click(&mut self, p: Pt, env: &Env, built: &Built, out: &mut Vec<EditorOut>) {
        let l = self.lang;
        if built.covers(p) {
            return;
        }
        match hit_test(&self.doc, &env.view, self.sel, p) {
            Hit::Body(id) | Hit::Handle(id) | Hit::Cap(Slot::Button(id) | Slot::Dir(id, _)) => {
                self.sel = Some(id);
                let mut items = vec![
                    item(l, 1, ui(UiText::ChangeKey), false, true),
                    item(
                        l,
                        2,
                        ui(UiText::Duplicate),
                        false,
                        self.doc.kind(id) != Some(Kind::Look),
                    ),
                    item(l, 3, ui(UiText::Delete), false, true),
                ];
                if let Some(Control::Button { mode, .. }) = self.doc.control(id) {
                    items.push(MenuItem::Separator);
                    items.push(item(
                        l,
                        4,
                        ui(UiText::ModeHoldItem),
                        *mode == ButtonMode::Hold,
                        true,
                    ));
                    items.push(item(
                        l,
                        5,
                        ui(UiText::ModeTapItem),
                        *mode == ButtonMode::Tap,
                        true,
                    ));
                }
                self.menu = Some(MenuCtx::Control(id));
                Self::effect(out, Effect::ShowMenu(MenuSpec { items }));
            }
            _ => {
                if !env.view.contains(p) {
                    return;
                }
                let has_look = self
                    .doc
                    .ids()
                    .iter()
                    .any(|i| self.doc.kind(*i) == Some(Kind::Look));
                let items = vec![MenuItem::Sub {
                    label: ui(UiText::AddHere).text(l),
                    items: vec![
                        item(l, 11, ui(UiText::PalJoystick), false, true),
                        item(l, 12, ui(UiText::PalLook), false, !has_look),
                        item(l, 13, ui(UiText::PalButton), false, true),
                    ],
                }];
                self.menu = Some(MenuCtx::Add(env.view.to_norm(p)));
                Self::effect(out, Effect::ShowMenu(MenuSpec { items }));
            }
        }
    }

    fn menu_pick(&mut self, id: u32, now: u64, _out: &mut Vec<EditorOut>) {
        let Some(ctx) = self.menu.take() else {
            return;
        };
        match (ctx, id) {
            (MenuCtx::Control(c), 1) => {
                let slot = match self.doc.kind(c) {
                    Some(Kind::Button) => Some(Slot::Button(c)),
                    Some(Kind::Joystick) => Some(Slot::Dir(c, Dir::Up)),
                    _ => None,
                };
                if let Some(s) = slot {
                    self.sel = Some(c);
                    self.start_capture(s);
                }
            }
            (MenuCtx::Control(c), 2) => {
                self.begin(Gesture::Single, now);
                let r = self.doc.duplicate(c);
                self.end();
                if let Ok(n) = r {
                    self.sel = Some(n);
                }
            }
            (MenuCtx::Control(c), 3) => {
                self.sel = Some(c);
                self.delete_selected(now);
            }
            (MenuCtx::Control(c), 4 | 5) => {
                let m = if id == 4 {
                    ButtonMode::Hold
                } else {
                    ButtonMode::Tap
                };
                self.single(now, |d| {
                    let _ = d.set_mode(c, m);
                });
            }
            (MenuCtx::Add(at), 11..=13) => {
                let kind = match id {
                    11 => Kind::Joystick,
                    12 => Kind::Look,
                    _ => Kind::Button,
                };
                self.begin(Gesture::Single, now);
                let r = self.doc.add(kind, at);
                self.end();
                if let Ok(n) = r {
                    self.sel = Some(n);
                    if kind == Kind::Button {
                        self.start_capture(Slot::Button(n));
                    }
                }
            }
            _ => {}
        }
    }

    fn delete_selected(&mut self, now: u64) {
        if let Some(id) = self.sel {
            self.begin(Gesture::Single, now);
            let _ = self.doc.remove(id);
            self.end();
            self.sel = None;
        }
    }

    fn undo(&mut self) {
        self.cancel_drag();
        if let Some(s) = self.hist.undo(&mut self.doc) {
            self.sel = s.filter(|i| self.doc.index_of(*i).is_some());
        }
    }

    fn redo(&mut self) {
        self.cancel_drag();
        if let Some(s) = self.hist.redo(&mut self.doc) {
            self.sel = s.filter(|i| self.doc.index_of(*i).is_some());
        }
    }

    // ---- keyboard ----

    fn key_down(&mut self, name: &str, mods: Mods, env: &Env, now: u64, out: &mut Vec<EditorOut>) {
        // 1. capture owns every key (F-keys included)
        if self.capture.is_some() {
            if name == "F8" {
                self.capture = None;
                self.last_advice = None;
            } else {
                self.capture_step(now, |c, d| c.key(d, name, now));
            }
            return;
        }
        // 2. a focused text field consumes everything
        if self.field.is_some() {
            self.field_key(name, mods, now, out);
            return;
        }
        // F8 cancels a drag
        if name == "F8" {
            self.cancel_drag();
            self.armed = None;
            self.slider_end();
            return;
        }
        if self.save_prompt && name == "Escape" {
            self.save_prompt = false;
            return;
        }
        if name == "KeyH" && !mods.ctrl {
            self.hide_held = true;
            return;
        }
        if self.drag.is_some() && !matches!(name, "Escape") {
            // A drag in progress: keys that edit are ignored until it ends.
            return;
        }
        if mods.ctrl {
            match name {
                "KeyZ" if mods.shift => self.redo(),
                "KeyZ" => self.undo(),
                "KeyY" => self.redo(),
                "KeyD" => {
                    if let Some(id) = self.sel {
                        self.begin(Gesture::Single, now);
                        let r = self.doc.duplicate(id);
                        self.end();
                        if let Ok(n) = r {
                            self.sel = Some(n);
                        }
                    }
                }
                "KeyS" => {
                    out.push(EditorOut::Save);
                }
                _ => {}
            }
            return;
        }
        if name == self.doc.profile().toggle_key {
            self.enter_test(env, now, out);
            return;
        }
        match name {
            "Escape" => self.escape(now, out),
            "Delete" | "Backspace" => self.delete_selected(now),
            "Tab" => self.tab(mods.shift),
            "Enter" | "NumpadEnter" => {
                if let Some(id) = self.sel {
                    match self.doc.kind(id) {
                        Some(Kind::Button) => self.start_capture(Slot::Button(id)),
                        Some(Kind::Joystick) => self.start_capture(Slot::Dir(id, Dir::Up)),
                        _ => {}
                    }
                }
            }
            "ArrowLeft" | "ArrowRight" | "ArrowUp" | "ArrowDown" => {
                self.nudge(name, mods, env, now)
            }
            _ => {}
        }
    }

    fn key_repeat(
        &mut self,
        name: &str,
        mods: Mods,
        env: &Env,
        now: u64,
        _out: &mut Vec<EditorOut>,
    ) {
        if self.test.is_some() || self.capture.is_some() {
            return;
        }
        if let Some(f) = &mut self.field {
            apply_field_key(&mut f.edit, name, mods);
            return;
        }
        if matches!(name, "ArrowLeft" | "ArrowRight" | "ArrowUp" | "ArrowDown")
            && self.drag.is_none()
        {
            self.nudge(name, mods, env, now);
        }
    }

    fn field_key(&mut self, name: &str, mods: Mods, now: u64, out: &mut Vec<EditorOut>) {
        match name {
            "Enter" | "NumpadEnter" => self.commit_field(now, out),
            "Escape" => {
                self.field = None;
                Self::effect(out, Effect::ImeAllowed(false));
            }
            "KeyA" if mods.ctrl => {
                if let Some(f) = &mut self.field {
                    f.edit.select_all();
                }
            }
            _ => {
                if let Some(f) = &mut self.field {
                    apply_field_key(&mut f.edit, name, mods);
                }
            }
        }
    }

    fn escape(&mut self, now: u64, out: &mut Vec<EditorOut>) {
        let _ = now;
        if self.drag.is_some() {
            self.cancel_drag();
        } else if self.armed.is_some() {
            self.armed = None;
        } else if self.revert_ask {
            self.revert_ask = false;
        } else if self.sel.is_some() {
            self.sel = None;
        } else if self.hist.dirty() {
            self.save_prompt = true;
            self.collapsed = false;
        } else {
            out.push(EditorOut::Leave);
        }
    }

    fn tab(&mut self, back: bool) {
        let ids = self.doc.ids();
        if ids.is_empty() {
            return;
        }
        let i = self.sel.and_then(|s| ids.iter().position(|x| *x == s));
        let n = ids.len();
        let next = match (i, back) {
            (None, false) => 0,
            (None, true) => n - 1,
            (Some(i), false) => (i + 1) % n,
            (Some(i), true) => (i + n - 1) % n,
        };
        self.sel = Some(ids[next]);
        self.last_advice = None;
    }

    fn nudge(&mut self, name: &str, mods: Mods, env: &Env, now: u64) {
        let Some(id) = self.sel else {
            return;
        };
        let Some(c) = self.doc.control(id) else {
            return;
        };
        let px = if mods.shift { 10.0 } else { 1.0 };
        let (dx, dy) = match name {
            "ArrowLeft" => (-px, 0.0),
            "ArrowRight" => (px, 0.0),
            "ArrowUp" => (0.0, -px),
            _ => (0.0, px),
        };
        let cur = pt_of(c);
        let w = env.view.picture.w.max(1.0);
        let h = env.view.picture.h.max(1.0);
        self.begin(Gesture::Nudge, now);
        let _ = self.doc.move_to(
            id,
            Pt {
                x: cur.x + dx / w,
                y: cur.y + dy / h,
            },
        );
        self.end();
    }

    // ------------------------------------------------------------- Test ----

    fn enter_test(&mut self, env: &Env, now: u64, out: &mut Vec<EditorOut>) {
        if self.test.is_some() {
            return;
        }
        self.commit_field(now, out);
        self.cancel_drag();
        self.slider_end();
        self.armed = None;
        let profile = self.doc.profile().clone();
        let Ok(mut engine) = Engine::new(profile.clone()) else {
            return;
        };
        let rotation = if profile.orientation.matches(env.rotation) {
            env.rotation
        } else if profile.orientation == Orientation::Landscape {
            Rotation::R90
        } else {
            Rotation::R0
        };
        let (pw, ph) = if env.panel.0 > 0.0 && env.panel.1 > 0.0 {
            env.panel
        } else {
            (1206.0, 2622.0)
        };
        let view = View::from_panel(pw, ph, rotation);
        let _ = engine.set_view(view);
        let _ = engine.activate();
        let has_look = profile
            .controls
            .iter()
            .any(|c| matches!(c, Control::Look { .. }));
        let last = engine.stats();
        self.test = Some(TestRun {
            engine,
            view,
            tracker: HeldTracker::new(&profile),
            dots: TouchDots::default(),
            held: Vec::new(),
            full_until: 0,
            flash_until: 0,
            last,
            has_look,
        });
        if let Some(g) = &mut self.guide {
            g.on_test_entered();
        }
        if has_look {
            Self::effect(out, Effect::GrabPointer(true));
        }
    }

    fn leave_test(&mut self, out: &mut Vec<EditorOut>) {
        if let Some(t) = self.test.take()
            && t.has_look
        {
            Self::effect(out, Effect::GrabPointer(false));
        }
    }

    fn feed(&mut self, input: Input<'_>, now: u64) {
        let Some(t) = &mut self.test else {
            return;
        };
        let o = t.engine.handle(input, now);
        for f in o.frames {
            t.dots = dots_from_frame(t.dots, &f, &t.view);
        }
        let s = t.engine.stats();
        if s.dropped_presses > t.last.dropped_presses {
            t.full_until = now + 2000;
        }
        if s.look_recenters > t.last.look_recenters {
            t.flash_until = now + 250;
        }
        t.last = s;
        if t.dots.dots.iter().any(Option::is_some)
            && let Some(g) = &mut self.guide
        {
            g.on_contact_seen();
        }
    }

    fn test_key(&mut self, name: &str, pressed: bool, now: u64) {
        let Some(t) = &mut self.test else {
            return;
        };
        if self.doc.owner_of(name).is_some() {
            t.held.retain(|k| k != name);
            if pressed {
                t.held.push(name.to_string());
            }
        }
        t.tracker.on_key(name, pressed);
        let input = if pressed {
            Input::Press(name)
        } else {
            Input::Release(name)
        };
        self.feed(input, now);
    }

    fn test_event(&mut self, ev: &UiEvent<'_>, env: &Env, now: u64, out: &mut Vec<EditorOut>) {
        let toggle = self.doc.profile().toggle_key.clone();
        let panic = self.doc.profile().panic_key.clone();
        match ev {
            UiEvent::Key {
                name,
                pressed: true,
                ..
            } => {
                if *name == toggle || *name == panic || matches!(*name, "Escape" | "F8" | "F12") {
                    self.leave_test(out);
                } else if !matches!(*name, "F1" | "F2" | "F3" | "F4" | "F5" | "F9") {
                    self.test_key(name, true, now);
                }
            }
            UiEvent::Key {
                name,
                pressed: false,
                ..
            } => {
                if !(*name == toggle || *name == panic) {
                    self.test_key(name, false, now);
                }
            }
            UiEvent::PointerDown { p, button, .. } => {
                self.pointer = Some(*p);
                let built = self.build(env);
                let ws = widgets_of(&built);
                if let Some((id, _)) = hit_widget(&ws, *p) {
                    if built.ws[id.0 as usize].1 == Act::TestBack && *button == "MouseLeft" {
                        self.leave_test(out);
                    }
                } else if is_mouse_name(button) {
                    self.test_key(button, true, now);
                }
            }
            UiEvent::PointerUp { button, .. } => {
                if is_mouse_name(button) {
                    self.test_key(button, false, now);
                }
            }
            UiEvent::PointerMove { p, .. } => self.pointer = Some(*p),
            UiEvent::RawMotion { dx, dy } => {
                self.feed(Input::MouseMotion { dx: *dx, dy: *dy }, now);
            }
            UiEvent::Tick => {
                if let Some(t) = &mut self.test {
                    for f in t.engine.tick(now) {
                        t.dots = dots_from_frame(t.dots, &f, &t.view);
                    }
                }
            }
            UiEvent::FocusLost => self.leave_test(out),
            _ => {}
        }
        if self.test.is_none() {
            self.hide_held = false;
        }
    }

    // ------------------------------------------------------------ scene ----

    pub fn widgets(&self, env: &Env, _lang: Lang) -> Vec<Widget> {
        widgets_of(&self.build(env))
    }

    pub fn scene(&self, env: &Env, lang: Lang, now_ms: u64) -> Scene {
        let scale = env.view.scale;
        let mut s = Scene::new(0, scale);
        let built = self.build(env);
        if self.test.is_some() {
            self.draw_test(env, lang, now_ms, &built, &mut s);
            return s;
        }
        let v = &env.view;
        if !env.has_picture {
            s.statics.push(Prim::Rect {
                r: v.picture,
                radius: 0.0,
                tone: Tone::Field,
            });
        }
        self.draw_edges(v, &mut s.statics);
        self.draw_controls(env, lang, &mut s.statics);
        // guide ring
        if let Some(g) = &self.guide
            && let Some(i) = g.target(self.doc.profile())
            && let Some(c) = self.doc.profile().controls.get(i)
        {
            let ctr = v.to_px(pt_of(c));
            let r = match c {
                Control::Joystick { radius, .. } => radius * v.short_px() + v.lp(10.0),
                Control::Look { reach, .. } => reach * v.short_px() * 1.414 + v.lp(6.0),
                Control::Button { .. } => v.lp(BUTTON_RADIUS_LP + 12.0),
            };
            s.statics.push(Prim::Ring {
                c: ctr,
                r,
                width: v.lp(3.0),
                tone: Tone::Warn,
            });
        }
        for g in &self.drag_info.guides {
            let Guide { a, b } = *g;
            s.statics.push(Prim::Line {
                a,
                b,
                width: v.lp(1.5),
                dashed: true,
                tone: Tone::Guide,
            });
        }
        if let (Some(val), Some(p)) = (self.drag_info.value, self.pointer) {
            s.statics.push(Prim::text(
                rc(p.x + v.lp(14.0), p.y - v.lp(24.0), v.lp(60.0), v.lp(20.0)),
                fmt_num(val, 2, lang),
                Font::Small,
                Align::Left,
                Tone::Selected,
            ));
        }
        if let Some(p) = self.pointer {
            let carrying = match (&self.drag, self.armed) {
                (Some(Drag::Place { kind }), _) => Some(*kind),
                (_, Some(k)) => Some(k),
                _ => None,
            };
            if let Some(k) = carrying {
                self.ghost(k, p, v, &mut s.statics);
            }
        }
        // dock and banners last
        let all = widgets_of(&built);
        let (canvas, dock_ws) = all.split_at(built.canvas_n.min(all.len()));
        s.statics.extend(widgets_to_prims(canvas, lang, scale));
        if let Some(d) = built.dock {
            s.statics.push(Prim::Rect {
                r: d,
                radius: v.lp(10.0),
                tone: Tone::Card,
            });
            s.statics.push(Prim::Frame {
                r: d,
                radius: v.lp(10.0),
                width: v.lp(1.0),
                dashed: false,
                tone: Tone::CardLine,
            });
        }
        s.statics.extend(widgets_to_prims(dock_ws, lang, scale));
        s
    }

    fn draw_edges(&self, v: &EditView, out: &mut Vec<Prim>) {
        let p = v.picture;
        let (bx, by) = (p.w * super::EDGE_BAND, p.h * super::EDGE_BAND);
        for r in [
            rc(p.x, p.y, p.w, by),
            rc(p.x, p.y + p.h - by, p.w, by),
            rc(p.x, p.y + by, bx, p.h - 2.0 * by),
            rc(p.x + p.w - bx, p.y + by, bx, p.h - 2.0 * by),
        ] {
            out.push(Prim::Hatch {
                r,
                tone: Tone::Hatch,
            });
        }
    }

    fn ghost(&self, k: Kind, p: Pt, v: &EditView, out: &mut Vec<Prim>) {
        match k {
            Kind::Button => out.push(Prim::Disc {
                c: p,
                r: v.lp(BUTTON_RADIUS_LP),
                tone: Tone::Ghost,
            }),
            Kind::Joystick => out.push(Prim::Ring {
                c: p,
                r: 0.11 * v.short_px(),
                width: v.lp(2.0),
                tone: Tone::Ghost,
            }),
            Kind::Look => {
                let h = 0.30 * v.short_px();
                out.push(Prim::Frame {
                    r: rc(p.x - h, p.y - h, 2.0 * h, 2.0 * h),
                    radius: 0.0,
                    width: v.lp(2.0),
                    dashed: true,
                    tone: Tone::Ghost,
                });
            }
        }
    }

    fn draw_controls(&self, env: &Env, lang: Lang, out: &mut Vec<Prim>) {
        let v = &env.view;
        for (idx, id) in self.doc.ids().iter().copied().enumerate() {
            let Some(c) = self.doc.profile().controls.get(idx) else {
                continue;
            };
            let selected = self.sel == Some(id) && self.test.is_none();
            let tone = if selected {
                Tone::Marker
            } else {
                Tone::MarkerDim
            };
            let ctr = v.to_px(pt_of(c));
            let name = self.short_name(id, lang);
            match c {
                Control::Joystick { radius, .. } => {
                    out.push(Prim::Ring {
                        c: ctr,
                        r: radius * v.short_px(),
                        width: v.lp(2.0),
                        tone,
                    });
                    out.push(Prim::Disc {
                        c: ctr,
                        r: v.lp(3.0),
                        tone,
                    });
                    if selected {
                        out.push(Prim::Ring {
                            c: ctr,
                            r: radius * v.short_px() + v.lp(4.0),
                            width: v.lp(2.0),
                            tone: Tone::Selected,
                        });
                    }
                }
                Control::Look { reach, .. } => {
                    let h = reach * v.short_px();
                    let r = rc(ctr.x - h, ctr.y - h, 2.0 * h, 2.0 * h);
                    out.push(Prim::Frame {
                        r,
                        radius: 0.0,
                        width: v.lp(2.0),
                        dashed: true,
                        tone,
                    });
                    let a = v.lp(8.0);
                    out.push(Prim::Line {
                        a: Pt {
                            x: ctr.x - a,
                            y: ctr.y,
                        },
                        b: Pt {
                            x: ctr.x + a,
                            y: ctr.y,
                        },
                        width: v.lp(2.0),
                        dashed: false,
                        tone,
                    });
                    out.push(Prim::Line {
                        a: Pt {
                            x: ctr.x,
                            y: ctr.y - a,
                        },
                        b: Pt {
                            x: ctr.x,
                            y: ctr.y + a,
                        },
                        width: v.lp(2.0),
                        dashed: false,
                        tone,
                    });
                    let lw = v.lp(110.0).min(2.0 * h - v.lp(8.0)).max(0.0);
                    out.push(Prim::text(
                        rc(r.x + v.lp(4.0), r.y + v.lp(2.0), lw, v.lp(18.0)),
                        ellipsize(&name, super::scene::max_chars(lw, Font::Small, v.scale)),
                        Font::Small,
                        Align::Left,
                        tone,
                    ));
                    if selected {
                        out.push(Prim::Frame {
                            r,
                            radius: 0.0,
                            width: v.lp(2.0),
                            dashed: false,
                            tone: Tone::Selected,
                        });
                    }
                }
                Control::Button { .. } => {
                    out.push(Prim::Disc {
                        c: ctr,
                        r: v.lp(BUTTON_RADIUS_LP),
                        tone,
                    });
                    if selected {
                        out.push(Prim::Ring {
                            c: ctr,
                            r: v.lp(BUTTON_RADIUS_LP + 4.0),
                            width: v.lp(2.0),
                            tone: Tone::Selected,
                        });
                    }
                    // name under the cap (or under the disc when the cap is above)
                    let below = cap_rects(&self.doc, v, id).first().map_or(
                        ctr.y + v.lp(BUTTON_RADIUS_LP),
                        |(_, r)| {
                            if r.y > ctr.y {
                                r.y + r.h
                            } else {
                                ctr.y + v.lp(BUTTON_RADIUS_LP)
                            }
                        },
                    );
                    let nw = v.lp(110.0);
                    out.push(Prim::text(
                        rc(ctr.x - nw / 2.0, below + v.lp(1.0), nw, v.lp(16.0)),
                        ellipsize(&name, super::scene::max_chars(nw, Font::Small, v.scale)),
                        Font::Small,
                        Align::Center,
                        tone,
                    ));
                }
            }
            for (slot, r) in cap_rects(&self.doc, v, id) {
                let listening = self.capture.as_ref().is_some_and(|c| c.slot() == slot);
                out.push(Prim::Rect {
                    r,
                    radius: v.lp(5.0),
                    tone: if listening {
                        Tone::FieldFocus
                    } else {
                        Tone::Card
                    },
                });
                out.push(Prim::Frame {
                    r,
                    radius: v.lp(5.0),
                    width: v.lp(if listening { 2.0 } else { 1.0 }),
                    dashed: false,
                    tone: if listening { Tone::Accent } else { tone },
                });
                let t = if listening {
                    "…".to_string()
                } else {
                    self.doc
                        .key_of(slot)
                        .map(|k| cap_text(k, lang))
                        .unwrap_or_default()
                };
                out.push(Prim::text(r, t, Font::Cap, Align::Center, Tone::Text));
            }
            if selected && let Some(h) = handle_pos(&self.doc, v, id) {
                out.push(Prim::Disc {
                    c: h,
                    r: v.lp(6.0),
                    tone: Tone::Selected,
                });
            }
        }
        let _ = button_hit_radius;
    }

    fn draw_test(&self, env: &Env, lang: Lang, now: u64, built: &Built, s: &mut Scene) {
        let v = &env.view;
        let Some(t) = &self.test else {
            return;
        };
        if !env.has_picture {
            s.statics.push(Prim::Rect {
                r: v.picture,
                radius: 0.0,
                tone: Tone::Field,
            });
        }
        self.draw_edges(v, &mut s.statics);
        self.draw_controls(env, lang, &mut s.statics);
        for (idx, c) in self.doc.profile().controls.iter().enumerate().take(64) {
            let ctr = v.to_px(pt_of(c));
            let shape = match c {
                Control::Joystick { radius, .. } => Shape::Disc {
                    c: ctr,
                    r: (0.3 * radius * v.short_px()).max(v.lp(8.0)),
                },
                Control::Look { reach, .. } => {
                    let h = reach * v.short_px();
                    Shape::Rect {
                        r: rc(ctr.x - h, ctr.y - h, 2.0 * h, 2.0 * h),
                        radius: 0.0,
                    }
                }
                Control::Button { .. } => Shape::Ring {
                    c: ctr,
                    r: v.lp(BUTTON_RADIUS_LP + 4.0),
                    width: v.lp(5.0),
                },
            };
            let look = matches!(c, Control::Look { .. });
            if !look {
                s.dynamics.push(Prim::PressFill {
                    control: idx as u8,
                    shape,
                    tone: Tone::Pressed,
                });
            }
        }
        if now < t.flash_until {
            for c in &self.doc.profile().controls {
                if let Control::Look { x, y, reach, .. } = c {
                    let ctr = v.to_px(Pt { x: *x, y: *y });
                    let h = reach * v.short_px();
                    s.dynamics.push(Prim::Frame {
                        r: rc(ctr.x - h, ctr.y - h, 2.0 * h, 2.0 * h),
                        radius: 0.0,
                        width: v.lp(4.0),
                        dashed: false,
                        tone: Tone::Accent,
                    });
                }
            }
        }
        for (i, d) in t.dots.dots.iter().enumerate() {
            if let Some((x, y)) = d {
                let c = v.to_px(Pt { x: *x, y: *y });
                s.dynamics.push(Prim::Disc {
                    c,
                    r: v.lp(14.0),
                    tone: Tone::Finger,
                });
                s.dynamics.push(Prim::Ring {
                    c,
                    r: v.lp(14.0),
                    width: v.lp(2.0),
                    tone: Tone::Accent,
                });
                s.dynamics.push(Prim::text(
                    rc(c.x - v.lp(14.0), c.y - v.lp(14.0), v.lp(28.0), v.lp(28.0)),
                    (i + 1).to_string(),
                    Font::Body,
                    Align::Center,
                    Tone::Text,
                ));
            }
        }
        s.statics
            .extend(widgets_to_prims(&widgets_of(built), lang, v.scale));
    }

    // ----------------------------------------------------------- widgets ----

    fn build(&self, env: &Env) -> Built {
        if self.test.is_some() {
            self.build_test(env)
        } else {
            self.build_edit(env)
        }
    }

    fn build_test(&self, env: &Env) -> Built {
        let v = &env.view;
        let sc = v.scale;
        let mut b = Built::default();
        let m = v.lp(8.0);
        let p = v.picture;
        let top = rc(p.x + m, p.y + m, p.w - 2.0 * m, v.lp(44.0));
        b.push(
            Widget {
                id: WidgetId(0),
                rect: top,
                kind: WidgetKind::Banner {
                    msg: Msg::TestBanner,
                    tone: Tone::Accent,
                },
                enabled: true,
            },
            Act::None,
        );
        let bw = v.lp(140.0);
        b.push(
            Widget {
                id: WidgetId(0),
                rect: rc(
                    top.x + top.w - bw - v.lp(6.0),
                    top.y + v.lp(5.0),
                    bw,
                    v.lp(34.0),
                ),
                kind: WidgetKind::Button {
                    label: ui(UiText::BackToEdit),
                    primary: false,
                },
                enabled: true,
            },
            Act::TestBack,
        );
        let Some(t) = &self.test else {
            return b;
        };
        b.canvas_n = usize::MAX;
        let h = v.lp(28.0);
        let y = p.y + p.h - m - h;
        let n = t.engine.contacts_down();
        let left = rc(p.x + m, y, v.lp(110.0), h);
        b.push(
            Widget {
                id: WidgetId(0),
                rect: left,
                kind: WidgetKind::Label(ui(UiText::FingersDown(n))),
                enabled: true,
            },
            Act::None,
        );
        let held = if t.held.is_empty() {
            "–".to_string()
        } else {
            t.held
                .iter()
                .map(|k| key_label(k, self.lang))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let _ = sc;
        b.push(
            Widget {
                id: WidgetId(0),
                rect: rc(
                    left.x + left.w + m,
                    y,
                    p.w - 2.0 * m - v.lp(110.0) - v.lp(120.0) - 2.0 * m,
                    h,
                ),
                kind: WidgetKind::Label(ui(UiText::HeldKeys(held))),
                enabled: true,
            },
            Act::None,
        );
        b.push(
            Widget {
                id: WidgetId(0),
                rect: rc(p.x + p.w - m - v.lp(120.0), y, v.lp(120.0), h),
                kind: WidgetKind::Label(ui(UiText::PanicHint(
                    self.doc.profile().panic_key.clone(),
                ))),
                enabled: true,
            },
            Act::None,
        );
        b
    }

    /// Test banner extra: only drawn while the 6th-finger flash is on.
    pub fn test_full_message(&self, now_ms: u64) -> Option<Msg> {
        let t = self.test.as_ref()?;
        (now_ms < t.full_until).then_some(Msg::TestFull)
    }

    fn build_edit(&self, env: &Env) -> Built {
        let v = &env.view;
        let sc = v.scale;
        let mut b = Built::default();
        let prof = self.doc.profile();

        // Canvas banners.
        let m = v.lp(8.0);
        let p = v.picture;
        if !env.has_picture {
            let w = p.w - 2.0 * m;
            let lines = line_count(
                &Msg::NoPicture.text(self.lang),
                w - v.lp(16.0),
                Font::Small,
                sc,
            );
            b.push(
                Widget {
                    id: WidgetId(0),
                    rect: rc(
                        p.x + m,
                        p.y + p.h / 2.0 - v.lp(20.0),
                        w,
                        v.lp(16.0 + 17.0 * lines as f64),
                    ),
                    kind: WidgetKind::Banner {
                        msg: Msg::NoPicture,
                        tone: Tone::Warn,
                    },
                    enabled: true,
                },
                Act::None,
            );
        } else if !prof.orientation.matches(env.rotation) {
            let w = (p.w - 2.0 * m).min(v.lp(520.0));
            let msg = Msg::OrientationBanner {
                profile: prof.orientation,
            };
            let r = rc(p.x + m, p.y + m, w, v.lp(44.0));
            b.push(
                Widget {
                    id: WidgetId(0),
                    rect: r,
                    kind: WidgetKind::Banner {
                        msg,
                        tone: Tone::Warn,
                    },
                    enabled: true,
                },
                Act::None,
            );
            let bw = v.lp(140.0).min(w * 0.4);
            b.push(
                Widget {
                    id: WidgetId(0),
                    rect: rc(r.x + r.w - bw - v.lp(6.0), r.y + v.lp(5.0), bw, v.lp(34.0)),
                    kind: WidgetKind::Button {
                        label: Msg::Fix(super::msg::Fix::RotateView),
                        primary: true,
                    },
                    enabled: true,
                },
                Act::Rotate,
            );
        }

        b.canvas_n = b.ws.len();
        // Dock.
        let grown = self.grown(env);
        let focus = self.focus_point(env);
        let placement = dock_placement_with(v, focus, grown, self.placement);
        let dock = dock_rect(v, placement, self.collapsed);
        if self.collapsed {
            b.push(
                Widget {
                    id: WidgetId(0),
                    rect: dock,
                    kind: WidgetKind::Button {
                        label: ui(UiText::CollapsedPill),
                        primary: true,
                    },
                    enabled: true,
                },
                Act::Expand,
            );
            return b;
        }
        b.dock = Some(dock);
        b.dock_rect = Some(dock);

        // Header (pinned).
        let mut col = Column::new(dock, sc);
        let title = col.row(TITLE_H - GAP);
        let name = if prof.name.trim().is_empty() {
            Msg::NewProfileName(1).text(self.lang)
        } else {
            prof.name.clone()
        };
        let tw = title.w - v.lp(36.0);
        b.push(
            Widget {
                id: WidgetId(0),
                rect: rc(title.x, title.y, tw, title.h),
                kind: WidgetKind::Heading(ui(UiText::Editing(ellipsize(
                    &name,
                    super::scene::max_chars(tw, Font::Title, sc).saturating_sub(10),
                )))),
                enabled: true,
            },
            Act::None,
        );
        b.push(
            Widget {
                id: WidgetId(0),
                rect: rc(
                    title.x + title.w - v.lp(32.0),
                    title.y + v.lp(2.0),
                    v.lp(32.0),
                    v.lp(32.0),
                ),
                kind: WidgetKind::Button {
                    label: Msg::Verbatim("–".into()),
                    primary: false,
                },
                enabled: true,
            },
            Act::Collapse,
        );
        let row = col.row(ROW_H);
        let halves = split(row, &[1.0, 1.0], sc);
        b.push(
            Widget {
                id: WidgetId(0),
                rect: halves[0],
                kind: WidgetKind::Button {
                    label: ui(UiText::UndoBtn),
                    primary: false,
                },
                enabled: self.hist.can_undo(),
            },
            Act::Undo,
        );
        b.push(
            Widget {
                id: WidgetId(0),
                rect: halves[1],
                kind: WidgetKind::Button {
                    label: ui(UiText::RedoBtn),
                    primary: false,
                },
                enabled: self.hist.can_redo(),
            },
            Act::Redo,
        );
        let header_bottom = col.y;

        // Footer (pinned at the bottom): fingers row, buttons row, note.
        let note_lines = line_count(
            &Msg::Ui(UiText::MouseNotControlling)
                .text(self.lang)
                .max(Msg::Ui(UiText::MouseNotControlling).text(Lang::En)),
            col.w,
            Font::Small,
            sc,
        );
        let note_h = note_lines as f64 * Font::Small.line_lp() * sc;
        let foot_h = (ROW_H + GAP) * 2.0 * sc + note_h + PAD_F * sc;
        let foot_top = dock.y + dock.h - foot_h;
        let mut fc = Column {
            x: col.x,
            w: col.w,
            y: foot_top,
            scale: sc,
        };
        let frow = fc.row(ROW_H);
        let n = self.doc.ids().len();
        let fing = if n > 5 {
            ui(UiText::FingersMany)
        } else {
            ui(UiText::Fingers(n))
        };
        b.push(
            Widget {
                id: WidgetId(0),
                rect: rc(frow.x, frow.y, frow.w - v.lp(40.0), frow.h),
                kind: WidgetKind::Label(fing),
                enabled: true,
            },
            Act::None,
        );
        b.push(
            Widget {
                id: WidgetId(0),
                rect: rc(
                    frow.x + frow.w - v.lp(32.0),
                    frow.y + v.lp(2.0),
                    v.lp(32.0),
                    v.lp(32.0),
                ),
                kind: WidgetKind::Button {
                    label: Msg::Verbatim("i".into()),
                    primary: false,
                },
                enabled: true,
            },
            Act::InfoToggle,
        );
        let brow = fc.row(ROW_H);
        let bs = split(brow, &[1.0, 1.0], sc);
        b.push(
            Widget {
                id: WidgetId(0),
                rect: bs[0],
                kind: WidgetKind::Button {
                    label: ui(UiText::TestBtn),
                    primary: false,
                },
                enabled: self.capture.is_none() && self.field.is_none(),
            },
            Act::Test,
        );
        b.push(
            Widget {
                id: WidgetId(0),
                rect: bs[1],
                kind: WidgetKind::Button {
                    label: ui(UiText::DoneBtn),
                    primary: true,
                },
                enabled: true,
            },
            Act::Done,
        );
        let nr = fc.text_rows(note_lines, Font::Small);
        b.push(
            Widget {
                id: WidgetId(0),
                rect: nr,
                kind: WidgetKind::Note(ui(UiText::MouseNotControlling)),
                enabled: true,
            },
            Act::None,
        );

        // Middle (scrolls).
        let region = rc(
            dock.x,
            header_bottom,
            dock.w,
            (foot_top - header_bottom - GAP * sc).max(0.0),
        );
        let mut mid = Vec::new();
        let mut mc = Column {
            x: col.x,
            w: col.w,
            y: region.y,
            scale: sc,
        };
        self.middle(&mut mc, env, &mut mid);
        let content_h = mc.y - region.y;
        let max_scroll = (content_h - region.h).max(0.0);
        let sc_off = self.scroll.min(max_scroll);
        for (mut w, a) in mid {
            w.rect.y -= sc_off;
            let r = w.rect;
            if r.y >= region.y - 0.5 && r.y + r.h <= region.y + region.h + 0.5 {
                b.push(w, a);
            }
        }
        b
    }

    fn note(col: &mut Column, mid: &mut Vec<(Widget, Act)>, msg: Msg) {
        let lines = line_count(
            // the longer language decides the height so both fit
            &longest(&msg),
            col.w,
            Font::Small,
            col.scale,
        );
        let r = col.text_rows(lines, Font::Small);
        mid.push((
            Widget {
                id: WidgetId(0),
                rect: r,
                kind: WidgetKind::Note(msg),
                enabled: true,
            },
            Act::None,
        ));
    }

    fn banner(col: &mut Column, mid: &mut Vec<(Widget, Act)>, msg: Msg, tone: Tone) {
        let inner = col.w - 16.0 * col.scale;
        let lines = line_count(&longest(&msg), inner, Font::Small, col.scale);
        let h_lp = 8.0 + lines as f64 * Font::Small.line_lp();
        let r = col.row(h_lp);
        mid.push((
            Widget {
                id: WidgetId(0),
                rect: r,
                kind: WidgetKind::Banner { msg, tone },
                enabled: true,
            },
            Act::None,
        ));
    }

    fn button(
        col: &mut Column,
        mid: &mut Vec<(Widget, Act)>,
        label: UiText,
        primary: bool,
        enabled: bool,
        act: Act,
    ) {
        let r = col.row(ROW_H);
        mid.push((
            Widget {
                id: WidgetId(0),
                rect: r,
                kind: WidgetKind::Button {
                    label: ui(label),
                    primary,
                },
                enabled,
            },
            act,
        ));
    }

    fn buttons2(
        col: &mut Column,
        mid: &mut Vec<(Widget, Act)>,
        a: (Msg, bool, bool, Act),
        b: (Msg, bool, bool, Act),
    ) {
        Self::buttons_w(col, mid, [1.0, 1.0], a, b);
    }

    fn buttons_w(
        col: &mut Column,
        mid: &mut Vec<(Widget, Act)>,
        weights: [f64; 2],
        a: (Msg, bool, bool, Act),
        b: (Msg, bool, bool, Act),
    ) {
        let r = col.row(ROW_H);
        let halves = split(r, &weights, col.scale);
        for (rect, (label, primary, enabled, act)) in halves.into_iter().zip([a, b]) {
            mid.push((
                Widget {
                    id: WidgetId(0),
                    rect,
                    kind: WidgetKind::Button { label, primary },
                    enabled,
                },
                act,
            ));
        }
    }

    fn labelled(
        col: &mut Column,
        mid: &mut Vec<(Widget, Act)>,
        label: Msg,
        kind: WidgetKind,
        act: Act,
        enabled: bool,
    ) {
        let r = col.row(ROW_H);
        let parts = split(r, &[0.42, 0.58], col.scale);
        mid.push((
            Widget {
                id: WidgetId(0),
                rect: parts[0],
                kind: WidgetKind::Label(label),
                enabled: true,
            },
            Act::None,
        ));
        mid.push((
            Widget {
                id: WidgetId(0),
                rect: parts[1],
                kind,
                enabled,
            },
            act,
        ));
    }

    fn cap_widget(&self, slot: Slot, lang_label: bool) -> WidgetKind {
        let listening = self.capture.as_ref().is_some_and(|c| c.slot() == slot);
        let label = if listening {
            "…".to_string()
        } else {
            self.doc
                .key_of(slot)
                .map(|k| key_label(k, self.lang))
                .unwrap_or_default()
        };
        let _ = lang_label;
        WidgetKind::KeyCap {
            slot,
            label,
            listening,
        }
    }

    fn name_field(&self, id: CtrlId) -> WidgetKind {
        let (text, focused) = match &self.field {
            Some(FieldFocus {
                target: FieldTarget::Label(i),
                edit,
            }) if *i == id => (edit.clone(), true),
            _ => {
                let label = match self.doc.control(id) {
                    Some(
                        Control::Joystick { label, .. }
                        | Control::Look { label, .. }
                        | Control::Button { label, .. },
                    ) => label.clone(),
                    None => String::new(),
                };
                (TextEdit::new(&label, 40), false)
            }
        };
        WidgetKind::Field {
            edit: text,
            focused,
            placeholder: Msg::Verbatim(self.doc.display_name(id, self.lang).unwrap_or_default()),
        }
    }

    fn slider_widget(&self, t: SliderTarget) -> WidgetKind {
        let (lo, hi, log) = Self::slider_range(t);
        let v = self.slider_value_of(t).unwrap_or(lo);
        WidgetKind::Slider {
            value: v,
            min: lo,
            max: hi,
            log,
            shown: fmt_num(v, 2, self.lang),
        }
    }

    fn middle(&self, col: &mut Column, env: &Env, mid: &mut Vec<(Widget, Act)>) {
        let prof = self.doc.profile();
        let lang = self.lang;
        if self.save_prompt {
            Self::banner(col, mid, Msg::SavePrompt, Tone::Warn);
            let r = col.row(ROW_H);
            for (rect, (label, primary, act)) in
                split(r, &[1.0, 1.4, 1.2], col.scale).into_iter().zip([
                    (UiText::SaveBtn, true, Act::SaveYes),
                    (UiText::DontSave, false, Act::SaveNo),
                    (UiText::KeepEditing, false, Act::SaveKeep),
                ])
            {
                mid.push((
                    Widget {
                        id: WidgetId(0),
                        rect,
                        kind: WidgetKind::Button {
                            label: ui(label),
                            primary,
                        },
                        enabled: true,
                    },
                    act,
                ));
            }
        }
        if self.revert_ask {
            Self::banner(col, mid, ui(UiText::RevertAsk), Tone::Warn);
            Self::buttons2(
                col,
                mid,
                (ui(UiText::RevertBtn), true, true, Act::RevertYes),
                (ui(UiText::KeepEditing), false, true, Act::RevertNo),
            );
        }
        if let Some(c) = &self.capture {
            let conflict = matches!(c.state, CaptureState::Conflict { .. });
            Self::banner(
                col,
                mid,
                c.message(&self.doc, lang),
                if conflict { Tone::Warn } else { Tone::Accent },
            );
            if conflict {
                Self::buttons2(
                    col,
                    mid,
                    (Msg::Swap, true, true, Act::CapSwap),
                    (Msg::Cancel, false, true, Act::CapCancel),
                );
            } else {
                Self::button(col, mid, UiText::BackBtn, false, false, Act::None);
                // replace the placeholder by the real chip
                if let Some((w, a)) = mid.last_mut() {
                    w.kind = WidgetKind::Button {
                        label: Msg::Cancel,
                        primary: false,
                    };
                    w.enabled = true;
                    *a = Act::CapCancel;
                }
            }
        }
        if self.info_open {
            let names: Vec<String> = self
                .doc
                .ids()
                .iter()
                .filter_map(|i| self.doc.display_name(*i, lang))
                .collect();
            Self::banner(col, mid, Msg::FingersInfo, Tone::Accent);
            Self::note(col, mid, Msg::Verbatim(ellipsize(&names.join(", "), 120)));
        }
        if let Some(g) = &self.guide {
            let r = col.row(24.0);
            mid.push((
                Widget {
                    id: WidgetId(0),
                    rect: r,
                    kind: WidgetKind::Label(ui(UiText::StepOf(g.step()))),
                    enabled: true,
                },
                Act::None,
            ));
            Self::banner(col, mid, g.msg(), Tone::Warn);
            if g.step() >= 6 {
                Self::buttons_w(
                    col,
                    mid,
                    [1.0, 1.7],
                    (ui(UiText::SaveAndPlay), true, true, Act::GuideSaveAndPlay),
                    (ui(UiText::SkipGuide), false, true, Act::GuideSkip),
                );
            } else {
                Self::buttons_w(
                    col,
                    mid,
                    [1.0, 1.7],
                    (ui(UiText::NextBtn), true, true, Act::GuideNext),
                    (ui(UiText::SkipGuide), false, true, Act::GuideSkip),
                );
            }
        }

        // Palette.
        let r = col.row(24.0);
        mid.push((
            Widget {
                id: WidgetId(0),
                rect: r,
                kind: WidgetKind::Label(ui(UiText::AddLabel)),
                enabled: true,
            },
            Act::None,
        ));
        let has_look = self
            .doc
            .ids()
            .iter()
            .any(|i| self.doc.kind(*i) == Some(Kind::Look));
        Self::button(
            col,
            mid,
            UiText::PalJoystick,
            false,
            true,
            Act::Arm(Kind::Joystick),
        );
        Self::buttons2(
            col,
            mid,
            (ui(UiText::PalLook), false, !has_look, Act::Arm(Kind::Look)),
            (ui(UiText::PalButton), false, true, Act::Arm(Kind::Button)),
        );
        if has_look {
            Self::note(col, mid, ui(UiText::OneLook));
        }

        // Properties.
        match self.sel.and_then(|s| self.doc.kind(s).map(|k| (s, k))) {
            None => self.profile_section(col, env, mid),
            Some((id, Kind::Button)) => {
                let r = col.row(24.0);
                mid.push((label_w(r, UiText::ButtonHdr, true), Act::None));
                Self::labelled(
                    col,
                    mid,
                    ui(UiText::NameLbl),
                    self.name_field(id),
                    Act::Field(FieldTarget::Label(id)),
                    true,
                );
                Self::button(col, mid, UiText::SuggestName, false, true, Act::Suggest);
                Self::labelled(
                    col,
                    mid,
                    ui(UiText::KeyLbl),
                    self.cap_widget(Slot::Button(id), false),
                    Act::Cap(Slot::Button(id)),
                    true,
                );
                Self::note(col, mid, ui(UiText::ClickToChange));
                let mode = match self.doc.control(id) {
                    Some(Control::Button { mode, .. }) => *mode,
                    _ => ButtonMode::Hold,
                };
                for (label, m) in [
                    (UiText::ModeHold, ButtonMode::Hold),
                    (UiText::ModeTap, ButtonMode::Tap),
                ] {
                    let r = col.row(ROW_H);
                    mid.push((
                        Widget {
                            id: WidgetId(0),
                            rect: r,
                            kind: WidgetKind::Check {
                                label: ui(label),
                                on: mode == m,
                            },
                            enabled: true,
                        },
                        Act::Mode(m),
                    ));
                }
                Self::note(col, mid, ui(UiText::ModeHint));
                self.control_advice(id, env, col, mid);
                Self::buttons2(
                    col,
                    mid,
                    (ui(UiText::Duplicate), false, true, Act::Dup),
                    (ui(UiText::Delete), false, true, Act::Del),
                );
            }
            Some((id, Kind::Joystick)) => {
                let r = col.row(24.0);
                mid.push((label_w(r, UiText::JoyHdr, true), Act::None));
                Self::labelled(
                    col,
                    mid,
                    ui(UiText::NameLbl),
                    self.name_field(id),
                    Act::Field(FieldTarget::Label(id)),
                    true,
                );
                // Up on its own row, then Left / Down / Right.
                let r = col.row(ROW_H);
                let three = split(r, &[1.0, 1.0, 1.0], col.scale);
                mid.push((
                    Widget {
                        id: WidgetId(0),
                        rect: three[1],
                        kind: self.cap_widget(Slot::Dir(id, Dir::Up), false),
                        enabled: true,
                    },
                    Act::Cap(Slot::Dir(id, Dir::Up)),
                ));
                let r = col.row(ROW_H);
                for (rect, d) in split(r, &[1.0, 1.0, 1.0], col.scale).into_iter().zip([
                    Dir::Left,
                    Dir::Down,
                    Dir::Right,
                ]) {
                    mid.push((
                        Widget {
                            id: WidgetId(0),
                            rect,
                            kind: self.cap_widget(Slot::Dir(id, d), false),
                            enabled: true,
                        },
                        Act::Cap(Slot::Dir(id, d)),
                    ));
                }
                let can = |p: KeyPreset| {
                    let mut d = self.doc.clone();
                    d.set_joystick_keys(id, p).is_ok()
                };
                Self::buttons_w(
                    col,
                    mid,
                    [0.9, 1.3],
                    (
                        ui(UiText::UseWasd),
                        false,
                        can(KeyPreset::Wasd),
                        Act::Keys(KeyPreset::Wasd),
                    ),
                    (
                        ui(UiText::UseArrows),
                        false,
                        can(KeyPreset::Arrows),
                        Act::Keys(KeyPreset::Arrows),
                    ),
                );
                Self::labelled(
                    col,
                    mid,
                    ui(UiText::RingSize),
                    self.slider_widget(SliderTarget::Ring(id)),
                    Act::Slider(SliderTarget::Ring(id)),
                    true,
                );
                Self::note(col, mid, ui(UiText::RingHint));
                self.control_advice(id, env, col, mid);
                Self::buttons2(
                    col,
                    mid,
                    (ui(UiText::Duplicate), false, true, Act::Dup),
                    (ui(UiText::Delete), false, true, Act::Del),
                );
            }
            Some((id, Kind::Look)) => {
                let r = col.row(24.0);
                mid.push((label_w(r, UiText::LookHdr, true), Act::None));
                Self::labelled(
                    col,
                    mid,
                    ui(UiText::NameLbl),
                    self.name_field(id),
                    Act::Field(FieldTarget::Label(id)),
                    true,
                );
                Self::labelled(
                    col,
                    mid,
                    ui(UiText::MouseSens),
                    self.slider_widget(SliderTarget::Sens(id)),
                    Act::Slider(SliderTarget::Sens(id)),
                    true,
                );
                let r = col.row(ROW_H);
                for (rect, (label, v)) in split(r, &[1.0, 1.0, 1.0], col.scale).into_iter().zip([
                    (UiText::Slow, SENS_PRESETS[0]),
                    (UiText::Medium, SENS_PRESETS[1]),
                    (UiText::Fast, SENS_PRESETS[2]),
                ]) {
                    mid.push((
                        Widget {
                            id: WidgetId(0),
                            rect,
                            kind: WidgetKind::Button {
                                label: ui(label),
                                primary: false,
                            },
                            enabled: true,
                        },
                        Act::SensPreset(v),
                    ));
                }
                Self::note(col, mid, Msg::SensHint);
                let (invert, idle) = match self.doc.control(id) {
                    Some(Control::Look {
                        invert_y,
                        release_after_ms,
                        ..
                    }) => (*invert_y, *release_after_ms),
                    _ => (false, 0),
                };
                let r = col.row(ROW_H);
                mid.push((
                    Widget {
                        id: WidgetId(0),
                        rect: r,
                        kind: WidgetKind::Check {
                            label: ui(UiText::InvertY),
                            on: invert,
                        },
                        enabled: true,
                    },
                    Act::InvertY,
                ));
                Self::labelled(
                    col,
                    mid,
                    ui(UiText::AreaSize),
                    self.slider_widget(SliderTarget::Reach(id)),
                    Act::Slider(SliderTarget::Reach(id)),
                    true,
                );
                Self::note(col, mid, ui(UiText::AreaHint));
                Self::button(col, mid, UiText::Advanced, false, true, Act::AdvancedToggle);
                if self.advanced_open {
                    let r = col.row(24.0);
                    mid.push((
                        Widget {
                            id: WidgetId(0),
                            rect: r,
                            kind: WidgetKind::Label(ui(UiText::IdleLift)),
                            enabled: true,
                        },
                        Act::None,
                    ));
                    let r = col.row(ROW_H);
                    mid.push((
                        Widget {
                            id: WidgetId(0),
                            rect: r,
                            kind: WidgetKind::Segmented {
                                options: IDLE_MS
                                    .iter()
                                    .map(|ms| ui(UiText::IdleSecs(*ms)))
                                    .collect(),
                                selected: IDLE_MS.iter().position(|m| *m == idle),
                            },
                            enabled: true,
                        },
                        Act::Idle(0),
                    ));
                }
                self.control_advice(id, env, col, mid);
                Self::button(col, mid, UiText::Delete, false, true, Act::Del);
            }
        }
        let _ = prof;
    }

    fn control_advice(
        &self,
        id: CtrlId,
        env: &Env,
        col: &mut Column,
        mid: &mut Vec<(Widget, Act)>,
    ) {
        if let Some(a) = &self.last_advice {
            Self::note(col, mid, a.clone());
        }
        for l in lints(&self.doc, env) {
            if l.target == Some(id) {
                Self::note(col, mid, Msg::Advice(l.advice));
            }
        }
    }

    fn profile_section(&self, col: &mut Column, env: &Env, mid: &mut Vec<(Widget, Act)>) {
        let prof = self.doc.profile();
        let r = col.row(24.0);
        mid.push((label_w(r, UiText::ProfileHdr, true), Act::None));
        let (edit, focused) = match &self.field {
            Some(FieldFocus {
                target: FieldTarget::ProfileName,
                edit,
            }) => (edit.clone(), true),
            _ => (TextEdit::new(&prof.name, 100), false),
        };
        Self::labelled(
            col,
            mid,
            ui(UiText::NameLbl),
            WidgetKind::Field {
                edit,
                focused,
                placeholder: Msg::NewProfileName(1),
            },
            Act::Field(FieldTarget::ProfileName),
            true,
        );
        Self::labelled(
            col,
            mid,
            ui(UiText::ScreenLbl),
            WidgetKind::Segmented {
                options: vec![ui(UiText::OrientLand), ui(UiText::OrientPort)],
                selected: Some(usize::from(prof.orientation == Orientation::Portrait)),
            },
            Act::Orient(0),
            true,
        );
        Self::labelled(
            col,
            mid,
            Msg::SlotToggle,
            self.cap_widget(Slot::Toggle, false),
            Act::Cap(Slot::Toggle),
            true,
        );
        Self::labelled(
            col,
            mid,
            ui(UiText::PanicKeyLbl),
            self.cap_widget(Slot::Panic, false),
            Act::Cap(Slot::Panic),
            true,
        );
        Self::note(col, mid, ui(UiText::KeepF8));
        let r = col.row(24.0);
        mid.push((label_w(r, UiText::ShowKeysLbl, false), Act::None));
        let r = col.row(ROW_H);
        mid.push((
            Widget {
                id: WidgetId(0),
                rect: r,
                kind: WidgetKind::Segmented {
                    options: vec![ui(UiText::OvAuto), ui(UiText::OvAlways), ui(UiText::OvOff)],
                    selected: Some(match self.overlay {
                        Overlay::Auto => 0,
                        Overlay::Always => 1,
                        Overlay::Off => 2,
                    }),
                },
                enabled: true,
            },
            Act::OverlayPick(0),
        ));
        if self.overlay == Overlay::Always {
            Self::note(col, mid, Msg::OverlayCostNote);
        }
        let all = lints(&self.doc, env);
        if !all.is_empty() {
            let r = col.row(ROW_H);
            mid.push((
                Widget {
                    id: WidgetId(0),
                    rect: r,
                    kind: WidgetKind::Button {
                        label: ui(UiText::AdviceCount(all.len())),
                        primary: false,
                    },
                    enabled: true,
                },
                Act::AdviceToggle,
            ));
            if self.advice_open {
                for l in all {
                    Self::note(col, mid, Msg::Advice(l.advice));
                }
            }
        }
        Self::button(
            col,
            mid,
            UiText::RevertBtn,
            false,
            self.hist.dirty(),
            Act::Revert,
        );
    }
}

const PAD_F: f64 = 12.0;

fn label_w(r: PxRect, t: UiText, heading: bool) -> Widget {
    Widget {
        id: WidgetId(0),
        rect: r,
        kind: if heading {
            WidgetKind::Heading(ui(t))
        } else {
            WidgetKind::Label(ui(t))
        },
        enabled: true,
    }
}

/// The longer of the two language texts (so wrapped heights fit both).
fn longest(m: &Msg) -> String {
    let (v, e) = (m.text(Lang::Vi), m.text(Lang::En));
    if e.chars().count() > v.chars().count() {
        e
    } else {
        v
    }
}

fn item(lang: Lang, id: u32, label: Msg, checked: bool, enabled: bool) -> MenuItem {
    MenuItem::Item {
        id,
        label: label.text(lang),
        checked,
        enabled,
    }
}

fn contains(r: PxRect, p: Pt) -> bool {
    p.x >= r.x && p.x <= r.x + r.w && p.y >= r.y && p.y <= r.y + r.h
}

fn apply_field_key(e: &mut TextEdit, name: &str, mods: Mods) {
    match name {
        "Backspace" => e.backspace(),
        "Delete" => e.delete(),
        "ArrowLeft" => e.left(mods.shift),
        "ArrowRight" => e.right(mods.shift),
        "Home" => e.home(mods.shift),
        "End" => e.end(mods.shift),
        _ => {}
    }
}

/// Widgets with their actions; ids are indexes.
#[derive(Default)]
struct Built {
    ws: Vec<(Widget, Act)>,
    /// How many leading widgets belong to the canvas (drawn under the dock).
    canvas_n: usize,
    dock: Option<PxRect>,
    dock_rect: Option<PxRect>,
}

impl Built {
    fn push(&mut self, mut w: Widget, a: Act) {
        w.id = WidgetId(self.ws.len() as u32);
        self.ws.push((w, a));
    }

    /// Whether `p` is on the dock card or any widget (banners included).
    fn covers(&self, p: Pt) -> bool {
        self.dock_rect.is_some_and(|d| contains(d, p)) || covers(&widgets_of(self), p)
    }
}

fn widgets_of(b: &Built) -> Vec<Widget> {
    b.ws.iter().map(|(w, _)| w.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::layout::{DockPlacement, widget_texts};
    use crate::menu::templates::{Template, template};
    use crate::menu::testkit::{Ed, Lcg, centre, env, env_sized};
    use std::collections::HashSet;

    fn has_effect(outs: &[EditorOut], f: impl Fn(&Effect) -> bool) -> bool {
        outs.iter()
            .any(|o| matches!(o, EditorOut::Effect(e) if f(e)))
    }

    fn pos(e: &Ed, i: usize) -> Pt {
        pt_of(&e.ed.profile().controls[i])
    }

    // ---- test 53: dock layout ----

    fn dock_widgets(e: &Ed) -> (PxRect, Vec<Widget>) {
        let b = e.ed.build(&e.env);
        let d = b.dock.expect("dock");
        let all = widgets_of(&b);
        (d, all[b.canvas_n.min(all.len())..].to_vec())
    }

    fn overlap(a: PxRect, b: PxRect) -> bool {
        a.x < b.x + b.w - 0.5
            && b.x < a.x + a.w - 0.5
            && a.y < b.y + b.h - 0.5
            && b.y < a.y + a.h - 0.5
    }

    #[test]
    fn dock_widgets_stay_inside_and_never_overlap() {
        let cases = [
            (
                1280.0,
                590.0,
                1.0,
                Rotation::R90,
                Template::ShooterLandscape,
            ),
            (
                1280.0,
                590.0,
                1.5,
                Rotation::R90,
                Template::ShooterLandscape,
            ),
            (
                1900.0,
                880.0,
                2.0,
                Rotation::R90,
                Template::ShooterLandscape,
            ),
            (400.0, 860.0, 1.0, Rotation::R0, Template::BasicPortrait),
        ];
        for (w, h, scale, rot, tpl) in cases {
            for sel in [None, Some(0usize), Some(1), Some(2)] {
                let mut e = Ed::with(template(tpl));
                e.env = env_sized(w, h, scale, rot);
                if let Some(n) = sel {
                    e.select(n);
                }
                let (dock, ws) = dock_widgets(&e);
                assert!(!ws.is_empty());
                for (i, a) in ws.iter().enumerate() {
                    let r = a.rect;
                    assert!(
                        r.x >= dock.x - 0.5
                            && r.y >= dock.y - 0.5
                            && r.x + r.w <= dock.x + dock.w + 0.5
                            && r.y + r.h <= dock.y + dock.h + 0.5,
                        "{w}x{h}@{scale} sel {sel:?}: widget {i} {r:?} outside {dock:?}"
                    );
                    for b in &ws[i + 1..] {
                        assert!(
                            !overlap(r, b.rect),
                            "{w}x{h}@{scale} sel {sel:?}: {a:?} overlaps {b:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn every_widget_text_fits_in_both_languages() {
        for lang in [Lang::Vi, Lang::En] {
            for sel in [None, Some(0usize), Some(1), Some(2)] {
                let mut e = Ed::shooter();
                if let Some(n) = sel {
                    e.select(n);
                }
                e.ed.advanced_open = true;
                let (_, ws) = dock_widgets(&e);
                for w in ws {
                    // User data (profile name, label) is ellipsized by the drawer.
                    if matches!(w.kind, WidgetKind::Field { .. } | WidgetKind::Row { .. }) {
                        continue;
                    }
                    for (t, wpx, font) in widget_texts(&w, lang, 1.0) {
                        assert!(
                            crate::menu::scene::fits(&t, wpx, font, 1.0),
                            "{lang:?} {t:?} ({} chars) does not fit {wpx}px",
                            t.chars().count()
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn dock_flips_to_the_side_away_from_the_selection() {
        let mut e = Ed::shooter();
        e.select(0); // the joystick, left
        assert_eq!(e.ed.placement, Some(DockPlacement::CardRight));
        e.select(0); // Tab to the look control (centre-right)
        e.select(3); // a right-hand button
        assert_eq!(e.ed.placement, Some(DockPlacement::CardLeft));
    }

    // ---- capture ----

    #[test]
    fn clicking_a_cap_binds_a_free_key_in_one_undo_step() {
        let mut e = Ed::shooter();
        e.select(4); // Nhảy, Space
        e.click_text("Space");
        assert!(e.ed.capturing());
        e.key("KeyZ");
        assert!(!e.ed.capturing());
        let Control::Button { key, .. } = &e.ed.profile().controls[4] else {
            panic!()
        };
        assert_eq!(key, "KeyZ");
        e.ctrl("KeyZ");
        let Control::Button { key, .. } = &e.ed.profile().controls[4] else {
            panic!()
        };
        assert_eq!(key, "Space");
    }

    #[test]
    fn taken_key_offers_swap_or_cancel() {
        let mut e = Ed::shooter();
        e.select(4);
        e.click_text("Space");
        e.key("KeyR"); // Nạp đạn
        assert!(e.ed.capturing());
        assert!(e.find("Đổi chỗ").is_some() && e.find("Huỷ").is_some());
        let before = e.ed.profile().clone();
        e.click_text("Huỷ");
        assert!(!e.ed.capturing());
        assert_eq!(e.ed.profile(), &before);
        e.click_text("Space");
        e.key("KeyR");
        e.click_text("Đổi chỗ");
        assert!(!e.ed.capturing());
        let keys: Vec<_> =
            e.ed.profile()
                .controls
                .iter()
                .filter_map(|c| match c {
                    Control::Button { key, .. } => Some(key.clone()),
                    _ => None,
                })
                .collect();
        let unique: HashSet<_> = keys.iter().collect();
        assert_eq!(unique.len(), keys.len(), "no duplicate after a swap");
        assert_eq!(keys[2], "KeyR"); // Nhảy is the third button
    }

    #[test]
    fn mouse_button_binds_and_esc_needs_two_presses() {
        let mut e = Ed::shooter();
        e.select(5); // Ngồi
        e.click_text("C");
        // a free mouse button binds
        let p = e.px(0.5, 0.5);
        e.ev(UiEvent::PointerDown {
            p,
            button: "MouseMiddle",
            mods: Mods::default(),
        });
        assert!(!e.ed.capturing());
        let Control::Button { key, .. } = &e.ed.profile().controls[5] else {
            panic!()
        };
        assert_eq!(key, "MouseMiddle");
        e.click_text("Chuột giữa");
        e.key("Escape");
        assert!(e.ed.capturing(), "first Esc only arms");
        e.key("Escape");
        assert!(!e.ed.capturing());
        let Control::Button { key, .. } = &e.ed.profile().controls[5] else {
            panic!()
        };
        assert_eq!(key, "Escape");
    }

    #[test]
    fn capture_swallows_function_keys_and_the_toggle_key() {
        let mut e = Ed::shooter();
        e.select(4);
        e.click_text("Space");
        e.key("F5");
        assert!(!e.ed.capturing(), "F5 is bindable");
        assert!(
            e.ed.passes_to_upstream("F5"),
            "not capturing: F5 goes upstream"
        );
        e.click_text("F5");
        assert!(!e.ed.passes_to_upstream("F1"), "capture owns F-keys");
        let outs = e.key("Backquote");
        assert!(!e.ed.in_test() && !has_effect(&outs, |f| matches!(f, Effect::GrabPointer(_))));
    }

    // ---- palette ----

    #[test]
    fn dragging_a_palette_item_adds_one_control_in_one_step() {
        let mut e = Ed::with(template(Template::Empty(Orientation::Landscape)));
        let from = centre(&e.find("Nút").unwrap());
        let to = e.px(0.4, 0.5);
        e.mv(from);
        e.down(from);
        e.mv(to);
        e.up(to);
        assert_eq!(e.ed.profile().controls.len(), 1);
        assert!(e.ed.capturing(), "a new button listens for its key");
        e.click_text("Huỷ");
        assert_eq!(e.ed.profile().controls.len(), 1);
        e.ctrl("KeyZ");
        assert_eq!(e.ed.profile().controls.len(), 0);
    }

    #[test]
    fn escape_during_a_palette_drag_adds_nothing() {
        let mut e = Ed::with(template(Template::Empty(Orientation::Landscape)));
        let from = centre(&e.find("Nút").unwrap());
        let to = e.px(0.4, 0.5);
        e.mv(from);
        e.down(from);
        e.mv(to);
        e.key("Escape");
        e.up(to);
        assert_eq!(e.ed.profile().controls.len(), 0);
    }

    #[test]
    fn click_then_click_places_and_a_drop_on_the_dock_does_not() {
        let mut e = Ed::with(template(Template::Empty(Orientation::Landscape)));
        e.click_text("Cần di chuyển");
        let at = e.px(0.2, 0.6);
        e.click(at);
        assert_eq!(e.ed.profile().controls.len(), 1);
        assert!(!e.ed.capturing(), "a joystick needs no capture");
        // drag from the palette back onto the dock
        let from = centre(&e.find("Nút").unwrap());
        let onto = centre(&e.find("Xoay camera").unwrap());
        e.mv(from);
        e.down(from);
        e.mv(onto);
        e.up(onto);
        assert_eq!(e.ed.profile().controls.len(), 1);
    }

    #[test]
    fn second_look_is_not_offered() {
        let e = Ed::shooter();
        let w = e.widgets().into_iter().find(|w| matches!(&w.kind, WidgetKind::Button { label, .. } if label.text(Lang::Vi) == "Xoay camera")).unwrap();
        assert!(!w.enabled);
    }

    // ---- pointer on controls ----

    #[test]
    fn click_selects_and_drag_moves_in_one_undo_step() {
        let mut e = Ed::shooter();
        let from = e.px(0.15, 0.68);
        e.mv(from);
        e.down(from);
        assert_eq!(e.ed.selection(), Some(1));
        let to = Pt {
            x: from.x + 40.0,
            y: from.y - 30.0,
        };
        for k in 1..=8 {
            let f = f64::from(k) / 8.0;
            e.mv(Pt {
                x: from.x + (to.x - from.x) * f,
                y: from.y + (to.y - from.y) * f,
            });
        }
        e.up(to);
        let moved = pos(&e, 0);
        assert!(moved.x > 0.15 + 0.02 && moved.y < 0.68 - 0.02, "{moved:?}");
        e.ctrl("KeyZ");
        assert_eq!(pos(&e, 0), Pt { x: 0.15, y: 0.68 });
        e.ctrl("KeyY");
        assert_eq!(pos(&e, 0), moved);
    }

    #[test]
    fn click_without_moving_changes_nothing() {
        let mut e = Ed::shooter();
        let p = e.px(0.15, 0.68);
        e.click(p);
        assert_eq!(e.ed.selection(), Some(1));
        assert!(!e.ed.dirty());
        let q = e.px(0.5, 0.2);
        e.click(q);
        assert_eq!(e.ed.selection(), None);
    }

    #[test]
    fn wheel_burst_is_one_step_and_resizes_the_selected_ring() {
        let mut e = Ed::shooter();
        let c = e.px(0.15, 0.68);
        e.click(c);
        let r0 = match &e.ed.profile().controls[0] {
            Control::Joystick { radius, .. } => *radius,
            _ => panic!(),
        };
        for _ in 0..3 {
            e.ev(UiEvent::Wheel {
                p: c,
                dy: 1.0,
                mods: Mods::default(),
            });
        }
        let r1 = match &e.ed.profile().controls[0] {
            Control::Joystick { radius, .. } => *radius,
            _ => panic!(),
        };
        assert!((r1 - r0 - 0.03).abs() < 1e-9, "{r0} {r1}");
        e.ctrl("KeyZ");
        let r2 = match &e.ed.profile().controls[0] {
            Control::Joystick { radius, .. } => *radius,
            _ => panic!(),
        };
        assert!((r2 - r0).abs() < 1e-9);
    }

    #[test]
    fn right_click_shows_a_native_menu_and_picks_apply() {
        let mut e = Ed::shooter();
        let p = e.px(0.15, 0.68);
        let outs = e.ev(UiEvent::PointerDown {
            p,
            button: "MouseRight",
            mods: Mods::default(),
        });
        assert!(has_effect(&outs, |f| matches!(f, Effect::ShowMenu(_))));
        let n = e.ed.profile().controls.len();
        e.ev(UiEvent::MenuPick(3)); // Xoá
        assert_eq!(e.ed.profile().controls.len(), n - 1);
        // empty picture: "Thêm ở đây"
        let q = e.px(0.5, 0.5);
        let outs = e.ev(UiEvent::PointerDown {
            p: q,
            button: "MouseRight",
            mods: Mods::default(),
        });
        assert!(has_effect(
            &outs,
            |f| matches!(f, Effect::ShowMenu(m) if matches!(m.items[0], MenuItem::Sub { .. }))
        ));
        e.ev(UiEvent::MenuPick(11)); // joystick (the old one is gone)
        assert_eq!(e.ed.profile().controls.len(), n);
    }

    // ---- keyboard ----

    #[test]
    fn keyboard_shortcuts() {
        let mut e = Ed::shooter();
        e.key("Tab");
        assert_eq!(e.ed.selection(), Some(1));
        e.key_m(
            "Tab",
            Mods {
                shift: true,
                ..Mods::default()
            },
        );
        assert_eq!(e.ed.selection(), Some(10), "Shift+Tab wraps to the last");
        e.ctrl("KeyD");
        assert_eq!(e.ed.profile().controls.len(), 11);
        assert_eq!(e.ed.selection(), Some(11));
        e.key("Delete");
        assert_eq!(e.ed.profile().controls.len(), 10);
        assert_eq!(e.ed.selection(), None);
        e.ctrl("KeyZ");
        assert_eq!(e.ed.profile().controls.len(), 11);
        e.ev(UiEvent::Key {
            name: "KeyY",
            pressed: true,
            mods: Mods {
                ctrl: true,
                ..Mods::default()
            },
        });
        assert_eq!(e.ed.profile().controls.len(), 10);
        let outs = e.ctrl("KeyS");
        assert!(outs.contains(&EditorOut::Save));
        // H hides while held
        e.ev(UiEvent::Key {
            name: "KeyH",
            pressed: true,
            mods: Mods::default(),
        });
        assert!(e.ed.hidden());
        e.ev(UiEvent::Key {
            name: "KeyH",
            pressed: false,
            mods: Mods::default(),
        });
        assert!(!e.ed.hidden());
    }

    #[test]
    fn arrow_nudge_moves_by_picture_pixels_and_bursts_are_one_step() {
        let mut e = Ed::shooter();
        e.select(2);
        let p0 = pos(&e, 2);
        e.key("ArrowRight");
        e.key_m(
            "ArrowRight",
            Mods {
                shift: true,
                ..Mods::default()
            },
        );
        e.ev(UiEvent::KeyRepeat {
            name: "ArrowDown",
            mods: Mods::default(),
        });
        let p1 = pos(&e, 2);
        let (w, h) = (e.env.view.picture.w, e.env.view.picture.h);
        assert!((p1.x - p0.x - 11.0 / w).abs() < 1e-3, "{p0:?} {p1:?}");
        assert!((p1.y - p0.y - 1.0 / h).abs() < 1e-3);
        e.ctrl("KeyZ");
        assert_eq!(pos(&e, 2), p0, "a burst is one undo step");
    }

    #[test]
    fn enter_listens_on_the_selected_control() {
        let mut e = Ed::shooter();
        e.select(0);
        e.key("Enter");
        assert!(e.ed.capturing());
        e.key("Escape");
        e.key("Escape"); // binds Escape to Up: fine, it is free
        assert!(!e.ed.capturing());
    }

    #[test]
    fn escape_cascades_and_asks_before_leaving_with_changes() {
        let mut e = Ed::shooter();
        e.select(2);
        assert_eq!(e.ed.selection(), Some(3));
        let outs = e.key("Escape");
        assert!(e.ed.selection().is_none() && outs.is_empty());
        let outs = e.key("Escape");
        assert!(outs.contains(&EditorOut::Leave), "clean: leave at once");
        // dirty: the save prompt
        e.select(2);
        e.key("Delete");
        let outs = e.key("Escape");
        assert!(!outs.contains(&EditorOut::Leave));
        assert!(e.find("Không lưu").is_some());
        let outs = e.click_outs("Không lưu");
        assert!(outs.contains(&EditorOut::Leave) && !outs.contains(&EditorOut::Save));
    }

    #[test]
    fn done_saves_then_leaves() {
        let mut e = Ed::shooter();
        let w = e.find("Xong").unwrap();
        let p = centre(&w);
        e.mv(p);
        let outs = e.down(p);
        assert_eq!(
            outs.iter()
                .filter(|o| matches!(o, EditorOut::Save | EditorOut::Leave))
                .cloned()
                .collect::<Vec<_>>(),
            vec![EditorOut::Save, EditorOut::Leave]
        );
    }

    // ---- text fields ----

    fn field_centre(e: &Ed) -> Pt {
        centre(
            &e.widgets()
                .into_iter()
                .find(|w| matches!(w.kind, WidgetKind::Field { .. }))
                .expect("a field"),
        )
    }

    #[test]
    fn name_field_types_commits_and_undoes_as_one_step() {
        let mut e = Ed::shooter();
        let c = field_centre(&e);
        e.click(c);
        assert!(e.ed.field_focused());
        e.ev(UiEvent::Text("Đấu trường"));
        // the toggle key is typed text, not "enter Test"
        let outs = e.key("Backquote");
        assert!(!e.ed.in_test() && !has_effect(&outs, |f| matches!(f, Effect::GrabPointer(_))));
        e.key("Enter");
        assert!(!e.ed.field_focused());
        assert!(e.ed.profile().name.starts_with("Đấu trường"));
        e.ctrl("KeyZ");
        assert_eq!(
            e.ed.profile().name,
            "Hồ sơ 1"
                .to_string()
                .replace("Hồ sơ 1", &template(Template::ShooterLandscape).name)
        );
    }

    #[test]
    fn escape_reverts_a_field_and_click_elsewhere_commits() {
        let mut e = Ed::shooter();
        let old = e.ed.profile().name.clone();
        let c = field_centre(&e);
        e.click(c);
        e.ev(UiEvent::Text("X"));
        e.key("Escape");
        assert_eq!(e.ed.profile().name, old);
        assert!(!e.ed.field_focused());
        let c = field_centre(&e);
        e.click(c);
        e.ev(UiEvent::Text("Y"));
        let away = e.px(0.5, 0.3);
        e.click(away);
        assert!(!e.ed.field_focused());
        assert!(e.ed.profile().name.contains('Y'));
    }

    #[test]
    fn label_field_is_limited_to_forty_characters() {
        let mut e = Ed::shooter();
        e.select(2);
        let c = field_centre(&e);
        e.click(c);
        e.ev(UiEvent::Text(&"ế".repeat(60)));
        e.key("Enter");
        let Control::Button { label, .. } = &e.ed.profile().controls[2] else {
            panic!()
        };
        assert_eq!(label.chars().count(), 40);
    }

    // ---- dock widgets ----

    #[test]
    fn orientation_segmented_switches_and_undoes() {
        let mut e = Ed::shooter();
        let w = e
            .widgets()
            .into_iter()
            .find(
                |w| matches!(&w.kind, WidgetKind::Segmented { options, .. } if options.len() == 2),
            )
            .unwrap();
        e.click(Pt {
            x: w.rect.x + w.rect.w * 0.85,
            y: w.rect.y + w.rect.h / 2.0,
        });
        assert_eq!(e.ed.profile().orientation, Orientation::Portrait);
        e.ctrl("KeyZ");
        assert_eq!(e.ed.profile().orientation, Orientation::Landscape);
    }

    #[test]
    fn overlay_choice_is_reported_not_stored_in_the_profile() {
        let mut e = Ed::shooter();
        let before = e.ed.profile().clone();
        let w = e
            .widgets()
            .into_iter()
            .find(
                |w| matches!(&w.kind, WidgetKind::Segmented { options, .. } if options.len() == 3),
            )
            .unwrap();
        e.mv(Pt {
            x: w.rect.x + w.rect.w * 0.9,
            y: w.rect.y + w.rect.h / 2.0,
        });
        let outs = e.down(Pt {
            x: w.rect.x + w.rect.w * 0.9,
            y: w.rect.y + w.rect.h / 2.0,
        });
        assert!(outs.contains(&EditorOut::SetOverlay(Overlay::Off)));
        assert_eq!(e.ed.profile(), &before);
    }

    #[test]
    fn slider_steppers_and_track_edit_values_with_undo() {
        let mut e = Ed::shooter();
        e.select(1); // look
        let sens = |e: &Ed| match &e.ed.profile().controls[1] {
            Control::Look { sensitivity, .. } => *sensitivity,
            _ => panic!(),
        };
        let w = e
            .widgets()
            .into_iter()
            .find(|w| matches!(w.kind, WidgetKind::Slider { log: true, .. }))
            .unwrap();
        let parts = slider_parts(w.rect, 1.0);
        let s0 = sens(&e);
        e.click(centre_of(parts.plus));
        assert!(sens(&e) > s0);
        e.ctrl("KeyZ");
        assert_eq!(sens(&e), s0);
        // drag along the track
        let y = parts.track.y + parts.track.h / 2.0;
        e.mv(Pt {
            x: parts.track.x + parts.track.w * 0.5,
            y,
        });
        e.down(Pt {
            x: parts.track.x + parts.track.w * 0.5,
            y,
        });
        assert!(
            (sens(&e) - 1.0).abs() < 0.05,
            "middle of a 0.1..10 log slider is 1"
        );
        e.mv(Pt {
            x: parts.track.x + parts.track.w * 0.9,
            y,
        });
        e.up(Pt {
            x: parts.track.x + parts.track.w * 0.9,
            y,
        });
        assert!(sens(&e) > 3.0);
        e.ctrl("KeyZ");
        assert_eq!(sens(&e), s0, "one drag = one step");
        // presets
        e.click_text("Nhanh");
        assert!((sens(&e) - 1.8).abs() < 1e-9);
    }

    fn centre_of(r: PxRect) -> Pt {
        Pt {
            x: r.x + r.w / 2.0,
            y: r.y + r.h / 2.0,
        }
    }

    #[test]
    fn joystick_presets_are_disabled_when_a_key_is_taken() {
        let mut e = Ed::shooter();
        e.select(0);
        // WASD is in use by this very joystick: allowed; arrows are free.
        assert!(e.find("Dùng phím mũi tên").is_some());
        e.click_text("Dùng phím mũi tên");
        let Control::Joystick { up, .. } = &e.ed.profile().controls[0] else {
            panic!()
        };
        assert_eq!(up, "ArrowUp");
    }

    #[test]
    fn button_mode_check_and_duplicate_and_delete_buttons() {
        let mut e = Ed::shooter();
        e.select(2); // Bắn, hold
        e.click_text("Bấm nhanh = chạm 1 lần (0,04 giây)");
        let Control::Button { mode, .. } = &e.ed.profile().controls[2] else {
            panic!()
        };
        assert_eq!(*mode, ButtonMode::Tap);
        e.click_text("Nhân bản");
        assert_eq!(e.ed.profile().controls.len(), 11);
        e.click_text("Xoá");
        assert_eq!(e.ed.profile().controls.len(), 10);
    }

    #[test]
    fn collapse_and_expand_the_dock() {
        let mut e = Ed::shooter();
        e.click_text("–");
        assert!(e.find("Bảng sửa ▸").is_some() && e.find("Xong").is_none());
        e.click_text("Bảng sửa ▸");
        assert!(e.find("Xong").is_some());
    }

    #[test]
    fn orientation_mismatch_shows_a_banner_with_a_rotate_button() {
        let mut e = Ed::shooter();
        e.env = env_sized(400.0, 860.0, 1.0, Rotation::R0);
        let outs = {
            let w = e.find("Xoay hình (F12)").expect("rotate button");
            let p = centre(&w);
            e.mv(p);
            e.down(p)
        };
        assert!(has_effect(&outs, |f| matches!(f, Effect::Rotate)));
        e.env.has_picture = false;
        assert!(e.find("Xoay hình (F12)").is_none());
        let s = e.ed.scene(&e.env, Lang::Vi, 0);
        assert!(
            s.prims()
                .any(|p| matches!(p, Prim::Text { text, .. } if text.contains("Chưa có hình")))
        );
    }

    // ---- Test mode ----

    fn enter_test(e: &mut Ed) -> Vec<EditorOut> {
        let w = e.find("Thử").unwrap();
        let p = centre(&w);
        e.mv(p);
        e.down(p)
    }

    #[test]
    fn test_mode_shows_dots_and_never_touches_the_document() {
        let mut e = Ed::shooter();
        let before = e.ed.profile().clone();
        let outs = enter_test(&mut e);
        assert!(e.ed.in_test());
        assert!(has_effect(&outs, |f| matches!(
            f,
            Effect::GrabPointer(true)
        )));
        e.ev(UiEvent::Key {
            name: "KeyW",
            pressed: true,
            mods: Mods::default(),
        });
        assert!(e.ed.dots().dots.iter().any(Option::is_some));
        assert_eq!(e.ed.pressed_mask() & 1, 1, "joystick lit");
        assert_eq!(e.ed.test_fingers(), 1);
        e.ev(UiEvent::Key {
            name: "KeyW",
            pressed: false,
            mods: Mods::default(),
        });
        assert!(e.ed.dots().dots.iter().all(Option::is_none));
        assert_eq!(e.ed.pressed_mask(), 0);
        assert_eq!(e.ed.profile(), &before);
        assert!(!e.ed.dirty());
    }

    #[test]
    fn leaving_test_clears_everything_for_every_exit() {
        for exit in ["Backquote", "Escape", "F8", "F12"] {
            let mut e = Ed::shooter();
            enter_test(&mut e);
            e.ev(UiEvent::Key {
                name: "KeyW",
                pressed: true,
                mods: Mods::default(),
            });
            let outs = e.key(exit);
            assert!(!e.ed.in_test(), "{exit}");
            assert!(
                has_effect(&outs, |f| matches!(f, Effect::GrabPointer(false))),
                "{exit}"
            );
            assert!(e.ed.dots().dots.iter().all(Option::is_none), "{exit}");
        }
        let mut e = Ed::shooter();
        enter_test(&mut e);
        e.ev(UiEvent::FocusLost);
        assert!(!e.ed.in_test());
        enter_test(&mut e);
        e.click_text("Quay lại sửa");
        assert!(!e.ed.in_test());
    }

    #[test]
    fn test_mode_f_key_rules() {
        let mut e = Ed::shooter();
        assert!(e.ed.passes_to_upstream("F1"));
        enter_test(&mut e);
        for k in ["F1", "F2", "F3", "F4", "F5", "F8", "F9"] {
            assert!(!e.ed.passes_to_upstream(k), "{k}");
        }
        for k in ["F6", "F7", "F10", "F11", "F12"] {
            assert!(e.ed.passes_to_upstream(k), "{k}");
        }
        assert!(e.ed.in_test(), "F1-F5 are ignored, Test continues");
        e.key("F1");
        assert!(e.ed.in_test());
    }

    #[test]
    fn sixth_finger_is_reported_in_test() {
        // Six hold buttons: taps would lift again after 40 ms.
        let mut doc =
            EditDoc::from_profile(template(Template::Empty(Orientation::Landscape))).unwrap();
        let mut keys = Vec::new();
        for i in 0..6 {
            let id = doc
                .add(
                    Kind::Button,
                    Pt {
                        x: 0.1 + 0.1 * f64::from(i),
                        y: 0.5,
                    },
                )
                .unwrap();
            keys.push(doc.key_of(Slot::Button(id)).unwrap().to_string());
        }
        let mut e = Ed::with(doc.profile().clone());
        enter_test(&mut e);
        for k in &keys[..5] {
            e.ev(UiEvent::Key {
                name: k,
                pressed: true,
                mods: Mods::default(),
            });
        }
        assert_eq!(e.ed.test_full_message(e.now), None);
        e.ev(UiEvent::Key {
            name: &keys[5],
            pressed: true,
            mods: Mods::default(),
        });
        assert_eq!(e.ed.test_fingers(), 5);
        assert_eq!(e.ed.test_full_message(e.now), Some(Msg::TestFull));
        assert_eq!(e.ed.test_full_message(e.now + 2500), None);
    }

    #[test]
    fn test_mode_look_motion_moves_a_dot_and_recentres() {
        let mut e = Ed::shooter();
        enter_test(&mut e);
        e.ev(UiEvent::RawMotion { dx: 10.0, dy: 0.0 });
        assert!(e.ed.dots().dots.iter().any(Option::is_some));
        let a = e.ed.dots();
        e.ev(UiEvent::RawMotion { dx: 20.0, dy: 0.0 });
        assert_ne!(a, e.ed.dots());
        // a huge move re-centres: still inside the picture
        e.ev(UiEvent::RawMotion {
            dx: 4000.0,
            dy: 0.0,
        });
        for d in e.ed.dots().dots.iter().flatten() {
            assert!((0.0..=1.0).contains(&d.0) && (0.0..=1.0).contains(&d.1));
        }
    }

    #[test]
    fn test_mode_works_without_a_picture_and_with_a_mismatched_rotation() {
        let mut e = Ed::shooter();
        e.env.has_picture = false;
        e.env.rotation = Rotation::R0;
        enter_test(&mut e);
        assert!(e.ed.in_test());
        e.ev(UiEvent::Key {
            name: "KeyW",
            pressed: true,
            mods: Mods::default(),
        });
        assert!(e.ed.dots().dots.iter().any(Option::is_some));
    }

    // ---- guide ----

    #[test]
    fn guide_advances_when_the_user_really_does_the_step() {
        let mut e = Ed::shooter();
        e.ed = Editor::open("a.json", template(Template::ShooterLandscape), true).unwrap();
        e.key("KeyQ"); // any event runs the check
        assert_eq!(
            e.ed.guide_step(),
            Some(2),
            "picture present and right way up"
        );
        let from = e.px(0.15, 0.68);
        let to = Pt {
            x: from.x + 80.0,
            y: from.y,
        };
        e.mv(from);
        e.down(from);
        e.mv(to);
        e.up(to);
        assert_eq!(e.ed.guide_step(), Some(3));
        // skipping ends it and reports it
        let outs = e.click_outs("Bỏ qua hướng dẫn");
        assert!(outs.contains(&EditorOut::GuideDone));
        assert_eq!(e.ed.guide_step(), None);
    }

    // ---- misc ----

    #[test]
    fn scene_hides_with_h_and_draws_every_control() {
        let e = Ed::shooter();
        let s = e.ed.scene(&e.env, Lang::Vi, 0);
        assert!(!s.is_empty());
        // a ring for the joystick and a dashed frame for the look
        assert!(s.statics.iter().any(|p| matches!(p, Prim::Ring { .. })));
        assert!(
            s.statics
                .iter()
                .any(|p| matches!(p, Prim::Frame { dashed: true, .. }))
        );
        let mut e = e;
        e.ev(UiEvent::Key {
            name: "KeyH",
            pressed: true,
            mods: Mods::default(),
        });
        assert!(e.ed.hidden());
    }

    #[test]
    fn cap_text_is_short() {
        assert_eq!(cap_text("MouseLeft", Lang::Vi), "M1");
        assert_eq!(cap_text("ShiftLeft", Lang::Vi), "Shift");
        assert_eq!(cap_text("KeyW", Lang::Vi), "W");
        assert_eq!(cap_text("Backspace", Lang::Vi).chars().count(), 5);
    }

    #[test]
    fn open_with_draft_marks_the_editor_dirty() {
        let saved = template(Template::ShooterLandscape);
        let mut draft = saved.clone();
        draft.name = "Nháp".into();
        let e = Editor::open_with_draft("a.json", saved, draft).unwrap();
        assert!(e.dirty());
        assert_eq!(e.profile().name, "Nháp");
    }

    // ---- test 88 and the fuzz ----

    #[test]
    fn a_scripted_session_writes_a_profile_the_validator_accepts() {
        let mut e = Ed::with(template(Template::Empty(Orientation::Landscape)));
        e.click_text("Cần di chuyển");
        let p = e.px(0.2, 0.7);
        e.click(p);
        e.click_text("Xoay camera");
        let p = e.px(0.4, 0.4);
        e.click(p);
        e.click_text("Nút");
        let p = e.px(0.5, 0.8);
        e.click(p);
        e.key("KeyJ");
        e.select(0);
        e.ctrl("KeyD");
        e.select(2);
        e.ev(UiEvent::Wheel {
            p: e.px(0.4, 0.4),
            dy: -1.0,
            mods: Mods::default(),
        });
        let json = e.ed.profile().to_json_pretty();
        let back = Profile::from_json(&json).expect("valid");
        assert_eq!(&back, e.ed.profile());
        assert!(e.ed.profile().problems().is_empty());
    }

    #[test]
    fn fuzz_editor() {
        let names = [
            "KeyW",
            "KeyA",
            "Space",
            "Tab",
            "Enter",
            "Escape",
            "Backquote",
            "F8",
            "F12",
            "KeyH",
            "KeyZ",
            "KeyY",
            "KeyD",
            "KeyS",
            "Delete",
            "Backspace",
            "ArrowLeft",
            "ArrowUp",
            "Digit1",
            "BracketLeft",
            "NotAKey",
            "MouseLeft",
            "Home",
            "End",
            "KeyA",
            "ControlLeft",
        ];
        let buttons = [
            "MouseLeft",
            "MouseRight",
            "MouseMiddle",
            "MouseBack",
            "MouseForward",
        ];
        let texts = ["a", "ế", "Đ", "\u{1}", "ệ", "xyz\n"];
        let mut seeds_checked = 0;
        for seed in 1..=20u64 {
            let mut r = Lcg(seed * 7919);
            let mut e = Ed::with(template(if seed % 3 == 0 {
                Template::BasicPortrait
            } else if seed % 3 == 1 {
                Template::ShooterLandscape
            } else {
                Template::Empty(Orientation::Landscape)
            }));
            let open = e.ed.profile().clone();
            let mut changes = 0usize;
            for i in 0..3000 {
                let p = Pt {
                    x: r.unit() * 1500.0 - 100.0,
                    y: r.unit() * 700.0 - 50.0,
                };
                let mods = Mods {
                    shift: r.below(4) == 0,
                    ctrl: r.below(5) == 0,
                    alt: r.below(6) == 0,
                };
                let w = e.widgets();
                // aim at a widget a third of the time so dock actions happen
                let p = if r.below(3) == 0 && !w.is_empty() {
                    centre(r.pick(&w))
                } else {
                    p
                };
                let outs = match r.below(20) {
                    0..=5 => e.ev(UiEvent::PointerMove { p, mods }),
                    6 | 7 => e.ev(UiEvent::PointerDown {
                        p,
                        button: r.pick(&buttons),
                        mods,
                    }),
                    8 | 9 => e.ev(UiEvent::PointerUp {
                        p,
                        button: r.pick(&buttons),
                        mods,
                    }),
                    10 => e.ev(UiEvent::Wheel {
                        p,
                        dy: r.unit() * 4.0 - 2.0,
                        mods,
                    }),
                    11 | 12 => e.ev(UiEvent::Key {
                        name: r.pick(&names),
                        pressed: true,
                        mods,
                    }),
                    13 => e.ev(UiEvent::Key {
                        name: r.pick(&names),
                        pressed: false,
                        mods,
                    }),
                    14 => e.ev(UiEvent::KeyRepeat {
                        name: r.pick(&names),
                        mods,
                    }),
                    15 => e.ev(UiEvent::Text(r.pick(&texts))),
                    16 => e.ev(UiEvent::Tick),
                    17 => e.ev(UiEvent::RawMotion {
                        dx: r.unit() * 200.0 - 100.0,
                        dy: r.unit() * 200.0 - 100.0,
                    }),
                    18 => match r.below(4) {
                        0 => e.ev(UiEvent::FocusLost),
                        1 => e.ev(UiEvent::MenuPick(r.below(14) as u32)),
                        2 => {
                            e.env.has_picture = r.below(2) == 0;
                            e.ev(UiEvent::EnvChanged)
                        }
                        _ => {
                            e.env = if r.below(2) == 0 {
                                env()
                            } else {
                                env_sized(400.0, 860.0, 1.0 + r.below(2) as f64 * 0.5, Rotation::R0)
                            };
                            e.ev(UiEvent::EnvChanged)
                        }
                    },
                    _ => e.ev(UiEvent::Key {
                        name: "Tab",
                        pressed: true,
                        mods,
                    }),
                };
                changes += outs.iter().filter(|o| **o == EditorOut::Changed).count();
                let prof = e.ed.profile();
                assert!(
                    prof.problems().is_empty(),
                    "seed {seed} event {i}: {:?}",
                    prof.problems()
                );
                let ids = e.ed.doc.ids();
                assert_eq!(ids.len(), prof.controls.len());
                assert_eq!(ids.iter().collect::<HashSet<_>>().len(), ids.len());
                if let Some(s) = e.ed.selection() {
                    assert!(
                        e.ed.doc.index_of(s).is_some(),
                        "seed {seed} event {i}: dangling selection"
                    );
                }
                if i % 25 == 0 {
                    let s = e.ed.scene(&e.env, Lang::Vi, e.now);
                    let _ = s.to_svg(e.env.view.window, e.ed.pressed_mask());
                }
            }
            // Undo everything, then redo everything.
            e.ev(UiEvent::FocusLost);
            for _ in 0..3 {
                e.ev(UiEvent::Key {
                    name: "Escape",
                    pressed: true,
                    mods: Mods::default(),
                });
            }
            let last = e.ed.profile().clone();
            let mut n = 0;
            loop {
                let before = e.ed.profile().clone();
                e.ctrl("KeyZ");
                n += 1;
                if &before == e.ed.profile() || n > 400 {
                    break;
                }
            }
            if changes <= 90 && !e.ed.in_test() {
                seeds_checked += 1;
                assert_eq!(e.ed.profile(), &open, "seed {seed}: undo-all");
            }
            for _ in 0..n + 5 {
                e.ctrl("KeyY");
            }
            assert_eq!(e.ed.profile(), &last, "seed {seed}: redo-all");
        }
        assert!(
            seeds_checked >= 3,
            "only {seeds_checked} seeds were light enough to check undo-all"
        );
    }
}
