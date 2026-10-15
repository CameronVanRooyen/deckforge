use deckforge_common::{Beatgrid, TrackAudio};
use serde::{Deserialize, Serialize};

use crate::downbeat::estimate_bar_phase;
use crate::rhythm::{band, db, median, percentile, sixteenths, Sixteenths, BANDS, SIXTEENTHS_PER_BEAT};

const BEATS_PER_BAR: usize = 4;
const SIXTEENTHS_PER_BAR: usize = BEATS_PER_BAR * SIXTEENTHS_PER_BEAT;
const NOVELTY_SCALES_BARS: [usize; 3] = [1, 2, 4];
const PATTERN_WEIGHT: f64 = 1.0;
const PATTERN_MIN_LEVEL: f64 = 0.01;
const MIN_NOVELTY_SCALE: f64 = 0.1;
const PEAK_NOVELTY_SCALE: f64 = 0.05;
const RAMP_DISCOUNT: f64 = 0.5;
const BOUNDARY_WEIGHT: f64 = 0.5;
const BOUNDARY_COST: f64 = 0.6;
const MAX_PHRASE_BARS: usize = 40;
const LENGTH_PRIOR: [(usize, f64); 13] = [
    (8, 0.42),
    (4, 0.18),
    (16, 0.13),
    (12, 0.04),
    (2, 0.02),
    (3, 0.02),
    (5, 0.02),
    (6, 0.02),
    (7, 0.015),
    (9, 0.015),
    (24, 0.015),
    (10, 0.01),
    (32, 0.01),
];
const RARE_LENGTH_PRIOR: f64 = 0.003;
const FILL_REFERENCE_BEATS: usize = 8;
const FILL_BAR_DROP_DB: f64 = -3.0;
const FILL_HALF_BAR_DROP_DB: f64 = -4.0;
const FILL_MIN_LEVEL_DB: f64 = -20.0;
const DB_RANGE: f64 = 30.0;
const MAX_BUILD_BARS: f64 = 16.0;
const RISING_SLOPE: f64 = 0.2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PhraseKind {
    Intro,
    Build,
    Drop,
    Break,
    Outro,
}

