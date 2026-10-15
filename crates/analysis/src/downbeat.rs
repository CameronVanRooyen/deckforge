use crate::rhythm::{band, db, median, percentile, Sixteenths, SIXTEENTHS_PER_BEAT};

const BEATS_PER_BAR: usize = 4;
const PARITY_WEIGHT: f64 = 0.3;
const ONSET_WEIGHT: f64 = 1.0;
const ENTRY_WEIGHT: f64 = 2.0;
const CHANGE_WEIGHT: f64 = 2.0;
const PARITY_SATURATION: f64 = 1.5;
const ONSET_SEARCH_SECS: f64 = 210.0;
const ENTRY_RISE_DB: f64 = 6.0;
const CHANGE_WINDOW_BEATS: usize = 16;
const CHANGE_PEAK_RADIUS: usize = 4;

/// Bar phase of a constant grid: which analysed beat (0..3) starts the first bar, and how clearly it won.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BarPhase {
    pub phase: usize,
    pub confidence: f64,
}

type Votes = [f64; BEATS_PER_BAR];

/// Backbeat parity: snare-band over sub-bass power in the first sixteenth is higher on beats 2 and 4.
fn parity_votes(s: &Sixteenths) -> Votes {
    let beats = s.beats();
    let loudness: Vec<f64> = (0..beats).map(|b| s.mean(band::TOTAL, b * SIXTEENTHS_PER_BEAT, (b + 1) * SIXTEENTHS_PER_BEAT)).collect();
    let floor = percentile(&loudness, 0.25);
    let mut ratios = [Vec::new(), Vec::new()];
    for (b, loud) in loudness.iter().enumerate() {
        if *loud > floor {
            let p = s.power[b * SIXTEENTHS_PER_BEAT];
            ratios[b % 2].push((f64::from(p[band::SNARE]) + 1e-12).ln() - (f64::from(p[band::SUB]) + 1e-12).ln());
        }
    }
    if ratios.iter().any(Vec::is_empty) {
        return [0.0; BEATS_PER_BAR];
    }
    let [even, odd] = ratios.map(|mut r| median(&mut r));
    let downbeat_parity = usize::from(even >= odd);
    let margin = ((even - odd).abs() / PARITY_SATURATION).min(1.0);
    std::array::from_fn(|r| if r % 2 == downbeat_parity { margin } else { -margin })
}

/// The first strong sub-bass sixteenth is expected near a bar line and rules out the bar half it falls in.
fn onset_votes(s: &Sixteenths) -> Votes {
    let limit = ((ONSET_SEARCH_SECS / (s.beat_secs / SIXTEENTHS_PER_BEAT as f64)) as usize).min(s.power.len());
    if limit == 0 {
        return [0.0; BEATS_PER_BAR];
    }
    let sub: Vec<f64> = s.power[..limit].iter().map(|p| f64::from(p[band::SUB])).collect();
    let mean = sub.iter().sum::<f64>() / limit as f64;
    let max = sub.iter().cloned().fold(0.0, f64::max);
    let threshold = 0.6 * mean + 0.4 * max;
    let Some(first) = sub.iter().position(|v| *v > threshold) else {
        return [0.0; BEATS_PER_BAR];
    };
    let steps = SIXTEENTHS_PER_BEAT * BEATS_PER_BAR;
    std::array::from_fn(|r| match (first + steps - (r * SIXTEENTHS_PER_BEAT) % steps) % steps {
        0..=2 | 13..=15 => 1.0,
        5..=8 => -1.0,
        _ => 0.0,
    })
}

fn beat_db(s: &Sixteenths, band: usize) -> Vec<f64> {
    (0..s.beats()).map(|b| db(s.mean(band, b * SIXTEENTHS_PER_BEAT, (b + 1) * SIXTEENTHS_PER_BEAT))).collect()
}

fn normalised(votes: Votes) -> Votes {
    let sum: f64 = votes.iter().sum();
    if sum > 0.0 { votes.map(|v| v / sum - 1.0 / BEATS_PER_BAR as f64) } else { [0.0; BEATS_PER_BAR] }
}

fn window_mean(v: &[f64], from: usize, to: usize) -> f64 {
    v[from..to].iter().sum::<f64>() / (to - from) as f64
}

/// Bass and kick entries after a quieter stretch tend to land on bar lines.
fn entry_votes(s: &Sixteenths) -> Votes {
    let low = beat_db(s, band::LOW);
    let beats = low.len();
    if beats < 12 {
        return [0.0; BEATS_PER_BAR];
    }
    let jump = |k: usize| window_mean(&low, k, k + 4) - window_mean(&low, k - 8, k);
    let mut votes = [0.0; BEATS_PER_BAR];
    for k in 8..beats - 4 {
        let j = jump(k);
        if j > ENTRY_RISE_DB && (k.saturating_sub(2).max(8)..(k + 3).min(beats - 4)).all(|n| jump(n) <= j) {
            votes[k % BEATS_PER_BAR] += j;
        }
    }
    normalised(votes)
}

/// Multi-band changes between the 16 beats before and after a beat peak on bar lines.
fn change_votes(s: &Sixteenths) -> Votes {
    let features: Vec<Vec<f64>> =
        [band::TOTAL, band::SUB, band::SNARE, band::HAT, band::MID, band::HIGH].iter().map(|&b| beat_db(s, b)).collect();
    let beats = s.beats();
    let w = CHANGE_WINDOW_BEATS;
    if beats < 2 * w + 1 {
        return [0.0; BEATS_PER_BAR];
    }
    let mut novelty = vec![0.0f64; beats];
    for (k, n) in novelty.iter_mut().enumerate().take(beats - w).skip(w) {
        *n = features.iter().map(|f| (window_mean(f, k, k + w) - window_mean(f, k - w, k)).abs()).sum();
    }
    let mut votes = [0.0; BEATS_PER_BAR];
    for k in 1..beats - 1 {
        let lo = k.saturating_sub(CHANGE_PEAK_RADIUS);
        let hi = (k + CHANGE_PEAK_RADIUS + 1).min(beats);
        if novelty[k] > 0.0 && novelty[lo..hi].iter().all(|v| *v <= novelty[k]) {
            votes[k % BEATS_PER_BAR] += novelty[k];
        }
    }
    normalised(votes)
}

/// Decides the bar phase once for the whole track from backbeat parity, the first sub-bass onset, bass entries
/// and structural changes.
pub fn estimate_bar_phase(s: &Sixteenths) -> BarPhase {
    let cues = [
        (PARITY_WEIGHT, parity_votes(s)),
        (ONSET_WEIGHT, onset_votes(s)),
        (ENTRY_WEIGHT, entry_votes(s)),
        (CHANGE_WEIGHT, change_votes(s)),
    ];
    let total: Votes = std::array::from_fn(|r| cues.iter().map(|(w, v)| w * v[r]).sum());
    let phase = (0..BEATS_PER_BAR).max_by(|&a, &b| total[a].total_cmp(&total[b]).then(b.cmp(&a))).unwrap_or(0);
    let runner_up = (0..BEATS_PER_BAR).filter(|&r| r != phase).map(|r| total[r]).fold(f64::NEG_INFINITY, f64::max);
    let spread = total[phase].abs().max(1e-9);
    BarPhase { phase, confidence: ((total[phase] - runner_up) / spread).clamp(0.0, 1.0) }
}
