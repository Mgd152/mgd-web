//! Golden SVG screens of the keymap menu (UI_SPEC 6.14 test 55).
//!
//! Every screen of section 3 is driven through the public `Shell` API with an
//! in-memory file system and compared byte for byte with
//! `tests/data/menu/golden/<name>.svg`. Run with `KEYMAP_UPDATE_GOLDEN=1` to
//! rewrite the files after an intended change, then look at them.
//!
//! The goldens prove the layout, not the Direct2D rendering.

use keymap::menu::scene::fits;
use keymap::menu::{
    Command, Effect, Env, Lang, Link, MemFs, Mode, Mods, Pt, PxRect, Scene, Shell, Template,
    UiEvent, Widget, WidgetKind, template,
};
use keymap::menu::{EditView, Prim};
use keymap::{Profile, Rotation};
use std::path::PathBuf;

const KEYMAPS: &str = "keymaps";
const PREFS: &str = "keymap-ui.json";

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/menu/golden")
}

fn check(name: &str, svg: &str) {
    let path = golden_dir().join(format!("{name}.svg"));
    if std::env::var("KEYMAP_UPDATE_GOLDEN").is_ok() {
        std::fs::create_dir_all(golden_dir()).unwrap();
        std::fs::write(&path, svg).unwrap();
        return;
    }
    let want = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("missing golden {name}; run with KEYMAP_UPDATE_GOLDEN=1"));
    assert!(
        want == svg,
        "golden {name} differs; run with KEYMAP_UPDATE_GOLDEN=1 and review the diff"
    );
}

/// Sanity check of the display list: no text exceeds its box. (Painters
/// already ellipsize, so the real per-caption budget test is on the widgets,
/// in the unit tests of `editor` and `shell`.)
fn assert_texts_fit(name: &str, sc: &Scene) {
    for p in sc.prims() {
        if let Prim::Text { r, text, font, .. } = p {
            assert!(
                fits(text, r.w, *font, sc.scale),
                "{name}: {text:?} ({} chars) does not fit {:.0}px of {font:?}",
                text.chars().count(),
                r.w
            );
        }
    }
}

struct Screen {
    window: (f64, f64),
    picture: PxRect,
    rotation: Rotation,
    has_picture: bool,
    link: Link,
    grow: bool,
}

impl Screen {
    /// 19.5:9 landscape picture filling its window.
    fn landscape() -> Screen {
        Screen {
            window: (1280.0, 590.0),
            picture: PxRect {
                x: 0.0,
                y: 0.0,
                w: 1280.0,
                h: 590.0,
            },
            rotation: Rotation::R90,
            has_picture: true,
            link: Link::Usb,
            grow: false,
        }
    }
    fn hub() -> Screen {
        Screen {
            window: (1100.0, 620.0),
            picture: PxRect {
                x: 0.0,
                y: 0.0,
                w: 1100.0,
                h: 620.0,
            },
            ..Screen::landscape()
        }
    }
    fn portrait() -> Screen {
        Screen {
            window: (400.0, 860.0),
            picture: PxRect {
                x: 0.0,
                y: 0.0,
                w: 400.0,
                h: 860.0,
            },
            rotation: Rotation::R0,
            ..Screen::landscape()
        }
    }
    fn side() -> Screen {
        Screen {
            window: (1584.0, 590.0),
            grow: true,
            ..Screen::landscape()
        }
    }
    fn env(&self) -> Env {
        Env {
            link: self.link,
            display_mode: false,
            scroll_busy: false,
            rotation: self.rotation,
            panel: (1206.0, 2622.0),
            has_picture: self.has_picture,
            view: EditView {
                window: self.window,
                picture: self.picture,
                scale: 1.0,
            },
            can_grow_window: self.grow,
        }
    }
}

fn profile(name: &str, t: Template) -> Profile {
    let mut p = template(t);
    p.name = name.to_string();
    p
}

fn fs_with(profiles: &[(&str, Profile)], prefs: &str) -> MemFs {
    let mut fs = MemFs::new();
    for (file, p) in profiles {
        fs.put(&format!("{KEYMAPS}/{file}"), &p.to_json_pretty());
    }
    fs.put(PREFS, prefs);
    fs
}

const PREFS_PLAYABLE: &str =
    r#"{"version":1,"active":"ho-so-1.json","notice_ack":1,"guide_done":true,"overlay":"always"}"#;

struct Drv {
    sh: Shell<MemFs>,
    scr: Screen,
    now: u64,
}

impl Drv {
    fn new(fs: MemFs, scr: Screen) -> Drv {
        Drv {
            sh: Shell::new(fs, KEYMAPS, PREFS),
            scr,
            now: 1000,
        }
    }

    fn ev(&mut self, e: UiEvent<'_>) -> Vec<Effect> {
        self.now += 20;
        self.sh.event(e, &self.scr.env(), self.now)
    }

    fn cmd(&mut self, c: Command) {
        self.ev(UiEvent::Command(c));
    }

