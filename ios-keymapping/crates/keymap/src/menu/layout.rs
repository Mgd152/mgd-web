//! Dock placement, widgets and their painting (UI_SPEC 3.2, 6.10).
//!
//! Widgets are rebuilt on every change by the editor / shell, which keeps its
//! own map from `WidgetId` to action. Everything is in window pixels
//! (`view.lp` converts logical px).

use super::doc::Slot;
use super::hit::EditView;
use super::msg::{Lang, Msg};
use super::scene::{Align, Font, Prim, Tone, ellipsize, max_chars};
use super::text::TextEdit;
use super::{Pt, PxRect};

/// Dock width in logical px.
pub const DOCK_W: f64 = 304.0;
pub const ROW_H: f64 = 36.0;
pub const GAP: f64 = 8.0;
pub const PAD: f64 = 12.0;
pub const TITLE_H: f64 = 44.0;
pub const CAP_H: f64 = 28.0;
pub const SLIDER_TRACK: f64 = 4.0;
pub const SLIDER_KNOB: f64 = 16.0;
pub const STEPPER_W: f64 = 24.0;
pub const SLIDER_SHOWN_W: f64 = 52.0;
/// Card margin to the picture edge.
pub const CARD_MARGIN: f64 = 8.0;
/// Width of the collapsed pill.
pub const PILL_W: f64 = 132.0;
pub const PILL_H: f64 = 44.0;
/// A card flips only after the focus is this fraction past the middle.
pub const FLIP_HYSTERESIS: f64 = 0.08;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DockPlacement {
    /// Reserved strip right of the picture (window grown).
    Side,
    CardLeft,
    CardRight,
    SheetTop,
    SheetBottom,
}

/// Where the dock goes, without memory of the last choice.
pub fn dock_placement(view: &EditView, focus: Option<Pt>, grown: bool) -> DockPlacement {
    dock_placement_with(view, focus, grown, None)
}

/// Same, with the previous placement so a drag along the middle does not
/// make the card flicker (hysteresis of 8 % of the picture side).
pub fn dock_placement_with(
    view: &EditView,
    focus: Option<Pt>,
    grown: bool,
    prev: Option<DockPlacement>,
) -> DockPlacement {
    if grown {
        return DockPlacement::Side;
    }
    let p = view.picture;
    if p.w < 2.0 * view.lp(DOCK_W) {
        // Bottom sheet, flipped to the top when the focus is in the lower half.
        let mid = p.y + p.h / 2.0;
        let hys = FLIP_HYSTERESIS * p.h;
        let Some(f) = focus else {
            return match prev {
                Some(DockPlacement::SheetTop) => DockPlacement::SheetTop,
                _ => DockPlacement::SheetBottom,
            };
        };
        return match prev {
            Some(DockPlacement::SheetBottom) => {
                if f.y > mid + hys {
                    DockPlacement::SheetTop
                } else {
                    DockPlacement::SheetBottom
                }
            }
            Some(DockPlacement::SheetTop) => {
                if f.y < mid - hys {
                    DockPlacement::SheetBottom
                } else {
                    DockPlacement::SheetTop
                }
            }
            _ => {
                if f.y > mid {
                    DockPlacement::SheetTop
                } else {
                    DockPlacement::SheetBottom
                }
            }
        };
    }
    let mid = p.x + p.w / 2.0;
    let hys = FLIP_HYSTERESIS * p.w;
    let Some(f) = focus else {
        return match prev {
            Some(DockPlacement::CardLeft) => DockPlacement::CardLeft,
            _ => DockPlacement::CardRight,
        };
    };
    match prev {
        Some(DockPlacement::CardRight) => {
            if f.x > mid + hys {
                DockPlacement::CardLeft
            } else {
                DockPlacement::CardRight
            }
        }
        Some(DockPlacement::CardLeft) => {
            if f.x < mid - hys {
                DockPlacement::CardRight
            } else {
                DockPlacement::CardLeft
            }
        }
        _ => {
            if f.x > mid {
                DockPlacement::CardLeft
            } else {
                DockPlacement::CardRight
            }
        }
    }
}

