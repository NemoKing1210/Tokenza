//! Renders the tray indicator (ring, bars, pie or number) into an RGBA bitmap on the fly.

use std::f32::consts::PI;

use tiny_skia::{Color, FillRule, LineCap, Paint, PathBuilder, Pixmap, Stroke, Transform};
use tokenza_core::{UsageWindow, WindowKind};

use crate::logos::Logo;

pub const SIZE: u32 = 32;

#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct Levels {
    /// Worst used percentage over every window, if any data exists.
    pub worst: Option<f32>,
    pub session: Option<f32>,
    pub weekly: Option<f32>,
}

impl Levels {
    pub fn from_windows<'a>(windows: impl Iterator<Item = &'a UsageWindow>) -> Self {
        let mut l = Levels::default();
        let max = |a: Option<f32>, b: f32| Some(a.map_or(b, |a| a.max(b)));
        for w in windows {
            let Some(p) = w.used_pct else { continue };
            l.worst = max(l.worst, p);
            match w.kind {
                WindowKind::Session5h | WindowKind::Daily => l.session = max(l.session, p),
                WindowKind::Weekly | WindowKind::Monthly => l.weekly = max(l.weekly, p),
                WindowKind::Credits => {}
            }
        }
        l
    }
}

/// Threshold colors: < 70 accent, 70–90 amber, > 90 red (same bands as the flyout).
fn tone(pct: f32) -> Color {
    if pct > 90.0 {
        Color::from_rgba8(0xE8, 0x4A, 0x3F, 0xFF)
    } else if pct >= 70.0 {
        Color::from_rgba8(0xF5, 0xA6, 0x23, 0xFF)
    } else {
        Color::from_rgba8(0x3A, 0x96, 0xDD, 0xFF)
    }
}

fn track_color() -> Color {
    // Mid gray with alpha reads on both light and dark taskbars.
    Color::from_rgba8(0x90, 0x90, 0x98, 0x88)
}

fn paint(c: Color) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(c);
    p.anti_alias = true;
    p
}

fn stroke(width: f32) -> Stroke {
    Stroke {
        width,
        line_cap: LineCap::Round,
        ..Stroke::default()
    }
}

fn arc_path(cx: f32, cy: f32, r: f32, frac: f32) -> Option<tiny_skia::Path> {
    let frac = frac.clamp(0.0, 1.0);
    let steps = ((frac * 120.0).ceil() as i32).max(2);
    let mut pb = PathBuilder::new();
    for i in 0..=steps {
        let a = -PI / 2.0 + 2.0 * PI * frac * (i as f32 / steps as f32);
        let (x, y) = (cx + r * a.cos(), cy + r * a.sin());
        if i == 0 {
            pb.move_to(x, y);
        } else {
            pb.line_to(x, y);
        }
    }
    pb.finish()
}

fn line(x0: f32, x1: f32, y: f32) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    pb.move_to(x0, y);
    pb.line_to(x1, y);
    pb.finish()
}

/// A filled pie slice starting at 12 o'clock and running clockwise.
fn sector_path(cx: f32, cy: f32, r: f32, frac: f32) -> Option<tiny_skia::Path> {
    let frac = frac.clamp(0.0, 1.0);
    let steps = ((frac * 120.0).ceil() as i32).max(2);
    let mut pb = PathBuilder::new();
    pb.move_to(cx, cy);
    for i in 0..=steps {
        let a = -PI / 2.0 + 2.0 * PI * frac * (i as f32 / steps as f32);
        pb.line_to(cx + r * a.cos(), cy + r * a.sin());
    }
    pb.close();
    pb.finish()
}

/// 3x5 pixel digits, one row per entry (bit 2 is the left column).
const DIGITS: [[u8; 5]; 10] = [
    [0b111, 0b101, 0b101, 0b101, 0b111],
    [0b010, 0b110, 0b010, 0b010, 0b111],
    [0b111, 0b001, 0b111, 0b100, 0b111],
    [0b111, 0b001, 0b111, 0b001, 0b111],
    [0b101, 0b101, 0b111, 0b001, 0b001],
    [0b111, 0b100, 0b111, 0b001, 0b111],
    [0b111, 0b100, 0b111, 0b101, 0b111],
    [0b111, 0b001, 0b001, 0b001, 0b001],
    [0b111, 0b101, 0b111, 0b101, 0b111],
    [0b111, 0b101, 0b111, 0b001, 0b111],
];

fn fill_rect(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, color: Color) {
    if let Some(rect) = tiny_skia::Rect::from_xywh(x, y, w, h) {
        let mut p = paint(color);
        // Pixel-font squares must stay crisp, so no smoothing on their edges.
        p.anti_alias = false;
        pm.fill_rect(rect, &p, Transform::identity(), None);
    }
}

