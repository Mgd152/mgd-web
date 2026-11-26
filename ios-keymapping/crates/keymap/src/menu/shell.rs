//! `Shell`: modes (Off, Hub, Edit, Test, Play), the Hub, the notice, refusals
//! and the effects the app must run (UI_SPEC 2, 3.5-3.9, 4, 6.2).
//!
//! The shell never emits a touch frame: no `Effect` carries one, and
//! `Effect::StartPlay` is returned only from Off and Hub.

use super::editor::{Editor, EditorOut};
use super::layout::{
    Column, ROW_H, Widget, WidgetId, WidgetKind, hit_widget, line_count, split, widgets_to_prims,
};
pub use super::lint::{Env, Link};
pub use super::msg::PlayRefusal;
use super::msg::{Fix, Lang, Msg};
use super::prefs::{Overlay, UiPrefs};
use super::scene::{Align, Font, Prim, Scene, Shape, Tone, ellipsize, max_chars};
use super::store::{Entry, Fs, Health, ProfileStore, StoreError, TrashToken};
use super::templates::{Template, template};
use super::tracker::TouchDots;
use super::ui_text::UiText;
use super::{Mods, NOTICE_VERSION, Pt, PxRect};
use crate::profile::{Control, Orientation, Profile};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Off,
    Hub,
    Edit,
    Test,
    Play,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CursorKind {
    Arrow,
    Hand,
    Move,
    ResizeNwse,
    Text,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Command {
    ToggleHub,
    OpenHub,
    OpenEdit,
    ChooseProfile(usize),
    ShowGuide,
    ShowNotice,
    ShowStuckHelp,
    Panic,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlayEnd {
    Toggle,
    Panic,
    FocusLost,
    Disconnected,
    Orientation,
}

#[derive(Clone, Debug)]
pub enum UiEvent<'a> {
    PointerMove {
        p: Pt,
        mods: Mods,
    },
    PointerDown {
        p: Pt,
        button: &'a str,
        mods: Mods,
    },
    PointerUp {
        p: Pt,
        button: &'a str,
        mods: Mods,
    },
    Wheel {
        p: Pt,
        dy: f64,
        mods: Mods,
    },
    Key {
        name: &'a str,
        pressed: bool,
        mods: Mods,
    },
    KeyRepeat {
        name: &'a str,
        mods: Mods,
    },
    Text(&'a str),
    RawMotion {
        dx: f64,
        dy: f64,
    },
    Tick,
    FocusLost,
    EnvChanged,
    Command(Command),
    MenuPick(u32),
    Pasted(Option<&'a str>),
    Dropped {
        name: &'a str,
        text: Option<&'a str>,
    },
    PlayEnded {
        reason: PlayEnd,
        look_sensitivity: Option<f64>,
    },
    PlayStarted,
}

#[derive(Clone, PartialEq, Debug)]
pub enum MenuItem {
    Item {
        id: u32,
        label: String,
        checked: bool,
        enabled: bool,
    },
    Separator,
    Sub {
        label: String,
        items: Vec<MenuItem>,
    },
}

#[derive(Clone, PartialEq, Debug)]
pub struct MenuSpec {
    pub items: Vec<MenuItem>,
}

/// What the app must do. No variant carries a touch frame.
#[derive(Clone, PartialEq, Debug)]
pub enum Effect {
    Redraw,
    Toast { msg: Msg, secs: f32 },
    Cursor(CursorKind),
    StartPlay,
    GrabPointer(bool),
    ShowMenu(MenuSpec),
    CopyText(String),
    RequestPaste,
    OpenFolder,
    ImeAllowed(bool),
    Rotate,
    ReserveRight(f64),
    Title,
}

/// What a key means while playing (UI_SPEC 4.7 rule 2).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlayKey {
    Engine,
    SensDown,
    SensUp,
}

/// Menu ids of `native_menu()` start here (the app adds its own below).
pub const NATIVE_BASE: u32 = 1000;
const NATIVE_OPEN_HUB: u32 = NATIVE_BASE + 1;
const NATIVE_EDIT: u32 = NATIVE_BASE + 2;
const NATIVE_GUIDE: u32 = NATIVE_BASE + 3;
const NATIVE_NOTICE: u32 = NATIVE_BASE + 4;
const NATIVE_STUCK: u32 = NATIVE_BASE + 5;
const NATIVE_PROFILE: u32 = NATIVE_BASE + 100;

const UNDO_TOAST_MS: u64 = 8000;
const AUTO_HIDE_MS: u64 = 5000;
const DRAFT_DELAY_MS: u64 = 1000;
const MAX_MENU_PROFILES: usize = 9;

#[derive(Clone, PartialEq, Debug)]
enum Screen {
    List,
    Templates { sel: usize, guide: bool },
    Notice { ticked: bool, then_play: bool },
    Stuck,
    Problems(String),
}

#[derive(Clone, PartialEq, Debug)]
enum HubAct {
    None,
    Close,
    Guide,
    Row(usize),
    Play(usize),
    Choose(usize),
    Edit(usize),
    More(usize),
    New,
    Share,
    Stuck,
    Rules,
    Fix(Fix),
    Restore,
    DiscardDraft,
    UndoDelete,
    SensRevert,
    Tpl(usize),
    Back,
    NoticeTick,
    NoticeNext,
    NoticeExit,
}

#[derive(Clone, PartialEq, Debug)]
enum PopupCtx {
    Row(usize),
    Share,
}

struct UndoToast {
    token: TrashToken,
    name: String,
    until: u64,
}

struct HubBuilt {
    ws: Vec<(Widget, HubAct)>,
    card: PxRect,
}

impl HubBuilt {
    fn push(&mut self, mut w: Widget, a: HubAct) {
        w.id = WidgetId(self.ws.len() as u32);
        self.ws.push((w, a));
    }
}

pub struct Shell<F: Fs> {
    store: ProfileStore<F>,
    prefs: UiPrefs,
    prefs_path: String,
    mode: Mode,
    entries: Vec<Entry>,
    active: Option<(String, Profile)>,
    screen: Screen,
    sel: usize,
    scroll: usize,
    editor: Option<Editor>,
    undo_toast: Option<UndoToast>,
    draft_offer: Option<String>,
    draft_due: Option<u64>,
    popup: Option<PopupCtx>,
    version: u64,
    play_started: Option<u64>,
    /// The `Tự ẩn` overlay has timed out (the version was bumped once).
    auto_hidden: bool,
    now: u64,
    last_env: Option<Env>,
}

fn contains(r: PxRect, p: Pt) -> bool {
    p.x >= r.x && p.x <= r.x + r.w && p.y >= r.y && p.y <= r.y + r.h
}

fn pt_of(c: &Control) -> Pt {
    match c {
        Control::Joystick { x, y, .. }
        | Control::Look { x, y, .. }
        | Control::Button { x, y, .. } => Pt { x: *x, y: *y },
    }
}

impl<F: Fs> Shell<F> {
    /// Loads prefs and the profile list; never fails (broken prefs mean
    /// defaults).
    pub fn new(fs: F, keymaps_dir: &str, prefs_path: &str) -> Shell<F> {
        let prefs = UiPrefs::load(&fs, prefs_path);
        let mut store = ProfileStore::new(fs, keymaps_dir);
        store.purge_trash();
        let mut s = Shell {
            store,
            prefs,
            prefs_path: prefs_path.to_string(),
            mode: Mode::Off,
            entries: Vec::new(),
            active: None,
            screen: Screen::List,
            sel: 0,
            scroll: 0,
            editor: None,
            undo_toast: None,
            draft_offer: None,
            draft_due: None,
            popup: None,
            version: 1,
            play_started: None,
            auto_hidden: false,
            now: 0,
            last_env: None,
        };
        s.refresh();
        s.find_draft_offer();
        s
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }
    pub fn lang(&self) -> Lang {
        self.prefs.lang
    }
    pub fn prefs(&self) -> &UiPrefs {
        &self.prefs
    }
    pub fn store(&self) -> &ProfileStore<F> {
        &self.store
    }
    /// For the app and tests that must reach the file system the shell owns
    /// (failure injection in tests).
    pub fn store_mut(&mut self) -> &mut ProfileStore<F> {
        &mut self.store
    }
    pub fn scene_version(&self) -> u64 {
        self.version
    }
    pub fn swallows_input(&self) -> bool {
        matches!(self.mode, Mode::Hub | Mode::Edit | Mode::Test)
    }
    pub fn active_profile(&self) -> Option<&Profile> {
        self.active.as_ref().map(|(_, p)| p)
    }
    pub fn active_file(&self) -> Option<&str> {
        self.active.as_ref().map(|(f, _)| f.as_str())
    }
    pub fn editor(&self) -> Option<&Editor> {
        self.editor.as_ref()
    }
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn dots(&self) -> TouchDots {
        self.editor.as_ref().map(Editor::dots).unwrap_or_default()
    }

    /// Bit i = control i is held in Test (Play's mask comes from the app).
    pub fn pressed_mask(&self) -> u64 {
        self.editor.as_ref().map_or(0, Editor::pressed_mask)
    }

    fn touch(&mut self) {
        self.version += 1;
    }

    // ----------------------------------------------------------- storage ----

    fn refresh(&mut self) {
        self.entries = self.store.list_in(self.prefs.lang);
        if self.sel >= self.entries.len() {
            self.sel = self.entries.len().saturating_sub(1);
        }
        let file = self.prefs.active.clone();
        self.active = file.and_then(|f| {
            let healthy = self
                .entries
                .iter()
                .any(|e| e.file == f && e.health == Health::Ok);
            if healthy {
                self.store.load(&f).ok().map(|p| (f, p))
            } else {
                None
            }
        });
    }

    fn find_draft_offer(&mut self) {
        self.draft_offer =
            self.store.drafts().into_iter().find(|f| {
                self.store.draft(f).is_some() && self.entries.iter().any(|e| &e.file == f)
            });
    }

    fn save_prefs(&mut self) {
        let _ = self.prefs.save(self.store.fs_mut(), &self.prefs_path);
    }

    fn set_active(&mut self, file: Option<String>) {
        self.prefs.active = file;
        self.save_prefs();
        self.refresh();
    }

    fn entry_name(&self, file: &str) -> String {
        self.entries
            .iter()
            .find(|e| e.file == file)
            .map(|e| e.name.clone())
            .unwrap_or_else(|| file.to_string())
    }

    // ------------------------------------------------------------- play ----

    pub fn can_play(&self, env: &Env) -> Result<(), PlayRefusal> {
        let Some((file, profile)) = &self.active else {
            return match self.prefs.active.as_ref().and_then(|f| {
                self.entries
                    .iter()
                    .find(|e| &e.file == f && matches!(e.health, Health::Broken(_)))
            }) {
                Some(_) => Err(PlayRefusal::BrokenProfile),
                None => Err(PlayRefusal::NoProfile),
            };
        };
        let _ = file;
        if self.prefs.notice_ack < NOTICE_VERSION {
            return Err(PlayRefusal::NoticeNotAccepted);
        }
        match env.link {
            Link::None => return Err(PlayRefusal::NotConnected),
            Link::Wifi => return Err(PlayRefusal::Wifi),
            Link::Usb => {}
        }
        if env.display_mode {
            return Err(PlayRefusal::DisplayMode);
        }
        if env.scroll_busy {
            return Err(PlayRefusal::ScrollBusy);
        }
        if !profile.orientation.matches(env.rotation) {
            return Err(PlayRefusal::Orientation {
                profile: profile.orientation,
            });
        }
        Ok(())
    }

    pub fn play_key(&self, name: &str) -> PlayKey {
        if self
            .active
            .as_ref()
            .is_some_and(|(_, p)| profile_binds(p, name))
        {
            return PlayKey::Engine;
        }
        if name == self.prefs.sens_down_key {
            PlayKey::SensDown
        } else if name == self.prefs.sens_up_key {
            PlayKey::SensUp
        } else {
            PlayKey::Engine
        }
    }

    pub fn title_suffix(&self, lang: Lang) -> String {
        match self.mode {
            Mode::Play => format!(" · {}", Msg::Ui(UiText::TitleOn).text(lang)),
            Mode::Off => match &self.active {
                Some((_, p)) => format!(
                    " · {}",
                    Msg::Ui(UiText::TitlePress(p.toggle_key.clone())).text(lang)
                ),
                None => String::new(),
            },
            _ => String::new(),
        }
    }

    /// Whether an F-key the shell did not consume should continue into
    /// Mirrorly's own shortcut handling (table 4.1).
    pub fn passes_to_upstream(&self, name: &str) -> bool {
        match self.mode {
            Mode::Off => name != "F8",
            Mode::Hub => matches!(
                name,
                "F1" | "F2" | "F3" | "F4" | "F5" | "F6" | "F7" | "F10" | "F11" | "F12"
            ),
            Mode::Edit | Mode::Test => self
                .editor
                .as_ref()
                .is_some_and(|e| e.passes_to_upstream(name)),
            Mode::Play => name != "F9" && name != "F8",
        }
    }

    // ------------------------------------------------------------ events ----

    pub fn event(&mut self, ev: UiEvent<'_>, env: &Env, now_ms: u64) -> Vec<Effect> {
        self.now = now_ms;
        self.last_env = Some(*env);
        let before = self.version;
        let mut fx = Vec::new();
        self.expire(now_ms);
        self.dispatch(&ev, env, now_ms, &mut fx);
        // The editor's own state can change on any event.
        if self.editor.is_some() {
            self.touch_if_editing(&ev);
        } else if self.mode == Mode::Hub && !matches!(ev, UiEvent::PointerMove { .. }) {
            self.touch();
        }
        if self.version != before {
            fx.push(Effect::Redraw);
        }
        fx
    }

    fn touch_if_editing(&mut self, ev: &UiEvent<'_>) {
        match ev {
            UiEvent::RawMotion { .. } => {}
            UiEvent::Tick if self.mode != Mode::Test => {}
            _ => self.touch(),
        }
    }

    fn expire(&mut self, now: u64) {
        if let Some(t) = &self.undo_toast
            && now >= t.until
        {
            self.undo_toast = None;
            self.touch();
        }
    }

    fn dispatch(&mut self, ev: &UiEvent<'_>, env: &Env, now: u64, fx: &mut Vec<Effect>) {
        match ev {
            UiEvent::PlayStarted => {
                self.start_play(now, fx);
                return;
            }
            UiEvent::PlayEnded {
                reason,
                look_sensitivity,
            } => {
                self.end_play(*reason, *look_sensitivity, fx);
                return;
            }
            UiEvent::Command(c) => {
                self.command(c.clone(), env, now, fx);
                return;
            }
            UiEvent::MenuPick(id) => {
                if self.popup.is_some() || *id < NATIVE_BASE {
                    if self.mode == Mode::Edit {
                        self.editor_event(ev, env, now, fx);
                    } else {
                        self.popup_pick(*id, env, now, fx);
                    }
                } else {
                    self.native_pick(*id, env, now, fx);
                }
                return;
            }
            UiEvent::Tick => {
                self.tick(now, fx);
            }
            _ => {}
        }
        match self.mode {
            Mode::Off => self.off_event(ev, env, fx),
            Mode::Hub => self.hub_event(ev, env, now, fx),
            Mode::Edit | Mode::Test => self.editor_event(ev, env, now, fx),
            Mode::Play => {}
        }
    }

    fn tick(&mut self, now: u64, fx: &mut Vec<Effect>) {
        if self.mode == Mode::Play
            && self.prefs.overlay == Overlay::Auto
            && !self.auto_hidden
            && let Some(s) = self.play_started
            && now >= s + AUTO_HIDE_MS
        {
            self.auto_hidden = true;
            self.touch();
        }
        if let Some(due) = self.draft_due
            && now >= due
        {
            self.draft_due = None;
            self.write_draft();
        }
        let _ = fx;
    }

    fn write_draft(&mut self) {
        if let Some(e) = &self.editor
            && e.dirty()
        {
            let (file, p) = (e.file().to_string(), e.profile().clone());
            let _ = self.store.write_draft(&file, &p);
        }
    }

    pub fn next_deadline(&self) -> Option<u64> {
        let mut d: Vec<u64> = Vec::new();
        if let Some(t) = &self.undo_toast {
            d.push(t.until);
        }
        if let Some(t) = self.draft_due {
            d.push(t);
        }
        if self.mode == Mode::Play
            && self.prefs.overlay == Overlay::Auto
            && !self.auto_hidden
            && let Some(s) = self.play_started
        {
            d.push(s + AUTO_HIDE_MS);
        }
        if let Some(e) = &self.editor
            && let Some(t) = e.next_deadline()
        {
            d.push(t);
        }
        d.into_iter().min()
    }

    fn toast(fx: &mut Vec<Effect>, msg: Msg, secs: f32) {
        fx.push(Effect::Toast { msg, secs });
    }

    fn start_play(&mut self, now: u64, fx: &mut Vec<Effect>) {
        if self.mode == Mode::Play {
            return;
        }
        self.mode = Mode::Play;
        self.play_started = Some(now);
        self.auto_hidden = false;
        self.touch();
        let (toggle, panic) = self
            .active
            .as_ref()
            .map(|(_, p)| (p.toggle_key.clone(), p.panic_key.clone()))
            .unwrap_or_else(|| ("Backquote".into(), "F8".into()));
        Self::toast(fx, Msg::PlayOn { toggle, panic }, 5.0);
        fx.push(Effect::Title);
    }

    fn end_play(&mut self, reason: PlayEnd, sens: Option<f64>, fx: &mut Vec<Effect>) {
        if self.mode != Mode::Play {
            return;
        }
        self.mode = Mode::Off;
        self.play_started = None;
        self.touch();
        if let Some(new) = sens {
            self.save_play_sensitivity(new);
        }
        let msg = if reason == PlayEnd::Panic {
            Msg::PanicDone
        } else {
            Msg::PlayOff
        };
        Self::toast(fx, msg, 3.0);
        fx.push(Effect::Title);
    }

    fn save_play_sensitivity(&mut self, new: f64) {
        let Some((file, profile)) = self.active.clone() else {
            return;
        };
        let Some(old) = profile.controls.iter().find_map(|c| match c {
            Control::Look { sensitivity, .. } => Some(*sensitivity),
            _ => None,
        }) else {
            return;
        };
        if !new.is_finite() || (new - old).abs() < 1e-9 {
            return;
        }
        if self.rewrite_sensitivity(&file, &profile, new).is_some() {
            self.prefs.last_sensitivity_change = Some((file, old, new));
            self.save_prefs();
            self.refresh();
        }
    }

    fn rewrite_sensitivity(&mut self, file: &str, profile: &Profile, value: f64) -> Option<()> {
        let mut doc = super::doc::EditDoc::from_profile(profile.clone()).ok()?;
        let id = doc
            .ids()
            .iter()
            .copied()
            .find(|i| doc.kind(*i) == Some(super::doc::Kind::Look))?;
        doc.set_sensitivity(id, value).ok()?;
        self.store.save(file, doc.profile()).ok()
    }

    // ------------------------------------------------------------- Off ----

    fn toggle_key(&self) -> String {
        self.active
            .as_ref()
            .map(|(_, p)| p.toggle_key.clone())
            .unwrap_or_else(|| "Backquote".to_string())
    }

    fn off_event(&mut self, ev: &UiEvent<'_>, env: &Env, fx: &mut Vec<Effect>) {
        if let UiEvent::Key {
            name,
            pressed: true,
            ..
        } = ev
        {
            if *name == "F8" {
                Self::toast(fx, Msg::PanicDone, 3.0);
            } else if *name == self.toggle_key() {
                match self.can_play(env) {
                    Ok(()) => fx.push(Effect::StartPlay),
                    Err(r) => Self::toast(fx, Msg::Refusal(r), 5.0),
                }
            }
        }
    }

    fn command(&mut self, c: Command, env: &Env, now: u64, fx: &mut Vec<Effect>) {
        match (self.mode, c) {
            (Mode::Off, Command::ToggleHub | Command::OpenHub) => self.open_hub(env, fx),
            (Mode::Hub, Command::ToggleHub) => self.close_hub(),
            (Mode::Hub, Command::OpenHub) => {}
            (Mode::Off | Mode::Hub, Command::OpenEdit) => {
                if self.mode == Mode::Off {
                    self.open_hub(env, fx);
                }
                match self.chosen_row() {
                    Some(i) => self.open_edit(i, false, env, fx),
                    None => {
                        self.screen = Screen::Templates {
                            sel: self.preselect(env),
                            guide: false,
                        }
                    }
                }
            }
            (Mode::Off | Mode::Hub, Command::ChooseProfile(n)) => {
                let valid: Vec<String> = self.valid_files();
                if let Some(f) = valid.get(n) {
                    self.set_active(Some(f.clone()));
                    self.touch();
                    fx.push(Effect::Title);
                }
            }
            (Mode::Off | Mode::Hub, Command::ShowGuide) => {
                if self.mode == Mode::Off {
                    self.open_hub(env, fx);
                }
                match self.chosen_row() {
                    Some(i) => self.open_edit(i, true, env, fx),
                    None => {
                        self.screen = Screen::Templates {
                            sel: self.preselect(env),
                            guide: true,
                        }
                    }
                }
            }
            (Mode::Off | Mode::Hub, Command::ShowNotice) => {
                if self.mode == Mode::Off {
                    self.open_hub(env, fx);
                }
                self.screen = Screen::Notice {
                    ticked: self.prefs.notice_ack >= NOTICE_VERSION,
                    then_play: false,
                };
            }
            (Mode::Off | Mode::Hub, Command::ShowStuckHelp) => {
                if self.mode == Mode::Off {
                    self.open_hub(env, fx);
                }
                self.screen = Screen::Stuck;
            }
            (Mode::Off | Mode::Hub, Command::Panic) => Self::toast(fx, Msg::PanicDone, 3.0),
            (Mode::Edit | Mode::Test, Command::Panic) => {
                let ev = UiEvent::Key {
                    name: "F8",
                    pressed: true,
                    mods: Mods::default(),
                };
                self.editor_event(&ev, env, now, fx);
            }
            _ => {}
        }
        self.touch();
    }

    fn valid_files(&self) -> Vec<String> {
        self.entries
            .iter()
            .filter(|e| e.health == Health::Ok)
            .take(MAX_MENU_PROFILES)
            .map(|e| e.file.clone())
            .collect()
    }

    /// Row index of the chosen (active) profile in `entries`.
    fn chosen_row(&self) -> Option<usize> {
        let f = self.active.as_ref().map(|(f, _)| f.as_str())?;
        self.entries.iter().position(|e| e.file == f)
    }

    fn open_hub(&mut self, env: &Env, fx: &mut Vec<Effect>) {
        let _ = env;
        self.mode = Mode::Hub;
        self.screen = Screen::List;
        self.scroll = 0;
        if let Some(i) = self.chosen_row() {
            self.sel = i;
        }
        self.refresh();
        self.find_draft_offer();
        self.touch();
        fx.push(Effect::Title);
    }

    fn close_hub(&mut self) {
        self.mode = Mode::Off;
        self.screen = Screen::List;
        self.touch();
    }

    pub fn native_menu(&self) -> MenuItem {
        let l = self.prefs.lang;
        let t = |u: UiText| Msg::Ui(u).text(l);
        let item = |id: u32, label: String, checked: bool, enabled: bool| MenuItem::Item {
            id,
            label,
            checked,
            enabled,
        };
        let active = self.active.as_ref().map(|(f, _)| f.clone());
        let profiles: Vec<MenuItem> = self
            .entries
            .iter()
            .filter(|e| e.health == Health::Ok)
            .take(MAX_MENU_PROFILES)
            .enumerate()
            .map(|(i, e)| {
                item(
                    NATIVE_PROFILE + i as u32,
                    e.name.clone(),
                    active.as_deref() == Some(e.file.as_str()),
                    true,
                )
            })
            .collect();
        MenuItem::Sub {
            label: t(UiText::HubTitle),
            items: vec![
                item(NATIVE_OPEN_HUB, t(UiText::OpenPanel), false, true),
                item(NATIVE_EDIT, t(UiText::EditButtons), false, true),
                MenuItem::Sub {
                    label: t(UiText::ProfilesMenu),
                    items: profiles,
                },
                MenuItem::Separator,
                item(NATIVE_GUIDE, t(UiText::FirstGuide), false, true),
                item(NATIVE_NOTICE, t(UiText::RulesLink), false, true),
                item(NATIVE_STUCK, t(UiText::StuckLink), false, true),
                MenuItem::Separator,
                item(NATIVE_BASE + 99, t(UiText::PanicNote), false, false),
            ],
        }
    }

    fn native_pick(&mut self, id: u32, env: &Env, now: u64, fx: &mut Vec<Effect>) {
        let c = match id {
            NATIVE_OPEN_HUB => Command::OpenHub,
            NATIVE_EDIT => Command::OpenEdit,
            NATIVE_GUIDE => Command::ShowGuide,
            NATIVE_NOTICE => Command::ShowNotice,
            NATIVE_STUCK => Command::ShowStuckHelp,
            i if i >= NATIVE_PROFILE => Command::ChooseProfile((i - NATIVE_PROFILE) as usize),
            _ => return,
        };
        self.command(c, env, now, fx);
    }

    // ------------------------------------------------------------- Hub ----

    fn preselect(&self, env: &Env) -> usize {
        if env.rotation.is_landscape() { 0 } else { 2 }
    }

    fn templates(env: &Env) -> [Template; 4] {
        [
            Template::ShooterLandscape,
            Template::BasicLandscape,
            Template::BasicPortrait,
            Template::Empty(if env.rotation.is_landscape() {
                Orientation::Landscape
            } else {
                Orientation::Portrait
            }),
        ]
    }

    fn hub_event(&mut self, ev: &UiEvent<'_>, env: &Env, now: u64, fx: &mut Vec<Effect>) {
        match ev {
            UiEvent::Key {
                name,
                pressed: true,
                mods,
            } => self.hub_key(name, *mods, env, now, fx),
            UiEvent::KeyRepeat { name, .. } => {
                if matches!(*name, "ArrowUp" | "ArrowDown") {
                    self.hub_key(name, Mods::default(), env, now, fx);
                }
            }
            UiEvent::PointerDown {
                p,
                button: "MouseLeft",
                ..
            } => {
                let built = self.hub_build(env);
                let ws: Vec<Widget> = built.ws.iter().map(|(w, _)| w.clone()).collect();
                if let Some((id, _)) = hit_widget(&ws, *p) {
                    let act = built.ws[id.0 as usize].1.clone();
                    self.hub_act(act, env, now, fx);
                } else if !contains(built.card, *p) && self.screen == Screen::List {
                    self.close_hub();
                    fx.push(Effect::Title);
                }
            }
            UiEvent::Wheel { dy, .. } => {
                if self.screen == Screen::List {
                    if *dy < 0.0 {
                        let max = self.entries.len().saturating_sub(1);
                        self.scroll = (self.scroll + 1).min(max);
                    } else if *dy > 0.0 {
                        self.scroll = self.scroll.saturating_sub(1);
                    }
                }
            }
            UiEvent::Pasted(Some(text)) => self.import(text, "", fx),
            UiEvent::Dropped {
                name,
                text: Some(text),
            } => self.import(text, name, fx),
            UiEvent::FocusLost | UiEvent::EnvChanged => {}
            _ => {}
        }
    }

    fn import(&mut self, text: &str, hint: &str, fx: &mut Vec<Effect>) {
        match self.store.import_text(text, hint) {
            Ok(file) => {
                if self.active.is_none() {
                    self.prefs.active = Some(file);
                    self.save_prefs();
                }
                self.refresh();
                Self::toast(fx, Msg::Ui(UiText::ImportOk), 3.0);
            }
            Err(e) => Self::toast(fx, store_error_msg(e), 5.0),
        }
    }

    fn hub_key(&mut self, name: &str, mods: Mods, env: &Env, now: u64, fx: &mut Vec<Effect>) {
        if name == "F8" {
            Self::toast(fx, Msg::PanicDone, 3.0);
            return;
        }
        match &self.screen {
            Screen::List => match name {
                "Escape" => {
                    self.close_hub();
                    fx.push(Effect::Title);
                }
                "KeyK" if mods.ctrl => {
                    self.close_hub();
                    fx.push(Effect::Title);
                }
                "ArrowUp" => self.sel = self.sel.saturating_sub(1),
                "ArrowDown" => {
                    self.sel = (self.sel + 1).min(self.entries.len().saturating_sub(1));
                }
                "Enter" | "NumpadEnter" => {
                    if self.entries.get(self.sel).is_some() {
                        self.hub_act(HubAct::Play(self.sel), env, now, fx);
                    }
                }
                "Delete" => self.delete_row(self.sel, now),
                "KeyZ" if mods.ctrl => self.undelete(),
                "KeyV" if mods.ctrl => fx.push(Effect::RequestPaste),
                _ => {}
            },
            Screen::Templates { sel, guide } => {
                let (sel, guide) = (*sel, *guide);
                match name {
                    "Escape" => self.screen = Screen::List,
                    "ArrowUp" => {
                        self.screen = Screen::Templates {
                            sel: sel.saturating_sub(1),
                            guide,
                        }
                    }
                    "ArrowDown" => {
                        self.screen = Screen::Templates {
                            sel: (sel + 1).min(3),
                            guide,
                        }
                    }
                    "Enter" | "NumpadEnter" => self.hub_act(HubAct::Tpl(sel), env, now, fx),
                    _ => {}
                }
            }
            Screen::Notice { ticked, then_play } => {
                let (ticked, then_play) = (*ticked, *then_play);
                match name {
                    "Escape" => self.screen = Screen::List,
                    "Space" => {
                        self.screen = Screen::Notice {
                            ticked: !ticked,
                            then_play,
                        }
                    }
                    _ => {}
                }
            }
            Screen::Stuck | Screen::Problems(_) => {
                if name == "Escape" || name == "Enter" {
                    self.screen = Screen::List;
                }
            }
        }
    }

    fn delete_row(&mut self, i: usize, now: u64) {
        let Some(e) = self.entries.get(i) else {
            return;
        };
        let (file, name) = (e.file.clone(), e.name.clone());
        let was_active = self.active_file() == Some(file.as_str());
        if let Ok(token) = self.store.delete(&file) {
            self.undo_toast = Some(UndoToast {
                token,
                name,
                until: now + UNDO_TOAST_MS,
            });
            if was_active {
                self.prefs.active = None;
                self.save_prefs();
            }
            self.refresh();
        }
    }

    fn undelete(&mut self) {
        if let Some(t) = self.undo_toast.take() {
            let was = t.name.clone();
            if let Ok(file) = self.store.undelete(t.token)
                && self.prefs.active.is_none()
                && self.entries.is_empty()
            {
                self.prefs.active = Some(file);
                self.save_prefs();
            }
            let _ = was;
            self.refresh();
        }
    }

    fn hub_act(&mut self, act: HubAct, env: &Env, now: u64, fx: &mut Vec<Effect>) {
        match act {
            HubAct::None => {}
            HubAct::Close => {
                if self.screen == Screen::List {
                    self.close_hub();
                    fx.push(Effect::Title);
                } else {
                    self.screen = Screen::List;
                }
            }
            HubAct::Back => self.screen = Screen::List,
            HubAct::Guide => match self.chosen_row() {
                Some(i) => self.open_edit(i, true, env, fx),
                None => {
                    self.screen = Screen::Templates {
                        sel: self.preselect(env),
                        guide: true,
                    }
                }
            },
            HubAct::Row(i) => self.sel = i,
            HubAct::Play(i) => {
                if let Some(e) = self.entries.get(i)
                    && e.health == Health::Ok
                {
                    let f = e.file.clone();
                    self.sel = i;
                    if self.active_file() != Some(f.as_str()) {
                        self.set_active(Some(f));
                        fx.push(Effect::Title);
                    }
                    self.hub_play(fx);
                }
            }
            HubAct::Choose(i) => {
                if let Some(e) = self.entries.get(i)
                    && e.health == Health::Ok
                {
                    let f = e.file.clone();
                    self.sel = i;
                    self.set_active(Some(f));
                    fx.push(Effect::Title);
                }
            }
            HubAct::Edit(i) => self.open_edit(i, false, env, fx),
            HubAct::More(i) => {
                let l = self.prefs.lang;
                let t = |u: UiText| Msg::Ui(u).text(l);
                let broken = self
                    .entries
                    .get(i)
                    .is_none_or(|e| matches!(e.health, Health::Broken(_)));
                let it = |id: u32, label: String, enabled: bool| MenuItem::Item {
                    id,
                    label,
                    checked: false,
                    enabled,
                };
                self.popup = Some(PopupCtx::Row(i));
                fx.push(Effect::ShowMenu(MenuSpec {
                    items: vec![
                        it(1, t(UiText::Rename), !broken),
                        it(2, t(UiText::Duplicate), !broken),
                        it(3, t(UiText::Delete), true),
                    ],
                }));
            }
            HubAct::New => {
                self.screen = Screen::Templates {
                    sel: self.preselect(env),
                    guide: !self.prefs.guide_done,
                }
            }
            HubAct::Share => {
                let l = self.prefs.lang;
                let t = |u: UiText| Msg::Ui(u).text(l);
                let has = self
                    .chosen_row()
                    .or(Some(self.sel))
                    .is_some_and(|i| self.entries.get(i).is_some_and(|e| e.health == Health::Ok));
                let it = |id: u32, label: String, enabled: bool| MenuItem::Item {
                    id,
                    label,
                    checked: false,
                    enabled,
                };
                self.popup = Some(PopupCtx::Share);
                fx.push(Effect::ShowMenu(MenuSpec {
                    items: vec![
                        it(1, t(UiText::CopyProfile), has),
                        it(2, t(UiText::PasteProfile), true),
                        it(3, t(UiText::OpenFolder), true),
                    ],
                }));
            }
            HubAct::Stuck => self.screen = Screen::Stuck,
            HubAct::Rules => {
                self.screen = Screen::Notice {
                    ticked: self.prefs.notice_ack >= NOTICE_VERSION,
                    then_play: false,
                }
            }
            HubAct::Fix(f) => match f {
                Fix::OpenProfileList => self.screen = Screen::List,
                Fix::ReadNotice => {
                    self.screen = Screen::Notice {
                        ticked: false,
                        then_play: false,
                    }
                }
                Fix::RotateView => fx.push(Effect::Rotate),
                Fix::ShowHowToFix => {
                    let file = self
                        .entries
                        .iter()
                        .find(|e| matches!(e.health, Health::Broken(_)))
                        .or(self.entries.get(self.sel))
                        .map(|e| e.file.clone());
                    if let Some(f) = file {
                        self.screen = Screen::Problems(f);
                    }
                }
            },
            HubAct::Restore => self.restore_draft(env, fx),
            HubAct::DiscardDraft => {
                if let Some(f) = self.draft_offer.take() {
                    self.store.drop_draft(&f);
                    self.find_draft_offer();
                }
            }
            HubAct::UndoDelete => self.undelete(),
            HubAct::SensRevert => {
                if let Some((file, from, _)) = self.prefs.last_sensitivity_change.clone()
                    && let Ok(p) = self.store.load(&file)
                {
                    self.rewrite_sensitivity(&file, &p, from);
                    self.prefs.last_sensitivity_change = None;
                    self.save_prefs();
                    self.refresh();
                }
            }
            HubAct::Tpl(i) => {
                let guide = match &self.screen {
                    Screen::Templates { guide, .. } => *guide,
                    _ => !self.prefs.guide_done,
                };
                let tpl = Self::templates(env)[i.min(3)];
                match self.store.create(tpl) {
                    Ok(file) => {
                        if self.active.is_none() {
                            self.prefs.active = Some(file.clone());
                            self.save_prefs();
                        }
                        self.refresh();
                        if let Some(idx) = self.entries.iter().position(|e| e.file == file) {
                            self.open_edit(idx, guide, env, fx);
                        }
                    }
                    Err(_) => Self::toast(fx, Msg::SaveFailed, 5.0),
                }
            }
            HubAct::NoticeTick => {
                if let Screen::Notice { ticked, then_play } = self.screen {
                    self.screen = Screen::Notice {
                        ticked: !ticked,
                        then_play,
                    };
                }
            }
            HubAct::NoticeNext => {
                if let Screen::Notice {
                    ticked: true,
                    then_play,
                } = self.screen
                {
                    self.prefs.notice_ack = NOTICE_VERSION;
                    self.save_prefs();
                    self.screen = Screen::List;
                    if then_play {
                        self.hub_play(fx);
                    }
                }
            }
            HubAct::NoticeExit => self.screen = Screen::List,
        }
        let _ = now;
    }

    /// `CHƠI` on the chosen row.
    fn hub_play(&mut self, fx: &mut Vec<Effect>) {
        if self.prefs.notice_ack < NOTICE_VERSION {
            self.screen = Screen::Notice {
                ticked: false,
                then_play: true,
            };
            return;
        }
        let env = self.last_env;
        if let Some(env) = env
            && self.can_play(&env).is_ok()
        {
            fx.push(Effect::StartPlay);
        }
    }

    fn popup_pick(&mut self, id: u32, env: &Env, now: u64, fx: &mut Vec<Effect>) {
        let Some(ctx) = self.popup.take() else {
            return;
        };
        match (ctx, id) {
            (PopupCtx::Row(i), 1) => self.open_edit(i, false, env, fx),
            (PopupCtx::Row(i), 2) => {
                if let Some(e) = self.entries.get(i) {
                    let f = e.file.clone();
                    let _ = self.store.duplicate(&f);
                    self.refresh();
                }
            }
            (PopupCtx::Row(i), 3) => self.delete_row(i, now),
            (PopupCtx::Share, 1) => {
                let i = self.chosen_row().unwrap_or(self.sel);
                if let Some(e) = self.entries.get(i)
                    && let Ok(text) = self.store.export_text(&e.file)
                {
                    fx.push(Effect::CopyText(text));
                    Self::toast(fx, Msg::Copied, 4.0);
                }
            }
            (PopupCtx::Share, 2) => fx.push(Effect::RequestPaste),
            (PopupCtx::Share, 3) => fx.push(Effect::OpenFolder),
            _ => {}
        }
        self.touch();
    }

    // -------------------------------------------------------- Edit / Test ----

    fn open_edit(&mut self, row: usize, guide: bool, env: &Env, fx: &mut Vec<Effect>) {
        let Some(e) = self.entries.get(row) else {
            return;
        };
        let file = e.file.clone();
        if matches!(e.health, Health::Broken(_)) {
            self.screen = Screen::Problems(file);
            return;
        }
        let Ok(profile) = self.store.load(&file) else {
            self.screen = Screen::Problems(file);
            return;
        };
        self.start_editor(Editor::open(&file, profile, guide), env, fx);
    }

    fn start_editor(
        &mut self,
        ed: Result<Editor, crate::profile::ProfileError>,
        env: &Env,
        fx: &mut Vec<Effect>,
    ) {
        let Ok(mut ed) = ed else {
            return;
        };
        ed.set_overlay(self.prefs.overlay);
        ed.set_lang(self.prefs.lang);
        self.editor = Some(ed);
        self.mode = Mode::Edit;
        self.screen = Screen::List;
        self.touch();
        if env.can_grow_window {
            fx.push(Effect::ReserveRight(super::layout::DOCK_W));
        }
        fx.push(Effect::Title);
    }

    fn restore_draft(&mut self, env: &Env, fx: &mut Vec<Effect>) {
        let Some(file) = self.draft_offer.take() else {
            return;
        };
        let (Ok(saved), Some(draft)) = (self.store.load(&file), self.store.draft(&file)) else {
            self.find_draft_offer();
            return;
        };
        self.start_editor(Editor::open_with_draft(&file, saved, draft), env, fx);
    }

    fn editor_event(&mut self, ev: &UiEvent<'_>, env: &Env, now: u64, fx: &mut Vec<Effect>) {
        let Some(ed) = self.editor.as_mut() else {
            self.mode = Mode::Hub;
            return;
        };
        ed.set_overlay(self.prefs.overlay);
        ed.set_lang(self.prefs.lang);
        let outs = ed.event(ev, env, now);
        self.handle_outs(outs, env, now, fx);
        self.sync_mode();
    }

    fn sync_mode(&mut self) {
        if let Some(ed) = &self.editor {
            self.mode = if ed.in_test() { Mode::Test } else { Mode::Edit };
        }
    }

    fn handle_outs(&mut self, outs: Vec<EditorOut>, env: &Env, now: u64, fx: &mut Vec<Effect>) {
        let mut veto = false;
        for o in outs {
            match o {
                EditorOut::Effect(e) => fx.push(e),
                EditorOut::Save => {
                    if !self.save_editor(fx) {
                        veto = true;
                    }
                }
                EditorOut::Leave => {
                    if !veto {
                        self.leave_edit(fx);
                    }
                }
                EditorOut::SaveAndPlay => {
                    if !veto && self.save_editor(fx) {
                        self.leave_edit(fx);
                        self.hub_play(fx);
                    } else {
                        veto = true;
                    }
                }
                EditorOut::Changed => self.draft_due = Some(now + DRAFT_DELAY_MS),
                EditorOut::SetOverlay(o) => {
                    self.prefs.overlay = o;
                    self.save_prefs();
                }
                EditorOut::GuideDone => {
                    self.prefs.guide_done = true;
                    self.save_prefs();
                }
            }
        }
        let _ = env;
    }

    fn save_editor(&mut self, fx: &mut Vec<Effect>) -> bool {
        let Some(ed) = &self.editor else {
            return false;
        };
        let (file, profile) = (ed.file().to_string(), ed.profile().clone());
        match self.store.save(&file, &profile) {
            Ok(()) => {
                if let Some(ed) = &mut self.editor {
                    ed.mark_saved();
                }
                self.store.drop_draft(&file);
                self.draft_due = None;
                self.refresh();
                true
            }
            Err(_) => {
                Self::toast(fx, Msg::SaveFailed, 6.0);
                false
            }
        }
    }

    fn leave_edit(&mut self, fx: &mut Vec<Effect>) {
        if let Some(ed) = self.editor.take() {
            self.store.drop_draft(ed.file());
        }
        self.draft_due = None;
        self.mode = Mode::Hub;
        self.screen = Screen::List;
        self.refresh();
        self.find_draft_offer();
        fx.push(Effect::ReserveRight(0.0));
        fx.push(Effect::ImeAllowed(false));
        fx.push(Effect::Title);
    }

    // ------------------------------------------------------------ scenes ----

    /// What to draw now; `None` = draw nothing.
    pub fn scene(&self, env: &Env, now_ms: u64) -> Option<Scene> {
        let lang = self.prefs.lang;
        let mut s = match self.mode {
            Mode::Off => return None,
            Mode::Play => self.play_scene(env, now_ms)?,
            Mode::Hub => self.hub_scene(env, lang),
            Mode::Edit | Mode::Test => {
                let ed = self.editor.as_ref()?;
                if ed.hidden() {
                    return None;
                }
                let mut s = ed.scene(env, lang, now_ms);
                if let Some(m) = ed.test_full_message(now_ms) {
                    let v = &env.view;
                    let r = PxRect {
                        x: v.picture.x + v.lp(8.0),
                        y: v.picture.y + v.picture.h - v.lp(72.0),
                        w: v.picture.w - v.lp(16.0),
                        h: v.lp(28.0),
                    };
                    s.dynamics.push(Prim::Rect {
                        r,
                        radius: v.lp(6.0),
                        tone: Tone::Danger,
                    });
                    s.dynamics.push(Prim::text(
                        r,
                        m.text(lang),
                        Font::Body,
                        Align::Center,
                        Tone::OnAccent,
                    ));
                }
                s
            }
        };
        s.version = self.version;
        Some(s)
    }

    fn play_scene(&self, env: &Env, now_ms: u64) -> Option<Scene> {
        let (_, p) = self.active.as_ref()?;
        match self.prefs.overlay {
            Overlay::Off => return None,
            Overlay::Auto => {
                let start = self.play_started?;
                if now_ms >= start + AUTO_HIDE_MS {
                    return None;
                }
            }
            Overlay::Always => {}
        }
        let v = &env.view;
        let lang = self.prefs.lang;
        let mut s = Scene::new(0, v.scale);
        for (i, c) in p.controls.iter().enumerate() {
            let ctr = v.to_px(pt_of(c));
            match c {
                Control::Joystick {
                    radius,
                    up,
                    down,
                    left,
                    right,
                    ..
                } => {
                    let r = radius * v.short_px();
                    s.statics.push(Prim::Ring {
                        c: ctr,
                        r,
                        width: v.lp(2.0),
                        tone: Tone::Ghost,
                    });
                    for (k, dx, dy) in [
                        (up, 0.0, -1.0),
                        (down, 0.0, 1.0),
                        (left, -1.0, 0.0),
                        (right, 1.0, 0.0),
                    ] {
                        let cap = PxRect {
                            x: ctr.x + dx * r.max(v.lp(30.0)) - v.lp(18.0),
                            y: ctr.y + dy * r.max(v.lp(30.0)) - v.lp(14.0),
                            w: v.lp(36.0),
                            h: v.lp(28.0),
                        };
                        cap_prims(&mut s.statics, cap, k, lang, v.scale);
                    }
                    s.dynamics.push(Prim::PressFill {
                        control: i as u8,
                        shape: Shape::Disc {
                            c: ctr,
                            r: (0.3 * r).max(v.lp(8.0)),
                        },
                        tone: Tone::Pressed,
                    });
                }
                Control::Look { reach, .. } => {
                    let h = reach * v.short_px();
                    s.statics.push(Prim::Frame {
                        r: PxRect {
                            x: ctr.x - h,
                            y: ctr.y - h,
                            w: 2.0 * h,
                            h: 2.0 * h,
                        },
                        radius: 0.0,
                        width: v.lp(2.0),
                        dashed: true,
                        tone: Tone::Ghost,
                    });
                    let lw = v.lp(110.0).min(2.0 * h);
                    s.statics.push(Prim::text(
                        PxRect {
                            x: ctr.x - lw / 2.0,
                            y: ctr.y - v.lp(9.0),
                            w: lw,
                            h: v.lp(18.0),
                        },
                        Msg::ControlName {
                            kind: super::doc::Kind::Look,
                            n: 0,
                        }
                        .text(lang),
                        Font::Small,
                        Align::Center,
                        Tone::MarkerDim,
                    ));
                }
                Control::Button { key, .. } => {
                    s.statics.push(Prim::Disc {
                        c: ctr,
                        r: v.lp(22.0),
                        tone: Tone::Ghost,
                    });
                    let cap = PxRect {
                        x: ctr.x - v.lp(18.0),
                        y: ctr.y - v.lp(14.0),
                        w: v.lp(36.0),
                        h: v.lp(28.0),
                    };
                    cap_prims(&mut s.statics, cap, key, lang, v.scale);
                    s.dynamics.push(Prim::PressFill {
                        control: i as u8,
                        shape: Shape::Ring {
                            c: ctr,
                            r: v.lp(26.0),
                            width: v.lp(5.0),
                        },
                        tone: Tone::Pressed,
                    });
                }
            }
        }
        Some(s)
    }

    fn hub_scene(&self, env: &Env, lang: Lang) -> Scene {
        let v = &env.view;
        let mut s = Scene::new(0, v.scale);
        let built = self.hub_build(env);
        s.statics.push(Prim::Rect {
            r: PxRect {
                x: 0.0,
                y: 0.0,
                w: v.window.0,
                h: v.window.1,
            },
            radius: 0.0,
            tone: Tone::Scrim,
        });
        s.statics.push(Prim::Rect {
            r: built.card,
            radius: v.lp(12.0),
            tone: Tone::Card,
        });
        s.statics.push(Prim::Frame {
            r: built.card,
            radius: v.lp(12.0),
            width: v.lp(1.0),
            dashed: false,
            tone: Tone::CardLine,
        });
        let ws: Vec<Widget> = built.ws.iter().map(|(w, _)| w.clone()).collect();
        s.statics.extend(widgets_to_prims(&ws, lang, v.scale));
        s
    }

    /// The widgets of the current Hub screen with their actions.
    pub fn hub_widgets(&self, env: &Env) -> Vec<Widget> {
        self.hub_build(env).ws.into_iter().map(|(w, _)| w).collect()
    }
}

fn cap_prims(out: &mut Vec<Prim>, cap: PxRect, key: &str, lang: Lang, scale: f64) {
    out.push(Prim::Rect {
        r: cap,
        radius: 5.0 * scale,
        tone: Tone::Card,
    });
    out.push(Prim::Frame {
        r: cap,
        radius: 5.0 * scale,
        width: scale,
        dashed: false,
        tone: Tone::MarkerDim,
    });
    out.push(Prim::text(
        cap,
        super::editor::cap_text(key, lang),
        Font::Cap,
        Align::Center,
        Tone::Text,
    ));
}

fn profile_binds(p: &Profile, name: &str) -> bool {
    if p.toggle_key == name || p.panic_key == name {
        return true;
    }
    p.controls.iter().any(|c| match c {
        Control::Button { key, .. } => key == name,
        Control::Joystick {
            up,
            down,
            left,
            right,
            ..
        } => [up, down, left, right].iter().any(|k| *k == name),
        Control::Look { .. } => false,
    })
}

fn store_error_msg(e: StoreError) -> Msg {
    match e {
        StoreError::Invalid(p) => Msg::Problem(p),
        StoreError::NotFound | StoreError::NameTaken | StoreError::Fs(_) => Msg::SaveFailed,
    }
}

// ------------------------------------------------------------ Hub widgets ----

fn w_label(r: PxRect, m: Msg) -> Widget {
    Widget {
        id: WidgetId(0),
        rect: r,
        kind: WidgetKind::Label(m),
        enabled: true,
    }
}

fn w_btn(r: PxRect, label: Msg, primary: bool, enabled: bool) -> Widget {
    Widget {
        id: WidgetId(0),
        rect: r,
        kind: WidgetKind::Button { label, primary },
        enabled,
    }
}

fn longest(m: &Msg) -> String {
    let (v, e) = (m.text(Lang::Vi), m.text(Lang::En));
    if e.chars().count() > v.chars().count() {
        e
    } else {
        v
    }
}

impl<F: Fs> Shell<F> {
    fn banner(col: &mut Column, b: &mut HubBuilt, msg: Msg, tone: Tone) {
        let inner = col.w - 16.0 * col.scale;
        let lines = line_count(&longest(&msg), inner, Font::Small, col.scale);
        let r = col.row(8.0 + lines as f64 * Font::Small.line_lp());
        b.push(
            Widget {
                id: WidgetId(0),
                rect: r,
                kind: WidgetKind::Banner { msg, tone },
                enabled: true,
            },
            HubAct::None,
        );
    }

    fn note(col: &mut Column, b: &mut HubBuilt, msg: Msg) {
        let lines = line_count(&longest(&msg), col.w, Font::Small, col.scale);
        let r = col.text_rows(lines, Font::Small);
        b.push(
            Widget {
                id: WidgetId(0),
                rect: r,
                kind: WidgetKind::Note(msg),
                enabled: true,
            },
            HubAct::None,
        );
    }

    fn hub_build(&self, env: &Env) -> HubBuilt {
        let v = &env.view;
        let sc = v.scale;
        let cw = v.lp(560.0).min(v.window.0 - v.lp(24.0)).max(v.lp(200.0));
        let ch = v.lp(520.0).min(v.window.1 - v.lp(24.0)).max(v.lp(200.0));
        let card = PxRect {
            x: (v.window.0 - cw) / 2.0,
            y: (v.window.1 - ch) / 2.0,
            w: cw,
            h: ch,
        };
        let mut b = HubBuilt {
            ws: Vec::new(),
            card,
        };
        let mut col = Column::new(card, sc);
        let title = col.row(32.0);
        let close_w = v.lp(84.0);
        let guide_w = v.lp(110.0);
        b.push(
            w_label(
                PxRect {
                    x: title.x,
                    y: title.y,
                    w: title.w - close_w - guide_w - v.lp(12.0),
                    h: title.h,
                },
                Msg::Ui(UiText::HubTitle),
            ),
            HubAct::None,
        );
        // Title uses the heading style.
        if let Some((w, _)) = b.ws.last_mut() {
            w.kind = WidgetKind::Heading(Msg::Ui(UiText::HubTitle));
        }
        let (back_msg, back_act) = match self.screen {
            Screen::List => (Msg::Ui(UiText::CloseBtn), HubAct::Close),
            _ => (Msg::Ui(UiText::BackBtn), HubAct::Back),
        };
        if self.screen == Screen::List {
            b.push(
                w_btn(
                    PxRect {
                        x: title.x + title.w - close_w - guide_w - v.lp(6.0),
                        y: title.y,
                        w: guide_w,
                        h: title.h,
                    },
                    Msg::Ui(UiText::GuideBtn),
                    false,
                    true,
                ),
                HubAct::Guide,
            );
        }
        b.push(
            w_btn(
                PxRect {
                    x: title.x + title.w - close_w,
                    y: title.y,
                    w: close_w,
                    h: title.h,
                },
                back_msg,
                false,
                true,
            ),
            back_act,
        );

        match &self.screen {
            Screen::List => self.build_list(env, &mut col, &mut b),
            Screen::Templates { sel, .. } => self.build_templates(env, *sel, &mut col, &mut b),
            Screen::Notice { ticked, then_play } => {
                self.build_notice(*ticked, *then_play, &mut col, &mut b)
            }
            Screen::Stuck => self.build_stuck(&mut col, &mut b),
            Screen::Problems(f) => self.build_problems(f, &mut col, &mut b),
        }
        b
    }

    fn build_list(&self, env: &Env, col: &mut Column, b: &mut HubBuilt) {
        let v = &env.view;
        let sc = v.scale;
        let card = b.card;
        // Footer, pinned.
        let toggle = self.toggle_key();
        let panic = self
            .active
            .as_ref()
            .map(|(_, p)| p.panic_key.clone())
            .unwrap_or_else(|| "F8".into());
        let footer_note = Msg::Ui(UiText::KeysFooter { toggle, panic });
        let note_lines = line_count(&longest(&footer_note), col.w, Font::Small, sc);
        let foot_h = ((ROW_H + 8.0) * 2.0 + note_lines as f64 * Font::Small.line_lp() + 4.0) * sc;
        let foot_top = card.y + card.h - 12.0 * sc - foot_h;
        let mut fc = Column {
            x: col.x,
            w: col.w,
            y: foot_top,
            scale: sc,
        };
        let r = fc.row(ROW_H);
        let halves = split(r, &[1.0, 1.0], sc);
        b.push(
            w_btn(halves[0], Msg::Ui(UiText::NewProfile), false, true),
            HubAct::New,
        );
        b.push(
            w_btn(halves[1], Msg::Ui(UiText::ShareMenu), false, true),
            HubAct::Share,
        );
        Self::note(&mut fc, b, footer_note);
        let r = fc.row(ROW_H);
        let halves = split(r, &[1.0, 1.0], sc);
        b.push(
            w_btn(halves[0], Msg::Ui(UiText::StuckLink), false, true),
            HubAct::Stuck,
        );
        b.push(
            w_btn(halves[1], Msg::Ui(UiText::RulesLink), false, true),
            HubAct::Rules,
        );

        let limit = foot_top - 8.0 * sc;
        let fits = |col: &Column, h_lp: f64| col.y + h_lp * sc <= limit;

        // Banners: draft recovery, sensitivity change, undo toast.
        if let Some(f) = &self.draft_offer {
            let name = self.entry_name(f);
            Self::banner(col, b, Msg::DraftRecovery { name }, Tone::Warn);
            let r = col.row(ROW_H);
            let halves = split(r, &[1.0, 1.0], sc);
            b.push(
                w_btn(halves[0], Msg::Ui(UiText::Restore), true, true),
                HubAct::Restore,
            );
            b.push(
                w_btn(halves[1], Msg::Ui(UiText::Discard), false, true),
                HubAct::DiscardDraft,
            );
        }
        if let (Some((file, from, to)), Some((af, _))) =
            (&self.prefs.last_sensitivity_change, &self.active)
            && file == af
        {
            Self::banner(
                col,
                b,
                Msg::SensitivityChanged {
                    from: *from,
                    to: *to,
                },
                Tone::Accent,
            );
            let r = col.row(ROW_H);
            b.push(
                w_btn(
                    split(r, &[1.0, 1.0], sc)[0],
                    Msg::SensitivityRevert,
                    false,
                    true,
                ),
                HubAct::SensRevert,
            );
        }
        if let Some((_, p)) = &self.active
            && (profile_binds(p, &self.prefs.sens_down_key)
                || profile_binds(p, &self.prefs.sens_up_key))
        {
            Self::note(col, b, Msg::HelperKeysShadowed);
        }
        if let Some(t) = &self.undo_toast {
            Self::banner(
                col,
                b,
                Msg::Deleted {
                    name: t.name.clone(),
                },
                Tone::Accent,
            );
            let r = col.row(ROW_H);
            b.push(
                w_btn(split(r, &[1.0, 1.0], sc)[0], Msg::Undo, false, true),
                HubAct::UndoDelete,
            );
        }

        if self.entries.is_empty() {
            if fits(col, 80.0) {
                let r = col.row(28.0);
                b.push(w_label(r, Msg::EmptyHub), HubAct::None);
                let r = col.row(ROW_H + 8.0);
                b.push(
                    w_btn(
                        PxRect {
                            x: r.x,
                            y: r.y,
                            w: r.w.min(v.lp(260.0)),
                            h: r.h,
                        },
                        Msg::CreateFirst,
                        true,
                        true,
                    ),
                    HubAct::New,
                );
            }
            return;
        }

        let active = self.active_file().map(str::to_string);
        for (i, e) in self.entries.iter().enumerate().skip(self.scroll) {
            let is_active = active.as_deref() == Some(e.file.as_str());
            let refusal = if is_active {
                match self.can_play(env) {
                    Err(PlayRefusal::NoticeNotAccepted) | Ok(()) => None,
                    Err(r) => Some(r),
                }
            } else {
                None
            };
            let mut need = 44.0 + 8.0 + 36.0 + 8.0;
            if let Some(r) = &refusal {
                need += 8.0
                    + 14.0
                        * line_count(
                            &longest(&Msg::Refusal(r.clone())),
                            col.w - 16.0 * sc,
                            Font::Small,
                            sc,
                        ) as f64
                    + 8.0;
            }
            if !fits(col, need) {
                break;
            }
            let (title, sub, broken) = match &e.health {
                Health::Ok => {
                    let o = match e.orientation {
                        Some(Orientation::Landscape) => UiText::Landscape,
                        _ => UiText::Portrait,
                    };
                    let mut sub = format!(
                        "{} · {}",
                        Msg::Ui(o).text(self.prefs.lang),
                        Msg::Ui(UiText::ControlsCount(e.controls)).text(self.prefs.lang)
                    );
                    if is_active {
                        sub.push_str(" · ");
                        sub.push_str(&Msg::Ui(UiText::InUse).text(self.prefs.lang));
                    }
                    (e.name.clone(), sub, false)
                }
                Health::Broken(pm) => (
                    e.file.clone(),
                    Msg::Problem(pm.clone()).text(self.prefs.lang),
                    true,
                ),
            };
            let r = col.row(44.0);
            b.push(
                Widget {
                    id: WidgetId(0),
                    rect: r,
                    kind: WidgetKind::Row {
                        title,
                        sub,
                        selected: i == self.sel,
                        broken,
                    },
                    enabled: true,
                },
                HubAct::Row(i),
            );
            if let Some(rf) = &refusal {
                Self::banner(col, b, Msg::Refusal(rf.clone()), Tone::Warn);
            }
            let r = col.row(36.0);
            if broken {
                let parts = split(r, &[3.0, 1.0], sc);
                b.push(
                    w_btn(parts[0], Msg::Fix(Fix::ShowHowToFix), false, true),
                    HubAct::Fix(Fix::ShowHowToFix),
                );
                b.push(
                    w_btn(parts[1], Msg::Verbatim("⋯".into()), false, true),
                    HubAct::More(i),
                );
            } else {
                let parts = split(r, &[2.0, 1.0, 1.0], sc);
                let fix = refusal.as_ref().and_then(PlayRefusal::fix);
                match (is_active, fix, &refusal) {
                    (true, Some(f), _) => {
                        b.push(w_btn(parts[0], Msg::Fix(f), true, true), HubAct::Fix(f));
                    }
                    (true, None, Some(_)) => {
                        b.push(
                            w_btn(parts[0], Msg::Ui(UiText::PlayBig), true, false),
                            HubAct::None,
                        );
                    }
                    (true, None, None) => {
                        b.push(
                            w_btn(parts[0], Msg::Ui(UiText::PlayBig), true, true),
                            HubAct::Play(i),
                        );
                    }
                    (false, _, _) => {
                        b.push(
                            w_btn(parts[0], Msg::Ui(UiText::Choose), false, true),
                            HubAct::Choose(i),
                        );
                    }
                }
                b.push(
                    w_btn(parts[1], Msg::Ui(UiText::EditBtn), false, true),
                    HubAct::Edit(i),
                );
                b.push(
                    w_btn(parts[2], Msg::Verbatim("⋯".into()), false, true),
                    HubAct::More(i),
                );
            }
        }
    }

    fn build_templates(&self, env: &Env, sel: usize, col: &mut Column, b: &mut HubBuilt) {
        let sc = env.view.scale;
        let lang = self.prefs.lang;
        let r = col.row(24.0);
        b.push(w_label(r, Msg::Ui(UiText::PickTemplate)), HubAct::None);
        for (i, t) in Self::templates(env).into_iter().enumerate() {
            let p = template(t);
            let mut title = Msg::TemplateName(t).text(lang);
            if t == Template::ShooterLandscape {
                title = format!("{title} · {}", Msg::Recommended.text(lang));
            }
            let o = match p.orientation {
                Orientation::Landscape => UiText::Landscape,
                Orientation::Portrait => UiText::Portrait,
            };
            let sub = format!(
                "{} · {}",
                Msg::Ui(o).text(lang),
                Msg::Ui(UiText::ControlsCount(p.controls.len())).text(lang)
            );
            let r = col.row(44.0);
            b.push(
                Widget {
                    id: WidgetId(0),
                    rect: r,
                    kind: WidgetKind::Row {
                        title,
                        sub,
                        selected: i == sel,
                        broken: false,
                    },
                    enabled: true,
                },
                HubAct::Tpl(i),
            );
        }
        Self::banner(col, b, Msg::TemplateHint, Tone::Warn);
        let _ = sc;
    }

    fn build_notice(&self, ticked: bool, then_play: bool, col: &mut Column, b: &mut HubBuilt) {
        let sc = col.scale;
        let r = col.row(28.0);
        b.push(w_label(r, Msg::NoticeTitle), HubAct::None);
        for n in 1..=4 {
            Self::note(col, b, Msg::NoticeBullet(n));
        }
        let r = col.row(ROW_H);
        b.push(
            Widget {
                id: WidgetId(0),
                rect: r,
                kind: WidgetKind::Check {
                    label: Msg::NoticeAccept,
                    on: ticked,
                },
                enabled: true,
            },
            HubAct::NoticeTick,
        );
        let r = col.row(ROW_H);
        let parts = split(r, &[1.0, 1.0], sc);
        b.push(
            w_btn(parts[0], Msg::Ui(UiText::ExitBtn), false, true),
            HubAct::NoticeExit,
        );
        b.push(
            w_btn(parts[1], Msg::Ui(UiText::NextBtn), true, ticked),
            HubAct::NoticeNext,
        );
        let _ = then_play;
    }

    fn build_stuck(&self, col: &mut Column, b: &mut HubBuilt) {
        let r = col.row(28.0);
        b.push(w_label(r, Msg::StuckTitle), HubAct::None);
        for n in 1..=3u8 {
            Self::banner(col, b, Msg::StuckStep(n), Tone::Accent);
        }
        let r = col.row(ROW_H);
        b.push(
            w_btn(
                split(r, &[1.0, 1.0], col.scale)[0],
                Msg::Ui(UiText::CloseBtn),
                true,
                true,
            ),
            HubAct::Back,
        );
    }

    fn build_problems(&self, file: &str, col: &mut Column, b: &mut HubBuilt) {
        let r = col.row(24.0);
        b.push(w_label(r, Msg::Ui(UiText::ProblemsTitle)), HubAct::None);
        let r = col.row(24.0);
        b.push(
            w_label(
                r,
                Msg::Verbatim(ellipsize(file, max_chars(col.w, Font::Body, col.scale))),
            ),
            HubAct::None,
        );
        if let Some(Entry {
            health: Health::Broken(pm),
            ..
        }) = self.entries.iter().find(|e| e.file == file)
        {
            Self::banner(col, b, Msg::Problem(pm.clone()), Tone::Warn);
        }
        let r = col.row(ROW_H);
        b.push(
            w_btn(
                split(r, &[1.0, 1.0], col.scale)[0],
                Msg::Ui(UiText::CloseBtn),
                true,
                true,
            ),
            HubAct::Back,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::Rotation;
    use crate::menu::msg::Fix;
    use crate::menu::store::{FsOp, MemFs};
    use crate::menu::templates::{Template, template};
    use crate::menu::testkit::{Lcg, caption, centre, env, env_sized};
    use crate::profile::Profile;

    const DIR: &str = "keymaps";
    const PREFS: &str = "keymap-ui.json";
    const PLAYABLE: &str =
        r#"{"active":"ho-so-1.json","notice_ack":1,"guide_done":true,"overlay":"always"}"#;

    fn prof(name: &str, t: Template) -> Profile {
        let mut p = template(t);
        p.name = name.into();
        p
    }

    fn fs_of(ps: &[(&str, Profile)], prefs: &str) -> MemFs {
        let mut fs = MemFs::new();
        for (f, p) in ps {
            fs.put(&format!("{DIR}/{f}"), &p.to_json_pretty());
        }
        fs.put(PREFS, prefs);
        fs
    }

    struct T {
        sh: Shell<MemFs>,
        env: Env,
        now: u64,
    }

    impl T {
        fn new(fs: MemFs) -> T {
            T {
                sh: Shell::new(fs, DIR, PREFS),
                env: env(),
                now: 1000,
            }
        }
        /// One shooter profile, chosen, notice accepted.
        fn playable() -> T {
            T::new(fs_of(
                &[("ho-so-1.json", prof("Hồ sơ 1", Template::ShooterLandscape))],
                PLAYABLE,
            ))
        }
        fn two() -> T {
            T::new(fs_of(
                &[
                    ("ho-so-1.json", prof("Hồ sơ 1", Template::ShooterLandscape)),
                    ("ho-so-2.json", prof("Hồ sơ 2", Template::BasicLandscape)),
                ],
                PLAYABLE,
            ))
        }
        fn ev(&mut self, e: UiEvent<'_>) -> Vec<Effect> {
            self.now += 10;
            self.sh.event(e, &self.env, self.now)
        }
        fn cmd(&mut self, c: Command) -> Vec<Effect> {
            self.ev(UiEvent::Command(c))
        }
        fn key(&mut self, name: &str) -> Vec<Effect> {
            let mut o = self.ev(UiEvent::Key {
                name,
                pressed: true,
                mods: Mods::default(),
            });
            o.extend(self.ev(UiEvent::Key {
                name,
                pressed: false,
                mods: Mods::default(),
            }));
            o
        }
        fn ctrl(&mut self, name: &str) -> Vec<Effect> {
            self.ev(UiEvent::Key {
                name,
                pressed: true,
                mods: Mods {
                    ctrl: true,
                    ..Mods::default()
                },
            })
        }
        fn widgets(&self) -> Vec<Widget> {
            match self.sh.mode() {
                Mode::Hub => self.sh.hub_widgets(&self.env),
                _ => self
                    .sh
                    .editor()
                    .map(|e| e.widgets(&self.env, Lang::Vi))
                    .unwrap_or_default(),
            }
        }
        fn has(&self, text: &str) -> bool {
            self.widgets()
                .iter()
                .any(|w| w.enabled && caption(w).as_deref() == Some(text))
        }
        fn click(&mut self, text: &str) -> Vec<Effect> {
            let w = self
                .widgets()
                .into_iter()
                .find(|w| w.enabled && caption(w).as_deref() == Some(text))
                .unwrap_or_else(|| panic!("no enabled widget {text:?}"));
            let p = centre(&w);
            self.ev(UiEvent::PointerMove {
                p,
                mods: Mods::default(),
            });
            let mut o = self.ev(UiEvent::PointerDown {
                p,
                button: "MouseLeft",
                mods: Mods::default(),
            });
            o.extend(self.ev(UiEvent::PointerUp {
                p,
                button: "MouseLeft",
                mods: Mods::default(),
            }));
            o
        }
        fn goto_edit(&mut self) {
            self.cmd(Command::OpenHub);
            self.click("Sửa");
            assert_eq!(self.sh.mode(), Mode::Edit);
        }
        fn goto_test(&mut self) {
            self.goto_edit();
            self.click("Thử");
            assert_eq!(self.sh.mode(), Mode::Test);
        }
        fn prefs_json(&self) -> String {
            self.sh.store().fs().get(PREFS).unwrap_or("").to_string()
        }
    }

    fn started(fx: &[Effect]) -> bool {
        fx.contains(&Effect::StartPlay)
    }
    fn toast_has(fx: &[Effect], m: Msg) -> bool {
        fx.iter()
            .any(|e| matches!(e, Effect::Toast { msg, .. } if *msg == m))
    }

    // ---- test 72: the transition table ----

    #[test]
    fn off_opens_the_hub_and_the_toggle_key_requests_play() {
        let mut t = T::playable();
        assert_eq!(t.sh.mode(), Mode::Off);
        t.cmd(Command::OpenHub);
        assert_eq!(t.sh.mode(), Mode::Hub);
        t.cmd(Command::ToggleHub);
        assert_eq!(t.sh.mode(), Mode::Off);
        t.cmd(Command::ToggleHub);
        assert_eq!(t.sh.mode(), Mode::Hub);
        t.key("Escape");
        assert_eq!(t.sh.mode(), Mode::Off);
        let fx = t.key("Backquote");
        assert!(started(&fx));
        assert_eq!(t.sh.mode(), Mode::Off, "waits for PlayStarted");
        // refused: a toast, still Off
        t.env.link = Link::None;
        let fx = t.key("Backquote");
        assert!(!started(&fx) && toast_has(&fx, Msg::Refusal(PlayRefusal::NotConnected)));
    }

    #[test]
    fn hub_play_edit_and_close() {
        let mut t = T::playable();
        t.cmd(Command::OpenHub);
        let fx = t.click("CHƠI");
        assert!(started(&fx));
        assert_eq!(t.sh.mode(), Mode::Hub, "Hub stays until PlayStarted");
        t.ev(UiEvent::PlayStarted);
        assert_eq!(t.sh.mode(), Mode::Play);
        t.ev(UiEvent::PlayEnded {
            reason: PlayEnd::Toggle,
            look_sensitivity: None,
        });
        assert_eq!(t.sh.mode(), Mode::Off);
        t.cmd(Command::OpenHub);
        t.click("Sửa");
        assert_eq!(t.sh.mode(), Mode::Edit);
        assert!(t.sh.editor().is_some());
        // a broken profile stays in the Hub and shows its problem
    }

    #[test]
    fn edit_done_returns_to_the_hub_saved_and_drops_the_draft() {
        let mut t = T::playable();
        t.goto_edit();
        let c = t.sh.editor().unwrap().profile().controls.len();
        t.key("Tab");
        t.key("Delete");
        assert_eq!(t.sh.editor().unwrap().profile().controls.len(), c - 1);
        t.ev(UiEvent::Tick);
        t.now += 1500;
        t.ev(UiEvent::Tick);
        assert!(
            t.sh.store()
                .fs()
                .exists(&format!("{DIR}/.draft/ho-so-1.json")),
            "autosaved draft"
        );
        t.click("Xong");
        assert_eq!(t.sh.mode(), Mode::Hub);
        assert!(t.sh.editor().is_none());
        assert!(
            !t.sh
                .store()
                .fs()
                .exists(&format!("{DIR}/.draft/ho-so-1.json"))
        );
        let saved = t.sh.store().load("ho-so-1.json").unwrap();
        assert_eq!(saved.controls.len(), c - 1);
        assert!(t.sh.store().fs().exists(&format!("{DIR}/ho-so-1.json.bak")));
    }

    #[test]
    fn escape_in_edit_asks_when_dirty_and_discards_on_request() {
        let mut t = T::playable();
        t.goto_edit();
        t.key("Escape");
        assert_eq!(t.sh.mode(), Mode::Hub, "clean: straight back");
        t.click("Sửa");
        t.key("Tab");
        t.key("Delete");
        t.key("Escape");
        assert_eq!(t.sh.mode(), Mode::Edit);
        assert!(t.has("Không lưu"));
        t.click("Không lưu");
        assert_eq!(t.sh.mode(), Mode::Hub);
        let saved = t.sh.store().load("ho-so-1.json").unwrap();
        assert_eq!(saved.controls.len(), 10, "not saved");
        assert!(
            !t.sh
                .store()
                .fs()
                .exists(&format!("{DIR}/.draft/ho-so-1.json"))
        );
        // "Lưu" saves
        t.click("Sửa");
        t.key("Tab");
        t.key("Delete");
        t.key("Escape");
        t.click("Lưu");
        assert_eq!(t.sh.mode(), Mode::Hub);
        assert_eq!(t.sh.store().load("ho-so-1.json").unwrap().controls.len(), 9);
    }

    #[test]
    fn a_failed_save_keeps_the_editor_open() {
        let mut t = T::playable();
        t.goto_edit();
        t.key("Tab");
        t.key("Delete");
        t.sh.store_mut().fs_mut().fail_next(FsOp::WriteAtomic);
        let fx = t.click("Xong");
        assert!(toast_has(&fx, Msg::SaveFailed));
        assert_eq!(t.sh.mode(), Mode::Edit);
        assert!(t.sh.editor().unwrap().dirty());
        assert_eq!(
            t.sh.store().load("ho-so-1.json").unwrap().controls.len(),
            10,
            "old file intact"
        );
        t.click("Xong");
        assert_eq!(t.sh.mode(), Mode::Hub);
    }

    #[test]
    fn test_mode_round_trip() {
        for exit in ["Backquote", "Escape", "F8", "F12"] {
            let mut t = T::playable();
            t.goto_test();
            t.key(exit);
            assert_eq!(t.sh.mode(), Mode::Edit, "{exit}");
            assert!(t.sh.dots().dots.iter().all(Option::is_none));
        }
        let mut t = T::playable();
        t.goto_test();
        t.ev(UiEvent::FocusLost);
        assert_eq!(t.sh.mode(), Mode::Edit);
        t.click("Thử");
        t.click("Quay lại sửa");
        assert_eq!(t.sh.mode(), Mode::Edit);
    }

    #[test]
    fn play_started_and_ended_from_any_mode() {
        for start in ["off", "hub", "edit"] {
            let mut t = T::playable();
            match start {
                "hub" => {
                    t.cmd(Command::OpenHub);
                }
                "edit" => t.goto_edit(),
                _ => {}
            }
            let fx = t.ev(UiEvent::PlayStarted);
            assert_eq!(t.sh.mode(), Mode::Play, "{start}");
            assert!(fx.iter().any(|e| matches!(
                e,
                Effect::Toast {
                    msg: Msg::PlayOn { .. },
                    ..
                }
            )));
            let fx = t.ev(UiEvent::PlayEnded {
                reason: PlayEnd::Panic,
                look_sensitivity: None,
            });
            assert_eq!(t.sh.mode(), Mode::Off);
            assert!(toast_has(&fx, Msg::PanicDone));
        }
    }

    #[test]
    fn env_changes_never_lose_the_draft_or_the_mode() {
        let mut t = T::playable();
        t.goto_edit();
        t.key("Tab");
        t.key("Delete");
        t.env = env_sized(400.0, 860.0, 1.0, Rotation::R0);
        t.ev(UiEvent::EnvChanged);
        assert_eq!(t.sh.mode(), Mode::Edit);
        assert!(t.sh.editor().unwrap().dirty());
        assert_eq!(t.sh.editor().unwrap().profile().controls.len(), 9);
        let mut h = T::playable();
        h.cmd(Command::OpenHub);
        h.env.rotation = Rotation::R0;
        h.ev(UiEvent::EnvChanged);
        assert_eq!(h.sh.mode(), Mode::Hub);
    }

    #[test]
    fn panic_command_in_off_and_hub_only_toasts() {
        let mut t = T::playable();
        let fx = t.cmd(Command::Panic);
        assert!(toast_has(&fx, Msg::PanicDone) && t.sh.mode() == Mode::Off);
        t.cmd(Command::OpenHub);
        let fx = t.cmd(Command::Panic);
        assert!(toast_has(&fx, Msg::PanicDone) && t.sh.mode() == Mode::Hub);
    }

    // ---- test 73: refusals ----

    #[test]
    fn every_refusal_comes_from_the_env() {
        let mut t = T::playable();
        assert_eq!(t.sh.can_play(&t.env), Ok(()));
        let mut e = t.env;
        e.link = Link::None;
        assert_eq!(t.sh.can_play(&e), Err(PlayRefusal::NotConnected));
        e.link = Link::Wifi;
        assert_eq!(t.sh.can_play(&e), Err(PlayRefusal::Wifi));
        let mut e = t.env;
        e.display_mode = true;
        assert_eq!(t.sh.can_play(&e), Err(PlayRefusal::DisplayMode));
        let mut e = t.env;
        e.scroll_busy = true;
        assert_eq!(t.sh.can_play(&e), Err(PlayRefusal::ScrollBusy));
        let mut e = t.env;
        e.rotation = Rotation::R0;
        assert_eq!(
            t.sh.can_play(&e),
            Err(PlayRefusal::Orientation {
                profile: Orientation::Landscape
            })
        );
        // no chosen profile
        let n = T::new(fs_of(
            &[("ho-so-1.json", prof("A", Template::BasicLandscape))],
            r#"{"notice_ack":1}"#,
        ));
        assert_eq!(n.sh.can_play(&n.env), Err(PlayRefusal::NoProfile));
        // notice
        let n = T::new(fs_of(
            &[("ho-so-1.json", prof("A", Template::BasicLandscape))],
            r#"{"active":"ho-so-1.json"}"#,
        ));
        assert_eq!(n.sh.can_play(&n.env), Err(PlayRefusal::NoticeNotAccepted));
        // chosen but broken
        let mut fs = fs_of(&[], r#"{"active":"kaput.json","notice_ack":1}"#);
        fs.put(&format!("{DIR}/kaput.json"), "{ not json");
        let b = T::new(fs);
        assert_eq!(b.sh.can_play(&b.env), Err(PlayRefusal::BrokenProfile));
        t.cmd(Command::OpenHub);
    }

    #[test]
    fn hub_replaces_the_play_button_by_the_reason_and_offers_the_fix() {
        let mut t = T::playable();
        t.env.rotation = Rotation::R0;
        t.cmd(Command::OpenHub);
        assert!(!t.has("CHƠI"));
        assert!(t.has("Xoay hình (F12)"));
        let fx = t.click("Xoay hình (F12)");
        assert!(fx.contains(&Effect::Rotate));
        let mut u = T::playable();
        u.env.link = Link::Wifi;
        u.cmd(Command::OpenHub);
        assert!(!u.has("CHƠI") && !u.has("Xoay hình (F12)"));
        let s = u.sh.scene(&u.env, u.now).unwrap();
        assert!(
            s.prims()
                .any(|p| matches!(p, Prim::Text { text, .. } if text.contains("Wi-Fi")))
        );
        assert_eq!(PlayRefusal::Wifi.fix(), None);
        let _ = Fix::RotateView;
    }

    // ---- test 74: the notice ----

    #[test]
    fn notice_gates_the_first_play_only() {
        let mut t = T::new(fs_of(
            &[("ho-so-1.json", prof("Hồ sơ 1", Template::ShooterLandscape))],
            r#"{"active":"ho-so-1.json","guide_done":true}"#,
        ));
        t.cmd(Command::OpenHub);
        let fx = t.click("CHƠI");
        assert!(!started(&fx), "the notice comes first");
        assert!(t.has("Tôi đã đọc và tự chịu trách nhiệm"));
        assert!(
            !t.widgets()
                .iter()
                .any(|w| caption(w).as_deref() == Some("Tiếp") && w.enabled)
        );
        t.click("Tôi đã đọc và tự chịu trách nhiệm");
        let fx = t.click("Tiếp");
        assert!(started(&fx), "accepting continues into Play");
        assert!(
            t.prefs_json().contains("\"notice_ack\":1")
                || t.prefs_json().contains("\"notice_ack\": 1")
        );
        t.click("CHƠI");
        let fx = t.click("CHƠI");
        assert!(started(&fx), "no second notice");
        // a lower acknowledged version shows it again
        let mut t2 = T::new(fs_of(
            &[("ho-so-1.json", prof("Hồ sơ 1", Template::ShooterLandscape))],
            r#"{"active":"ho-so-1.json","notice_ack":0}"#,
        ));
        t2.cmd(Command::OpenHub);
        assert!(!started(&t2.click("CHƠI")));
        assert!(t2.has("Thoát"));
        t2.click("Thoát");
        assert!(t2.has("Hướng dẫn"), "back on the list");
        // Esc also leaves without accepting
        t2.click("Lưu ý về luật chơi");
        t2.key("Escape");
        assert_eq!(t2.sh.prefs().notice_ack, 0);
    }

    #[test]
    fn the_notice_can_be_reread_and_keeps_its_wording() {
        let mut t = T::playable();
        t.cmd(Command::ShowNotice);
        assert_eq!(t.sh.mode(), Mode::Hub);
        let s = t.sh.scene(&t.env, t.now).unwrap();
        let all: String = s
            .prims()
            .filter_map(|p| match p {
                Prim::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(" ");
        assert!(all.contains("chưa được kiểm chứng") || all.contains("chưa kiểm chứng"));
        assert!(all.contains("không macro") || all.contains("Không macro"));
    }

    // ---- test 75-77 ----

    #[test]
    fn toggle_key_in_edit_goes_to_test_only_without_capture_or_focus() {
        let mut t = T::playable();
        t.goto_edit();
        t.key("Tab");
        t.key("Tab");
        t.key("Tab"); // a button
        t.key("Enter"); // capture
        assert!(t.sh.editor().unwrap().capturing());
        t.key("Backquote");
        assert_eq!(
            t.sh.mode(),
            Mode::Edit,
            "bound instead (or a conflict prompt)"
        );
        // cancel whatever is left, then focus the name field
        t.ev(UiEvent::FocusLost);
        assert!(!t.sh.editor().unwrap().capturing());
        t.key("Escape"); // deselect
        let w = t
            .widgets()
            .into_iter()
            .find(|w| matches!(w.kind, WidgetKind::Field { .. }))
            .unwrap();
        let p = centre(&w);
        t.ev(UiEvent::PointerDown {
            p,
            button: "MouseLeft",
            mods: Mods::default(),
        });
        assert!(t.sh.editor().unwrap().field_focused());
        t.key("Backquote");
        assert_eq!(t.sh.mode(), Mode::Edit);
        t.key("Escape");
        assert!(!t.sh.editor().unwrap().field_focused());
        t.key("Backquote");
        assert_eq!(t.sh.mode(), Mode::Test);
    }

    #[test]
    fn f8_in_every_mode() {
        let mut t = T::playable();
        let fx = t.key("F8");
        assert!(toast_has(&fx, Msg::PanicDone));
        t.cmd(Command::OpenHub);
        let fx = t.key("F8");
        assert!(toast_has(&fx, Msg::PanicDone) && t.sh.mode() == Mode::Hub);
        t.click("Sửa");
        // capture: F8 cancels it and stays in Edit
        t.key("Tab");
        t.key("Enter");
        assert!(t.sh.editor().unwrap().capturing());
        t.key("F8");
        assert!(!t.sh.editor().unwrap().capturing() && t.sh.mode() == Mode::Edit);
        t.click("Thử");
        t.key("F8");
        assert_eq!(t.sh.mode(), Mode::Edit);
        t.key("Escape");
        t.key("Escape");
        t.ev(UiEvent::PlayStarted);
        t.key("F8");
        assert_eq!(
            t.sh.mode(),
            Mode::Play,
            "Play ends through PlayEnded from the app"
        );
        assert!(!t.sh.passes_to_upstream("F8") && !t.sh.passes_to_upstream("F9"));
    }

    #[test]
    fn focus_loss_in_every_mode() {
        let mut t = T::playable();
        t.ev(UiEvent::FocusLost);
        assert_eq!(t.sh.mode(), Mode::Off);
        t.cmd(Command::OpenHub);
        t.ev(UiEvent::FocusLost);
        assert_eq!(t.sh.mode(), Mode::Hub);
        t.click("Sửa");
        t.key("Tab");
        t.key("Enter");
        t.ev(UiEvent::FocusLost);
        assert_eq!(t.sh.mode(), Mode::Edit);
        assert!(!t.sh.editor().unwrap().capturing());
        t.click("Thử");
        t.ev(UiEvent::FocusLost);
        assert_eq!(t.sh.mode(), Mode::Edit);
        t.key("Escape");
        t.ev(UiEvent::PlayStarted);
        t.ev(UiEvent::FocusLost);
        assert_eq!(t.sh.mode(), Mode::Play, "the app ends Play with PlayEnded");
    }

    // ---- test 78 ----

    fn look_sens(p: &Profile) -> f64 {
        p.controls
            .iter()
            .find_map(|c| match c {
                Control::Look { sensitivity, .. } => Some(*sensitivity),
                _ => None,
            })
            .unwrap()
    }

    #[test]
    fn changed_sensitivity_is_saved_with_a_backup_and_can_be_undone() {
        let mut t = T::playable();
        t.ev(UiEvent::PlayStarted);
        let fx = t.ev(UiEvent::PlayEnded {
            reason: PlayEnd::Toggle,
            look_sensitivity: Some(1.21),
        });
        assert!(toast_has(&fx, Msg::PlayOff));
        assert!((look_sens(&t.sh.store().load("ho-so-1.json").unwrap()) - 1.21).abs() < 1e-9);
        assert!(t.sh.store().fs().exists(&format!("{DIR}/ho-so-1.json.bak")));
        assert_eq!(
            t.sh.prefs().last_sensitivity_change,
            Some(("ho-so-1.json".to_string(), 1.0, 1.21))
        );
        assert!((look_sens(t.sh.active_profile().unwrap()) - 1.21).abs() < 1e-9);
        t.cmd(Command::OpenHub);
        assert!(t.has("Trả về như cũ"));
        let s = t.sh.scene(&t.env, t.now).unwrap();
        assert!(
            s.prims()
                .any(|p| matches!(p, Prim::Text { text, .. } if text.contains("1,00 → 1,21")))
        );
        t.click("Trả về như cũ");
        assert!((look_sens(&t.sh.store().load("ho-so-1.json").unwrap()) - 1.0).abs() < 1e-9);
        assert_eq!(t.sh.prefs().last_sensitivity_change, None);
        assert!(!t.has("Trả về như cũ"));
    }

    #[test]
    fn unchanged_or_missing_sensitivity_saves_nothing() {
        let mut t = T::playable();
        let before =
            t.sh.store()
                .fs()
                .get(&format!("{DIR}/ho-so-1.json"))
                .unwrap()
                .to_string();
        t.ev(UiEvent::PlayStarted);
        t.ev(UiEvent::PlayEnded {
            reason: PlayEnd::Toggle,
            look_sensitivity: Some(1.0),
        });
        t.ev(UiEvent::PlayStarted);
        t.ev(UiEvent::PlayEnded {
            reason: PlayEnd::Disconnected,
            look_sensitivity: None,
        });
        assert_eq!(
            t.sh.store()
                .fs()
                .get(&format!("{DIR}/ho-so-1.json"))
                .unwrap(),
            before
        );
        assert!(!t.sh.store().fs().exists(&format!("{DIR}/ho-so-1.json.bak")));
        assert_eq!(t.sh.prefs().last_sensitivity_change, None);
    }

    // ---- test 79 ----

    #[test]
    fn profile_keys_win_over_the_helper_keys() {
        let t = T::playable();
        assert_eq!(t.sh.play_key("BracketLeft"), PlayKey::SensDown);
        assert_eq!(t.sh.play_key("BracketRight"), PlayKey::SensUp);
        assert_eq!(t.sh.play_key("KeyW"), PlayKey::Engine);
        let mut p = prof("Hồ sơ 1", Template::BasicLandscape);
        if let Control::Button { key, .. } = &mut p.controls[2] {
            *key = "BracketLeft".into();
        }
        let t = T::new(fs_of(&[("ho-so-1.json", p)], PLAYABLE));
        assert_eq!(t.sh.play_key("BracketLeft"), PlayKey::Engine);
        assert_eq!(t.sh.play_key("BracketRight"), PlayKey::SensUp);
        let mut t = t;
        t.cmd(Command::OpenHub);
        let s = t.sh.scene(&t.env, t.now).unwrap();
        assert!(
            s.prims().any(
                |p| matches!(p, Prim::Text { text, .. } if text.contains("không chỉnh độ nhạy"))
            )
        );
        // no profile: helpers still work
        let n = T::new(fs_of(&[], "{}"));
        assert_eq!(n.sh.play_key("BracketRight"), PlayKey::SensUp);
    }

    // ---- test 80 ----

    #[test]
    fn hub_delete_and_undo_within_eight_seconds() {
        let mut t = T::two();
        t.cmd(Command::OpenHub);
        t.key("ArrowDown");
        t.key("Delete");
        assert_eq!(t.sh.entries().len(), 1);
        assert!(t.has("Hoàn tác"));
        assert!(t.sh.next_deadline().is_some());
        t.now += 7500;
        t.ev(UiEvent::Tick);
        assert!(t.has("Hoàn tác"), "still offered at 7.5 s");
        t.click("Hoàn tác");
        assert_eq!(t.sh.entries().len(), 2);
        assert!(!t.has("Hoàn tác"));
        // Ctrl+Z does the same
        t.key("ArrowDown");
        t.key("Delete");
        assert_eq!(t.sh.entries().len(), 1);
        t.ctrl("KeyZ");
        assert_eq!(t.sh.entries().len(), 2);
        // expiry
        t.key("ArrowDown");
        t.key("Delete");
        t.now += 8100;
        t.ev(UiEvent::Tick);
        assert!(!t.has("Hoàn tác"));
        t.ctrl("KeyZ");
        assert_eq!(t.sh.entries().len(), 1, "no undo after the toast expired");
        assert_eq!(t.sh.next_deadline(), None);
    }

    #[test]
    fn deleting_the_chosen_profile_unchooses_it() {
        let mut t = T::playable();
        t.cmd(Command::OpenHub);
        t.key("Delete");
        assert!(t.sh.active_profile().is_none());
        assert!(t.has("Tạo hồ sơ đầu tiên"));
        t.click("Hoàn tác");
        assert_eq!(t.sh.entries().len(), 1);
    }

    // ---- test 81 ----

    #[test]
    fn a_draft_from_a_crash_is_offered_and_can_be_restored_or_dropped() {
        let mut fs = fs_of(
            &[("ho-so-1.json", prof("Hồ sơ 1", Template::ShooterLandscape))],
            PLAYABLE,
        );
        let mut draft = prof("Nháp chưa lưu", Template::ShooterLandscape);
        draft.controls.pop();
        fs.put(
            &format!("{DIR}/.draft/ho-so-1.json"),
            &draft.to_json_pretty(),
        );
        let mut t = T::new(fs.clone());
        t.cmd(Command::OpenHub);
        let s = t.sh.scene(&t.env, t.now).unwrap();
        assert!(
            s.prims()
                .any(|p| matches!(p, Prim::Text { text, .. } if text.contains("chưa lưu")))
        );
        t.click("Khôi phục");
        assert_eq!(t.sh.mode(), Mode::Edit);
        let e = t.sh.editor().unwrap();
        assert!(e.dirty());
        assert_eq!(e.profile().name, "Nháp chưa lưu");
        assert_eq!(e.profile().controls.len(), 9);
        // "Bỏ" removes the draft
        let mut t = T::new(fs);
        t.cmd(Command::OpenHub);
        t.click("Bỏ");
        assert!(
            !t.sh
                .store()
                .fs()
                .exists(&format!("{DIR}/.draft/ho-so-1.json"))
        );
        assert!(!t.has("Khôi phục"));
        assert_eq!(t.sh.mode(), Mode::Hub);
    }

    // ---- test 82-83 ----

    #[test]
    fn swallows_input_per_mode() {
        let mut t = T::playable();
        assert!(!t.sh.swallows_input());
        t.cmd(Command::OpenHub);
        assert!(t.sh.swallows_input());
        t.click("Sửa");
        assert!(t.sh.swallows_input());
        t.click("Thử");
        assert!(t.sh.swallows_input());
        t.key("Escape");
        t.key("Escape");
        t.ev(UiEvent::PlayStarted);
        assert!(!t.sh.swallows_input());
    }

    #[test]
    fn start_play_is_never_requested_from_edit_or_test() {
        // Structural: no Effect carries a frame (the enum is closed over the
        // variants below), and StartPlay only comes out in Off / Hub.
        let _ = |e: Effect| match e {
            Effect::Redraw
            | Effect::Toast { .. }
            | Effect::Cursor(_)
            | Effect::StartPlay
            | Effect::GrabPointer(_)
            | Effect::ShowMenu(_)
            | Effect::CopyText(_)
            | Effect::RequestPaste
            | Effect::OpenFolder
            | Effect::ImeAllowed(_)
            | Effect::Rotate
            | Effect::ReserveRight(_)
            | Effect::Title => (),
        };
        let mut t = T::playable();
        t.goto_edit();
        let mut r = Lcg(5);
        let names = [
            "Backquote",
            "Enter",
            "Escape",
            "Tab",
            "KeyW",
            "F8",
            "F12",
            "Delete",
        ];
        for _ in 0..800 {
            let before = t.sh.mode();
            let fx = if r.below(3) == 0 {
                t.key(r.pick(&names))
            } else {
                let p = Pt {
                    x: r.unit() * 1280.0,
                    y: r.unit() * 590.0,
                };
                t.ev(UiEvent::PointerDown {
                    p,
                    button: "MouseLeft",
                    mods: Mods::default(),
                })
            };
            if matches!(before, Mode::Edit | Mode::Test) {
                assert!(!started(&fx) || matches!(t.sh.mode(), Mode::Hub | Mode::Off));
            }
            if t.sh.mode() == Mode::Hub {
                t.click("Sửa");
            }
        }
    }

    // ---- the rest of the shell ----

    #[test]
    fn scene_is_none_in_off_and_follows_the_overlay_in_play() {
        let mut t = T::playable();
        assert!(t.sh.scene(&t.env, t.now).is_none());
        t.ev(UiEvent::PlayStarted);
        let s = t.sh.scene(&t.env, t.now).expect("Always: shown");
        assert!(!s.statics.is_empty());
        assert!(
            !s.dynamics.is_empty()
                && s.dynamics
                    .iter()
                    .all(|p| matches!(p, Prim::PressFill { .. }))
        );
        assert!(
            s.statics
                .iter()
                .all(|p| !matches!(p, Prim::PressFill { .. }))
        );
        assert!(t.sh.scene(&t.env, t.now + 3_600_000).is_some());
        // Tự ẩn: 5 s
        let mut a = T::new(fs_of(
            &[("ho-so-1.json", prof("Hồ sơ 1", Template::ShooterLandscape))],
            r#"{"active":"ho-so-1.json","notice_ack":1,"overlay":"auto"}"#,
        ));
        a.ev(UiEvent::PlayStarted);
        let start = a.now;
        assert!(a.sh.scene(&a.env, start + 4999).is_some());
        assert!(a.sh.scene(&a.env, start + 5000).is_none());
        assert_eq!(a.sh.next_deadline(), Some(start + 5000));
        let v = a.sh.scene_version();
        a.now = start + 5000;
        a.ev(UiEvent::Tick);
        assert!(
            a.sh.scene_version() > v,
            "the redraw that removes the overlay"
        );
        assert_eq!(a.sh.next_deadline(), None);
        // Tắt
        let mut o = T::new(fs_of(
            &[("ho-so-1.json", prof("Hồ sơ 1", Template::ShooterLandscape))],
            r#"{"active":"ho-so-1.json","notice_ack":1,"overlay":"off"}"#,
        ));
        o.ev(UiEvent::PlayStarted);
        assert!(o.sh.scene(&o.env, o.now).is_none());
        // a Play scene has a PressFill for every control and no text in dynamics
        let s = t.sh.scene(&t.env, t.now).unwrap();
        let fills = s
            .dynamics
            .iter()
            .filter(|p| matches!(p, Prim::PressFill { .. }))
            .count();
        assert_eq!(fills, 9, "look has none (no finger of its own to light)");
    }

    #[test]
    fn scene_version_moves_only_when_something_can_differ() {
        let mut t = T::playable();
        let v = t.sh.scene_version();
        t.ev(UiEvent::Tick);
        t.ev(UiEvent::PointerMove {
            p: Pt { x: 5.0, y: 5.0 },
            mods: Mods::default(),
        });
        t.key("KeyW");
        assert_eq!(t.sh.scene_version(), v, "Off: nothing to redraw");
        let fx = t.cmd(Command::OpenHub);
        assert!(t.sh.scene_version() > v && fx.contains(&Effect::Redraw));
        let v = t.sh.scene_version();
        t.click("Hướng dẫn");
        assert!(t.sh.scene_version() > v);
    }

    #[test]
    fn title_suffix_and_native_menu() {
        let mut t = T::two();
        assert_eq!(t.sh.title_suffix(Lang::Vi), " · Bàn phím: bấm ` để chơi");
        assert_eq!(t.sh.title_suffix(Lang::En), " · Keyboard: press ` to play");
        t.ev(UiEvent::PlayStarted);
        assert_eq!(t.sh.title_suffix(Lang::Vi), " · Bàn phím BẬT");
        t.ev(UiEvent::PlayEnded {
            reason: PlayEnd::Toggle,
            look_sensitivity: None,
        });
        let MenuItem::Sub { label, items } = t.sh.native_menu() else {
            panic!()
        };
        assert_eq!(label, "Bàn phím và chuột");
        let MenuItem::Sub {
            items: profiles, ..
        } = &items[2]
        else {
            panic!()
        };
        assert_eq!(profiles.len(), 2);
        assert!(
            matches!(&profiles[0], MenuItem::Item { checked: true, label, .. } if label == "Hồ sơ 1")
        );
        // choosing the second profile through its id
        let id = match &profiles[1] {
            MenuItem::Item { id, .. } => *id,
            _ => panic!(),
        };
        t.ev(UiEvent::MenuPick(id));
        assert_eq!(t.sh.active_file(), Some("ho-so-2.json"));
        let MenuItem::Item { id: open, .. } = &items[0] else {
            panic!()
        };
        t.ev(UiEvent::MenuPick(*open));
        assert_eq!(t.sh.mode(), Mode::Hub);
        // at most 9 profiles
        let ps: Vec<(String, Profile)> = (1..=12)
            .map(|i| {
                (
                    format!("p{i:02}.json"),
                    prof(&format!("P{i}"), Template::BasicLandscape),
                )
            })
            .collect();
        let refs: Vec<(&str, Profile)> = ps.iter().map(|(f, p)| (f.as_str(), p.clone())).collect();
        let big = T::new(fs_of(&refs, "{}"));
        let MenuItem::Sub { items, .. } = big.sh.native_menu() else {
            panic!()
        };
        let MenuItem::Sub {
            items: profiles, ..
        } = &items[2]
        else {
            panic!()
        };
        assert_eq!(profiles.len(), 9);
    }

    #[test]
    fn english_is_complete_and_selectable() {
        let t = T::new(fs_of(
            &[("ho-so-1.json", prof("A", Template::ShooterLandscape))],
            r#"{"active":"ho-so-1.json","notice_ack":1,"lang":"en"}"#,
        ));
        assert_eq!(t.sh.lang(), Lang::En);
        let mut t = t;
        t.cmd(Command::OpenHub);
        let s = t.sh.scene(&t.env, t.now).unwrap();
        assert!(
            s.prims()
                .any(|p| matches!(p, Prim::Text { text, .. } if text == "PLAY"))
        );
        assert!(
            !s.prims()
                .any(|p| matches!(p, Prim::Text { text, .. } if text.contains("CHƠI")))
        );
    }

    #[test]
    fn creating_a_profile_from_a_template_starts_the_guide() {
        let mut t = T::new(fs_of(&[], r#"{"notice_ack":1}"#));
        t.cmd(Command::OpenHub);
        assert!(t.has("Tạo hồ sơ đầu tiên"));
        t.click("Tạo hồ sơ đầu tiên");
        assert!(
            t.has("Bắn súng – màn hình ngang · đề nghị"),
            "preselected for a landscape picture"
        );
        t.click("Bắn súng – màn hình ngang · đề nghị");
        assert_eq!(t.sh.mode(), Mode::Edit);
        assert_eq!(t.sh.entries()[0].name, "Hồ sơ 1");
        assert_eq!(t.sh.active_file(), Some("ho-so-1.json"));
        assert!(t.sh.editor().unwrap().guide_step().is_some());
        // skipping the guide remembers it
        t.click("Bỏ qua hướng dẫn");
        assert!(
            t.prefs_json().contains("\"guide_done\":true")
                || t.prefs_json().contains("\"guide_done\": true")
        );
        // the Trống template and the portrait preselection
        let mut p = T::new(fs_of(&[], r#"{"notice_ack":1,"guide_done":true}"#));
        p.env = env_sized(400.0, 860.0, 1.0, Rotation::R0);
        p.cmd(Command::OpenHub);
        p.click("Tạo hồ sơ đầu tiên");
        p.click("Trống");
        assert_eq!(p.sh.mode(), Mode::Edit);
        assert!(p.sh.editor().unwrap().profile().controls.is_empty());
        assert_eq!(
            p.sh.editor().unwrap().profile().orientation,
            Orientation::Portrait
        );
        assert!(p.sh.editor().unwrap().guide_step().is_none());
    }

    #[test]
    fn dropping_or_pasting_a_profile_imports_valid_ones_only() {
        let mut t = T::playable();
        t.cmd(Command::OpenHub);
        let text = prof("Từ bạn", Template::BasicPortrait).to_json_pretty();
        let fx = t.ev(UiEvent::Dropped {
            name: "tu-ban.json",
            text: Some(&text),
        });
        assert!(fx.iter().any(|e| matches!(
            e,
            Effect::Toast {
                msg: Msg::Ui(UiText::ImportOk),
                ..
            }
        )));
        assert_eq!(t.sh.entries().len(), 2);
        let n = t.sh.store().fs().paths().len();
        let fx = t.ev(UiEvent::Pasted(Some("{ not a profile")));
        assert!(fx.iter().any(|e| matches!(
            e,
            Effect::Toast {
                msg: Msg::Problem(_),
                ..
            }
        )));
        assert_eq!(t.sh.store().fs().paths().len(), n, "nothing written");
        t.ev(UiEvent::Pasted(None));
        assert_eq!(t.sh.entries().len(), 2);
        // Ctrl+V asks the app for the clipboard
        let fx = t.ctrl("KeyV");
        assert!(fx.contains(&Effect::RequestPaste));
        // outside the Hub a drop does nothing here (upstream keeps its meaning)
        let mut o = T::playable();
        o.ev(UiEvent::Dropped {
            name: "x.json",
            text: Some(&text),
        });
        assert_eq!(o.sh.entries().len(), 1);
    }

    #[test]
    fn share_menu_copies_the_chosen_profile() {
        let mut t = T::playable();
        t.cmd(Command::OpenHub);
        let fx = t.click("Chia sẻ hồ sơ ▾");
        assert!(
            fx.iter()
                .any(|e| matches!(e, Effect::ShowMenu(m) if m.items.len() == 3))
        );
        let fx = t.ev(UiEvent::MenuPick(1));
        let want = t.sh.store().export_text("ho-so-1.json").unwrap();
        assert!(fx.contains(&Effect::CopyText(want)));
        assert!(toast_has(&fx, Msg::Copied));
        t.click("Chia sẻ hồ sơ ▾");
        assert!(t.ev(UiEvent::MenuPick(3)).contains(&Effect::OpenFolder));
        t.click("Chia sẻ hồ sơ ▾");
        assert!(t.ev(UiEvent::MenuPick(2)).contains(&Effect::RequestPaste));
    }

    #[test]
    fn row_menu_duplicates_and_deletes() {
        let mut t = T::two();
        t.cmd(Command::OpenHub);
        let more = t
            .widgets()
            .into_iter()
            .filter(|w| caption(w).as_deref() == Some("⋯"))
            .nth(1)
            .unwrap();
        let p = centre(&more);
        let fx = t.ev(UiEvent::PointerDown {
            p,
            button: "MouseLeft",
            mods: Mods::default(),
        });
        assert!(fx.iter().any(|e| matches!(e, Effect::ShowMenu(_))));
        t.ev(UiEvent::MenuPick(2));
        assert_eq!(t.sh.entries().len(), 3);
        assert!(t.sh.entries().iter().any(|e| e.name == "Hồ sơ 2 (bản sao)"));
    }

    #[test]
    fn broken_profiles_are_listed_with_their_problem_and_cannot_be_chosen() {
        let mut fs = fs_of(
            &[("ho-so-1.json", prof("Hồ sơ 1", Template::ShooterLandscape))],
            PLAYABLE,
        );
        let mut bad = prof("Lỗi", Template::BasicLandscape);
        if let Control::Button { key, .. } = &mut bad.controls[2] {
            *key = "KeyW".into();
        }
        fs.put(&format!("{DIR}/thu.json"), &bad.to_json_pretty());
        let before = fs.get(&format!("{DIR}/thu.json")).unwrap().to_string();
        let mut t = T::new(fs);
        t.cmd(Command::OpenHub);
        assert!(t.has("Xem cách sửa"));
        t.click("Xem cách sửa");
        let s = t.sh.scene(&t.env, t.now).unwrap();
        assert!(
            s.prims()
                .any(|p| matches!(p, Prim::Text { text, .. } if text.contains("bị dùng 2 lần")))
        );
        t.click("Quay lại");
        t.cmd(Command::ChooseProfile(5));
        assert_eq!(t.sh.active_file(), Some("ho-so-1.json"));
        assert_eq!(
            t.sh.store().fs().get(&format!("{DIR}/thu.json")).unwrap(),
            before
        );
    }

    #[test]
    fn reserve_right_follows_edit_when_the_window_can_grow() {
        let mut t = T::playable();
        t.env.can_grow_window = true;
        t.cmd(Command::OpenHub);
        let fx = t.click("Sửa");
        assert!(fx.contains(&Effect::ReserveRight(crate::menu::layout::DOCK_W)));
        let fx = t.click("Xong");
        assert!(fx.contains(&Effect::ReserveRight(0.0)));
        let mut n = T::playable();
        n.cmd(Command::OpenHub);
        let fx = n.click("Sửa");
        assert!(
            !fx.iter()
                .any(|e| matches!(e, Effect::ReserveRight(w) if *w > 0.0))
        );
    }

    #[test]
    fn f_key_table_for_the_app() {
        let mut t = T::playable();
        assert!(t.sh.passes_to_upstream("F9") && !t.sh.passes_to_upstream("F8"));
        t.cmd(Command::OpenHub);
        for k in ["F1", "F3", "F6", "F10", "F11", "F12"] {
            assert!(t.sh.passes_to_upstream(k), "{k}");
        }
        assert!(!t.sh.passes_to_upstream("F9"));
        t.click("Sửa");
        assert!(t.sh.passes_to_upstream("F1"));
        assert!(!t.sh.passes_to_upstream("F9"));
    }

    #[test]
    fn every_hub_caption_fits_in_both_languages() {
        use crate::menu::layout::widget_texts;
        for lang in ["vi", "en"] {
            let prefs = format!(r#"{{"active":"ho-so-1.json","notice_ack":1,"lang":"{lang}"}}"#);
            let mut fs = fs_of(
                &[
                    ("ho-so-1.json", prof("Hồ sơ 1", Template::ShooterLandscape)),
                    ("ho-so-2.json", prof("Hồ sơ 2", Template::BasicLandscape)),
                ],
                &prefs,
            );
            fs.put(&format!("{DIR}/kaput.json"), "{ not json");
            let mut draft = prof("Hồ sơ 1", Template::ShooterLandscape);
            draft.controls.pop();
            fs.put(
                &format!("{DIR}/.draft/ho-so-1.json"),
                &draft.to_json_pretty(),
            );
            for (rot, link, scenario) in [
                (Rotation::R90, Link::Usb, 0),
                (Rotation::R0, Link::Usb, 1),
                (Rotation::R90, Link::Wifi, 2),
                (Rotation::R90, Link::None, 3),
            ] {
                let mut t = T::new(fs.clone());
                t.env.rotation = rot;
                t.env.link = link;
                t.cmd(Command::OpenHub);
                let mut screens: Vec<Vec<Widget>> = vec![t.widgets()];
                if scenario == 0 {
                    for open in [Command::ShowNotice, Command::ShowStuckHelp] {
                        t.cmd(open);
                        screens.push(t.widgets());
                        t.key("Escape");
                    }
                    t.click_by_caption_prefix("Tạo", "Create");
                    screens.push(t.widgets());
                }
                for ws in screens {
                    for w in ws {
                        if matches!(w.kind, WidgetKind::Row { .. } | WidgetKind::Field { .. }) {
                            continue;
                        }
                        let l = if lang == "en" { Lang::En } else { Lang::Vi };
                        for (txt, wpx, font) in widget_texts(&w, l, 1.0) {
                            assert!(
                                crate::menu::scene::fits(&txt, wpx, font, 1.0),
                                "{lang}: {txt:?} ({} chars) does not fit {wpx:.0}px",
                                txt.chars().count()
                            );
                        }
                    }
                }
            }
        }
    }

    // ---- test 85 ----

    #[test]
    fn fuzz_shell() {
        let names = [
            "Backquote",
            "Enter",
            "Escape",
            "Tab",
            "KeyW",
            "Space",
            "F8",
            "F9",
            "F12",
            "KeyK",
            "ArrowDown",
            "ArrowUp",
            "KeyZ",
            "KeyV",
            "Home",
        ];
        let buttons = ["MouseLeft", "MouseRight", "MouseMiddle"];
        let cmds = [
            Command::OpenHub,
            Command::ToggleHub,
            Command::OpenEdit,
            Command::ShowGuide,
            Command::ShowNotice,
            Command::ShowStuckHelp,
            Command::Panic,
            Command::ChooseProfile(0),
            Command::ChooseProfile(1),
        ];
        let json = prof("Từ bạn", Template::BasicPortrait).to_json_pretty();
        let originals = ["ho-so-1.json", "ho-so-2.json"];
        for seed in 1..=20u64 {
            let mut r = Lcg(seed * 104729);
            let mut t = T::two();
            for i in 0..2000 {
                let p = Pt {
                    x: r.unit() * 1300.0 - 10.0,
                    y: r.unit() * 600.0 - 5.0,
                };
                let w = t.widgets();
                let p = if r.below(3) == 0 && !w.is_empty() {
                    centre(r.pick(&w))
                } else {
                    p
                };
                let mods = Mods {
                    shift: r.below(5) == 0,
                    ctrl: r.below(6) == 0,
                    alt: false,
                };
                // Random failures on the file system.
                if r.below(40) == 0 {
                    let op = *r.pick(&[
                        FsOp::WriteAtomic,
                        FsOp::Rename,
                        FsOp::Remove,
                        FsOp::Read,
                        FsOp::List,
                    ]);
                    t.sh.store_mut().fs_mut().fail_nth(op, r.below(3) as usize);
                }
                let fx = match r.below(16) {
                    0..=2 => t.ev(UiEvent::PointerMove { p, mods }),
                    3 | 4 => t.ev(UiEvent::PointerDown {
                        p,
                        button: r.pick(&buttons),
                        mods,
                    }),
                    5 => t.ev(UiEvent::PointerUp {
                        p,
                        button: r.pick(&buttons),
                        mods,
                    }),
                    6 => t.ev(UiEvent::Wheel {
                        p,
                        dy: r.unit() * 2.0 - 1.0,
                        mods,
                    }),
                    7 | 8 => {
                        let name = *r.pick(&names);
                        // keep the originals: no delete in this fuzz
                        t.ev(UiEvent::Key {
                            name,
                            pressed: true,
                            mods,
                        })
                    }
                    9 => t.ev(UiEvent::Key {
                        name: r.pick(&names),
                        pressed: false,
                        mods,
                    }),
                    10 => t.ev(UiEvent::Tick),
                    11 => t.ev(UiEvent::RawMotion {
                        dx: r.unit() * 50.0,
                        dy: r.unit() * 50.0,
                    }),
                    12 => t.ev(UiEvent::Command(r.pick(&cmds).clone())),
                    13 => match r.below(6) {
                        0 => t.ev(UiEvent::FocusLost),
                        1 => t.ev(UiEvent::EnvChanged),
                        2 => t.ev(UiEvent::MenuPick(r.below(4) as u32)),
                        3 => t.ev(UiEvent::Pasted(Some(&json))),
                        4 => t.ev(UiEvent::Dropped {
                            name: "x.json",
                            text: Some("{"),
                        }),
                        _ => t.ev(UiEvent::Text("ệ")),
                    },
                    14 => match r.below(4) {
                        0 => t.ev(UiEvent::PlayStarted),
                        1 => t.ev(UiEvent::PlayEnded {
                            reason: PlayEnd::Toggle,
                            look_sensitivity: Some(0.5 + r.unit()),
                        }),
                        2 => {
                            t.env.link = if r.below(2) == 0 {
                                Link::Usb
                            } else {
                                Link::None
                            };
                            t.ev(UiEvent::EnvChanged)
                        }
                        _ => {
                            t.env = if r.below(2) == 0 {
                                env()
                            } else {
                                env_sized(400.0, 860.0, 1.0, Rotation::R0)
                            };
                            t.ev(UiEvent::EnvChanged)
                        }
                    },
                    _ => t.click_any(),
                };
                if matches!(t.sh.mode(), Mode::Edit | Mode::Test) {
                    assert!(
                        !started(&fx) || matches!(t.sh.mode(), Mode::Hub | Mode::Off),
                        "seed {seed} event {i}"
                    );
                }
                // Every mode can be drawn, and the scene never panics.
                let _ = t.sh.scene(&t.env, t.now);
                let _ = t.sh.next_deadline();
            }
            // The originals are still valid (a failed write leaves the old file).
            let fs = t.sh.store().fs().clone();
            // Flush any failure that is still armed.
            for _ in 0..6 {
                let _ = fs.list(DIR);
                let _ = fs.read("keymap-ui.json");
            }
            let store = ProfileStore::new(fs, DIR);
            for f in originals {
                assert!(
                    store
                        .list()
                        .iter()
                        .any(|e| e.file == f && e.health == Health::Ok),
                    "seed {seed}: {f} lost or broken"
                );
            }
        }
    }

    impl T {
        fn click_by_caption_prefix(&mut self, vi: &str, en: &str) {
            let w = self.widgets().into_iter().find(|w| {
                caption(w).is_some_and(|c| (c.starts_with(vi) || c.starts_with(en)) && w.enabled)
            });
            if let Some(w) = w {
                let p = centre(&w);
                self.ev(UiEvent::PointerDown {
                    p,
                    button: "MouseLeft",
                    mods: Mods::default(),
                });
            }
        }

        /// Click something harmless if there is anything to click.
        fn click_any(&mut self) -> Vec<Effect> {
            let w = self.widgets();
            let Some(w) = w.into_iter().find(|w| w.enabled && w.kind.interactive()) else {
                return Vec::new();
            };
            let p = centre(&w);
            self.ev(UiEvent::PointerDown {
                p,
                button: "MouseLeft",
                mods: Mods::default(),
            })
        }
    }
}