/// The dock's rectangle (the pill when `collapsed`).
pub fn dock_rect(view: &EditView, placement: DockPlacement, collapsed: bool) -> PxRect {
    let p = view.picture;
    let m = view.lp(CARD_MARGIN);
    let dw = view.lp(DOCK_W);
    let full = match placement {
        DockPlacement::Side => PxRect {
            x: (view.window.0 - dw).max(p.x + p.w),
            y: 0.0,
            w: dw,
            h: view.window.1,
        },
        DockPlacement::CardLeft => PxRect {
            x: p.x + m,
            y: p.y + m,
            w: dw.min(p.w - 2.0 * m),
            h: p.h - 2.0 * m,
        },
        DockPlacement::CardRight => {
            let w = dw.min(p.w - 2.0 * m);
            PxRect {
                x: p.x + p.w - w - m,
                y: p.y + m,
                w,
                h: p.h - 2.0 * m,
            }
        }
        DockPlacement::SheetTop => PxRect {
            x: p.x + m,
            y: p.y + m,
            w: p.w - 2.0 * m,
            h: 0.42 * p.h - m,
        },
        DockPlacement::SheetBottom => PxRect {
            x: p.x + m,
            y: p.y + p.h - (0.42 * p.h - m) - m,
            w: p.w - 2.0 * m,
            h: 0.42 * p.h - m,
        },
    };
    if !collapsed {
        return full;
    }
    let (pw, ph) = (view.lp(PILL_W).min(full.w), view.lp(PILL_H));
    let left = matches!(placement, DockPlacement::CardLeft);
    let bottom = matches!(placement, DockPlacement::SheetBottom);
    PxRect {
        x: if left { full.x } else { full.x + full.w - pw },
        y: if bottom { full.y + full.h - ph } else { full.y },
        w: pw,
        h: ph,
    }
}

// ------------------------------------------------------------- widgets ----

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct WidgetId(pub u32);

#[derive(Clone, PartialEq, Debug)]
pub struct Widget {
    pub id: WidgetId,
    pub rect: PxRect,
    pub kind: WidgetKind,
    pub enabled: bool,
}

#[derive(Clone, PartialEq, Debug)]
pub enum WidgetKind {
    Label(Msg),
    /// Section title (bigger, bold).
    Heading(Msg),
    /// Wrapped small text; the widget height holds all lines.
    Note(Msg),
    Banner {
        msg: Msg,
        tone: Tone,
    },
    Button {
        label: Msg,
        primary: bool,
    },
    Check {
        label: Msg,
        on: bool,
    },
    Segmented {
        options: Vec<Msg>,
        selected: Option<usize>,
    },
    Slider {
        value: f64,
        min: f64,
        max: f64,
        log: bool,
        shown: String,
    },
    KeyCap {
        slot: Slot,
        label: String,
        listening: bool,
    },
    Field {
        edit: TextEdit,
        focused: bool,
        placeholder: Msg,
    },
    Row {
        title: String,
        sub: String,
        selected: bool,
        broken: bool,
    },
}

impl WidgetKind {
    pub fn interactive(&self) -> bool {
        !matches!(
            self,
            WidgetKind::Label(_)
                | WidgetKind::Heading(_)
                | WidgetKind::Note(_)
                | WidgetKind::Banner { .. }
        )
    }
}

/// Topmost enabled interactive widget under `p`, and the 0..1 position along
/// its width.
pub fn hit_widget(ws: &[Widget], p: Pt) -> Option<(WidgetId, f64)> {
    ws.iter().rev().find_map(|w| {
        let r = w.rect;
        let inside = w.enabled
            && w.kind.interactive()
            && p.x >= r.x
            && p.x <= r.x + r.w
            && p.y >= r.y
            && p.y <= r.y + r.h;
        inside.then(|| {
            let t = if r.w > 0.0 {
                ((p.x - r.x) / r.w).clamp(0.0, 1.0)
            } else {
                0.0
            };
            (w.id, t)
        })
    })
}

/// Whether any widget (interactive or not, enabled or not) covers `p`.
pub fn covers(ws: &[Widget], p: Pt) -> bool {
    ws.iter().any(|w| {
        p.x >= w.rect.x
            && p.x <= w.rect.x + w.rect.w
            && p.y >= w.rect.y
            && p.y <= w.rect.y + w.rect.h
    })
}

/// Slider position (0..1) -> value; `log` uses a logarithmic scale.
pub fn slider_value(min: f64, max: f64, log: bool, t: f64) -> f64 {
    let t = if t.is_finite() {
        t.clamp(0.0, 1.0)
    } else {
        0.0
    };
    if log && min > 0.0 && max > min {
        (min.ln() + t * (max.ln() - min.ln())).exp()
    } else {
        min + t * (max - min)
    }
}

/// Inverse of `slider_value`.
pub fn slider_t(min: f64, max: f64, log: bool, v: f64) -> f64 {
    if max <= min || !v.is_finite() {
        return 0.0;
    }
    let t = if log && min > 0.0 {
        (v.max(min).ln() - min.ln()) / (max.ln() - min.ln())
    } else {
        (v - min) / (max - min)
    };
    t.clamp(0.0, 1.0)
}

