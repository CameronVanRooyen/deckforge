//! Signed-distance-field label font (Geist, semibold, tabular figures) baked by `tools/bake_glyphs.swift`.

use std::sync::OnceLock;

const METRICS: &str = include_str!("../assets/glyphs.txt");
pub const ATLAS_PNG: &[u8] = include_bytes!("../assets/glyphs.png");

#[derive(Debug, Clone, Copy, Default)]
pub struct Glyph {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub bearing_x: f32,
    pub top: f32,
    pub advance: f32,
}

#[derive(Debug)]
pub struct Font {
    pub size: f32,
    pub spread: f32,
    pub ascent: f32,
    pub descent: f32,
    pub cap_height: f32,
    pub atlas_w: f32,
    pub atlas_h: f32,
    ascii: [Option<Glyph>; 95],
    extra: Vec<(char, Glyph)>,
}

impl Font {
    pub fn glyph(&self, c: char) -> Option<&Glyph> {
        let code = c as u32;
        if (32..127).contains(&code) {
            self.ascii[(code - 32) as usize].as_ref()
        } else {
            self.extra.iter().find(|(e, _)| *e == c).map(|(_, g)| g)
        }
    }

    /// Advance width of `text` at `size` pixels, including `tracking` (in em) between glyphs.
    pub fn measure(&self, text: &str, size: f64, tracking: f64) -> f64 {
        let k = size / f64::from(self.size);
        let mut w = 0.0;
        let mut n = 0;
        for c in text.chars() {
            if let Some(g) = self.glyph(c) {
                w += f64::from(g.advance) * k;
                n += 1;
            }
        }
        w + tracking * size * (n.max(1) - 1) as f64
    }

    /// Cap height in pixels at `size`.
    pub fn cap(&self, size: f64) -> f64 {
        f64::from(self.cap_height) * size / f64::from(self.size)
    }
}

fn parse() -> Font {
    let mut font = Font {
        size: 56.0,
        spread: 8.0,
        ascent: 0.0,
        descent: 0.0,
        cap_height: 0.0,
        atlas_w: 1.0,
        atlas_h: 1.0,
        ascii: [None; 95],
        extra: Vec::new(),
    };
    for line in METRICS.lines().filter(|l| !l.starts_with('#')) {
        let f: Vec<&str> = line.split_whitespace().collect();
        let num = |i: usize| f.get(i).and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
        if f.first() == Some(&"font") {
            (font.size, font.spread) = (num(1), num(2));
            (font.ascent, font.descent, font.cap_height) = (num(3), num(4), num(5));
            continue;
        }
        let Some(c) = f
            .first()
            .and_then(|v| v.parse::<u32>().ok())
            .and_then(char::from_u32)
        else {
            continue;
        };
        let g = Glyph {
            x: num(1),
            y: num(2),
            w: num(3),
            h: num(4),
            bearing_x: num(5),
            top: num(6),
            advance: num(7),
        };
        let code = c as u32;
        if (32..127).contains(&code) {
            font.ascii[(code - 32) as usize] = Some(g);
        } else {
            font.extra.push((c, g));
        }
    }
    let (w, h) = atlas_size();
    (font.atlas_w, font.atlas_h) = (w as f32, h as f32);
    font
}

fn atlas_size() -> (u32, u32) {
    png::Decoder::new(std::io::Cursor::new(ATLAS_PNG))
        .read_info()
        .map(|r| (r.info().width, r.info().height))
        .unwrap_or((1, 1))
}

pub fn font() -> &'static Font {
    static FONT: OnceLock<Font> = OnceLock::new();
    FONT.get_or_init(parse)
}

/// Decodes the atlas into tightly packed single-channel rows.
pub fn atlas_pixels() -> Option<(u32, u32, Vec<u8>)> {
    let mut reader = png::Decoder::new(std::io::Cursor::new(ATLAS_PNG))
        .read_info()
        .ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    if info.color_type != png::ColorType::Grayscale || info.bit_depth != png::BitDepth::Eight {
        return None;
    }
    buf.truncate(info.buffer_size());
    Some((info.width, info.height, buf))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_baked_font_covers_labels_and_has_tabular_digits() {
        let f = font();
        for c in "INTRO BUILD DROP BREAK OUTRO in 0123456789./·".chars() {
            assert!(f.glyph(c).is_some(), "missing {c:?}");
        }
        let advances: Vec<f32> = ('0'..='9').map(|c| f.glyph(c).unwrap().advance).collect();
        assert!(advances.windows(2).all(|w| (w[0] - w[1]).abs() < 1e-3));
        assert!(f.measure("DROP 16", 10.0, 0.0) > f.measure("DROP", 10.0, 0.0));
    }

    #[test]
    fn the_atlas_decodes_to_the_size_the_metrics_assume() {
        let (w, h, px) = atlas_pixels().expect("atlas");
        assert_eq!((w as f32, h as f32), (font().atlas_w, font().atlas_h));
        assert_eq!(px.len(), (w * h) as usize);
        let g = font().glyph('I').unwrap();
        let centre = px[((g.y + g.h * 0.5) as u32 * w + (g.x + g.w * 0.5) as u32) as usize];
        assert!(centre > 128, "the stem of I is inside the glyph");
    }
}
