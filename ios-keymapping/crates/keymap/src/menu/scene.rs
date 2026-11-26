//! Display list of the menu: primitives the Direct2D pass draws, and a
//! deterministic SVG rendering used for the golden screens (UI_SPEC 6.10).
//!
//! The model never measures text: every `Text` has a fixed box and its
//! string is written (wrapped, ellipsized) to fit. `fits` is the budget rule.

use super::{Pt, PxRect};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tone {
    Marker,
    MarkerDim,
    Selected,
    Pressed,
    Ghost,
    Guide,
    Hatch,
    Warn,
    Danger,
    Card,
    CardLine,
    Scrim,
    Text,
    TextSoft,
    Accent,
    OnAccent,
    Field,
    FieldFocus,
    Disabled,
    Finger,
    /// Translucent backing under canvas labels (legible over any picture).
    Plate,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Font {
    Body,
    Small,
    Title,
    Cap,
    Icon,
}

impl Font {
    /// Font size in logical px.
    pub fn size_lp(self) -> f64 {
        match self {
            Font::Body => 14.0,
            Font::Small => 12.0,
            Font::Title => 18.0,
            Font::Cap => 11.0,
            Font::Icon => 16.0,
        }
    }

    /// Average advance of one character in logical px (the model's budget
    /// estimate; the drawer still clips).
    pub fn char_lp(self) -> f64 {
        self.size_lp() * 0.58
    }

    /// Line height in logical px.
    pub fn line_lp(self) -> f64 {
        (self.size_lp() * 1.4).round()
    }
}

/// How many characters of `font` fit in `w_px` at `scale`.
pub fn max_chars(w_px: f64, font: Font, scale: f64) -> usize {
    let per = font.char_lp() * scale;
    if per <= 0.0 || w_px <= 0.0 {
        return 0;
    }
    ((w_px + 0.5) / per).floor() as usize
}

/// Whether `text` fits one line of width `w_px`.
pub fn fits(text: &str, w_px: f64, font: Font, scale: f64) -> bool {
    text.chars().count() <= max_chars(w_px, font, scale)
}

/// Cut `text` to `max` characters, ending with `…` when it was longer.
pub fn ellipsize(text: &str, max: usize) -> String {
    let n = text.chars().count();
    if n <= max {
        return text.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let mut s: String = text.chars().take(max - 1).collect();
    s.push('…');
    s
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Align {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Layer {
    Static,
    Dynamic,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Shape {
    Disc { c: Pt, r: f64 },
    Rect { r: PxRect, radius: f64 },
    Ring { c: Pt, r: f64, width: f64 },
}

#[derive(Clone, PartialEq, Debug)]
pub enum Prim {
    Disc {
        c: Pt,
        r: f64,
        tone: Tone,
    },
    Ring {
        c: Pt,
        r: f64,
        width: f64,
        tone: Tone,
    },
    Rect {
        r: PxRect,
        radius: f64,
        tone: Tone,
    },
    Frame {
        r: PxRect,
        radius: f64,
        width: f64,
        dashed: bool,
        tone: Tone,
    },
    Line {
        a: Pt,
        b: Pt,
        width: f64,
        dashed: bool,
        tone: Tone,
    },
    Hatch {
        r: PxRect,
        tone: Tone,
    },
    /// Clipped / ellipsized to `r` by the drawer.
    Text {
        r: PxRect,
        text: String,
        font: Font,
        align: Align,
        tone: Tone,
    },
    /// Filled only when bit `control` of the pressed mask is set.
    PressFill {
        control: u8,
        shape: Shape,
        tone: Tone,
    },
}

impl Prim {
    pub fn text(r: PxRect, text: impl Into<String>, font: Font, align: Align, tone: Tone) -> Prim {
        Prim::Text {
            r,
            text: text.into(),
            font,
            align,
            tone,
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Scene {
    /// Changes whenever the statics change (cache key of the app's bitmap).
    pub version: u64,
    /// Opacity of the whole scene (Play overlay: 0.22 style).
    pub alpha: f32,
    /// Logical px -> physical px, for font sizes.
    pub scale: f64,
    pub statics: Vec<Prim>,
    pub dynamics: Vec<Prim>,
}

impl Scene {
    pub fn new(version: u64, scale: f64) -> Scene {
        Scene {
            version,
            alpha: 1.0,
            scale,
            statics: Vec::new(),
            dynamics: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.statics.is_empty() && self.dynamics.is_empty()
    }

    pub fn prims(&self) -> impl Iterator<Item = &Prim> {
        self.statics.iter().chain(self.dynamics.iter())
    }
}

// ----------------------------------------------------------------- SVG ----

/// Fixed light palette: (fill/stroke colour, opacity).
pub fn palette(t: Tone) -> (&'static str, f64) {
    match t {
        Tone::Marker => ("#2563eb", 1.0),
        Tone::MarkerDim => ("#2563eb", 0.7),
        Tone::Selected => ("#ea580c", 1.0),
        Tone::Pressed => ("#16a34a", 1.0),
        Tone::Ghost => ("#2563eb", 0.4),
        Tone::Guide => ("#ec4899", 1.0),
        Tone::Hatch => ("#6b7280", 0.35),
        Tone::Warn => ("#b45309", 1.0),
        Tone::Danger => ("#dc2626", 1.0),
        Tone::Card => ("#ffffff", 1.0),
        Tone::CardLine => ("#cbd5e1", 1.0),
        Tone::Scrim => ("#0f172a", 0.45),
        Tone::Text => ("#0f172a", 1.0),
        Tone::TextSoft => ("#475569", 1.0),
        Tone::Accent => ("#2563eb", 1.0),
        Tone::OnAccent => ("#ffffff", 1.0),
        Tone::Field => ("#eef2f7", 1.0),
        Tone::FieldFocus => ("#dbeafe", 1.0),
        Tone::Disabled => ("#94a3b8", 1.0),
        Tone::Finger => ("#38bdf8", 0.9),
        Tone::Plate => ("#ffffff", 0.8),
    }
}

/// Number with at most 2 decimals and no trailing zeros ("-0" never appears).
fn n(v: f64) -> String {
    let mut s = format!("{v:.2}");
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    if s == "-0" { "0".to_string() } else { s }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn fill(t: Tone) -> String {
    let (c, o) = palette(t);
    if o < 1.0 {
        format!("fill=\"{c}\" fill-opacity=\"{}\"", n(o))
    } else {
        format!("fill=\"{c}\"")
    }
}

fn stroke(t: Tone, w: f64, dashed: bool) -> String {
    let (c, o) = palette(t);
    let mut s = format!("fill=\"none\" stroke=\"{c}\" stroke-width=\"{}\"", n(w));
    if o < 1.0 {
        s.push_str(&format!(" stroke-opacity=\"{}\"", n(o)));
    }
    if dashed {
        s.push_str(" stroke-dasharray=\"6 4\"");
    }
    s
}

fn rect_svg(r: &PxRect, radius: f64, attrs: &str) -> String {
    let rx = if radius > 0.0 {
        format!(" rx=\"{}\"", n(radius))
    } else {
        String::new()
    };
    format!(
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{rx} {attrs}/>",
        n(r.x),
        n(r.y),
        n(r.w),
        n(r.h)
    )
}

fn shape_svg(s: &Shape, tone: Tone) -> String {
    match s {
        Shape::Disc { c, r } => format!(
            "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" {}/>",
            n(c.x),
            n(c.y),
            n(*r),
            fill(tone)
        ),
        Shape::Rect { r, radius } => rect_svg(r, *radius, &fill(tone)),
        Shape::Ring { c, r, width } => format!(
            "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" {}/>",
            n(c.x),
            n(c.y),
            n(*r),
            stroke(tone, *width, false)
        ),
    }
}

fn prim_svg(p: &Prim, scale: f64, mask: u64, out: &mut Vec<String>) {
    match p {
        Prim::Disc { c, r, tone } => out.push(shape_svg(&Shape::Disc { c: *c, r: *r }, *tone)),
        Prim::Ring { c, r, width, tone } => out.push(shape_svg(
            &Shape::Ring {
                c: *c,
                r: *r,
                width: *width,
            },
            *tone,
        )),
        Prim::Rect { r, radius, tone } => out.push(rect_svg(r, *radius, &fill(*tone))),
        Prim::Frame {
            r,
            radius,
            width,
            dashed,
            tone,
        } => out.push(rect_svg(r, *radius, &stroke(*tone, *width, *dashed))),
        Prim::Line {
            a,
            b,
            width,
            dashed,
            tone,
        } => {
            let (c, o) = palette(*tone);
            let mut s = format!(
                "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{c}\" stroke-width=\"{}\"",
                n(a.x),
                n(a.y),
                n(b.x),
                n(b.y),
                n(*width)
            );
            if o < 1.0 {
                s.push_str(&format!(" stroke-opacity=\"{}\"", n(o)));
            }
            if *dashed {
                s.push_str(" stroke-dasharray=\"6 4\"");
            }
            s.push_str("/>");
            out.push(s);
        }
        Prim::Hatch { r, tone } => {
            let (_, o) = palette(*tone);
            out.push(rect_svg(
                r,
                0.0,
                &format!("fill=\"url(#hatch)\" fill-opacity=\"{}\"", n(o)),
            ));
        }
        Prim::Text {
            r,
            text,
            font,
            align,
            tone,
        } => {
            let (x, anchor) = match align {
                Align::Left => (r.x + 2.0 * scale, "start"),
                Align::Center => (r.x + r.w / 2.0, "middle"),
                Align::Right => (r.x + r.w - 2.0 * scale, "end"),
            };
            let weight = match font {
                Font::Title | Font::Cap => " font-weight=\"600\"",
                _ => "",
            };
            let (c, _) = palette(*tone);
            out.push(format!(
                "<text x=\"{}\" y=\"{}\" font-size=\"{}\" text-anchor=\"{anchor}\" dominant-baseline=\"central\" fill=\"{c}\"{weight}>{}</text>",
                n(x),
                n(r.y + r.h / 2.0),
                n(font.size_lp() * scale),
                esc(text)
            ));
        }
        Prim::PressFill {
            control,
            shape,
            tone,
        } => {
            if *control < 64 && mask & (1u64 << *control) != 0 {
                out.push(shape_svg(shape, *tone));
            }
        }
    }
}

impl Scene {
    /// SVG of the scene on a white-ish picture background. `pressed_mask`
    /// switches `PressFill` prims on. The output is byte-stable.
    pub fn to_svg(&self, window: (f64, f64), pressed_mask: u64) -> String {
        let mut lines = vec![
            format!(
                "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\" font-family=\"Segoe UI, Arial, sans-serif\">",
                n(window.0),
                n(window.1),
                n(window.0),
                n(window.1)
            ),
            "<defs><pattern id=\"hatch\" width=\"8\" height=\"8\" patternUnits=\"userSpaceOnUse\" patternTransform=\"rotate(45)\"><line x1=\"0\" y1=\"0\" x2=\"0\" y2=\"8\" stroke=\"#6b7280\" stroke-width=\"2\"/></pattern></defs>".to_string(),
            format!(
                "<rect width=\"{}\" height=\"{}\" fill=\"#d9dee5\"/>",
                n(window.0),
                n(window.1)
            ),
            format!("<g opacity=\"{}\">", n(f64::from(self.alpha))),
        ];
        for p in self.prims() {
            prim_svg(p, self.scale, pressed_mask, &mut lines);
        }
        lines.push("</g>".to_string());
        lines.push("</svg>".to_string());
        let mut s = lines.join("\n");
        s.push('\n');
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: f64, y: f64, w: f64, h: f64) -> PxRect {
        PxRect { x, y, w, h }
    }

    #[test]
    fn number_format_is_stable() {
        assert_eq!(n(1.0), "1");
        assert_eq!(n(1.5), "1.5");
        assert_eq!(n(-0.001), "0");
        assert_eq!(n(12.3456), "12.35");
    }

    #[test]
    fn press_fill_obeys_the_mask() {
        let mut s = Scene::new(1, 1.0);
        s.dynamics.push(Prim::PressFill {
            control: 2,
            shape: Shape::Disc {
                c: Pt { x: 10.0, y: 10.0 },
                r: 5.0,
            },
            tone: Tone::Pressed,
        });
        let off = s.to_svg((100.0, 100.0), 0b011);
        let on = s.to_svg((100.0, 100.0), 0b100);
        assert!(!off.contains("<circle"));
        assert!(on.contains("<circle cx=\"10\" cy=\"10\" r=\"5\""));
    }

    #[test]
    fn svg_escapes_text_and_is_deterministic() {
        let mut s = Scene::new(1, 1.0);
        s.statics.push(Prim::text(
            r(0.0, 0.0, 100.0, 20.0),
            "a<b & \"c\"",
            Font::Body,
            Align::Left,
            Tone::Text,
        ));
        let a = s.to_svg((200.0, 100.0), 0);
        assert!(a.contains("a&lt;b &amp; &quot;c&quot;"));
        assert_eq!(a, s.to_svg((200.0, 100.0), 0));
    }

    #[test]
    fn budget_helpers() {
        // 280 lp of Body text is 34 characters (UI_SPEC 6.10).
        assert_eq!(max_chars(280.0, Font::Body, 1.0), 34);
        assert!(fits(
            "Bấm nhanh = chạm 1 lần (0,04 giây)",
            280.0,
            Font::Body,
            1.0
        ));
        assert_eq!(ellipsize("abcdef", 4), "abc…");
        assert_eq!(ellipsize("abc", 4), "abc");
    }
}