/// One stepper press: 1/20 of the range (Shift: 1/100), clamped.
pub fn slider_step(min: f64, max: f64, log: bool, v: f64, up: bool, fine: bool) -> f64 {
    let d = if fine { 0.01 } else { 0.05 };
    let t = slider_t(min, max, log, v) + if up { d } else { -d };
    slider_value(min, max, log, t).clamp(min, max)
}

/// Where the parts of a slider widget are.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct SliderParts {
    pub minus: PxRect,
    pub track: PxRect,
    pub plus: PxRect,
    pub shown: PxRect,
}

pub fn slider_parts(r: PxRect, scale: f64) -> SliderParts {
    let (sw, shown_w, gap) = (STEPPER_W * scale, SLIDER_SHOWN_W * scale, 4.0 * scale);
    let minus = PxRect {
        x: r.x,
        y: r.y,
        w: sw,
        h: r.h,
    };
    let shown = PxRect {
        x: r.x + r.w - shown_w,
        y: r.y,
        w: shown_w,
        h: r.h,
    };
    let plus = PxRect {
        x: shown.x - gap - sw,
        y: r.y,
        w: sw,
        h: r.h,
    };
    let track = PxRect {
        x: minus.x + sw + gap,
        y: r.y,
        w: (plus.x - gap - (minus.x + sw + gap)).max(1.0),
        h: r.h,
    };
    SliderParts {
        minus,
        track,
        plus,
        shown,
    }
}

// ------------------------------------------------------------- columns ----

/// Hands out rows top to bottom inside a rectangle.
#[derive(Clone, Copy, Debug)]
pub struct Column {
    pub x: f64,
    pub w: f64,
    pub y: f64,
    pub scale: f64,
}

impl Column {
    /// A column inside `r` with the standard padding.
    pub fn new(r: PxRect, scale: f64) -> Column {
        Column {
            x: r.x + PAD * scale,
            w: (r.w - 2.0 * PAD * scale).max(1.0),
            y: r.y + PAD * scale,
            scale,
        }
    }

    pub fn row(&mut self, h_lp: f64) -> PxRect {
        let r = PxRect {
            x: self.x,
            y: self.y,
            w: self.w,
            h: h_lp * self.scale,
        };
        self.y += (h_lp + GAP) * self.scale;
        r
    }

    pub fn skip(&mut self, h_lp: f64) {
        self.y += h_lp * self.scale;
    }

    /// Wrapped-text row of `lines` lines of `font`.
    pub fn text_rows(&mut self, lines: usize, font: Font) -> PxRect {
        let h = lines.max(1) as f64 * font.line_lp();
        let r = PxRect {
            x: self.x,
            y: self.y,
            w: self.w,
            h: h * self.scale,
        };
        self.y += (h + GAP / 2.0) * self.scale;
        r
    }

    /// Characters per line of `font` in this column.
    pub fn chars(&self, font: Font) -> usize {
        max_chars(self.w, font, self.scale)
    }
}

/// Split a row horizontally by weights with a 6 lp gap.
pub fn split(r: PxRect, weights: &[f64], scale: f64) -> Vec<PxRect> {
    let gap = 6.0 * scale;
    let total: f64 = weights.iter().sum();
    let avail = r.w - gap * (weights.len().saturating_sub(1)) as f64;
    let mut x = r.x;
    weights
        .iter()
        .map(|wt| {
            let w = avail * wt / total;
            let out = PxRect {
                x,
                y: r.y,
                w,
                h: r.h,
            };
            x += w + gap;
            out
        })
        .collect()
}

/// Greedy word wrap by character count; over-long words are broken.
pub fn wrap(text: &str, max: usize) -> Vec<String> {
    let max = max.max(1);
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut cur_n = 0usize;
    for word in text.split_whitespace() {
        let mut wn = word.chars().count();
        let mut word = word.to_string();
        while wn > max {
            if cur_n > 0 {
                lines.push(std::mem::take(&mut cur));
                cur_n = 0;
            }
            let head: String = word.chars().take(max).collect();
            lines.push(head);
            word = word.chars().skip(max).collect();
            wn -= max;
        }
        if cur_n == 0 {
            cur = word;
            cur_n = wn;
        } else if cur_n + 1 + wn <= max {
            cur.push(' ');
            cur.push_str(&word);
            cur_n += 1 + wn;
        } else {
            lines.push(std::mem::take(&mut cur));
            cur = word;
            cur_n = wn;
        }
    }
    if cur_n > 0 || lines.is_empty() {
        lines.push(cur);
    }
    lines
}

