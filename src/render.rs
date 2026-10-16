//! Draws the menu bar clocks into an RGBA image.
//!
//! Rendering the text ourselves (instead of using the status item's plain title) is what
//! allows any installed font, any size and weight, stacked layouts, custom colors,
//! letter spacing and pixel-level vertical alignment.

use crate::clock::Segment;
use crate::config::{Config, Layout};
use ab_glyph::{point, Font, FontVec, GlyphId, PxScale, ScaleFont, VariableFont};
use std::collections::HashMap;
use std::rc::Rc;

/// Height of the macOS menu bar image, in points (tray-icon scales taller images to this).
pub const BAR_HEIGHT_PT: f32 = 22.0;
/// Render at 2× so text is crisp on Retina displays.
pub const SCALE: f32 = 2.0;

/// Families tried, in order, when the configured font is empty or missing.
const SYSTEM_FAMILIES: &[&str] = &[
    "SF Pro", "SF Pro Text", "SF Pro Display", "System Font", ".SF NS", "Helvetica Neue",
    "Helvetica", "Arial", "Segoe UI", "DejaVu Sans", "Noto Sans",
];
/// Extra fonts used for characters the main font doesn't have (symbols, CJK, …).
const FALLBACK_FAMILIES: &[&str] = &[
    "Helvetica Neue", "Apple Symbols", "Hiragino Sans", "PingFang SC", "Apple SD Gothic Neo",
    "Arial Unicode MS", "DejaVu Sans", "Noto Sans", "Noto Sans CJK JP",
];

pub struct Image {
    pub width: u32,
    pub height: u32,
    /// Straight (non-premultiplied) RGBA.
    pub rgba: Vec<u8>,
}

pub struct Renderer {
    db: fontdb::Database,
    cache: HashMap<(String, u16), Option<Rc<FontVec>>>,
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}

impl Renderer {
    pub fn new() -> Self {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        Renderer { db, cache: HashMap::new() }
    }

    /// Installed font family names, sorted, without hidden (dot-prefixed) system families.
    pub fn families(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .db
            .faces()
            .filter_map(|f| f.families.first().map(|(name, _)| name.clone()))
            .filter(|n| !n.starts_with('.'))
            .collect();
        v.sort_by_key(|s| s.to_lowercase());
        v.dedup();
        v
    }

    fn load(&mut self, family: &str, weight: u16) -> Option<Rc<FontVec>> {
        let key = (family.to_string(), weight);
        if let Some(hit) = self.cache.get(&key) {
            return hit.clone();
        }
        let families = [fontdb::Family::Name(family)];
        let query = fontdb::Query {
            families: &families,
            weight: fontdb::Weight(weight),
            ..Default::default()
        };
        let font = self.db.query(&query).and_then(|id| {
            // Only accept an exact family match; fontdb never substitutes for named families.
            self.db.with_face_data(id, |data, index| {
                FontVec::try_from_vec_and_index(data.to_vec(), index).ok()
            })?
        });
        let font = font.map(|mut f| {
            // Variable fonts (like the macOS system font) get the exact weight.
            f.set_variation(b"wght", weight as f32);
            Rc::new(f)
        });
        self.cache.insert(key, font.clone());
        font
    }

    /// The configured font, or the system font.
    fn primary(&mut self, family: &str, weight: u16) -> Rc<FontVec> {
        if !family.trim().is_empty() {
            if let Some(f) = self.load(family.trim(), weight) {
                return f;
            }
        }
        for name in SYSTEM_FAMILIES {
            if let Some(f) = self.load(name, weight) {
                return f;
            }
        }
        self.generic(weight).expect("no usable fonts are installed")
    }

    fn generic(&mut self, weight: u16) -> Option<Rc<FontVec>> {
        let families = [fontdb::Family::SansSerif];
        let query = fontdb::Query {
            families: &families,
            weight: fontdb::Weight(weight),
            ..Default::default()
        };
        let id = self.db.query(&query).or_else(|| self.db.faces().next().map(|f| f.id))?;
        self.db
            .with_face_data(id, |data, index| FontVec::try_from_vec_and_index(data.to_vec(), index).ok())?
            .map(Rc::new)
    }

    fn fonts(&mut self, c: &Config) -> Vec<Rc<FontVec>> {
        let mut list = vec![self.primary(&c.font_family, c.font_weight)];
        for name in FALLBACK_FAMILIES {
            if let Some(f) = self.load(name, c.font_weight) {
                list.push(f);
            }
        }
        list
    }