impl PhraseKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Intro => "intro",
            Self::Build => "build",
            Self::Drop => "drop",
            Self::Break => "break",
            Self::Outro => "outro",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "intro" => Self::Intro,
            "build" => Self::Build,
            "drop" => Self::Drop,
            "break" => Self::Break,
            "outro" => Self::Outro,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Phrase {
    pub start_secs: f64,
    pub end_secs: f64,
    pub kind: PhraseKind,
    pub bars: u32,
    /// 0..1, mean loudness relative to the loudest phrase of the track.
    pub energy: f32,
    /// Where a fill-in leading into the next phrase starts (a bar or half a bar before it).
    #[serde(default)]
    pub fill_start_secs: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PhraseAnalysis {
    /// Grid beat index of the first bar start at or after the start of the track (0 = the grid's first beat).
    pub downbeat_beat: i64,
    /// 0..1, how clearly the bar phase won; 1 when the caller supplied it.
    pub downbeat_confidence: f32,
    pub phrases: Vec<Phrase>,
}

/// Bar-synchronous view of a track: levels per band in dB and the sub-bass rhythm inside each bar.
struct Bars {
    first_beat: usize,
    levels: Vec<[f64; BANDS]>,
    pattern: Vec<[f64; SIXTEENTHS_PER_BAR]>,
    power: Vec<[f64; BANDS]>,
}

impl Bars {
    fn new(s: &Sixteenths, first_beat: usize) -> Self {
        let count = s.beats().saturating_sub(first_beat) / BEATS_PER_BAR;
        let base = first_beat * SIXTEENTHS_PER_BEAT;
        let power: Vec<[f64; BANDS]> = (0..count)
            .map(|bar| {
                let from = base + bar * SIXTEENTHS_PER_BAR;
                std::array::from_fn(|b| s.mean(b, from, from + SIXTEENTHS_PER_BAR))
            })
            .collect();
        let audible_sub = percentile(&power.iter().map(|p| p[band::SUB]).collect::<Vec<_>>(), 0.9) * PATTERN_MIN_LEVEL;
        let pattern = (0..count)
            .map(|bar| {
                let from = base + bar * SIXTEENTHS_PER_BAR;
                if power[bar][band::SUB] < audible_sub {
                    return [0.0; SIXTEENTHS_PER_BAR];
                }
                let sub: [f64; SIXTEENTHS_PER_BAR] = std::array::from_fn(|i| f64::from(s.power[from + i][band::SUB]));
                let total: f64 = sub.iter().sum::<f64>() + 1e-12;
                sub.map(|v| v / total)
            })
            .collect();
        let levels = power.iter().map(|p| p.map(db)).collect();
        Self { first_beat, levels, pattern, power }
    }

    fn len(&self) -> usize {
        self.levels.len()
    }
}

fn mean_rows(rows: &[[f64; BANDS]]) -> [f64; BANDS] {
    let n = rows.len().max(1) as f64;
    std::array::from_fn(|f| rows.iter().map(|r| r[f]).sum::<f64>() / n)
}

/// Change strength at every bar line (index j = between bar j-1 and bar j), normalised by its median.
fn novelty(bars: &Bars) -> Vec<f64> {
    let n = bars.len();
    let mut curve = vec![0.0f64; n + 1];
    if n < 2 {
        return curve;
    }
    let mean = mean_rows(&bars.levels);
    let spread: [f64; BANDS] = std::array::from_fn(|f| {
        (bars.levels.iter().map(|r| (r[f] - mean[f]).powi(2)).sum::<f64>() / n as f64).sqrt() + 1e-6
    });
    let scaled: Vec<[f64; BANDS]> = bars.levels.iter().map(|r| std::array::from_fn(|f| r[f] / spread[f])).collect();
    let step = |j: usize, w: usize| -> [f64; BANDS] {
        if j == 0 || j >= n {
            return [0.0; BANDS];
        }
        let before = mean_rows(&scaled[j.saturating_sub(w)..j]);
        let after = mean_rows(&scaled[j..(j + w).min(n)]);
        std::array::from_fn(|f| after[f] - before[f])
    };
    for (j, value) in curve.iter_mut().enumerate().take(n).skip(1) {
        let mut s = 0.0;
        for w in NOVELTY_SCALES_BARS {
            let (here, earlier, later) = (step(j, w), step(j.saturating_sub(w), w), step(j + w, w));
            s += (0..BANDS).map(|f| (here[f] - RAMP_DISCOUNT * 0.5 * (earlier[f] + later[f])).abs()).sum::<f64>() / BANDS as f64;
        }
        s += PATTERN_WEIGHT * bars.pattern[j - 1].iter().zip(&bars.pattern[j]).map(|(a, b)| (a - b).abs()).sum::<f64>();
        *value = s / NOVELTY_SCALES_BARS.len() as f64;
    }
    let mut inner: Vec<f64> = curve[1..n].to_vec();
    let peak = inner.iter().cloned().fold(0.0, f64::max);
    let m = median(&mut inner).max(MIN_NOVELTY_SCALE).max(PEAK_NOVELTY_SCALE * peak);
    curve.iter_mut().for_each(|v| *v /= m);
    curve
}

fn length_log_prior(bars: usize) -> f64 {
    LENGTH_PRIOR.iter().find(|(l, _)| *l == bars).map_or(RARE_LENGTH_PRIOR, |(_, p)| *p).ln()
}

/// The last phrase may be cut short by the end of the track: score it as the likeliest length it could have had.
fn truncated_log_prior(bars: usize) -> f64 {
    LENGTH_PRIOR.iter().filter(|(l, _)| *l >= bars).map(|(_, p)| p.ln()).fold(length_log_prior(bars), f64::max)
}

/// Bar indices where phrases start (always including 0): the segmentation maximising the phrase-length prior plus
/// the change strength at each boundary.
fn segment(novelty: &[f64], bars: usize) -> Vec<usize> {
    if bars == 0 {
        return vec![0];
    }
    let mut best = vec![f64::NEG_INFINITY; bars + 1];
    let mut prev = vec![0usize; bars + 1];
    best[0] = 0.0;
    for j in 1..=bars {
        for len in 1..=MAX_PHRASE_BARS.min(j) {
            let i = j - len;
            let prior = if j < bars { length_log_prior(len) } else { truncated_log_prior(len) };
            let score = best[i] + prior + if i > 0 { BOUNDARY_WEIGHT * novelty[i] - BOUNDARY_COST } else { 0.0 };
            if score > best[j] {
                best[j] = score;
                prev[j] = i;
            }
        }
    }
    let mut starts = Vec::new();
    let mut j = bars;
    while j > 0 {
        j = prev[j];
        starts.push(j);
    }
    starts.reverse();
    starts
}

/// Beat (relative to the analysed start) where a fill-in begins before the phrase starting at `boundary_beat`:
/// the sub-bass drops out for the last bar or half bar.
fn fill_start(s: &Sixteenths, boundary_beat: usize, loud_sub_db: f64) -> Option<usize> {
    if boundary_beat < FILL_REFERENCE_BEATS + BEATS_PER_BAR {
        return None;
    }
    let sub_db = |from_beat: usize, to_beat: usize| db(s.mean(band::SUB, from_beat * SIXTEENTHS_PER_BEAT, to_beat * SIXTEENTHS_PER_BEAT));
    let reference = sub_db(boundary_beat - BEATS_PER_BAR - FILL_REFERENCE_BEATS, boundary_beat - BEATS_PER_BAR);
    if reference < loud_sub_db + FILL_MIN_LEVEL_DB {
        return None;
    }
    if sub_db(boundary_beat - BEATS_PER_BAR, boundary_beat) - reference < FILL_BAR_DROP_DB {
        return Some(boundary_beat - BEATS_PER_BAR);
    }
    let half = BEATS_PER_BAR / 2;
    (sub_db(boundary_beat - half, boundary_beat) - reference < FILL_HALF_BAR_DROP_DB).then(|| boundary_beat - half)
}

struct SegmentStats {
    energy: f64,
    kick: f64,
    brightness: f64,
    rising: bool,
    bars: f64,
}

fn segment_stats(bars: &Bars, from: usize, to: usize, refs: &[f64; BANDS]) -> SegmentStats {
    let n = (to - from).max(1);
    let amp = |b: usize, f: usize| (bars.power[b][f].sqrt() / refs[f].sqrt().max(1e-12)).min(1.0);
    let norm = |b: usize, f: usize| ((bars.levels[b][f] - db(refs[f]) + DB_RANGE) / DB_RANGE).clamp(0.0, 1.0);
    let composite = |b: usize| 0.5 * norm(b, band::TOTAL) + 0.5 * norm(b, band::HIGH);
    let third = (n / 3).max(1);
    let mean_composite = |a: usize, b: usize| (a..b).map(composite).sum::<f64>() / (b - a).max(1) as f64;
    let slope = mean_composite(to - third, to) - mean_composite(from, from + third);
    SegmentStats {
        energy: (from..to).map(|b| amp(b, band::TOTAL)).sum::<f64>() / n as f64,
        kick: (from..to).map(|b| amp(b, band::LOW)).sum::<f64>() / n as f64,
        brightness: mean_composite(from, to),
        rising: n >= 2 && slope > RISING_SLOPE,
        bars: n as f64,
    }
}

fn label(stats: &[SegmentStats]) -> Vec<PhraseKind> {
    let n = stats.len();
    let e_max = stats.iter().map(|s| s.energy).fold(0.0, f64::max);
    let k_max = stats.iter().map(|s| s.kick).fold(0.0, f64::max);
    let mut drop_like: Vec<bool> =
        stats.iter().map(|s| s.energy >= 0.8 * e_max && (k_max < 1e-6 || s.kick >= 0.5 * k_max)).collect();
    if !drop_like.iter().any(|d| *d)
        && let Some(best) = (0..n).max_by(|&a, &b| stats[a].energy.total_cmp(&stats[b].energy))
    {
        drop_like[best] = true;
    }
    let mut builds = vec![false; n];
    for i in 0..n.saturating_sub(1) {
        if drop_like[i + 1] && stats[i].rising && stats[i].bars <= MAX_BUILD_BARS && stats[i].energy < stats[i + 1].energy {
            builds[i] = true;
            drop_like[i] = false;
        }
    }
    for d in 1..n {
        if !drop_like[d] || drop_like[d - 1] {
            continue;
        }
        let mut start = d;
        let mut bars = 0.0;
        while start > 1
            && !drop_like[start - 1]
            && bars + stats[start - 1].bars <= MAX_BUILD_BARS
            && stats[start - 1].brightness < stats[start].brightness
            && stats[start - 1].energy < stats[start].energy
        {
            start -= 1;
            bars += stats[start].bars;
        }
        if d - start >= 2 {
            builds[start..d].iter_mut().for_each(|b| *b = true);
        }
    }
    let first_drop = drop_like.iter().position(|d| *d).unwrap_or(n);
    let last_drop = drop_like.iter().rposition(|d| *d).unwrap_or(0);
    (0..n)
        .map(|i| {
            if drop_like[i] {
                PhraseKind::Drop
            } else if builds[i] {
                PhraseKind::Build
            } else if i < first_drop {
                PhraseKind::Intro
            } else if i > last_drop {
                PhraseKind::Outro
            } else {
                PhraseKind::Break
            }
        })
        .collect()
}

fn valid_grid(grid: Beatgrid) -> bool {
    grid.bpm > 0.0 && grid.bpm.is_finite() && grid.first_beat_secs.is_finite()
}

/// Labelled phrases on one global 4/4 bar phase: every phrase starts on a downbeat, the first at the first bar of
/// the track (beats before it are a pickup). `downbeat_beat` fixes the bar phase; `None` estimates it.
pub fn analyze_phrases_with_downbeat(track: &TrackAudio, grid: Beatgrid, downbeat_beat: Option<i64>) -> Option<PhraseAnalysis> {
    if !valid_grid(grid) {
        return None;
    }
    let s = sixteenths(track, grid);
    let (phase, confidence) = match downbeat_beat {
        Some(d) => ((d - s.first_beat_index).rem_euclid(BEATS_PER_BAR as i64) as usize, 1.0),
        None => {
            let p = estimate_bar_phase(&s);
            (p.phase, p.confidence)
        }
    };
    let bars = Bars::new(&s, phase);
    if bars.len() < 2 {
        return None;
    }
    let starts = segment(&novelty(&bars), bars.len());
    let refs: [f64; BANDS] = std::array::from_fn(|f| percentile(&bars.power.iter().map(|p| p[f]).collect::<Vec<_>>(), 0.95));
    let loud_sub_db = db(percentile(&bars.power.iter().map(|p| p[band::SUB]).collect::<Vec<_>>(), 0.9));
    let mut edges = starts.clone();
    edges.push(bars.len());
    let stats: Vec<SegmentStats> = edges.windows(2).map(|w| segment_stats(&bars, w[0], w[1], &refs)).collect();
    let kinds = label(&stats);
    let e_max = stats.iter().map(|s| s.energy).fold(0.0, f64::max);
    let bar_secs = s.beat_secs * BEATS_PER_BAR as f64;
    let beat_of_bar = |bar: usize| bars.first_beat + bar * BEATS_PER_BAR;
    let duration = track.duration_secs();
    let phrases = edges
        .windows(2)
        .zip(&stats)
        .zip(kinds)
        .enumerate()
        .map(|(i, ((w, st), kind))| {
            let last = i + 2 == edges.len();
            let start_secs = s.beat_secs_at(beat_of_bar(w[0]));
            let end_secs = if last { duration } else { s.beat_secs_at(beat_of_bar(w[1])) };
            let fill = if last { None } else { fill_start(&s, beat_of_bar(w[1]), loud_sub_db) };
            Phrase {
                start_secs,
                end_secs,
                kind,
                bars: if last { (((end_secs - start_secs) / bar_secs).round() as u32).max(1) } else { (w[1] - w[0]) as u32 },
                energy: if e_max > 0.0 { (st.energy / e_max).clamp(0.0, 1.0) as f32 } else { 0.0 },
                fill_start_secs: fill.map(|b| s.beat_secs_at(b)).filter(|t| *t > start_secs),
            }
        })
        .collect();
    Some(PhraseAnalysis {
        downbeat_beat: s.first_beat_index + phase as i64,
        downbeat_confidence: confidence as f32,
        phrases,
    })
}

/// Phrases with the bar phase estimated from the audio.
pub fn analyze_phrase_structure(track: &TrackAudio, grid: Beatgrid) -> Option<PhraseAnalysis> {
    analyze_phrases_with_downbeat(track, grid, None)
}

pub fn analyze_phrases(track: &TrackAudio, grid: Beatgrid) -> Vec<Phrase> {
    analyze_phrase_structure(track, grid).map(|a| a.phrases).unwrap_or_default()
}
