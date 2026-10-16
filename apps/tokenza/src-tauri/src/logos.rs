//! Brand marks painted into the per-provider tray icons.
//!
//! The data is shared with the UI (`src/logos.json`), so a mark only ever has to be added once.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::Deserialize;
use svgtypes::{SimplePathSegment, SimplifyingPathParser};
use tiny_skia::{Color, Path, PathBuilder, Transform};

const DATA: &str = include_str!("../../src/logos.json");

#[derive(Deserialize)]
struct Raw {
    path: String,
    #[serde(rename = "viewBox")]
    view_box: Option<String>,
    transform: Option<String>,
    color: Option<String>,
}

pub struct Logo {
    path: Path,
    /// x, y, width, height of the canvas the mark was drawn on.
    view_box: [f32; 4],
    /// The mark's own color; `None` means it follows the taskbar's light or dark.
    pub color: Option<Color>,
}

impl Logo {
    /// Maps the mark's canvas onto the box (x, y, w, h), centered and with its proportions kept.
    pub fn fit(&self, x: f32, y: f32, w: f32, h: f32) -> Transform {
        let [vx, vy, vw, vh] = self.view_box;
        let scale = (w / vw).min(h / vh);
        Transform::from_row(
            scale,
            0.0,
            0.0,
            scale,
            x + (w - vw * scale) / 2.0 - vx * scale,
            y + (h - vh * scale) / 2.0 - vy * scale,
        )
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn numbers(s: &str) -> Vec<f32> {
    s.split(|c: char| !(c.is_ascii_digit() || matches!(c, '.' | '-')))
        .filter_map(|n| n.parse().ok())
        .collect()
}

fn build_path(d: &str) -> Option<Path> {
    let mut pb = PathBuilder::new();
    for seg in SimplifyingPathParser::from(d) {
        match seg.ok()? {
            SimplePathSegment::MoveTo { x, y } => pb.move_to(x as f32, y as f32),
            SimplePathSegment::LineTo { x, y } => pb.line_to(x as f32, y as f32),
            SimplePathSegment::CurveTo {
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            } => pb.cubic_to(
                x1 as f32, y1 as f32, x2 as f32, y2 as f32, x as f32, y as f32,
            ),
            SimplePathSegment::Quadratic { x1, y1, x, y } => {
                pb.quad_to(x1 as f32, y1 as f32, x as f32, y as f32)
            }
            SimplePathSegment::ClosePath => pb.close(),
        }
    }
    pb.finish()
}

fn parse_color(hex: &str) -> Option<Color> {
    let hex = hex.strip_prefix('#')?;
    let byte = |i: usize| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok();
    Some(Color::from_rgba8(byte(0)?, byte(2)?, byte(4)?, 0xFF))
}

fn parse(raw: Raw) -> Option<Logo> {
    let mut path = build_path(&raw.path)?;
    if let Some(m) = raw.transform.as_deref().map(numbers) {
        let [a, b, c, d, e, f] = m[..].try_into().ok()?;
        path = path.transform(Transform::from_row(a, b, c, d, e, f))?;
    }
    let view_box = match raw.view_box.as_deref().map(numbers) {
        Some(v) => v[..].try_into().ok()?,
        None => [0.0, 0.0, 24.0, 24.0],
    };
    Some(Logo {
        path,
        view_box,
        color: raw.color.as_deref().and_then(parse_color),
    })
}

/// The mark of a built-in provider; `None` for providers the user defined.
pub fn logo(id: &str) -> Option<&'static Logo> {
    static LOGOS: OnceLock<HashMap<String, Logo>> = OnceLock::new();
    LOGOS
        .get_or_init(|| {
            let raw: HashMap<String, Raw> = serde_json::from_str(DATA).unwrap_or_default();
            raw.into_iter()
                .filter_map(|(id, r)| Some((id, parse(r)?)))
                .collect()
        })
        .get(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::KNOWN_PROVIDERS;

    #[test]
    fn every_built_in_provider_has_a_mark_that_fits_its_canvas() {
        for id in KNOWN_PROVIDERS {
            let logo = logo(id).unwrap_or_else(|| panic!("{id} has no usable mark"));
            let [vx, vy, vw, vh] = logo.view_box;
            let b = logo.path.bounds();
            // A mark that misses its canvas would paint nothing, or something cropped.
            assert!(
                b.left() < vx + vw && b.right() > vx && b.top() < vy + vh && b.bottom() > vy,
                "{id}: bounds {b:?} outside {:?}",
                logo.view_box
            );
        }
    }

    #[test]
    fn custom_providers_have_no_mark() {
        assert!(logo("custom-acme").is_none());
    }

    #[test]
    fn fit_centers_and_keeps_proportions() {
        let l = logo("claude").unwrap();
        let t = l.fit(6.0, 1.0, 20.0, 20.0);
        assert!((t.sx - 20.0 / 24.0).abs() < 1e-5 && t.sx == t.sy);
        assert!((t.tx - 6.0).abs() < 1e-4 && (t.ty - 1.0).abs() < 1e-4);
    }
}