/// The percentage as pixel-font digits centered horizontally, `cell` px per font pixel, top at `y0`.
fn draw_digits(pm: &mut Pixmap, pct: f32, cell: f32, y0: f32) {
    let size = SIZE as f32;
    let text = pct.round().clamp(0.0, 100.0).to_string();
    let gap = cell / 2.0;
    let width = text.len() as f32 * 3.0 * cell + (text.len() as f32 - 1.0) * gap;
    let x0 = (size - width) / 2.0;
    let color = tone(pct);
    for (n, ch) in text.chars().enumerate() {
        let glyph = DIGITS[ch.to_digit(10).unwrap_or(0) as usize];
        let gx = x0 + n as f32 * (3.0 * cell + gap);
        for (row, bits) in glyph.iter().enumerate() {
            for col in 0..3 {
                if bits & (0b100 >> col) != 0 {
                    fill_rect(
                        pm,
                        gx + col as f32 * cell,
                        y0 + row as f32 * cell,
                        cell,
                        cell,
                        color,
                    );
                }
            }
        }
    }
}

/// A horizontal gauge: a track from `x0` to `x1` with the used share filled in the threshold color.
fn draw_bar(pm: &mut Pixmap, pct: Option<f32>, (x0, x1): (f32, f32), y: f32, width: f32) {
    if let Some(p) = line(x0, x1, y) {
        pm.stroke_path(
            &p,
            &paint(track_color()),
            &stroke(width),
            Transform::identity(),
            None,
        );
    }
    if let Some(pct) = pct.filter(|p| *p > 0.0) {
        let end = x0 + (x1 - x0) * (pct / 100.0).clamp(0.0, 1.0);
        if let Some(p) = line(x0, end.max(x0 + 0.1), y) {
            pm.stroke_path(
                &p,
                &paint(tone(pct)),
                &stroke(width),
                Transform::identity(),
                None,
            );
        }
    }
}

/// Height of the strip a provider icon keeps for its gauge. A tray slot is a small square, so the mark
/// gets everything the gauge leaves.
fn gauge_height(style: &str) -> f32 {
    match style {
        // Digits are 5 font pixels tall at 2 px each.
        "number" => 10.0,
        _ => 7.0,
    }
}

/// Taskbar-neutral colors for marks that have no brand color of their own.
fn neutral_mark(dark_taskbar: bool) -> Color {
    if dark_taskbar {
        Color::from_rgba8(0xF2, 0xF2, 0xF2, 0xFF)
    } else {
        Color::from_rgba8(0x20, 0x20, 0x20, 0xFF)
    }
}

/// A provider's own icon: its mark on top and a compact gauge in the strip below.
///
/// The style picks the gauge: one bar for ring and pie (a circle does not fit the strip),
/// two thin bars for bars (5 h and weekly), and digits for number.
pub fn render_provider(style: &str, l: Levels, logo: &Logo, dark_taskbar: bool) -> Vec<u8> {
    let mut pm = Pixmap::new(SIZE, SIZE).expect("non-zero size");
    let mark = SIZE as f32 - gauge_height(style) - 1.0;
    pm.fill_path(
        logo.path(),
        &paint(logo.color.unwrap_or_else(|| neutral_mark(dark_taskbar))),
        FillRule::Winding,
        logo.fit(0.0, 0.0, SIZE as f32, mark),
        None,
    );
    match style {
        "bars" => {
            let span = (2.0, 30.0);
            draw_bar(&mut pm, l.session.or(l.worst), span, 26.5, 3.0);
            draw_bar(&mut pm, l.weekly.or(l.worst), span, 30.5, 3.0);
        }
        "number" => match l.worst {
            Some(pct) => draw_digits(&mut pm, pct, 2.0, 22.0),
            None => fill_rect(&mut pm, 8.0, 26.0, 16.0, 3.0, track_color()),
        },
        _ => draw_bar(&mut pm, l.worst, (3.5, 28.5), 28.0, 6.0),
    }
    unpremultiply(pm.data())
}

/// Returns `SIZE * SIZE * 4` RGBA bytes (straight alpha).
pub fn render(style: &str, l: Levels) -> Vec<u8> {
    let mut pm = Pixmap::new(SIZE, SIZE).expect("non-zero size");
    let c = SIZE as f32 / 2.0;
    match style {
        "bars" => {
            draw_bar(&mut pm, l.session.or(l.worst), (5.0, 27.0), 10.0, 7.0);
            draw_bar(&mut pm, l.weekly.or(l.worst), (5.0, 27.0), 22.0, 7.0);
        }
        "pie" => {
            if let Some(p) = sector_path(c, c, 14.0, 1.0) {
                pm.fill_path(
                    &p,
                    &paint(track_color()),
                    FillRule::Winding,
                    Transform::identity(),
                    None,
                );
            }
            if let Some(pct) = l.worst.filter(|p| *p > 0.0) {
                if let Some(p) = sector_path(c, c, 14.0, pct / 100.0) {
                    pm.fill_path(
                        &p,
                        &paint(tone(pct)),
                        FillRule::Winding,
                        Transform::identity(),
                        None,
                    );
                }
            }
        }
        "number" => match l.worst {
            // Three digits need a smaller cell to fit the 32 px canvas.
            Some(pct) => {
                let cell = if pct.round() >= 100.0 { 3.0 } else { 4.0 };
                draw_digits(&mut pm, pct, cell, (SIZE as f32 - 5.0 * cell) / 2.0)
            }
            None => fill_rect(&mut pm, 8.0, c - 2.0, 16.0, 4.0, track_color()),
        },
        _ => {
            if let Some(p) = arc_path(c, c, 12.0, 1.0) {
                pm.stroke_path(
                    &p,
                    &paint(track_color()),
                    &stroke(5.0),
                    Transform::identity(),
                    None,
                );
            }
            if let Some(pct) = l.worst.filter(|p| *p > 0.0) {
                if let Some(p) = arc_path(c, c, 12.0, pct / 100.0) {
                    pm.stroke_path(
                        &p,
                        &paint(tone(pct)),
                        &stroke(5.0),
                        Transform::identity(),
                        None,
                    );
                }
            }
        }
    }
    unpremultiply(pm.data())
}

