//! Draws the menu bar clocks into an RGBA image.
//!
//! Rendering the text ourselves (instead of using the status item's plain title) is what
//! allows any installed font, any size and weight, a different style per region, stacked
//! layouts, custom colors, letter spacing and pixel-level vertical alignment.

use crate::clock::{Segment, SegmentStyle};
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
    /// True when everything uses the automatic color, so macOS can recolor it for
    /// light and dark menu bars ("template image").
    pub template: bool,
}

type Chain = Rc<Vec<Rc<FontVec>>>;

pub struct Renderer {
    db: fontdb::Database,
    cache: HashMap<(String, u16), Option<Rc<FontVec>>>,
    chains: HashMap<(String, u16), Chain>,
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
        Renderer { db, cache: HashMap::new(), chains: HashMap::new() }
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

    /// The configured font (or the system font) followed by fallbacks for missing characters.
    fn chain(&mut self, family: &str, weight: u16) -> Chain {
        let key = (family.trim().to_string(), weight);
        if let Some(c) = self.chains.get(&key) {
            return c.clone();
        }
        let mut primary = None;
        if !key.0.is_empty() {
            primary = self.load(&key.0, weight);
        }
        if primary.is_none() {
            primary = SYSTEM_FAMILIES.iter().find_map(|name| self.load(name, weight));
        }
        let primary = primary
            .or_else(|| self.generic(weight))
            .expect("no usable fonts are installed");
        let mut list = vec![primary];
        for name in FALLBACK_FAMILIES {
            if let Some(f) = self.load(name, weight) {
                list.push(f);
            }
        }
        let chain = Rc::new(list);
        self.chains.insert(key, chain.clone());
        chain
    }

    /// Renders the menu bar image. Returns `None` when no clock is pinned to the bar.
    ///
    /// `auto_color` is used for clocks without their own color when the image can't be a
    /// template (because some other clock has a custom color).
    pub fn render(&mut self, segments: &[Segment], c: &Config, auto_color: [u8; 3]) -> Option<Image> {
        if segments.is_empty() {
            return None;
        }
        let template = segments.iter().all(|s| s.style.color.is_none());
        let color_of = |style: &SegmentStyle| -> [f32; 3] {
            let rgb = if template { [0, 0, 0] } else { style.color.unwrap_or(auto_color) };
            rgb.map(|v| v as f32 / 255.0)
        };
        let dim_alpha = if c.dim_labels { c.dim_amount } else { 1.0 };
        let tracking = c.letter_spacing * SCALE;

        // Build columns of lines of styled runs.
        let mut columns: Vec<Vec<Vec<Run>>> = Vec::new();
        for seg in segments {
            let st = &seg.style;
            let fonts = self.chain(&st.font_family, st.font_weight);
            let rgb = color_of(st);
            let time_px = st.font_size * SCALE;
            let run = |text: String, px: f32, alpha: f32| Run { text, px, rgb, alpha, fonts: fonts.clone() };
            let col = match c.layout {
                Layout::Inline => {
                    let mut line = Vec::new();
                    if let Some(l) = &seg.label {
                        line.push(run(format!("{l} "), time_px, dim_alpha));
                    }
                    line.push(run(seg.time.clone(), time_px, 1.0));
                    if let Some(d) = &seg.date {
                        line.push(run(format!(" {d}"), time_px, dim_alpha));
                    }
                    vec![line]
                }
                Layout::Stacked => {
                    let top: Vec<&str> =
                        [seg.label.as_deref(), seg.date.as_deref()].into_iter().flatten().collect();
                    let mut lines = Vec::new();
                    if !top.is_empty() {
                        lines.push(vec![run(top.join(" · "), time_px * c.label_scale, dim_alpha)]);
                    }
                    lines.push(vec![run(seg.time.clone(), time_px, 1.0)]);
                    lines
                }
            };
            columns.push(col);
        }

        let ctx = Ctx { tabular: c.tabular_digits, tracking };

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
        let sep_run = if sep.is_empty() {
            None
        } else {
            let fonts = self.chain(&c.font_family, c.font_weight);
            let rgb = if template {
                [0.0; 3]
            } else if c.use_custom_color {
                c.color.map(|v| v as f32 / 255.0)
            } else {
                auto_color.map(|v| v as f32 / 255.0)
            };
            Some(Run { text: sep.to_string(), px: c.font_size * SCALE, rgb, alpha: dim_alpha, fonts })
        };
        let sep_w = sep_run.as_ref().map(|r| ctx.line_width(std::slice::from_ref(r))).unwrap_or(0.0);
        let gap = c.spacing * SCALE;
        let n = columns.len() as f32;
        let total_w: f32 = col_sizes.iter().map(|s| s.0).sum::<f32>()
            + (n - 1.0) * if sep_run.is_some() { 2.0 * gap + sep_w } else { gap };
        let content_h = col_sizes.iter().map(|s| s.1).fold(0.0, f32::max);
        let height = (BAR_HEIGHT_PT * SCALE).max(content_h.ceil()) as u32;
        let width = (total_w.ceil() as u32).max(1) + 2; // 1px breathing room each side
        let mut canvas = Canvas::new(width, height);
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
        Some(canvas.into_image(template))
    }
}