    /// Renders the menu bar image. Returns `None` when no clock is pinned to the bar.
    pub fn render(&mut self, segments: &[Segment], c: &Config) -> Option<Image> {
        if segments.is_empty() {
            return None;
        }
        let fonts = self.fonts(c);
        let base: [f32; 3] = if c.use_custom_color {
            c.color.map(|v| v as f32 / 255.0)
        } else {
            [0.0, 0.0, 0.0] // used as a template image: macOS recolors it for light/dark
        };
        let dim_alpha = if c.dim_labels { c.dim_amount } else { 1.0 };
        let time_px = c.font_size * SCALE;
        let small_px = match c.layout {
            Layout::Inline => time_px,
            Layout::Stacked => time_px * c.label_scale,
        };
        let tracking = c.letter_spacing * SCALE;

        // Build columns of lines of styled runs.
        let columns: Vec<Vec<Vec<Run>>> = segments
            .iter()
            .map(|seg| match c.layout {
                Layout::Inline => {
                    let mut line = Vec::new();
                    if let Some(l) = &seg.label {
                        line.push(Run::new(format!("{l} "), time_px, dim_alpha));
                    }
                    line.push(Run::new(seg.time.clone(), time_px, 1.0));
                    if let Some(d) = &seg.date {
                        line.push(Run::new(format!(" {d}"), time_px, dim_alpha));
                    }
                    vec![line]
                }
                Layout::Stacked => {
                    let top: Vec<&str> =
                        [seg.label.as_deref(), seg.date.as_deref()].into_iter().flatten().collect();
                    let mut lines = Vec::new();
                    if !top.is_empty() {
                        lines.push(vec![Run::new(top.join(" · "), small_px, dim_alpha)]);
                    }
                    lines.push(vec![Run::new(seg.time.clone(), time_px, 1.0)]);
                    lines
                }
            })
            .collect();

        let ctx = Ctx { fonts: &fonts, tabular: c.tabular_digits, tracking };

        // Measure.
        let col_sizes: Vec<(f32, f32)> = columns
            .iter()
            .map(|lines| {
                let w = lines.iter().map(|l| ctx.line_width(l)).fold(0.0, f32::max);
                let h: f32 = lines.iter().map(|l| ctx.line_height(l)).sum();
                (w, h)
            })
            .collect();
        let sep = c.separator.trim();
        let sep_run = (!sep.is_empty()).then(|| Run::new(sep.to_string(), time_px, dim_alpha));
        let sep_w = sep_run.as_ref().map(|r| ctx.line_width(std::slice::from_ref(r))).unwrap_or(0.0);
        let gap = c.spacing * SCALE;
        let n = columns.len() as f32;
        let total_w: f32 = col_sizes.iter().map(|s| s.0).sum::<f32>()
            + (n - 1.0) * if sep_run.is_some() { 2.0 * gap + sep_w } else { gap };
        let content_h = col_sizes.iter().map(|s| s.1).fold(0.0, f32::max);
        let height = (BAR_HEIGHT_PT * SCALE).max(content_h.ceil()) as u32;
        let width = (total_w.ceil() as u32).max(1) + 2; // 1px breathing room each side
        let mut canvas = Canvas::new(width, height, base);
        let offset = -c.vertical_offset * SCALE; // positive config value moves text up

        // Draw.
        let mut x = 1.0;
        for (i, lines) in columns.iter().enumerate() {
            let (col_w, col_h) = col_sizes[i];
            let mut top = (height as f32 - col_h) / 2.0 + offset;
            for line in lines {
                let lw = ctx.line_width(line);
                let lh = ctx.line_height(line);
                let baseline = top + ctx.line_ascent(line);
                ctx.draw_line(&mut canvas, line, x + (col_w - lw) / 2.0, baseline);
                top += lh;
            }
            x += col_w;
            if i + 1 < columns.len() {
                match &sep_run {
                    Some(r) => {
                        x += gap;
                        let line = std::slice::from_ref(r);
                        let baseline = (height as f32 - ctx.line_height(line)) / 2.0
                            + ctx.line_ascent(line)
                            + offset;
                        ctx.draw_line(&mut canvas, line, x, baseline);
                        x += sep_w + gap;
                    }
                    None => x += gap,
                }
            }
        }
        Some(canvas.into_image())
    }
}

struct Run {
    text: String,
    px: f32,
    alpha: f32,
}

impl Run {
    fn new(text: String, px: f32, alpha: f32) -> Self {
        Run { text, px, alpha }
    }
}

struct Ctx<'a> {
    fonts: &'a [Rc<FontVec>],
    tabular: bool,
    tracking: f32,
}