/// tiny-skia stores premultiplied alpha; tray icons expect straight alpha.
fn unpremultiply(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    for px in data.chunks_exact(4) {
        let a = px[3] as u32;
        if a == 0 {
            out.extend_from_slice(&[0, 0, 0, 0]);
            continue;
        }
        for &ch in &px[..3] {
            let straight = (ch as u32 * 255 + a / 2).checked_div(a).unwrap_or(0);
            out.push(straight.min(255) as u8);
        }
        out.push(px[3]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alpha_sum(buf: &[u8]) -> u32 {
        buf.chunks_exact(4).map(|p| p[3] as u32).sum()
    }

    #[test]
    fn ring_has_right_size_and_grows_with_usage() {
        let low = render(
            "ring",
            Levels {
                worst: Some(10.0),
                ..Default::default()
            },
        );
        let high = render(
            "ring",
            Levels {
                worst: Some(90.0),
                ..Default::default()
            },
        );
        assert_eq!(low.len(), (SIZE * SIZE * 4) as usize);
        assert!(alpha_sum(&high) >= alpha_sum(&low));
    }

    #[test]
    fn tone_follows_thresholds() {
        assert_eq!(tone(50.0), Color::from_rgba8(0x3A, 0x96, 0xDD, 0xFF));
        assert_eq!(tone(80.0), Color::from_rgba8(0xF5, 0xA6, 0x23, 0xFF));
        assert_eq!(tone(95.0), Color::from_rgba8(0xE8, 0x4A, 0x3F, 0xFF));
    }

    #[test]
    fn bars_render_without_data() {
        let b = render("bars", Levels::default());
        assert!(alpha_sum(&b) > 0, "track must be drawn even without data");
    }

    #[test]
    fn every_style_draws_and_pie_grows_with_usage() {
        let at = |pct| Levels {
            worst: Some(pct),
            ..Default::default()
        };
        for style in ["ring", "bars", "pie", "number"] {
            let b = render(style, at(40.0));
            assert_eq!(b.len(), (SIZE * SIZE * 4) as usize, "{style}");
            assert!(alpha_sum(&b) > 0, "{style} draws something");
        }
        assert!(alpha_sum(&render("pie", at(90.0))) > alpha_sum(&render("pie", at(10.0))));
    }

    #[test]
    fn provider_icons_carry_the_mark_and_a_gauge() {
        let claude = crate::logos::logo("claude").unwrap();
        let cursor = crate::logos::logo("cursor").unwrap();
        let at = |pct| Levels {
            worst: Some(pct),
            session: Some(pct),
            weekly: Some(pct / 2.0),
        };
        for style in ["ring", "bars", "pie", "number"] {
            let with = render_provider(style, at(40.0), claude, true);
            assert_eq!(with.len(), (SIZE * SIZE * 4) as usize, "{style}");
            assert!(
                alpha_sum(&with)
                    > alpha_sum(&render_provider(style, Levels::default(), claude, true)) / 2,
                "{style} draws the mark with and without data"
            );
            assert_ne!(
                with,
                render_provider(style, at(40.0), cursor, true),
                "{style}: another provider looks different"
            );
        }
        assert!(
            alpha_sum(&render_provider("ring", at(90.0), claude, true))
                > alpha_sum(&render_provider("ring", at(10.0), claude, true)),
            "the gauge grows with usage"
        );
        assert_ne!(
            render_provider("ring", at(40.0), cursor, true),
            render_provider("ring", at(40.0), cursor, false),
            "a mark without a brand color follows the taskbar"
        );
        assert_eq!(
            render_provider("ring", at(40.0), claude, true),
            render_provider("ring", at(40.0), claude, false),
            "a brand color does not"
        );
    }

    #[test]
    fn number_shows_the_value_and_fits_three_digits() {
        let at = |pct| {
            render(
                "number",
                Levels {
                    worst: Some(pct),
                    ..Default::default()
                },
            )
        };
        assert_ne!(at(62.0), at(63.0), "different values look different");
        assert_ne!(at(8.0), at(100.0));
        assert!(alpha_sum(&at(100.0)) > 0, "three digits stay on the canvas");
        assert!(
            alpha_sum(&render("number", Levels::default())) > 0,
            "a dash stands in for missing data"
        );
    }
}