    fn click_at(&mut self, p: Pt) {
        self.ev(UiEvent::PointerMove {
            p,
            mods: Mods::default(),
        });
        self.ev(UiEvent::PointerDown {
            p,
            button: "MouseLeft",
            mods: Mods::default(),
        });
        self.ev(UiEvent::PointerUp {
            p,
            button: "MouseLeft",
            mods: Mods::default(),
        });
    }

    fn key(&mut self, name: &str) {
        self.ev(UiEvent::Key {
            name,
            pressed: true,
            mods: Mods::default(),
        });
        self.ev(UiEvent::Key {
            name,
            pressed: false,
            mods: Mods::default(),
        });
    }

    fn widgets(&self) -> Vec<Widget> {
        let env = self.scr.env();
        match self.sh.mode() {
            Mode::Hub => self.sh.hub_widgets(&env),
            _ => self
                .sh
                .editor()
                .map(|e| e.widgets(&env, Lang::Vi))
                .unwrap_or_default(),
        }
    }

    /// Centre of the first widget whose caption is `text`.
    fn find(&self, text: &str) -> Pt {
        for w in self.widgets() {
            let t = match &w.kind {
                WidgetKind::Button { label, .. } | WidgetKind::Check { label, .. } => {
                    Some(label.text(Lang::Vi))
                }
                WidgetKind::KeyCap { label, .. } => Some(label.clone()),
                WidgetKind::Row { title, .. } => Some(title.clone()),
                WidgetKind::Label(m) | WidgetKind::Heading(m) => Some(m.text(Lang::Vi)),
                _ => None,
            };
            if t.as_deref() == Some(text) && w.enabled {
                return Pt {
                    x: w.rect.x + w.rect.w / 2.0,
                    y: w.rect.y + w.rect.h / 2.0,
                };
            }
        }
        panic!("no widget {text:?}; have {:?}", self.widgets().len());
    }

    fn click(&mut self, text: &str) {
        let p = self.find(text);
        self.click_at(p);
    }

    fn scene(&self) -> Scene {
        self.sh.scene(&self.scr.env(), self.now).expect("a scene")
    }

    fn snap(&self, name: &str) {
        let sc = self.scene();
        assert_texts_fit(name, &sc);
        check(name, &sc.to_svg(self.scr.window, self.sh.pressed_mask()));
    }

    /// Select the n-th control (0-based) with Tab, the way a user reaches a
    /// control that the dock covers.
    fn select(&mut self, n: usize) {
        for _ in 0..=n {
            self.key("Tab");
        }
    }

    /// Window px of a control's centre (picture fractions in the profile).
    fn px(&self, fx: f64, fy: f64) -> Pt {
        let p = self.scr.picture;
        Pt {
            x: p.x + fx * p.w,
            y: p.y + fy * p.h,
        }
    }
}

fn two_profiles_and_one_broken() -> MemFs {
    let mut fs = fs_with(
        &[
            (
                "ho-so-1.json",
                profile("Hồ sơ 1", Template::ShooterLandscape),
            ),
            ("ho-so-2.json", profile("Hồ sơ 2", Template::BasicPortrait)),
        ],
        PREFS_PLAYABLE,
    );
    let mut broken = profile("Thử", Template::BasicLandscape);
    // Two controls on the same key: unusable.
    if let keymap::Control::Button { key, .. } = &mut broken.controls[2] {
        *key = "KeyW".into();
    }
    fs.put(
        &format!("{KEYMAPS}/thu-cam-ung.json"),
        &serde_json_text(&broken),
    );
    fs
}

/// A profile text that passes serde but fails validation.
fn serde_json_text(p: &Profile) -> String {
    p.to_json_pretty()
}

// ------------------------------------------------------------------ Hub ----