// ------------------------------------------------------------- painting ----

fn inset(r: PxRect, d: f64) -> PxRect {
    PxRect {
        x: r.x + d,
        y: r.y + d,
        w: (r.w - 2.0 * d).max(0.0),
        h: (r.h - 2.0 * d).max(0.0),
    }
}

/// Text lines of a wrapped widget at its width.
pub fn wrapped_lines(text: &str, w_px: f64, font: Font, scale: f64) -> Vec<String> {
    wrap(text, max_chars(w_px, font, scale))
}

/// Number of lines `text` takes in a column `w_px` wide.
pub fn line_count(text: &str, w_px: f64, font: Font, scale: f64) -> usize {
    wrapped_lines(text, w_px, font, scale).len()
}

fn one_line(
    out: &mut Vec<Prim>,
    r: PxRect,
    text: &str,
    font: Font,
    align: Align,
    tone: Tone,
    scale: f64,
) {
    let t = ellipsize(text, max_chars(r.w, font, scale));
    out.push(Prim::text(r, t, font, align, tone));
}

/// Primitives of a widget list, in order.
pub fn widgets_to_prims(ws: &[Widget], lang: Lang, scale: f64) -> Vec<Prim> {
    let mut out = Vec::new();
    for w in ws {
        paint(w, lang, scale, &mut out);
    }
    out
}