struct Run {
    text: String,
    px: f32,
    rgb: [f32; 3],
    alpha: f32,
    fonts: Chain,
}

impl Run {
    fn primary(&self) -> &FontVec {
        self.fonts[0].as_ref()
    }

    /// First font in the chain that has a glyph for `ch`.
    fn font_for(&self, ch: char) -> (&FontVec, GlyphId) {
        for f in self.fonts.iter() {
            let id = f.glyph_id(ch);
            if id.0 != 0 {
                return (f.as_ref(), id);
            }
        }
        (self.primary(), self.primary().glyph_id(ch))
    }
}

struct Ctx {
    tabular: bool,
    tracking: f32,
}

impl Ctx {
    /// The run with the largest text decides a line's metrics.
    fn tallest<'a>(&self, line: &'a [Run]) -> &'a Run {
        line.iter().max_by(|a, b| a.px.total_cmp(&b.px)).expect("empty line")
    }

    fn line_ascent(&self, line: &[Run]) -> f32 {
        let r = self.tallest(line);
        r.primary().as_scaled(PxScale::from(r.px)).ascent()
    }

    fn line_height(&self, line: &[Run]) -> f32 {
        let r = self.tallest(line);
        let s = r.primary().as_scaled(PxScale::from(r.px));
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
        let primary = run.primary().as_scaled(scale);
        let digit_w = ('0'..='9').map(|d| primary.h_advance(run.primary().glyph_id(d))).fold(0.0, f32::max);
        let mut x = x0;
        let mut prev: Option<(GlyphId, *const FontVec)> = None;
        for ch in run.text.chars() {
            let (font, id) = run.font_for(ch);
            let scaled = font.as_scaled(scale);
            let mut adv = scaled.h_advance(id);
            let mut shift = 0.0;
            if self.tabular && ch.is_ascii_digit() {
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
                            run.rgb,
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
    /// Premultiplied RGBA, 0…1.
    px: Vec<[f32; 4]>,
}

impl Canvas {
    fn new(w: u32, h: u32) -> Self {
        Canvas { w, h, px: vec![[0.0; 4]; (w * h) as usize] }
    }

    fn blend(&mut self, x: i32, y: i32, rgb: [f32; 3], a: f32) {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 {
            return;
        }
        let a = a.clamp(0.0, 1.0);
        let d = &mut self.px[(y as u32 * self.w + x as u32) as usize];
        for k in 0..3 {
            d[k] = rgb[k] * a + d[k] * (1.0 - a);
        }
        d[3] = a + d[3] * (1.0 - a);
    }

    fn into_image(self, template: bool) -> Image {
        let mut rgba = Vec::with_capacity(self.px.len() * 4);
        for [r, g, b, a] in self.px {
            let un = |v: f32| if a > 0.0 { ((v / a).clamp(0.0, 1.0) * 255.0).round() as u8 } else { 0 };
            rgba.extend_from_slice(&[un(r), un(g), un(b), (a * 255.0).round() as u8]);
        }
        Image { width: self.w, height: self.h, rgba, template }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(label: &str, time: &str, size: f32, color: Option<[u8; 3]>) -> Segment {
        Segment {
            label: Some(label.into()),
            time: time.into(),
            date: None,
            style: SegmentStyle { font_family: String::new(), font_weight: 400, font_size: size, color },
        }
    }

    #[test]
    fn renders_and_grows_with_size() {
        let mut r = Renderer::new();
        if r.db.is_empty() {
            return; // no fonts on this machine
        }
        let mut c = Config::default();
        let small = r.render(&[seg("NYC", "8:35 PM", 13.0, None), seg("TYO", "9:35 AM", 13.0, None)], &c, [0; 3]).unwrap();
        assert_eq!(small.height, (BAR_HEIGHT_PT * SCALE) as u32);
        assert!(small.template);
        assert!(small.rgba.chunks(4).any(|p| p[3] > 0), "image should not be blank");

        let big = r.render(&[seg("NYC", "8:35 PM", 13.0, None), seg("TYO", "9:35 AM", 20.0, None)], &c, [0; 3]).unwrap();
        assert!(big.width > small.width, "a larger region should widen the image");

        let colored = r.render(&[seg("NYC", "8:35 PM", 13.0, Some([255, 0, 0]))], &c, [0; 3]).unwrap();
        assert!(!colored.template);
        assert!(colored.rgba.chunks(4).any(|p| p[3] > 200 && p[0] > 200 && p[1] < 50));

        c.layout = Layout::Stacked;
        let stacked = r.render(&[seg("NYC", "8:35 PM", 13.0, None), seg("TYO", "9:35 AM", 20.0, None)], &c, [0; 3]).unwrap();
        assert!(stacked.width < big.width);
    }

    #[test]
    fn nothing_pinned_means_no_image() {
        let mut r = Renderer::new();
        assert!(r.render(&[], &Config::default(), [0; 3]).is_none());
    }
}