#[test]
fn hub_empty() {
    let mut d = Drv::new(fs_with(&[], r#"{"notice_ack":1}"#), Screen::hub());
    d.cmd(Command::OpenHub);
    d.snap("hub_empty");
}

#[test]
fn hub_list_with_a_broken_profile() {
    let mut d = Drv::new(two_profiles_and_one_broken(), Screen::hub());
    d.cmd(Command::OpenHub);
    d.snap("hub_list");
}

#[test]
fn hub_refusal_not_connected() {
    let mut scr = Screen::hub();
    scr.link = Link::None;
    let mut d = Drv::new(two_profiles_and_one_broken(), scr);
    d.cmd(Command::OpenHub);
    d.snap("hub_refusal_not_connected");
}

#[test]
fn hub_refusal_orientation() {
    let mut scr = Screen::hub();
    scr.rotation = Rotation::R0;
    let mut d = Drv::new(two_profiles_and_one_broken(), scr);
    d.cmd(Command::OpenHub);
    d.snap("hub_refusal_orientation");
}

#[test]
fn template_picker() {
    let mut d = Drv::new(fs_with(&[], r#"{"notice_ack":1}"#), Screen::hub());
    d.cmd(Command::OpenHub);
    d.click("Tạo hồ sơ đầu tiên");
    d.snap("template_picker");
}

#[test]
fn notice_unticked_and_ticked() {
    let mut d = Drv::new(
        fs_with(
            &[(
                "ho-so-1.json",
                profile("Hồ sơ 1", Template::ShooterLandscape),
            )],
            r#"{"active":"ho-so-1.json","guide_done":true}"#,
        ),
        Screen::hub(),
    );
    d.cmd(Command::OpenHub);
    d.click("CHƠI");
    d.snap("notice_unticked");
    d.click("Tôi đã đọc và tự chịu trách nhiệm");
    d.snap("notice_ticked");
}

#[test]
fn stuck_help_card() {
    let mut d = Drv::new(two_profiles_and_one_broken(), Screen::hub());
    d.cmd(Command::ShowStuckHelp);
    d.snap("stuck_help");
}

// ----------------------------------------------------------------- Edit ----

fn editing(scr: Screen) -> Drv {
    let mut d = Drv::new(
        fs_with(
            &[(
                "ho-so-1.json",
                profile("Hồ sơ 1", Template::ShooterLandscape),
            )],
            PREFS_PLAYABLE,
        ),
        scr,
    );
    d.cmd(Command::OpenHub);
    d.click("Sửa");
    assert_eq!(d.sh.mode(), Mode::Edit);
    d
}

#[test]
fn edit_nothing_selected() {
    editing(Screen::landscape()).snap("edit_nothing");
}

#[test]
fn edit_button_selected_card_left() {
    let mut d = editing(Screen::landscape());
    // "Nhảy" (5th control) is on the right: the dock goes left.
    d.select(4);
    d.snap("edit_button_card_left");
}

#[test]
fn edit_joystick_selected_card_right() {
    let mut d = editing(Screen::landscape());
    let p = d.px(0.15, 0.68);
    d.click_at(p);
    d.snap("edit_joystick_card_right");
}

#[test]
fn edit_look_selected() {
    let mut d = editing(Screen::landscape());
    d.select(1);
    d.snap("edit_look");
}

#[test]
fn edit_side_layout() {
    let mut d = editing(Screen::side());
    d.select(4);
    d.snap("edit_side");
}

#[test]
fn edit_portrait_sheet() {
    let mut d = Drv::new(
        fs_with(
            &[("ho-so-1.json", profile("Hồ sơ 1", Template::BasicPortrait))],
            PREFS_PLAYABLE,
        ),
        Screen::portrait(),
    );
    d.cmd(Command::OpenHub);
    d.click("Sửa");
    d.snap("edit_portrait_sheet");
}

#[test]
fn edit_capture_listening_and_conflict() {
    let mut d = editing(Screen::landscape());
    d.select(4);
    d.click("Space");
    d.snap("edit_capture_listening");
    d.key("KeyR"); // taken by "Nạp đạn"
    d.snap("edit_capture_conflict");
}

// ---------------------------------------------------------------- Guide ----

#[test]
fn guide_steps() {
    let mut scr = Screen::landscape();
    scr.has_picture = false;
    let mut d = Drv::new(fs_with(&[], r#"{"notice_ack":1}"#), scr);
    d.cmd(Command::OpenHub);
    d.click("Tạo hồ sơ đầu tiên");
    d.click("Bắn súng – màn hình ngang · đề nghị");
    assert_eq!(d.sh.mode(), Mode::Edit);
    assert_eq!(d.sh.editor().unwrap().guide_step(), Some(1));
    d.snap("guide_1");
    for step in 2..=6 {
        d.click("Tiếp");
        assert_eq!(d.sh.editor().unwrap().guide_step(), Some(step));
        d.snap(&format!("guide_{step}"));
    }
}

// ----------------------------------------------------------------- Test ----

#[test]
fn test_mode_with_two_contacts() {
    let mut d = editing(Screen::landscape());
    d.click("Thử");
    assert_eq!(d.sh.mode(), Mode::Test);
    d.ev(UiEvent::Key {
        name: "KeyW",
        pressed: true,
        mods: Mods::default(),
    });
    d.ev(UiEvent::RawMotion { dx: 30.0, dy: 10.0 });
    d.ev(UiEvent::Key {
        name: "MouseLeft",
        pressed: true,
        mods: Mods::default(),
    });
    assert!(d.sh.dots().dots.iter().flatten().count() >= 2);
    d.snap("test_two_contacts");
}

// ----------------------------------------------------------------- Play ----

#[test]
fn play_overlay_with_a_pressed_mask() {
    let mut d = Drv::new(
        fs_with(
            &[(
                "ho-so-1.json",
                profile("Hồ sơ 1", Template::ShooterLandscape),
            )],
            PREFS_PLAYABLE,
        ),
        Screen::landscape(),
    );
    d.ev(UiEvent::PlayStarted);
    assert_eq!(d.sh.mode(), Mode::Play);
    let sc = d.scene();
    assert_texts_fit("play_overlay", &sc);
    check("play_overlay", &sc.to_svg(d.scr.window, 0b1_0000_0101));
}
