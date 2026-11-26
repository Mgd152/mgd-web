//! Helpers shared by the unit tests of `editor` and `shell`.

use super::editor::{Editor, EditorOut};
use super::hit::EditView;
use super::layout::{Widget, WidgetKind};
use super::lint::{Env, Link};
use super::msg::Lang;
use super::shell::UiEvent;
use super::templates::{Template, template};
use super::{Mods, Pt, PxRect};
use crate::geom::Rotation;

/// 1280 x 590 landscape picture filling its window, scale 1, USB, R90.
pub fn env() -> Env {
    env_sized(1280.0, 590.0, 1.0, Rotation::R90)
}

pub fn env_sized(w: f64, h: f64, scale: f64, rotation: Rotation) -> Env {
    Env {
        link: Link::Usb,
        display_mode: false,
        scroll_busy: false,
        rotation,
        panel: (1206.0, 2622.0),
        has_picture: true,
        view: EditView {
            window: (w, h),
            picture: PxRect {
                x: 0.0,
                y: 0.0,
                w,
                h,
            },
            scale,
        },
        can_grow_window: false,
    }
}

/// Deterministic generator for fuzz tests.
pub struct Lcg(pub u64);

impl Lcg {
    pub fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
    pub fn unit(&mut self) -> f64 {
        (self.next() % 10_000) as f64 / 10_000.0
    }
    pub fn pick<'a, T>(&mut self, v: &'a [T]) -> &'a T {
        &v[self.below(v.len() as u64) as usize]
    }
}

/// Text of a widget caption (for finding widgets by what the user reads).
pub fn caption(w: &Widget) -> Option<String> {
    match &w.kind {
        WidgetKind::Button { label, .. } | WidgetKind::Check { label, .. } => {
            Some(label.text(Lang::Vi))
        }
        WidgetKind::KeyCap { label, .. } => Some(label.clone()),
        WidgetKind::Row { title, .. } => Some(title.clone()),
        WidgetKind::Label(m) | WidgetKind::Heading(m) => Some(m.text(Lang::Vi)),
        _ => None,
    }
}

pub fn centre(w: &Widget) -> Pt {
    Pt {
        x: w.rect.x + w.rect.w / 2.0,
        y: w.rect.y + w.rect.h / 2.0,
    }
}

/// An editor plus the environment and a clock.
pub struct Ed {
    pub ed: Editor,
    pub env: Env,
    pub now: u64,
}

impl Ed {
    pub fn shooter() -> Ed {
        Ed::with(template(Template::ShooterLandscape))
    }

    pub fn with(p: crate::profile::Profile) -> Ed {
        Ed {
            ed: Editor::open("a.json", p, false).unwrap(),
            env: env_sized(1700.0, 1000.0, 1.0, Rotation::R90),
            now: 1000,
        }
    }

    pub fn ev(&mut self, e: UiEvent<'_>) -> Vec<EditorOut> {
        self.now += 10;
        self.ed.event(&e, &self.env, self.now)
    }

    pub fn key_m(&mut self, name: &str, mods: Mods) -> Vec<EditorOut> {
        let mut o = self.ev(UiEvent::Key {
            name,
            pressed: true,
            mods,
        });
        o.extend(self.ev(UiEvent::Key {
            name,
            pressed: false,
            mods,
        }));
        o
    }

    pub fn key(&mut self, name: &str) -> Vec<EditorOut> {
        self.key_m(name, Mods::default())
    }

    pub fn ctrl(&mut self, name: &str) -> Vec<EditorOut> {
        self.key_m(
            name,
            Mods {
                ctrl: true,
                ..Mods::default()
            },
        )
    }

    pub fn down(&mut self, p: Pt) -> Vec<EditorOut> {
        self.ev(UiEvent::PointerDown {
            p,
            button: "MouseLeft",
            mods: Mods::default(),
        })
    }

    pub fn mv(&mut self, p: Pt) -> Vec<EditorOut> {
        self.ev(UiEvent::PointerMove {
            p,
            mods: Mods::default(),
        })
    }

    pub fn up(&mut self, p: Pt) -> Vec<EditorOut> {
        self.ev(UiEvent::PointerUp {
            p,
            button: "MouseLeft",
            mods: Mods::default(),
        })
    }

    pub fn click(&mut self, p: Pt) {
        self.mv(p);
        self.down(p);
        self.up(p);
    }

    pub fn widgets(&self) -> Vec<Widget> {
        self.ed.widgets(&self.env, Lang::Vi)
    }

    pub fn find(&self, text: &str) -> Option<Widget> {
        self.widgets()
            .into_iter()
            .find(|w| w.enabled && caption(w).as_deref() == Some(text))
    }

    pub fn click_text(&mut self, text: &str) {
        let w = self
            .find(text)
            .unwrap_or_else(|| panic!("no enabled widget {text:?}"));
        self.click(centre(&w));
    }

    /// Select the n-th control with Tab.
    pub fn select(&mut self, n: usize) {
        for _ in 0..=n {
            self.key("Tab");
        }
    }

    /// Click a dock widget by caption and return what the editor asked for.
    pub fn click_outs(&mut self, text: &str) -> Vec<EditorOut> {
        let w = self
            .find(text)
            .unwrap_or_else(|| panic!("no enabled widget {text:?}"));
        let p = centre(&w);
        self.mv(p);
        let mut o = self.down(p);
        o.extend(self.up(p));
        o
    }

    pub fn px(&self, fx: f64, fy: f64) -> Pt {
        self.env.view.to_px(Pt { x: fx, y: fy })
    }
}
