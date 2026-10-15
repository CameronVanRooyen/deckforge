use bytemuck::{Pod, Zeroable};

use crate::bars::{LineKind, grid_lines};
use crate::color::{Rgba, rgb_u32, with_alpha};
use crate::layout::PixelRect;
use crate::phrase_nav::{self, BarLevel};
use crate::style::Style;
use crate::text;
use crate::waveform::{MAX_LEVELS, Pyramid};

pub const HOT_CUE_SLOTS: usize = 8;
pub const DECKS: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SurfaceKind {
    Scroll,
    Overview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SurfaceId {
    pub kind: SurfaceKind,
    pub deck: usize,
}

impl SurfaceId {
    pub const ALL: [SurfaceId; 4] = [
        SurfaceId {
            kind: SurfaceKind::Scroll,
            deck: 0,
        },
        SurfaceId {
            kind: SurfaceKind::Scroll,
            deck: 1,
        },
        SurfaceId {
            kind: SurfaceKind::Overview,
            deck: 0,
        },
        SurfaceId {
            kind: SurfaceKind::Overview,
            deck: 1,
        },
    ];

    /// Parses `scroll-a`, `scroll-b`, `overview-a` and `overview-b`.
    pub fn parse(name: &str) -> Option<Self> {
        let (kind, deck) = name.split_once('-')?;
        let kind = match kind {
            "scroll" => SurfaceKind::Scroll,
            "overview" => SurfaceKind::Overview,
            _ => return None,
        };
        let deck = match deck {
            "a" => 0,
            "b" => 1,
            _ => return None,
        };
        Some(Self { kind, deck })
    }

    pub fn name(&self) -> &'static str {
        match (self.kind, self.deck) {
            (SurfaceKind::Scroll, 0) => "scroll-a",
            (SurfaceKind::Scroll, _) => "scroll-b",
            (SurfaceKind::Overview, 0) => "overview-a",
            (SurfaceKind::Overview, _) => "overview-b",
        }
    }

    pub fn index(&self) -> usize {
        Self::ALL
            .iter()
            .position(|s| s == self)
            .expect("all variants listed")
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlacedSurface {
    pub id: SurfaceId,
    pub rect: PixelRect,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HotCueView {
    pub secs: f64,
    pub rgb: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridView {
    pub bpm: f64,
    pub first_beat_secs: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoopView {
    pub in_secs: f64,
    pub out_secs: f64,
    pub active: bool,
}

/// Everything the renderer needs to know about one deck for a single frame.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DeckView {
    pub loaded: bool,
    pub token: u64,
    pub pos_secs: f64,
    pub duration_secs: f64,
    pub playing: bool,
    pub tempo_ratio: f64,
    pub grid: Option<GridView>,
    pub cue_secs: f64,
    pub hot_cues: [Option<HotCueView>; HOT_CUE_SLOTS],
    pub loop_region: Option<LoopView>,
    pub master: bool,
    pub beatmatch_view: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhraseKind {
    Intro,
    Build,
    Drop,
    Break,
    Outro,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhraseView {
    pub start_secs: f64,
    pub end_secs: f64,
    pub kind: PhraseKind,
    pub energy: f32,
    /// Start of the fill-in that leads out of this phrase into the next one.
    pub fill_start_secs: Option<f64>,
}

/// Per-frame inputs shared by all surfaces.
#[derive(Debug, Clone)]
pub struct FrameInput<'a> {
    pub size: (u32, u32),
    pub scale: f64,
    pub surfaces: &'a [PlacedSurface],
    pub decks: &'a [DeckView; DECKS],
    pub zoom_css_pps: [f64; DECKS],
    pub hover_px: [Option<f64>; DECKS],
    pub anim_secs: f64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct QuadInst {
    pub rect: [f32; 4],
    pub color: Rgba,
    pub kind: [f32; 4],
    pub uv: [f32; 4],
}

const KIND_RECT: f32 = 0.0;
const KIND_TRI_DOWN: f32 = 1.0;
const KIND_TRI_UP: f32 = 2.0;
const KIND_ROUND: f32 = 4.0;
const KIND_HATCH: f32 = 5.0;
const KIND_GLYPH: f32 = 6.0;

#[derive(Default)]
pub struct QuadList {
    pub items: Vec<QuadInst>,
}

impl QuadList {
    fn push(&mut self, rect: [f64; 4], color: Rgba, kind: [f32; 4]) {
        self.items.push(QuadInst {
            rect: rect.map(|v| v as f32),
            color,
            kind,
            uv: [0.0; 4],
        });
    }

    pub fn rect(&mut self, x: f64, y: f64, w: f64, h: f64, color: Rgba) {
        if w > 0.0 && h > 0.0 && color[3] > 0.0 {
            self.push([x, y, w, h], color, [KIND_RECT, 0.0, 0.0, 0.0]);
        }
    }

    pub fn round_rect(&mut self, x: f64, y: f64, w: f64, h: f64, radius: f64, color: Rgba) {
        if w > 0.0 && h > 0.0 && color[3] > 0.0 {
            self.push([x, y, w, h], color, [KIND_ROUND, radius as f32, 0.0, 0.0]);
        }
    }

    /// Diagonal stripes whose phase is pinned to `anchor_x`, so they travel with the waveform.
    #[allow(clippy::too_many_arguments)]
    pub fn hatch(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        period: f64,
        anchor_x: f64,
        color: Rgba,
    ) {
        if w > 0.0 && h > 0.0 && color[3] > 0.0 {
            self.push(
                [x, y, w, h],
                color,
                [KIND_HATCH, period as f32, anchor_x as f32, 0.0],
            );
        }
    }

    pub fn triangle(&mut self, x: f64, y: f64, w: f64, h: f64, down: bool, color: Rgba) {
        let kind = if down { KIND_TRI_DOWN } else { KIND_TRI_UP };
        self.push([x, y, w, h], color, [kind, 0.0, 0.0, 0.0]);
    }

    /// Draws `text` with its baseline at `baseline`, starting at `x`; returns the advance width.
    #[allow(clippy::too_many_arguments)]
    pub fn text(
        &mut self,
        x: f64,
        baseline: f64,
        size: f64,
        tracking: f64,
        text: &str,
        color: Rgba,
    ) -> f64 {
        let font = text::font();
        let k = size / f64::from(font.size);
        let px_per_unit = (2.0 * f64::from(font.spread) * k) as f32;
        let bias = if size < 22.0 { 0.12 } else { 0.0 };
        let mut pen = x;
        for c in text.chars() {
            let Some(g) = font.glyph(c) else { continue };
            if g.w > 0.0 {
                self.items.push(QuadInst {
                    rect: [
                        (pen + f64::from(g.bearing_x) * k) as f32,
                        (baseline - f64::from(g.top) * k) as f32,
                        (f64::from(g.w) * k) as f32,
                        (f64::from(g.h) * k) as f32,
                    ],
                    color,
                    kind: [KIND_GLYPH, px_per_unit, bias, 0.0],
                    uv: [
                        g.x / font.atlas_w,
                        g.y / font.atlas_h,
                        g.w / font.atlas_w,
                        g.h / font.atlas_h,
                    ],
                });
            }
            pen += f64::from(g.advance) * k + tracking * size;
        }
        pen - x
            - if text.is_empty() {
                0.0
            } else {
                tracking * size
            }
    }

    /// Draws a decimal number left-aligned at `x` with its glyphs' tops at `y`; returns the width used.
    pub fn number(&mut self, x: f64, y: f64, height: f64, value: u32, color: Rgba) -> f64 {
        let size = height / text::font().cap(1.0);
        let mut buf = [0u8; 12];
        self.text(
            x,
            y + height,
            size,
            0.0,
            fmt_int(&mut buf, i64::from(value)),
            color,
        )
    }
}

/// Formats an integer into `buf` without allocating.
pub fn fmt_int(buf: &mut [u8; 12], value: i64) -> &str {
    let mut n = value.unsigned_abs();
    let mut i = buf.len();
    loop {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    if value < 0 {
        i -= 1;
        buf[i] = b'-';
    }
    std::str::from_utf8(&buf[i..]).unwrap_or("")
}

pub fn phrase_color(style: &Style, kind: PhraseKind) -> Rgba {
    style.phrases[match kind {
        PhraseKind::Intro => 0,
        PhraseKind::Build => 1,
        PhraseKind::Drop => 2,
        PhraseKind::Break => 3,
        PhraseKind::Outro => 4,
    }]
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct WaveUniform {
    pub rect: [f32; 4],
    pub c0: [f32; 4],
    pub c1: [f32; 4],
    pub c3: [f32; 4],
    pub bg: Rgba,
    pub ghost: Rgba,
    pub tint: Rgba,
    pub low: [Rgba; 3],
    pub mid: [Rgba; 3],
    pub high: [Rgba; 3],
    pub levels: [[u32; 4]; MAX_LEVELS],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct QuadUniform {
    pub size: [f32; 2],
    pub pad: [f32; 2],
}

impl DeckView {
    /// Playback rate used to convert track time into wall-clock distance on the scrolling view.
    pub fn rate(&self) -> f64 {
        if self.tempo_ratio > 0.05 {
            self.tempo_ratio
        } else {
            1.0
        }
    }
}

/// Device pixels per track second on a scrolling view: wall-clock scaled so equal effective BPMs share beat spacing.
pub fn scroll_pps(zoom_css_pps: f64, scale: f64, rate: f64) -> f64 {
    zoom_css_pps * scale / rate
}

pub fn overview_band_heights(has_phrases: bool, h: f64, scale: f64) -> (f64, f64, f64) {
    let band = if has_phrases {
        (12.0 * scale).round()
    } else {
        0.0
    };
    let gap = if has_phrases {
        (2.0 * scale).round()
    } else {
        0.0
    };
    ((h - band - gap).max(1.0), gap, band)
}

pub fn wave_uniform(
    surface: &PlacedSurface,
    deck: &DeckView,
    pyramid: Option<&Pyramid>,
    phrases: &[PhraseView],
    style: &Style,
    scale: f64,
    zoom_css_pps: f64,
) -> WaveUniform {
    let has_phrases = !phrases.is_empty() && style.overview_phrase_band;
    let r = surface.rect;
    let mut u = WaveUniform::zeroed();
    u.rect = [r.x as f32, r.y as f32, r.w as f32, r.h as f32];
    u.bg = style.bg;
    u.ghost = style.ghost;
    u.tint = style.played_tint;
    u.low = style.low;
    u.mid = style.mid;
    u.high = style.high;
    u.c3[1] = scale as f32;
    let drawable = deck.loaded && pyramid.is_some();
    u.c1[0] = if drawable { 1.0 } else { 0.0 };
    u.c1[3] = if style.sub_pixel { 1.0 } else { 0.0 };
    if let Some(p) = pyramid {
        u.c0[3] = p.points_per_sec as f32;
        u.c3[2] = p.levels.len() as f32;
        u.c3[3] = p.len as f32;
        for (dst, l) in u.levels.iter_mut().zip(&p.levels) {
            *dst = [l.row_offset, l.len, 0, 0];
        }
    }
    match surface.id.kind {
        SurfaceKind::Scroll => {
            u.c3[0] = scroll_band_height(!phrases.is_empty(), scale) as f32;
            let pps_dev = scroll_pps(zoom_css_pps, scale, deck.rate());
            let pos_px = deck.pos_secs * pps_dev;
            let pos_i = pos_px.floor();
            u.c0[0] = pos_i as f32;
            u.c0[1] = (pos_px - pos_i) as f32;
            u.c0[2] = pps_dev as f32;
        }
        SurfaceKind::Overview => {
            let (wave_h, _, _) = overview_band_heights(has_phrases, r.h as f64, scale);
            u.c1[1] = 1.0;
            u.c1[2] = wave_h as f32;
            if let Some(p) = pyramid {
                u.c3[3] = (deck.duration_secs.max(0.001) * p.points_per_sec) as f32;
            }
        }
    }
    u
}

fn x_of(deck_pos: f64, pps: f64, cx: f64, t: f64) -> f64 {
    cx + (t - deck_pos) * pps
}

/// Where the other deck's next phrase start lands on this deck's scrolling view if both keep playing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ghost {
    pub deck: usize,
    pub wall_secs: f64,
    pub kind: PhraseKind,
    pub aligned: bool,
}

/// The ghost marker for `deck`, shown only on the master deck while the other deck has a phrase ahead.
pub fn ghost_for(
    decks: &[DeckView; DECKS],
    phrases: [&[PhraseView]; DECKS],
    deck: usize,
) -> Option<Ghost> {
    let own = &decks[deck];
    let other = 1 - deck.min(1);
    let od = &decks[other];
    if !own.master || !own.loaded || !od.loaded {
        return None;
    }
    let (next, wall) = phrase_nav::wall_secs_to_next(phrases[other], od.pos_secs, od.rate())?;
    let aligned = match (
        own.grid.filter(|g| g.bpm > 0.0),
        phrase_nav::wall_secs_to_next(phrases[deck], own.pos_secs, own.rate()),
    ) {
        (Some(g), Some((_, own_wall))) => {
            (own_wall - wall).abs() < 0.25 * 60.0 / (g.bpm * own.rate())
        }
        _ => false,
    };
    Some(Ghost {
        deck: other,
        wall_secs: wall,
        kind: phrases[other][next].kind,
        aligned,
    })
}

/// Everything geometry builders need to know about one surface.
pub struct SurfaceCtx<'a> {
    pub w: f64,
    pub h: f64,
    pub deck: &'a DeckView,
    pub deck_index: usize,
    pub phrases: &'a [PhraseView],
    pub style: &'a Style,
    pub scale: f64,
    pub ghost: Option<Ghost>,
}

const BAND_CSS: f64 = 16.0;
const TAB_TEXT_CSS: f64 = 10.0;
const LABEL_TEXT_CSS: f64 = 9.5;
const NAME_TRACKING: f64 = 0.06;

pub fn phrase_name(kind: PhraseKind) -> &'static str {
    match kind {
        PhraseKind::Intro => "INTRO",
        PhraseKind::Build => "BUILD",
        PhraseKind::Drop => "DROP",
        PhraseKind::Break => "BREAK",
        PhraseKind::Outro => "OUTRO",
    }
}

/// Height of the phrase band along the top of a scrolling view, in device pixels.
pub fn scroll_band_height(has_phrases: bool, scale: f64) -> f64 {
    if has_phrases {
        (BAND_CSS * scale).round()
    } else {
        0.0
    }
}

#[allow(clippy::too_many_arguments)]
fn line_quad(
    out: &mut QuadList,
    x: f64,
    width_css: f64,
    scale: f64,
    y: f64,
    h: f64,
    color: Rgba,
    snap: bool,
) -> f64 {
    let width = (width_css * scale).round().max(1.0);
    let left = x - width * 0.5;
    let left = if snap { (left + 0.5).floor() } else { left };
    out.rect(left, y, width, h, color);
    left
}

struct LevelLook {
    width_css: f64,
    color: Rgba,
    marker: Rgba,
}

fn level_look(style: &Style, level: BarLevel, deck: Rgba, phrase: Rgba) -> LevelLook {
    match level {
        BarLevel::Phrase => LevelLook {
            width_css: 3.0,
            color: phrase,
            marker: phrase,
        },
        BarLevel::Sixteen => LevelLook {
            width_css: 2.0,
            color: with_alpha(deck, 0.95),
            marker: deck,
        },
        BarLevel::Eight => LevelLook {
            width_css: 1.5,
            color: with_alpha(deck, 0.68),
            marker: with_alpha(deck, 0.85),
        },
        BarLevel::Four => LevelLook {
            width_css: 1.5,
            color: style.grid_four,
            marker: style.grid_four,
        },
        BarLevel::Bar => LevelLook {
            width_css: 1.0,
            color: style.grid_bar,
            marker: style.grid_marker,
        },
    }
}

const BEAT_ONE: Rgba = [250.0 / 255.0, 204.0 / 255.0, 21.0 / 255.0, 1.0];

/// Beat ticks and the phrase-anchored bar hierarchy (bar, 4, 8, 16, phrase) with `n/len` labels.
fn grid_quads(out: &mut QuadList, ctx: &SurfaceCtx<'_>, pps: f64, pos: f64, cx: f64, top: f64) {
    let SurfaceCtx {
        w,
        h,
        deck,
        deck_index,
        phrases,
        style,
        scale,
        ..
    } = *ctx;
    let Some(grid) = deck.grid.filter(|g| g.bpm > 0.0) else {
        return;
    };
    let beat_px = 60.0 / grid.bpm * pps;
    let bar_px = beat_px * 4.0;
    let margin = 40.0 * scale;
    let lines = grid_lines(
        phrases,
        grid,
        pos - (cx + margin) / pps,
        pos + (w - cx + margin) / pps,
    );
    let snap = !style.sub_pixel;
    let deck_color = style.deck[deck_index.min(1)];
    let visible = |level: BarLevel| match level {
        BarLevel::Bar => bar_px >= 10.0 * scale,
        BarLevel::Four => bar_px * 4.0 >= 12.0 * scale,
        BarLevel::Eight => bar_px * 8.0 >= 12.0 * scale,
        _ => true,
    };
    let labelled = |level: BarLevel| {
        let span = match level {
            BarLevel::Bar => 1.0,
            BarLevel::Four => 4.0,
            BarLevel::Eight => 8.0,
            BarLevel::Sixteen => 16.0,
            BarLevel::Phrase => return false,
        };
        span * bar_px >= if span > 1.0 { 46.0 } else { 88.0 } * scale
    };
    let x_of = |secs: f64| x_of(pos, pps, cx, secs);
    let wave_h = h - top;
    let tick = (wave_h * 0.11).round().max(3.0 * scale);

    if beat_px >= 6.0 * scale {
        let faint = with_alpha(style.grid_beat, style.grid_beat[3] * 0.3);
        for line in lines.iter().filter(|l| l.kind == LineKind::Beat) {
            let x = x_of(line.secs);
            if beat_px >= 22.0 * scale {
                line_quad(
                    out,
                    x,
                    1.0,
                    scale,
                    top + tick,
                    wave_h - 2.0 * tick,
                    faint,
                    snap,
                );
            }
            let beat_color = if deck.beatmatch_view { faint } else { style.grid_beat };
            line_quad(out, x, 1.0, scale, top, tick, beat_color, snap);
            line_quad(out, x, 1.0, scale, h - tick, tick, beat_color, snap);
        }
    }

    let tri_w = 6.0 * scale;
    let tri_h = 4.5 * scale;
    let label_size = LABEL_TEXT_CSS * scale;
    let mut buf = [0u8; 12];
    for line in lines.iter().filter(|l| l.kind != LineKind::Beat) {
        let mark = phrase_nav::classify(line, phrases, grid);
        if !visible(mark.level) {
            continue;
        }
        let x = x_of(line.secs);
        if deck.beatmatch_view {
            let phrase_start = mark.level == BarLevel::Phrase;
            line_quad(
                out,
                x,
                if phrase_start { 7.0 } else { 4.5 },
                scale,
                top,
                wave_h,
                style.grid_shadow,
                snap,
            );
            line_quad(
                out,
                x,
                if phrase_start { 3.5 } else { 1.75 },
                scale,
                top,
                wave_h,
                BEAT_ONE,
                snap,
            );
            if phrase_start {
                let s = 8.0 * scale;
                let t = 5.0 * scale;
                out.triangle(x - s * 0.5, top, s, t, true, BEAT_ONE);
                out.triangle(x - s * 0.5, h - t, s, t, false, BEAT_ONE);
            }
            continue;
        }
        let phrase = mark
            .phrase
            .and_then(|i| phrases.get(i))
            .map_or(deck_color, |p| phrase_color(style, p.kind));
        let look = level_look(style, mark.level, deck_color, phrase);
        let left = line_quad(out, x, look.width_css, scale, top, wave_h, look.color, snap);
        let width = (look.width_css * scale).round().max(1.0);
        if mark.level == BarLevel::Bar {
            out.rect(
                left + width,
                top,
                scale.round().max(1.0),
                wave_h,
                style.grid_shadow,
            );
        }
        let cx_line = left + width * 0.5;
        if mark.level != BarLevel::Phrase && bar_px >= 14.0 * scale {
            out.triangle(cx_line - tri_w * 0.5, top, tri_w, tri_h, true, look.marker);
            out.triangle(
                cx_line - tri_w * 0.5,
                h - tri_h,
                tri_w,
                tri_h,
                false,
                look.marker,
            );
        }
        if labelled(mark.level) && mark.bar_in_phrase >= 1 {
            let lx = left + width + 4.0 * scale;
            let baseline = (h - tick - 5.0 * scale).round();
            let strong = with_alpha(style.bar_number, 0.95);
            let n = fmt_int(&mut buf, mark.bar_in_phrase);
            let used = out.text(
                lx,
                baseline,
                label_size,
                0.0,
                n,
                if mark.level > BarLevel::Bar {
                    strong
                } else {
                    style.bar_number
                },
            );
            if mark.phrase_bars > 0 {
                let mut lbuf = [0u8; 12];
                let len = fmt_int(&mut lbuf, mark.phrase_bars);
                let slash = out.text(
                    lx + used,
                    baseline,
                    label_size,
                    0.0,
                    "/",
                    with_alpha(style.bar_number, 0.45),
                );
                out.text(
                    lx + used + slash,
                    baseline,
                    label_size,
                    0.0,
                    len,
                    with_alpha(style.bar_number, 0.45),
                );
            }
        }
    }
}

fn played(color: Rgba, past: bool) -> Rgba {
    if past {
        with_alpha(color, color[3] * 0.55)
    } else {
        color
    }
}

/// Phrase band: per-phrase rails, sticky `DROP 16` tabs, odd-length hatching, last-bar run-up and progress.
fn phrase_band(out: &mut QuadList, ctx: &SurfaceCtx<'_>, pps: f64, pos: f64, cx: f64, band: f64) {
    let SurfaceCtx {
        w,
        h,
        deck,
        phrases,
        style,
        scale,
        ..
    } = *ctx;
    let xof = |t: f64| x_of(pos, pps, cx, t);
    let grid = deck.grid.filter(|g| g.bpm > 0.0);
    let bar = grid.map(phrase_nav::bar_secs);
    let hatch = 6.0 * scale;
    let edge = 1.5 * scale;

    for (i, p) in phrases.iter().enumerate() {
        let (x1, x2) = (xof(p.start_secs) - edge, xof(p.end_secs) - edge);
        if x2 < 0.0 || x1 > w {
            continue;
        }
        let color = phrase_color(style, p.kind);
        let past = p.end_secs <= pos;
        out.rect(
            x1,
            0.0,
            x2 - x1 - scale,
            band,
            played(with_alpha(color, 0.17), past),
        );
        out.rect(
            x1,
            band - 2.0 * scale,
            x2 - x1 - scale,
            2.0 * scale,
            played(with_alpha(color, 0.75), past),
        );
        if let (Some(g), Some(bar)) = (grid, bar) {
            let (len, whole) = phrase_nav::phrase_bars(p, g);
            if !whole || len % 4 != 0 {
                let tail = p.start_secs + (len / 4 * 4) as f64 * bar;
                let tx = xof(tail.min(p.end_secs)) - edge;
                out.hatch(
                    tx,
                    0.0,
                    x2 - tx - scale,
                    band,
                    hatch,
                    xof(p.start_secs),
                    with_alpha(color, 0.5),
                );
            }
            if i > 0 {
                let prev = &phrases[i - 1];
                let run = xof((p.start_secs - bar).max(prev.start_secs)) - edge;
                let fill = prev
                    .fill_start_secs
                    .filter(|f| *f >= prev.start_secs && *f < p.start_secs)
                    .map(|f| xof(f) - edge);
                out.rect(
                    run,
                    0.0,
                    x1 - run,
                    band,
                    played(with_alpha(color, 0.38), past),
                );
                let from = fill.map_or(run, |f| f.min(run));
                let strength = if fill.is_some() { 0.16 } else { 0.11 };
                out.hatch(
                    from,
                    band,
                    x1 - from,
                    h - band,
                    hatch,
                    x1,
                    played(with_alpha(color, strength), past),
                );
                if let Some(fx) = fill {
                    fill_marker(out, style, fx, x1, band, scale, played(color, past));
                }
            }
        }
    }

    let ink = style.flag_text;
    let size = TAB_TEXT_CSS * scale;
    let font = text::font();
    let baseline = ((band + font.cap(size)) * 0.5).round();
    let pad = 6.0 * scale;
    let gap = 4.0 * scale;
    let current = phrase_nav::phrase_at(phrases, pos);
    let mut buf = [0u8; 12];
    for (i, p) in phrases.iter().enumerate() {
        let start = xof(p.start_secs) - edge;
        let next = phrases
            .get(i + 1)
            .map_or(xof(p.end_secs), |n| xof(n.start_secs))
            - edge;
        if next < 0.0 || start > w {
            continue;
        }
        let name = phrase_name(p.kind);
        let len = grid.map_or(0, |g| phrase_nav::phrase_bars(p, g).0);
        let num = if len > 0 { fmt_int(&mut buf, len) } else { "" };
        let name_w = font.measure(name, size, NAME_TRACKING);
        let num_w = font.measure(num, size, 0.0);
        let full = pad + name_w + if num.is_empty() { 0.0 } else { gap + num_w } + pad;
        let compact = pad + num_w.max(font.measure("D", size, 0.0)) + pad;
        let room = next - start - 2.0 * scale;
        let (tab_w, show_name) = if room >= full || i == phrases.len() - 1 {
            (full, true)
        } else if room >= compact {
            (compact, false)
        } else {
            continue;
        };
        let x = start.max(0.0).min(next - tab_w - 2.0 * scale);
        let color = played(
            phrase_color(style, p.kind),
            p.end_secs <= pos && current != Some(i),
        );
        out.round_rect(x, 0.0, tab_w, band, 3.0 * scale, color);
        out.rect(x, 0.0, 3.0 * scale, band, color);
        let mut tx = x + pad;
        if show_name {
            tx += out.text(tx, baseline, size, NAME_TRACKING, name, ink) + gap;
            out.text(tx, baseline, size, 0.0, num, with_alpha(ink, 0.62));
        } else if num.is_empty() {
            out.text(tx, baseline, size, 0.0, &name[..1], ink);
        } else {
            out.text(tx, baseline, size, 0.0, num, ink);
        }
        if current == Some(i) {
            let progress =
                ((pos - p.start_secs) / (p.end_secs - p.start_secs).max(1e-9)).clamp(0.0, 1.0);
            let bar_h = 2.0 * scale;
            out.rect(x, band - bar_h, tab_w, bar_h, with_alpha(ink, 0.18));
            out.rect(
                x,
                band - bar_h,
                tab_w * progress,
                bar_h,
                with_alpha(ink, 0.7),
            );
        }
    }
}

/// Fill-in leading into the next phrase: a solid band segment with a `FILL` label and a start notch.
fn fill_marker(
    out: &mut QuadList,
    style: &Style,
    x: f64,
    end: f64,
    band: f64,
    scale: f64,
    color: Rgba,
) {
    out.rect(x, 0.0, end - x, band, with_alpha(color, 0.72));
    out.rect(x, 0.0, (1.5 * scale).round(), band, style.playhead);
    let s = 4.0 * scale;
    out.triangle(x - s * 0.5 + 0.75 * scale, band, s, s, true, style.playhead);
    let size = 8.5 * scale;
    let font = text::font();
    if end - x >= font.measure("FILL", size, NAME_TRACKING) + 10.0 * scale {
        let baseline = ((band + font.cap(size)) * 0.5).round();
        out.text(
            x + 5.0 * scale,
            baseline,
            size,
            NAME_TRACKING,
            "FILL",
            with_alpha(style.flag_text, 0.85),
        );
    }
}

fn ghost_quads(
    out: &mut QuadList,
    ctx: &SurfaceCtx<'_>,
    ghost: Ghost,
    zoom_css_pps: f64,
    cx: f64,
    band: f64,
) {
    let SurfaceCtx {
        w, h, style, scale, ..
    } = *ctx;
    let x = cx + ghost.wall_secs * zoom_css_pps * scale;
    if x < -20.0 * scale || x > w + 20.0 * scale {
        return;
    }
    let color = if ghost.aligned {
        style.aligned
    } else {
        style.deck[ghost.deck.min(1)]
    };
    let dash = 6.0 * scale;
    let width = (2.0 * scale).round();
    let mut y = band;
    while y < h {
        out.rect(
            x - width * 0.5,
            y,
            width,
            dash.min(h - y),
            with_alpha(color, 0.85),
        );
        y += dash + 4.0 * scale;
    }
    let s = 5.0 * scale;
    out.triangle(x - s, band, 2.0 * s, s * 1.2, true, color);
    let size = LABEL_TEXT_CSS * scale;
    let font = text::font();
    let letter = if ghost.deck == 0 { "A" } else { "B" };
    let name = phrase_name(ghost.kind);
    let pad = 5.0 * scale;
    let chip_h = 15.0 * scale;
    let chip_w = pad * 2.0
        + font.measure(letter, size, 0.0)
        + 4.0 * scale
        + font.measure(name, size, NAME_TRACKING);
    let cy = h - chip_h - 12.0 * scale;
    let chip_x = x + 4.0 * scale;
    out.round_rect(chip_x, cy, chip_w, chip_h, 3.0 * scale, color);
    let baseline = (cy + (chip_h + font.cap(size)) * 0.5).round();
    let used = out.text(chip_x + pad, baseline, size, 0.0, letter, style.flag_text);
    out.text(
        chip_x + pad + used + 4.0 * scale,
        baseline,
        size,
        NAME_TRACKING,
        name,
        with_alpha(style.flag_text, 0.75),
    );
}

pub fn scroll_quads(out: &mut QuadList, ctx: &SurfaceCtx<'_>, zoom_css_pps: f64) {
    let SurfaceCtx {
        w,
        h,
        deck,
        deck_index,
        phrases,
        style,
        scale,
        ghost,
    } = *ctx;
    if !deck.loaded {
        return;
    }
    let pps = scroll_pps(zoom_css_pps, scale, deck.rate());
    let pos = deck.pos_secs;
    let cx = (w * 0.5).floor();
    let xof = |t: f64| x_of(pos, pps, cx, t);
    let band = scroll_band_height(!phrases.is_empty(), scale);

    if band > 0.0 {
        phrase_band(out, ctx, pps, pos, cx, band);
    }

    if let Some(l) = deck.loop_region {
        let (x1, x2) = (xof(l.in_secs), xof(l.out_secs));
        let (fill, edge) = if l.active { (0.16, 0.9) } else { (0.05, 0.35) };
        out.rect(
            x1,
            band,
            x2 - x1,
            h - band,
            with_alpha(style.loop_color, fill),
        );
        let edge_color = with_alpha(style.loop_color, edge);
        out.rect(x1, band, 2.0 * scale, h - band, edge_color);
        out.rect(x2 - 2.0 * scale, band, 2.0 * scale, h - band, edge_color);
        out.rect(x1, band, x2 - x1, 2.0 * scale, edge_color);
        out.rect(x1, h - 2.0 * scale, x2 - x1, 2.0 * scale, edge_color);
    }

    grid_quads(out, ctx, pps, pos, cx, band);

    if let Some(g) = ghost {
        ghost_quads(out, ctx, g, zoom_css_pps, cx, band);
    }

    let flag_h = 14.0 * scale;
    let flag_w = 15.0 * scale;
    let font = text::font();
    let digit = 9.0 * scale;
    const SLOTS: [&str; HOT_CUE_SLOTS] = ["1", "2", "3", "4", "5", "6", "7", "8"];
    for (i, cue) in deck.hot_cues.iter().enumerate() {
        let Some(c) = cue else { continue };
        let x = xof(c.secs);
        if x < -30.0 * scale || x > w + 30.0 * scale {
            continue;
        }
        let color = rgb_u32(c.rgb, 1.0);
        out.rect(x, band, scale, h - band, with_alpha(color, 0.7));
        out.round_rect(x, band, flag_w, flag_h, 2.0 * scale, color);
        out.rect(x, band, 2.0 * scale, flag_h, color);
        let tw = font.measure(SLOTS[i], digit, 0.0);
        out.text(
            x + (flag_w - tw) * 0.5,
            (band + (flag_h + font.cap(digit)) * 0.5).round(),
            digit,
            0.0,
            SLOTS[i],
            style.flag_text,
        );
    }

    let x = xof(deck.cue_secs);
    if x > -30.0 * scale && x < w + 30.0 * scale {
        out.rect(x, band, scale, h - band, with_alpha(style.cue, 0.85));
        let s = 7.0 * scale;
        out.triangle(x - s, band, 2.0 * s, s * 1.4, true, style.cue);
    }

    out.rect(cx - scale, band, 2.0 * scale, h - band, style.playhead);
    let s = 6.0 * scale;
    let deck_color = style.deck[deck_index.min(1)];
    out.triangle(cx - s, band, 2.0 * s, s * 1.3, true, deck_color);
    out.triangle(cx - s, h - s * 1.3, 2.0 * s, s * 1.3, false, deck_color);
}

/// Phrase band, played dimming, loop, markers and playhead for an overview, in surface-local device pixels.
pub fn overview_quads(
    out: &mut QuadList,
    ctx: &SurfaceCtx<'_>,
    hover_px: Option<f64>,
    anim_secs: f64,
) {
    let SurfaceCtx {
        w,
        h,
        deck,
        deck_index,
        phrases,
        style,
        scale,
        ..
    } = *ctx;
    if !deck.loaded || deck.duration_secs <= 0.0 {
        return;
    }
    let dur = deck.duration_secs;
    let (wave_h, gap, band_h) =
        overview_band_heights(!phrases.is_empty() && style.overview_phrase_band, h, scale);
    let xof = |t: f64| t / dur * w;
    let px = xof(deck.pos_secs.clamp(0.0, dur)).round();

    for p in phrases {
        let x1 = xof(p.start_secs).round();
        let x2 = xof(p.end_secs).round();
        let alpha = 0.4 + 0.5 * p.energy.clamp(0.0, 1.0);
        out.rect(
            x1,
            wave_h + gap,
            (x2 - x1 - scale).max(1.0),
            band_h,
            with_alpha(phrase_color(style, p.kind), alpha),
        );
    }

    let font = text::font();
    let label = 8.5 * scale;
    let grid = deck.grid.filter(|g| g.bpm > 0.0);
    for p in phrases {
        let x1 = xof(p.start_secs).round();
        let color = phrase_color(style, p.kind);
        if let Some(g) = grid {
            let bar = phrase_nav::bar_secs(g);
            let mut k = 16.0;
            while p.start_secs + k * bar < p.end_secs - bar * 0.5 {
                let x = xof(p.start_secs + k * bar).round();
                out.rect(x, 0.0, scale, (wave_h * 0.5).round(), style.grid_bar);
                k += 16.0;
            }
        }
        out.rect(x1, 0.0, scale, wave_h, with_alpha(color, 0.65));
        let name = phrase_name(p.kind);
        if band_h > 0.0
            && xof(p.end_secs) - x1 >= font.measure(name, label, NAME_TRACKING) + 8.0 * scale
        {
            let baseline = (wave_h + gap + (band_h + font.cap(label)) * 0.5).round();
            out.text(
                x1 + 4.0 * scale,
                baseline,
                label,
                NAME_TRACKING,
                name,
                with_alpha(style.flag_text, 0.85),
            );
        }
    }

    let dim = with_alpha(style.bg, 0.55);
    out.rect(0.0, 0.0, px, wave_h, dim);
    if band_h > 0.0 {
        out.rect(0.0, wave_h + gap, px, band_h, with_alpha(style.bg, 0.4));
    }

    if dur > 60.0 {
        let warn_start = dur - 30.0;
        let pulse = if deck.pos_secs > warn_start && deck.playing {
            0.09 + 0.06 * (anim_secs * 1000.0 / 160.0).sin()
        } else if deck.pos_secs > warn_start {
            0.09
        } else {
            0.05
        };
        out.rect(
            xof(warn_start),
            0.0,
            w - xof(warn_start),
            wave_h,
            with_alpha(style.warn, pulse as f32),
        );
    }

    if let Some(l) = deck.loop_region {
        let (x1, x2) = (xof(l.in_secs), xof(l.out_secs));
        let width = (x2 - x1).max(2.0 * scale);
        out.rect(
            x1,
            0.0,
            width,
            wave_h,
            with_alpha(style.loop_color, if l.active { 0.3 } else { 0.12 }),
        );
        out.rect(
            x1,
            0.0,
            width,
            scale,
            with_alpha(style.loop_color, if l.active { 0.95 } else { 0.5 }),
        );
    }

    for cue in deck.hot_cues.iter().flatten() {
        let x = xof(cue.secs).round();
        let color = rgb_u32(cue.rgb, 1.0);
        out.rect(x, 0.0, scale, wave_h, with_alpha(color, 0.55));
        out.rect(x, 0.0, 7.0 * scale, 8.0 * scale, color);
    }
    let x = xof(deck.cue_secs).round();
    let s = 4.0 * scale;
    out.triangle(x - s, wave_h - s * 1.4, 2.0 * s, s * 1.4, false, style.cue);

    if let Some(hx) = hover_px {
        out.rect(hx.round(), 0.0, scale, h, style.hover);
    }
    out.rect(px - scale, 0.0, 2.0 * scale, h, style.playhead);
    out.rect(
        px - 2.5 * scale,
        0.0,
        5.0 * scale,
        2.0 * scale,
        style.deck[deck_index.min(1)],
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deck_with_cues() -> DeckView {
        let mut d = DeckView {
            loaded: true,
            token: 1,
            pos_secs: 30.0,
            duration_secs: 240.0,
            playing: true,
            cue_secs: 20.0,
            ..Default::default()
        };
        d.hot_cues[0] = Some(HotCueView {
            secs: 40.0,
            rgb: 0xFF3B5C,
        });
        d.loop_region = Some(LoopView {
            in_secs: 32.0,
            out_secs: 36.0,
            active: true,
        });
        d.grid = Some(GridView {
            bpm: 120.0,
            first_beat_secs: 0.0,
        });
        d
    }

    #[test]
    fn surface_ids_round_trip_their_names() {
        for id in SurfaceId::ALL {
            assert_eq!(SurfaceId::parse(id.name()), Some(id));
        }
        assert_eq!(SurfaceId::parse("scroll-c"), None);
        assert_eq!(SurfaceId::parse("vu-a"), None);
    }

    #[test]
    fn scroll_playhead_sits_on_the_centre_column() {
        let mut q = QuadList::default();
        let (d, st) = (deck_with_cues(), Style::default());
        scroll_quads(
            &mut q,
            &SurfaceCtx {
                w: 800.0,
                h: 200.0,
                deck: &d,
                deck_index: 0,
                phrases: &[],
                style: &st,
                scale: 2.0,
                ghost: None,
            },
            100.0,
        );
        let head = q
            .items
            .iter()
            .find(|i| i.color == Style::default().playhead && i.rect[3] == 200.0)
            .expect("playhead");
        assert_eq!(head.rect[0], 400.0 - 2.0);
        assert_eq!(head.rect[2], 4.0);
    }

    #[test]
    fn scroll_markers_are_offset_from_the_playhead_by_time_times_zoom() {
        let mut q = QuadList::default();
        let (d, st) = (deck_with_cues(), Style::default());
        scroll_quads(
            &mut q,
            &SurfaceCtx {
                w: 800.0,
                h: 200.0,
                deck: &d,
                deck_index: 0,
                phrases: &[],
                style: &st,
                scale: 2.0,
                ghost: None,
            },
            20.0,
        );
        let hot_line = q
            .items
            .iter()
            .find(|i| i.rect[3] == 200.0 && (i.color[3] - 0.7).abs() < 1e-6)
            .expect("hot cue line");
        assert!((hot_line.rect[0] - (400.0 + 10.0 * 40.0)).abs() < 1e-3);
        let cue_line = q
            .items
            .iter()
            .find(|i| i.rect[3] == 200.0 && (i.color[3] - 0.85).abs() < 1e-6)
            .expect("cue line");
        assert!((cue_line.rect[0] - (400.0 - 10.0 * 40.0)).abs() < 1e-3);
    }

    #[test]
    fn overview_playhead_is_proportional_to_progress() {
        let mut q = QuadList::default();
        let mut d = deck_with_cues();
        d.pos_secs = 60.0;
        let st = Style::default();
        overview_quads(
            &mut q,
            &SurfaceCtx {
                w: 1000.0,
                h: 80.0,
                deck: &d,
                deck_index: 0,
                phrases: &[],
                style: &st,
                scale: 1.0,
                ghost: None,
            },
            None,
            0.0,
        );
        let head = q
            .items
            .iter()
            .find(|i| i.color == Style::default().playhead)
            .expect("playhead");
        assert_eq!(head.rect[0], 250.0 - 1.0);
    }

    #[test]
    fn phrases_shrink_the_overview_waveform_area() {
        let (with, _, band) = overview_band_heights(true, 100.0, 2.0);
        let (without, _, none) = overview_band_heights(false, 100.0, 2.0);
        assert_eq!(without, 100.0);
        assert_eq!(none, 0.0);
        assert_eq!(with + band + 4.0, 100.0);
    }

    fn scroll_scene(c: &crate::bars::fixtures::Case, style: &Style) -> (QuadList, f64, f64, f64) {
        let w = 1600.0;
        let (from, to) = (c.from_secs - 0.5, c.to_secs + 0.5);
        let zoom = w / (to - from);
        let mut d = DeckView {
            loaded: true,
            token: 1,
            pos_secs: (from + to) / 2.0,
            duration_secs: 600.0,
            ..Default::default()
        };
        d.grid = Some(c.grid);
        let mut q = QuadList::default();
        scroll_quads(
            &mut q,
            &SurfaceCtx {
                w,
                h: 200.0,
                deck: &d,
                deck_index: 0,
                phrases: &c.phrases,
                style,
                scale: 1.0,
                ghost: None,
            },
            zoom,
        );
        (q, d.pos_secs, zoom, w)
    }

    fn full_height_at(q: &QuadList, x: f64, top: f64, color: Rgba) -> bool {
        q.items.iter().any(|i| {
            i.kind[0] == KIND_RECT
                && i.color == color
                && f64::from(i.rect[1]) == top
                && f64::from(i.rect[3]) == 200.0 - top
                && (f64::from(i.rect[0]) + f64::from(i.rect[2]) * 0.5 - x).abs() < 1e-3
        })
    }

    #[test]
    fn every_bar_line_is_drawn_at_its_phrase_relative_level() {
        let style = Style::default();
        for c in crate::bars::fixtures::load() {
            let (q, pos, pps, w) = scroll_scene(&c, &style);
            let cx = (w * 0.5).floor();
            let top = scroll_band_height(!c.phrases.is_empty(), 1.0);
            let mut levels = 0;
            for l in c
                .lines
                .iter()
                .filter(|l| l.kind != LineKind::Beat && l.secs > c.from_secs && l.secs < c.to_secs)
            {
                let mark = phrase_nav::classify(l, &c.phrases, c.grid);
                let phrase = mark
                    .phrase
                    .map_or(style.deck[0], |i| phrase_color(&style, c.phrases[i].kind));
                let look = level_look(&style, mark.level, style.deck[0], phrase);
                let x = cx + (l.secs - pos) * pps;
                let width = look.width_css.round().max(1.0);
                assert!(
                    full_height_at(&q, x - width * 0.5 + width * 0.5, top, look.color),
                    "{}: {:?} line at {}",
                    c.name,
                    mark.level,
                    l.secs
                );
                levels += usize::from(mark.level > BarLevel::Bar);
            }
            if !c.phrases.is_empty() {
                assert!(levels > 0, "{}: no 4/8/16/phrase lines", c.name);
            }
        }
    }

    #[test]
    fn scroll_gridlines_follow_phrase_starts_not_the_track_start() {
        let style = Style::default();
        let case = crate::bars::fixtures::load()
            .into_iter()
            .find(|c| c.name == "drop after a two-bar fill")
            .unwrap();
        let (q, pos, pps, w) = scroll_scene(&case, &style);
        let cx = (w * 0.5).floor();
        let top = scroll_band_height(true, 1.0);
        let bar = 60.0 / 128.0 * 4.0;
        let x_of = |t: f64| cx + (t - pos) * pps;
        assert!(full_height_at(&q, x_of(18.0 * bar), top, style.phrases[2]));
        assert!(
            !full_height_at(&q, x_of(20.0 * bar), top, style.grid_four),
            "the old global lattice line must be gone"
        );
        assert!(full_height_at(&q, x_of(22.0 * bar), top, style.grid_four));
    }

    #[test]
    fn beatmatch_grid_emphasizes_downbeats_without_bar_counts() {
        let style = Style::default();
        let case = crate::bars::fixtures::load()
            .into_iter()
            .find(|c| c.name == "drop after a two-bar fill")
            .unwrap();
        let d = DeckView {
            loaded: true,
            token: 1,
            pos_secs: 20.0,
            duration_secs: 240.0,
            grid: Some(case.grid),
            beatmatch_view: true,
            ..Default::default()
        };
        let mut q = QuadList::default();
        let pps = 20.0;
        let cx = 400.0;
        grid_quads(
            &mut q,
            &SurfaceCtx {
                w: 800.0,
                h: 200.0,
                deck: &d,
                deck_index: 0,
                phrases: &case.phrases,
                style: &style,
                scale: 1.0,
                ghost: None,
            },
            pps,
            d.pos_secs,
            cx,
            16.0,
        );
        for bar in [18.0, 19.0] {
            let x = cx + (bar * 60.0 / 128.0 * 4.0 - d.pos_secs) * pps;
            assert!(q.items.iter().any(|i| {
                i.color == BEAT_ONE
                    && i.kind[0] == KIND_RECT
                    && (f64::from(i.rect[0]) + f64::from(i.rect[2]) * 0.5 - x).abs() < 1e-3
            }));
        }
        assert!(q.items.iter().any(|i| i.color == BEAT_ONE && i.kind[0] == KIND_TRI_DOWN));
        assert!(q.items.iter().any(|i| i.color == faint_grid_beat(&style)));
        assert!(q.items.iter().all(|i| i.kind[0] != KIND_GLYPH));
    }

    fn faint_grid_beat(style: &Style) -> Rgba {
        with_alpha(style.grid_beat, style.grid_beat[3] * 0.3)
    }

    #[test]
    fn zoomed_out_views_keep_only_the_phrase_and_sixteen_bar_levels() {
        let style = Style::default();
        let case = &crate::bars::fixtures::load()[1];
        let mut d = deck_with_cues();
        d.grid = Some(case.grid);
        let mut q = QuadList::default();
        scroll_quads(
            &mut q,
            &SurfaceCtx {
                w: 800.0,
                h: 200.0,
                deck: &d,
                deck_index: 0,
                phrases: &case.phrases,
                style: &style,
                scale: 1.0,
                ghost: None,
            },
            1.2,
        );
        assert!(
            q.items
                .iter()
                .all(|i| ![style.grid_beat, style.grid_bar, style.grid_four].contains(&i.color))
        );
        assert!(q.items.iter().any(|i| i.color == style.phrases[2]));
    }

    #[test]
    fn the_current_phrase_tab_sticks_to_the_left_edge_until_the_next_tab_pushes_it() {
        let style = Style::default();
        let bar = 2.0;
        let phrases: Vec<PhraseView> = [
            (0.0, 32.0, PhraseKind::Drop),
            (32.0, 40.0, PhraseKind::Break),
        ]
        .iter()
        .map(|&(a, b, kind)| PhraseView {
            start_secs: a * bar,
            end_secs: b * bar,
            kind,
            energy: 1.0,
            fill_start_secs: None,
        })
        .collect();
        let tab_x = |pos: f64| {
            let mut d = deck_with_cues();
            d.hot_cues = Default::default();
            d.loop_region = None;
            d.pos_secs = pos;
            let mut q = QuadList::default();
            scroll_quads(
                &mut q,
                &SurfaceCtx {
                    w: 800.0,
                    h: 200.0,
                    deck: &d,
                    deck_index: 0,
                    phrases: &phrases,
                    style: &style,
                    scale: 1.0,
                    ghost: None,
                },
                40.0,
            );
            q.items
                .iter()
                .find(|i| i.kind[0] == KIND_ROUND && i.color[..3] == style.phrases[2][..3])
                .map(|i| f64::from(i.rect[0]))
        };
        assert_eq!(
            tab_x(30.0),
            Some(0.0),
            "pinned at the left edge while the drop plays"
        );
        let pushed = tab_x(73.0).expect("still visible next to the break tab");
        assert!(pushed < 0.0, "pushed off by the next tab: {pushed}");
        let start = tab_x(4.0).unwrap();
        assert!(
            (start - (400.0 - 4.0 * 40.0 - 1.5)).abs() < 1e-3,
            "at its own start once that is on screen: {start}"
        );
    }

    #[test]
    fn the_ghost_marks_where_the_other_decks_next_phrase_lands_in_wall_time() {
        let bar = 2.0;
        let phrases = |starts: &[f64]| -> Vec<PhraseView> {
            starts
                .windows(2)
                .map(|w| PhraseView {
                    start_secs: w[0] * bar,
                    end_secs: w[1] * bar,
                    kind: PhraseKind::Break,
                    energy: 1.0,
                    fill_start_secs: None,
                })
                .collect()
        };
        let (a, b) = (phrases(&[0.0, 16.0, 32.0]), phrases(&[0.0, 8.0, 24.0]));
        let mut decks = [deck_with_cues(), deck_with_cues()];
        decks[0].master = true;
        decks[0].pos_secs = 12.0 * bar;
        decks[1].pos_secs = 2.0 * bar;
        decks[1].tempo_ratio = 2.0;
        let g = ghost_for(&decks, [&a, &b], 0).expect("ghost on the master");
        assert_eq!(g.deck, 1);
        assert!((g.wall_secs - 6.0).abs() < 1e-9);
        assert!(!g.aligned);
        assert!(
            ghost_for(&decks, [&a, &b], 1).is_none(),
            "only the master shows it"
        );
        decks[1].pos_secs = 0.0;
        assert!(ghost_for(&decks, [&a, &b], 0).unwrap().aligned);

        let st = Style::default();
        let mut q = QuadList::default();
        scroll_quads(
            &mut q,
            &SurfaceCtx {
                w: 800.0,
                h: 200.0,
                deck: &decks[0],
                deck_index: 0,
                phrases: &a,
                style: &st,
                scale: 1.0,
                ghost: Some(g),
            },
            20.0,
        );
        let dash = q
            .items
            .iter()
            .find(|i| i.kind[0] == KIND_RECT && i.color == with_alpha(st.deck[1], 0.85))
            .expect("dashed ghost line in deck B colour");
        assert!((f64::from(dash.rect[0]) + 1.0 - (400.0 + 6.0 * 20.0)).abs() < 1e-3);
    }

    #[test]
    fn slower_tempo_stretches_the_view_in_wall_clock_terms() {
        assert_eq!(scroll_pps(100.0, 2.0, 1.0), 200.0);
        assert!((scroll_pps(100.0, 2.0, 0.5) - 400.0).abs() < 1e-9);
    }
}