impl Ctx<'_> {
    fn primary(&self) -> &FontVec {
        self.fonts[0].as_ref()
    }

    /// First font that has a glyph for `ch`.
    fn font_for(&self, ch: char) -> (&FontVec, GlyphId) {
        for f in self.fonts {
            let id = f.glyph_id(ch);
            if id.0 != 0 {
                return (f.as_ref(), id);
            }
        }
        (self.primary(), self.primary().glyph_id(ch))
    }

    fn digit_width(&self, px: f32) -> f32 {
        let scaled = self.primary().as_scaled(PxScale::from(px));
        ('0'..='9')
            .map(|d| scaled.h_advance(self.primary().glyph_id(d)))
            .fold(0.0, f32::max)
    }

    fn line_ascent(&self, line: &[Run]) -> f32 {
        let px = line.iter().map(|r| r.px).fold(0.0, f32::max);
        self.primary().as_scaled(PxScale::from(px)).ascent()
    }

    fn line_height(&self, line: &[Run]) -> f32 {
        let px = line.iter().map(|r| r.px).fold(0.0, f32::max);
        let s = self.primary().as_scaled(PxScale::from(px));
        s.ascent() - s.descent()
    }

    fn line_width(&self, line: &[Run]) -> f32 {
        line.iter().map(|r| self.layout_run(r, 0.0, 0.0, None)).sum()
    }

    fn draw_line(&self, canvas: &mut Canvas, line: &[Run], mut x: f32, baseline: f32) {
        for r in line {
            x += self.layout_run(r, x, baseline, Some(canvas));
        }
    }

    /// Lays out (and optionally draws) one run, returning its advance width.
    fn layout_run(&self, run: &Run, x0: f32, baseline: f32, mut canvas: Option<&mut Canvas>) -> f32 {
        let scale = PxScale::from(run.px);
        let digit_w = self.digit_width(run.px);
        let mut x = x0;
        let mut prev: Option<(GlyphId, *const FontVec)> = None;
        for ch in run.text.chars() {
            let (font, id) = self.font_for(ch);
            let scaled = font.as_scaled(scale);
            let mut adv = scaled.h_advance(id);
            let mut shift = 0.0;
            let tab = self.tabular && ch.is_ascii_digit();
            if tab {
                shift = (digit_w - adv) / 2.0;
                adv = digit_w;
            } else if let Some((p, pf)) = prev {
                if std::ptr::eq(pf, font) {
                    x += scaled.kern(p, id);
                }
            }
            if let Some(canvas) = canvas.as_deref_mut() {
                let glyph = id.with_scale_and_position(scale, point(x + shift, baseline));
                if let Some(outline) = font.outline_glyph(glyph) {
                    let b = outline.px_bounds();
                    outline.draw(|gx, gy, cov| {
                        canvas.blend(
                            b.min.x as i32 + gx as i32,
                            b.min.y as i32 + gy as i32,
                            cov * run.alpha,
                        );
                    });
                }
            }
            x += adv + self.tracking;
            prev = Some((id, font as *const FontVec));
        }
        x - x0
    }
}

struct Canvas {
    w: u32,
    h: u32,
    color: [f32; 3],
    /// Coverage per pixel (0…1); the color is uniform, so alpha is all we need to track.
    alpha: Vec<f32>,
}

impl Canvas {
    fn new(w: u32, h: u32, color: [f32; 3]) -> Self {
        Canvas { w, h, color, alpha: vec![0.0; (w * h) as usize] }
    }

    fn blend(&mut self, x: i32, y: i32, a: f32) {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 {
            return;
        }
        let i = (y as u32 * self.w + x as u32) as usize;
        let d = self.alpha[i];
        self.alpha[i] = a.clamp(0.0, 1.0) + d * (1.0 - a.clamp(0.0, 1.0));
    }

    fn into_image(self) -> Image {
        let [r, g, b] = self.color.map(|v| (v * 255.0).round() as u8);
        let mut rgba = Vec::with_capacity(self.alpha.len() * 4);
        for a in self.alpha {
            rgba.extend_from_slice(&[r, g, b, (a * 255.0).round() as u8]);
        }
        Image { width: self.w, height: self.h, rgba }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::Segment;

    fn seg(label: &str, time: &str) -> Segment {
        Segment { label: Some(label.into()), time: time.into(), date: None }
    }

    #[test]
    fn renders_something_and_grows_with_size() {
        let mut r = Renderer::new();
        if r.db.is_empty() {
            return; // no fonts on this machine (minimal CI container)
        }
        let segs = vec![seg("NYC", "8:35 PM"), seg("TYO", "9:35 AM")];
        let mut c = Config::default();
        let small = r.render(&segs, &c).unwrap();
        assert_eq!(small.height, (BAR_HEIGHT_PT * SCALE) as u32);
        assert!(small.rgba.chunks(4).any(|p| p[3] > 0), "image should not be blank");
        c.font_size = 20.0;
        let big = r.render(&segs, &c).unwrap();
        assert!(big.width > small.width);
        c.layout = Layout::Stacked;
        let stacked = r.render(&segs, &c).unwrap();
        assert!(stacked.width < big.width);
    }

    #[test]
    fn nothing_pinned_means_no_image() {
        let mut r = Renderer::new();
        assert!(r.render(&[], &Config::default()).is_none());
    }
}