fn paint(w: &Widget, lang: Lang, scale: f64, out: &mut Vec<Prim>) {
    let r = w.rect;
    let rad = 6.0 * scale;
    let text_tone = if w.enabled {
        Tone::Text
    } else {
        Tone::Disabled
    };
    match &w.kind {
        WidgetKind::Label(m) => one_line(
            out,
            r,
            &m.text(lang),
            Font::Body,
            Align::Left,
            text_tone,
            scale,
        ),
        WidgetKind::Heading(m) => one_line(
            out,
            r,
            &m.text(lang),
            Font::Title,
            Align::Left,
            Tone::Text,
            scale,
        ),
        WidgetKind::Note(m) => {
            let lh = Font::Small.line_lp() * scale;
            for (i, line) in wrapped_lines(&m.text(lang), r.w, Font::Small, scale)
                .into_iter()
                .enumerate()
            {
                let lr = PxRect {
                    x: r.x,
                    y: r.y + i as f64 * lh,
                    w: r.w,
                    h: lh,
                };
                out.push(Prim::text(
                    lr,
                    line,
                    Font::Small,
                    Align::Left,
                    Tone::TextSoft,
                ));
            }
        }
        WidgetKind::Banner { msg, tone } => {
            out.push(Prim::Rect {
                r,
                radius: rad,
                tone: Tone::Field,
            });
            out.push(Prim::Rect {
                r: PxRect {
                    x: r.x,
                    y: r.y,
                    w: 4.0 * scale,
                    h: r.h,
                },
                radius: 0.0,
                tone: *tone,
            });
            let inner = PxRect {
                x: r.x + 10.0 * scale,
                y: r.y + 4.0 * scale,
                w: r.w - 16.0 * scale,
                h: r.h - 8.0 * scale,
            };
            let lh = Font::Small.line_lp() * scale;
            let lines = wrapped_lines(&msg.text(lang), inner.w, Font::Small, scale);
            let top = inner.y + ((inner.h - lines.len() as f64 * lh) / 2.0).max(0.0);
            for (i, line) in lines.into_iter().enumerate() {
                let lr = PxRect {
                    x: inner.x,
                    y: top + i as f64 * lh,
                    w: inner.w,
                    h: lh,
                };
                out.push(Prim::text(lr, line, Font::Small, Align::Left, Tone::Text));
            }
        }
        WidgetKind::Button { label, primary } => {
            let (bg, fg) = if !w.enabled {
                (Tone::Field, Tone::Disabled)
            } else if *primary {
                (Tone::Accent, Tone::OnAccent)
            } else {
                (Tone::Field, Tone::Text)
            };
            out.push(Prim::Rect {
                r,
                radius: rad,
                tone: bg,
            });
            if !*primary {
                out.push(Prim::Frame {
                    r,
                    radius: rad,
                    width: 1.0 * scale,
                    dashed: false,
                    tone: Tone::CardLine,
                });
            }
            one_line(
                out,
                inset(r, 4.0 * scale),
                &label.text(lang),
                Font::Body,
                Align::Center,
                fg,
                scale,
            );
        }
        WidgetKind::Check { label, on } => {
            let b = 18.0 * scale;
            let boxr = PxRect {
                x: r.x,
                y: r.y + (r.h - b) / 2.0,
                w: b,
                h: b,
            };
            out.push(Prim::Rect {
                r: boxr,
                radius: 3.0 * scale,
                tone: if *on { Tone::Accent } else { Tone::Field },
            });
            out.push(Prim::Frame {
                r: boxr,
                radius: 3.0 * scale,
                width: 1.0 * scale,
                dashed: false,
                tone: if w.enabled {
                    Tone::Accent
                } else {
                    Tone::Disabled
                },
            });
            if *on {
                let (a, m, z) = (
                    Pt {
                        x: boxr.x + 4.0 * scale,
                        y: boxr.y + 9.0 * scale,
                    },
                    Pt {
                        x: boxr.x + 8.0 * scale,
                        y: boxr.y + 13.0 * scale,
                    },
                    Pt {
                        x: boxr.x + 14.0 * scale,
                        y: boxr.y + 5.0 * scale,
                    },
                );
                for (p, q) in [(a, m), (m, z)] {
                    out.push(Prim::Line {
                        a: p,
                        b: q,
                        width: 2.0 * scale,
                        dashed: false,
                        tone: Tone::OnAccent,
                    });
                }
            }
            let tr = PxRect {
                x: r.x + b + 8.0 * scale,
                y: r.y,
                w: r.w - b - 8.0 * scale,
                h: r.h,
            };
            one_line(
                out,
                tr,
                &label.text(lang),
                Font::Small,
                Align::Left,
                text_tone,
                scale,
            );
        }
        WidgetKind::Segmented { options, selected } => {
            out.push(Prim::Rect {
                r,
                radius: rad,
                tone: Tone::Field,
            });
            let weights = vec![1.0; options.len().max(1)];
            for (i, (cell, o)) in split(r, &weights, scale)
                .into_iter()
                .zip(options)
                .enumerate()
            {
                let on = *selected == Some(i);
                if on {
                    out.push(Prim::Rect {
                        r: cell,
                        radius: rad,
                        tone: Tone::Accent,
                    });
                }
                let fg = if on {
                    Tone::OnAccent
                } else if w.enabled {
                    Tone::Text
                } else {
                    Tone::Disabled
                };
                one_line(
                    out,
                    inset(cell, 2.0 * scale),
                    &o.text(lang),
                    Font::Small,
                    Align::Center,
                    fg,
                    scale,
                );
            }
            out.push(Prim::Frame {
                r,
                radius: rad,
                width: 1.0 * scale,
                dashed: false,
                tone: Tone::CardLine,
            });
        }
        WidgetKind::Slider {
            value,
            min,
            max,
            log,
            shown,
        } => {
            let parts = slider_parts(r, scale);
            for (cell, glyph) in [(parts.minus, "−"), (parts.plus, "+")] {
                out.push(Prim::Rect {
                    r: cell,
                    radius: rad,
                    tone: Tone::Field,
                });
                out.push(Prim::text(
                    cell,
                    glyph,
                    Font::Body,
                    Align::Center,
                    text_tone,
                ));
            }
            let t = slider_t(*min, *max, *log, *value);
            let cy = r.y + r.h / 2.0;
            let th = SLIDER_TRACK * scale;
            let tr = parts.track;
            out.push(Prim::Rect {
                r: PxRect {
                    x: tr.x,
                    y: cy - th / 2.0,
                    w: tr.w,
                    h: th,
                },
                radius: th / 2.0,
                tone: Tone::CardLine,
            });
            out.push(Prim::Rect {
                r: PxRect {
                    x: tr.x,
                    y: cy - th / 2.0,
                    w: tr.w * t,
                    h: th,
                },
                radius: th / 2.0,
                tone: if w.enabled {
                    Tone::Accent
                } else {
                    Tone::Disabled
                },
            });
            out.push(Prim::Disc {
                c: Pt {
                    x: tr.x + tr.w * t,
                    y: cy,
                },
                r: SLIDER_KNOB * scale / 2.0,
                tone: if w.enabled {
                    Tone::Accent
                } else {
                    Tone::Disabled
                },
            });
            one_line(
                out,
                parts.shown,
                shown,
                Font::Body,
                Align::Right,
                text_tone,
                scale,
            );
        }
        WidgetKind::KeyCap {
            label, listening, ..
        } => {
            out.push(Prim::Rect {
                r,
                radius: rad,
                tone: if *listening {
                    Tone::FieldFocus
                } else {
                    Tone::Field
                },
            });
            out.push(Prim::Frame {
                r,
                radius: rad,
                width: if *listening { 2.0 } else { 1.0 } * scale,
                dashed: false,
                tone: if *listening {
                    Tone::Accent
                } else {
                    Tone::CardLine
                },
            });
            one_line(
                out,
                inset(r, 4.0 * scale),
                label,
                Font::Cap,
                Align::Center,
                text_tone,
                scale,
            );
        }
        WidgetKind::Field {
            edit,
            focused,
            placeholder,
        } => {
            out.push(Prim::Rect {
                r,
                radius: rad,
                tone: if *focused {
                    Tone::FieldFocus
                } else {
                    Tone::Field
                },
            });
            out.push(Prim::Frame {
                r,
                radius: rad,
                width: if *focused { 2.0 } else { 1.0 } * scale,
                dashed: false,
                tone: if *focused {
                    Tone::Accent
                } else {
                    Tone::CardLine
                },
            });
            let tr = inset(r, 6.0 * scale);
            if edit.text().is_empty() {
                one_line(
                    out,
                    tr,
                    &placeholder.text(lang),
                    Font::Body,
                    Align::Left,
                    Tone::Disabled,
                    scale,
                );
            } else {
                one_line(
                    out,
                    tr,
                    edit.text(),
                    Font::Body,
                    Align::Left,
                    Tone::Text,
                    scale,
                );
            }
            if *focused {
                let cx = tr.x + 2.0 * scale + edit.caret() as f64 * Font::Body.char_lp() * scale;
                let cx = cx.min(tr.x + tr.w);
                out.push(Prim::Line {
                    a: Pt {
                        x: cx,
                        y: tr.y + 2.0 * scale,
                    },
                    b: Pt {
                        x: cx,
                        y: tr.y + tr.h - 2.0 * scale,
                    },
                    width: 1.5 * scale,
                    dashed: false,
                    tone: Tone::Accent,
                });
            }
        }
        WidgetKind::Row {
            title,
            sub,
            selected,
            broken,
        } => {
            out.push(Prim::Rect {
                r,
                radius: rad,
                tone: if *selected {
                    Tone::FieldFocus
                } else {
                    Tone::Field
                },
            });
            out.push(Prim::Frame {
                r,
                radius: rad,
                width: if *selected { 2.0 } else { 1.0 } * scale,
                dashed: false,
                tone: if *broken {
                    Tone::Warn
                } else if *selected {
                    Tone::Accent
                } else {
                    Tone::CardLine
                },
            });
            let icon = PxRect {
                x: r.x + 6.0 * scale,
                y: r.y,
                w: 28.0 * scale,
                h: r.h,
            };
            out.push(Prim::text(
                icon,
                if *broken { "⚠" } else { "▣" },
                Font::Icon,
                Align::Center,
                if *broken { Tone::Warn } else { Tone::TextSoft },
            ));
            let tx = r.x + 40.0 * scale;
            let tw = r.w - 46.0 * scale;
            let half = r.h / 2.0;
            one_line(
                out,
                PxRect {
                    x: tx,
                    y: r.y + 2.0 * scale,
                    w: tw,
                    h: half,
                },
                title,
                Font::Body,
                Align::Left,
                Tone::Text,
                scale,
            );
            one_line(
                out,
                PxRect {
                    x: tx,
                    y: r.y + half,
                    w: tw,
                    h: half - 2.0 * scale,
                },
                sub,
                Font::Small,
                Align::Left,
                if *broken { Tone::Warn } else { Tone::TextSoft },
                scale,
            );
        }
    }
}

/// Texts of a widget that must fit their box: `(text, width px, font)`.
pub fn widget_texts(w: &Widget, lang: Lang, scale: f64) -> Vec<(String, f64, Font)> {
    let r = w.rect;
    match &w.kind {
        WidgetKind::Label(m) => vec![(m.text(lang), r.w, Font::Body)],
        WidgetKind::Heading(m) => vec![(m.text(lang), r.w, Font::Title)],
        WidgetKind::Button { label, .. } => vec![(label.text(lang), r.w - 8.0 * scale, Font::Body)],
        WidgetKind::Check { label, .. } => {
            vec![(label.text(lang), r.w - 26.0 * scale, Font::Small)]
        }
        WidgetKind::Segmented { options, .. } => {
            let weights = vec![1.0; options.len().max(1)];
            split(r, &weights, scale)
                .into_iter()
                .zip(options)
                .map(|(c, o)| (o.text(lang), c.w - 4.0 * scale, Font::Small))
                .collect()
        }
        WidgetKind::Slider { shown, .. } => {
            vec![(shown.clone(), slider_parts(r, scale).shown.w, Font::Body)]
        }
        WidgetKind::KeyCap { label, .. } => vec![(label.clone(), r.w - 8.0 * scale, Font::Cap)],
        WidgetKind::Field { edit, .. } => {
            vec![(edit.text().to_string(), r.w - 12.0 * scale, Font::Body)]
        }
        WidgetKind::Row { title, sub, .. } => vec![
            (title.clone(), r.w - 46.0 * scale, Font::Body),
            (sub.clone(), r.w - 46.0 * scale, Font::Small),
        ],
        WidgetKind::Note(_) | WidgetKind::Banner { .. } => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::hit::testing::{pt, view};

    fn view_w(w: f64, h: f64, scale: f64) -> EditView {
        EditView {
            window: (w, h),
            picture: PxRect {
                x: 0.0,
                y: 0.0,
                w,
                h,
            },
            scale,
        }
    }

    // test 52
    #[test]
    fn placement_goes_to_the_half_away_from_the_focus() {
        let v = view_w(1000.0, 500.0, 1.0);
        assert_eq!(dock_placement(&v, None, false), DockPlacement::CardRight);
        assert_eq!(
            dock_placement(&v, Some(pt(200.0, 250.0)), false),
            DockPlacement::CardRight
        );
        assert_eq!(
            dock_placement(&v, Some(pt(800.0, 250.0)), false),
            DockPlacement::CardLeft
        );
        assert_eq!(
            dock_placement(&v, Some(pt(800.0, 250.0)), true),
            DockPlacement::Side
        );
    }

    #[test]
    fn portrait_picture_gets_a_sheet_that_flips_vertically() {
        let v = view_w(400.0, 800.0, 1.0);
        assert_eq!(dock_placement(&v, None, false), DockPlacement::SheetBottom);
        assert_eq!(
            dock_placement(&v, Some(pt(200.0, 200.0)), false),
            DockPlacement::SheetBottom
        );
        assert_eq!(
            dock_placement(&v, Some(pt(200.0, 600.0)), false),
            DockPlacement::SheetTop
        );
        let r = dock_rect(&v, DockPlacement::SheetBottom, false);
        assert!(r.h <= 0.42 * 800.0 + 1e-9);
        assert!(r.y + r.h <= 800.0 && r.y >= 0.5 * 800.0);
        let t = dock_rect(&v, DockPlacement::SheetTop, false);
        assert!(t.y >= 0.0 && t.y + t.h <= 0.43 * 800.0);
    }

    #[test]
    fn hysteresis_keeps_the_card_near_the_middle() {
        let v = view_w(1000.0, 500.0, 1.0);
        let near = Some(pt(540.0, 250.0)); // 4 % past the middle
        assert_eq!(
            dock_placement_with(&v, near, false, Some(DockPlacement::CardRight)),
            DockPlacement::CardRight
        );
        assert_eq!(
            dock_placement_with(&v, near, false, Some(DockPlacement::CardLeft)),
            DockPlacement::CardLeft
        );
        let far = Some(pt(600.0, 250.0)); // 10 % past
        assert_eq!(
            dock_placement_with(&v, far, false, Some(DockPlacement::CardRight)),
            DockPlacement::CardLeft
        );
        let far_left = Some(pt(400.0, 250.0));
        assert_eq!(
            dock_placement_with(&v, far_left, false, Some(DockPlacement::CardLeft)),
            DockPlacement::CardRight
        );
    }

    #[test]
    fn collapsed_dock_is_a_44_px_pill() {
        let v = view_w(1000.0, 500.0, 1.0);
        for p in [
            DockPlacement::Side,
            DockPlacement::CardLeft,
            DockPlacement::CardRight,
            DockPlacement::SheetTop,
            DockPlacement::SheetBottom,
        ] {
            let pill = dock_rect(&v, p, true);
            let full = dock_rect(&v, p, false);
            assert!((pill.h - 44.0).abs() < 1e-9, "{p:?}");
            assert!(pill.x >= full.x - 1e-9 && pill.x + pill.w <= full.x + full.w + 1e-9);
            assert!(pill.y >= full.y - 1e-9 && pill.y + pill.h <= full.y + full.h + 1e-9);
        }
    }

    #[test]
    fn card_never_covers_a_control_in_the_other_half() {
        for scale in [1.0, 1.5, 2.0] {
            let w = 2.0 * DOCK_W * scale + 40.0;
            let v = view_w(w, 500.0, scale);
            // A button disc (22 lp) whose centre is at least 40 lp inside its half.
            for fx in [0.2, 0.35, 0.65, 0.9] {
                let c = pt(fx * w, 250.0);
                let place = dock_placement(&v, Some(c), false);
                let d = dock_rect(&v, place, false);
                let rad = 22.0 * scale;
                let overlaps = c.x + rad > d.x && c.x - rad < d.x + d.w;
                assert!(!overlaps, "scale {scale} fx {fx} {place:?}");
            }
        }
        let _ = view();
    }

    #[test]
    fn dock_card_stays_inside_the_picture() {
        let v = view_w(1000.0, 500.0, 2.0);
        for p in [DockPlacement::CardLeft, DockPlacement::CardRight] {
            let d = dock_rect(&v, p, false);
            assert!(d.x >= 0.0 && d.x + d.w <= 1000.0 && d.y >= 0.0 && d.y + d.h <= 500.0);
            assert!((d.w - 304.0 * 2.0).abs() < 1e-9 || d.w < 304.0 * 2.0);
        }
    }

    // test 54
    #[test]
    fn slider_log_mapping_round_trips() {
        for v in [0.1, 0.5, 1.0, 1.8, 7.0, 10.0] {
            let t = slider_t(0.1, 10.0, true, v);
            assert!((slider_value(0.1, 10.0, true, t) - v).abs() < 1e-9, "{v}");
        }
        assert!((slider_t(0.1, 10.0, true, 1.0) - 0.5).abs() < 1e-9);
        assert_eq!(slider_value(0.0, 4.0, false, 0.25), 1.0);
        assert!((slider_value(0.1, 10.0, true, 7.0) - 10.0).abs() < 1e-9);
        assert!((slider_value(0.1, 10.0, true, f64::NAN) - 0.1).abs() < 1e-12);
    }

    #[test]
    fn stepper_stays_in_bounds() {
        let mut v = 9.9;
        for _ in 0..40 {
            v = slider_step(0.1, 10.0, true, v, true, false);
            assert!((0.1..=10.0).contains(&v));
        }
        assert!((v - 10.0).abs() < 1e-9);
        for _ in 0..80 {
            v = slider_step(0.1, 10.0, true, v, false, false);
            assert!((0.1..=10.0).contains(&v));
        }
        assert!((v - 0.1).abs() < 1e-9);
        let fine = slider_step(0.0, 1.0, false, 0.5, true, true);
        assert!((fine - 0.51).abs() < 1e-9);
    }

    #[test]
    fn hit_widget_picks_topmost_enabled_interactive() {
        let r = |x: f64| PxRect {
            x,
            y: 0.0,
            w: 100.0,
            h: 30.0,
        };
        let ws = vec![
            Widget {
                id: WidgetId(1),
                rect: r(0.0),
                kind: WidgetKind::Button {
                    label: Msg::Cancel,
                    primary: false,
                },
                enabled: true,
            },
            Widget {
                id: WidgetId(2),
                rect: r(50.0),
                kind: WidgetKind::Button {
                    label: Msg::Cancel,
                    primary: false,
                },
                enabled: false,
            },
            Widget {
                id: WidgetId(3),
                rect: r(50.0),
                kind: WidgetKind::Label(Msg::Cancel),
                enabled: true,
            },
        ];
        assert_eq!(
            hit_widget(&ws, pt(75.0, 10.0)).map(|h| h.0),
            Some(WidgetId(1))
        );
        let (id, t) = hit_widget(&ws, pt(25.0, 10.0)).unwrap();
        assert_eq!(id, WidgetId(1));
        assert!((t - 0.25).abs() < 1e-9);
        assert!(hit_widget(&ws, pt(500.0, 10.0)).is_none());
        assert!(covers(&ws, pt(120.0, 10.0)));
    }

    #[test]
    fn wrap_breaks_on_words_and_long_words() {
        assert_eq!(wrap("aa bb cc", 5), vec!["aa bb", "cc"]);
        assert_eq!(wrap("abcdefgh", 3), vec!["abc", "def", "gh"]);
        assert_eq!(wrap("", 5), vec![""]);
        for l in wrap(
            "Hồ sơ này làm bằng bản mới hơn. Hãy cập nhật chương trình.",
            20,
        ) {
            assert!(l.chars().count() <= 20);
        }
    }

    #[test]
    fn slider_parts_tile_the_row() {
        let r = PxRect {
            x: 10.0,
            y: 0.0,
            w: 280.0,
            h: 36.0,
        };
        let p = slider_parts(r, 1.0);
        assert!(p.minus.x + p.minus.w <= p.track.x);
        assert!(p.track.x + p.track.w <= p.plus.x);
        assert!(p.plus.x + p.plus.w <= p.shown.x);
        assert!((p.shown.x + p.shown.w - 290.0).abs() < 1e-9);
    }

    // test 53 (dock content) is in editor.rs, which owns the dock widgets.
}
