use deckforge_common::{Beatgrid, TrackAudio};

use crate::filter::{envelope, Chain};
use realfft::RealFftPlanner;
use serde::{Deserialize, Serialize};

const TARGET_RATE: f64 = 11_025.0;
const FFT_SIZE: usize = 1024;
const HOP: usize = 128;
const FINE_RATE: f64 = 1_000.0;
const ATTACK_SPLIT_HZ: f64 = 2_000.0;
const ATTACK_SMOOTH_HZ: f64 = 1_000.0;
const ATTACK_DECIMATION: usize = 4;
const ATTACK_SEARCH_EARLY: f64 = -0.15;
const ATTACK_SEARCH_LATE: f64 = 0.04;
const ATTACK_RISE_SECS: f64 = 0.008;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BpmRange {
    pub min: f64,
    pub max: f64,
}

impl Default for BpmRange {
    fn default() -> Self {
        Self { min: 78.0, max: 180.0 }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BeatAnalysis {
    pub grid: Beatgrid,
    /// 0..1, how strongly the winning tempo stands out from alternatives.
    pub confidence: f32,
}

fn mono_decimated(track: &TrackAudio) -> (Vec<f32>, f64) {
    let factor = (track.sample_rate as f64 / TARGET_RATE).round().max(1.0) as usize;
    let frames = track.frames();
    let mut out = Vec::with_capacity(frames / factor + 1);
    let mut i = 0;
    while i + factor <= frames {
        let mut acc = 0.0;
        for j in i..i + factor {
            let (l, r) = track.frame(j);
            acc += l + r;
        }
        out.push(acc / (2 * factor) as f32);
        i += factor;
    }
    (out, track.sample_rate as f64 / factor as f64)
}

/// Spectral-flux onset strength envelope, mean-removed and half-wave rectified.
fn onset_envelope(mono: &[f32]) -> Vec<f32> {
    if mono.len() < FFT_SIZE {
        return Vec::new();
    }
    let mut planner = RealFftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(FFT_SIZE);
    let window: Vec<f32> = (0..FFT_SIZE)
        .map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / FFT_SIZE as f32).cos())
        .collect();
    let mut input = fft.make_input_vec();
    let mut spectrum = fft.make_output_vec();
    let bins = spectrum.len();
    let mut prev = vec![0f32; bins];
    let frames = (mono.len() - FFT_SIZE) / HOP + 1;
    let mut flux = Vec::with_capacity(frames);
    for f in 0..frames {
        let start = f * HOP;
        for (k, x) in input.iter_mut().enumerate() {
            *x = mono[start + k] * window[k];
        }
        if fft.process(&mut input, &mut spectrum).is_err() {
            flux.push(0.0);
            continue;
        }
        let mut sum = 0.0;
        for (k, c) in spectrum.iter().enumerate() {
            let mag = (1.0 + 100.0 * c.norm()).ln();
            let d = mag - prev[k];
            if d > 0.0 {
                sum += d;
            }
            prev[k] = mag;
        }
        flux.push(sum);
    }
    let radius = 32;
    let mut prefix = vec![0f64; flux.len() + 1];
    for (i, v) in flux.iter().enumerate() {
        prefix[i + 1] = prefix[i] + *v as f64;
    }
    let mut env: Vec<f32> = (0..flux.len())
        .map(|i| {
            let a = i.saturating_sub(radius);
            let b = (i + radius + 1).min(flux.len());
            let mean = (prefix[b] - prefix[a]) / (b - a) as f64;
            (flux[i] - mean as f32).max(0.0)
        })
        .collect();
    let max = env.iter().cloned().fold(0.0f32, f32::max);
    if max > 0.0 {
        env.iter_mut().for_each(|v| *v /= max);
    }
    env
}

fn autocorr(env: &[f32], max_lag: usize) -> Vec<f32> {
    let n = env.len();
    (0..=max_lag)
        .map(|lag| {
            if lag >= n {
                return 0.0;
            }
            let mut s = 0.0;
            for i in 0..n - lag {
                s += env[i] * env[i + lag];
            }
            s / (n - lag) as f32
        })
        .collect()
}

fn interp(v: &[f32], x: f64) -> f32 {
    let i = x.floor() as usize;
    if i + 1 >= v.len() {
        return 0.0;
    }
    let t = (x - i as f64) as f32;
    v[i] * (1.0 - t) + v[i + 1] * t
}

fn coarse_tempo(envs: &[(&[f32], f64)], range: BpmRange) -> (f64, f32) {
    let acs: Vec<(Vec<f32>, f64)> = envs
        .iter()
        .map(|(env, rate)| {
            let ac = autocorr(env, (rate * 60.0 / 50.0 * 4.0).ceil() as usize);
            let norm = ac.first().copied().filter(|v| *v > 0.0).unwrap_or(1.0);
            (ac.into_iter().map(|v| v / norm).collect(), *rate)
        })
        .collect();
    let mut best = (120.0, f32::MIN);
    let mut scores = Vec::new();
    let mut bpm = 60.0;
    while bpm <= 200.0 {
        let comb: f32 = acs
            .iter()
            .map(|(ac, rate)| {
                let lag = rate * 60.0 / bpm;
                (1..=4).map(|k| interp(ac, k as f64 * lag)).sum::<f32>()
            })
            .sum();
        let octave = (bpm / 125.0).log2();
        let prior = (-0.5 * (octave / 0.9).powi(2)).exp() as f32;
        let score = comb * prior;
        scores.push(score);
        if score > best.1 {
            best = (bpm, score);
        }
        bpm += 0.1;
    }
    let mean = scores.iter().sum::<f32>() / scores.len() as f32;
    let confidence = if best.1 > 0.0 { ((best.1 - mean) / best.1).clamp(0.0, 1.0) } else { 0.0 };
    let mut bpm = fold_into(best.0, range);
    let (ac, rate) = acs.last().map(|(a, r)| (a.as_slice(), *r)).unwrap_or((&[], 1.0));
    let lag = rate * 60.0 / bpm;
    if bpm * 2.0 < range.max && interp(ac, lag / 2.0) > 0.6 * interp(ac, lag) {
        bpm *= 2.0;
    }
    (bpm, confidence)
}

fn max_pool(v: &[f32], factor: usize) -> Vec<f32> {
    v.chunks(factor).map(|c| c.iter().cloned().fold(0.0, f32::max)).collect()
}

fn fold_into(mut bpm: f64, range: BpmRange) -> f64 {
    while bpm < range.min {
        bpm *= 2.0;
    }
    while bpm >= range.max {
        bpm /= 2.0;
    }
    bpm
}

/// Circular beat histogram score and phase (seconds into the beat) for a candidate tempo.
fn phase_histogram(env: &[f32], frame_time: impl Fn(usize) -> f64, bpm: f64) -> (f32, f64) {
    const BINS: usize = 96;
    let period = 60.0 / bpm;
    let mut hist = [0f32; BINS];
    for (i, &e) in env.iter().enumerate() {
        if e <= 0.0 {
            continue;
        }
        let t = frame_time(i).rem_euclid(period) / period;
        hist[((t * BINS as f64) as usize).min(BINS - 1)] += e;
    }
    let mut best = (0usize, f32::MIN);
    for b in 0..BINS {
        let s = hist[(b + BINS - 1) % BINS] * 0.5 + hist[b] + hist[(b + 1) % BINS] * 0.5;
        if s > best.1 {
            best = (b, s);
        }
    }
    (best.1, (best.0 as f64 + 0.5) / BINS as f64 * period)
}

/// Kick-weighted energy rise at 1 kHz resolution, used to place the grid precisely.
fn fine_onsets(track: &TrackAudio) -> Vec<f32> {
    let sr = track.sample_rate as f64;
    let step = sr / FINE_RATE;
    let lp = (-std::f64::consts::TAU * 150.0 / sr).exp() as f32;
    let smooth = (-1.0 / (0.004 * sr)).exp() as f32;
    let (mut z1, mut z2, mut env) = (0.0f32, 0.0f32, 0.0f32);
    let mut energy = Vec::with_capacity((track.frames() as f64 / step) as usize + 1);
    let mut next = step;
    for i in 0..track.frames() {
        let (l, r) = track.frame(i);
        let x = 0.5 * (l + r);
        z1 = x + lp * (z1 - x);
        z2 = z1 + lp * (z2 - z1);
        let e = z2 * z2;
        env = e + smooth * (env - e);
        if (i + 1) as f64 >= next {
            energy.push(env);
            next += step;
        }
    }
    let mut rise = vec![0f32; energy.len()];
    for i in 1..energy.len() {
        rise[i] = (energy[i] - energy[i - 1]).max(0.0);
    }
    let max = rise.iter().cloned().fold(0.0f32, f32::max);
    if max > 0.0 {
        rise.iter_mut().for_each(|v| *v /= max);
    }
    rise
}

/// Mean high-band envelope over one beat period, folded across the whole track; bin 0 is phase 0.
fn folded_attack_profile(track: &TrackAudio, period: f64) -> (Vec<f64>, f64) {
    let sr = f64::from(track.sample_rate);
    let (env, rate) = envelope(track, Chain::highpass(ATTACK_SPLIT_HZ, sr, 2), ATTACK_SMOOTH_HZ, ATTACK_DECIMATION);
    let bins = (period * rate).round().max(1.0) as usize;
    let mut sum = vec![0.0f64; bins];
    let mut count = vec![0u32; bins];
    for (n, v) in env.iter().enumerate() {
        let frac = (n as f64 / rate).rem_euclid(period) / period;
        let b = ((frac * bins as f64) as usize).min(bins - 1);
        sum[b] += f64::from(*v);
        count[b] += 1;
    }
    let profile = sum.iter().zip(&count).map(|(s, c)| if *c > 0 { s / f64::from(*c) } else { 0.0 }).collect();
    (profile, period / bins as f64)
}

/// Grid offset on the transient attack: the steepest rise of the beat-folded high-band envelope near the
/// coarse phase, then its half-amplitude point.
fn attack_phase(track: &TrackAudio, bpm: f64, coarse_phase: f64) -> f64 {
    let period = 60.0 / bpm;
    let (profile, bin_secs) = folded_attack_profile(track, period);
    let n = profile.len() as i64;
    let at = |i: i64| profile[i.rem_euclid(n) as usize];
    let span = |a: i64, b: i64| (a..b).map(at).sum::<f64>();
    let w = ((ATTACK_RISE_SECS / bin_secs).round() as i64).max(1);
    let centre = (coarse_phase / bin_secs).round() as i64;
    let early = ((ATTACK_SEARCH_EARLY.max(-0.45 * period)) / bin_secs).round() as i64;
    let late = ((ATTACK_SEARCH_LATE.min(0.45 * period)) / bin_secs).round() as i64;
    let Some(rise) = (centre + early..=centre + late).max_by(|&a, &b| {
        (span(a, a + w) - span(a - w, a)).total_cmp(&(span(b, b + w) - span(b - w, b)))
    }) else {
        return coarse_phase.rem_euclid(period);
    };
    let secs = |s: f64| (s / bin_secs).round() as i64;
    let base = (rise - secs(0.020)..rise).map(at).fold(f64::INFINITY, f64::min);
    let peak = (rise..rise + secs(0.015)).map(at).fold(0.0, f64::max);
    let half = base + 0.5 * (peak - base);
    let onset = (rise - secs(0.010)..rise + secs(0.015)).find(|&i| at(i) >= half).unwrap_or(rise);
    ((onset as f64 + 0.5) * bin_secs).rem_euclid(period)
}

fn tidy_bpm(bpm: f64) -> f64 {
    let rounded = bpm.round();
    if (bpm - rounded).abs() < 0.03 { rounded } else { (bpm * 100.0).round() / 100.0 }
}

pub fn analyze_beatgrid(track: &TrackAudio, range: BpmRange) -> Option<BeatAnalysis> {
    if track.duration_secs() < 4.0 {
        return None;
    }
    let (mono, rate) = mono_decimated(track);
    let env = onset_envelope(&mono);
    if env.is_empty() {
        return None;
    }
    let frame_rate = rate / HOP as f64;
    let frame_time = |i: usize| (i * HOP + FFT_SIZE / 2) as f64 / rate;
    let fine = fine_onsets(track);
    let has_kick = fine.iter().any(|&v| v > 0.0);
    let pooled = max_pool(&fine, 10);
    let mut envs: Vec<(&[f32], f64)> = vec![(&env, frame_rate)];
    if has_kick {
        envs.push((&pooled, FINE_RATE / 10.0));
    }
    let (coarse, confidence) = coarse_tempo(&envs, range);

    let mut best = (coarse, 0.0, f32::MIN);
    let mut bpm = coarse - 1.0;
    while bpm <= coarse + 1.0 {
        let (score, phase) = phase_histogram(&env, frame_time, bpm);
        if score > best.2 {
            best = (bpm, phase, score);
        }
        bpm += 0.005;
    }
    let bpm = tidy_bpm(best.0);
    let (_, coarse_phase) = if has_kick {
        phase_histogram(&fine, |i| i as f64 / FINE_RATE, bpm)
    } else {
        phase_histogram(&env, frame_time, bpm)
    };
    let first_beat_secs = attack_phase(track, bpm, coarse_phase);
    Some(BeatAnalysis { grid: Beatgrid { bpm, first_beat_secs }, confidence })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn click_track(bpm: f64, offset: f64, secs: f64, sr: u32) -> TrackAudio {
        let n = (secs * sr as f64) as usize;
        let mut s = vec![0f32; n * 2];
        let period = 60.0 / bpm;
        let mut t = offset;
        let mut beat = 0;
        while t < secs {
            let start = (t * sr as f64) as usize;
            let accent = if beat % 4 == 0 { 1.0 } else { 0.7 };
            for k in 0..(0.08 * sr as f64) as usize {
                if start + k >= n {
                    break;
                }
                let tt = k as f64 / sr as f64;
                let freq = 55.0 + 120.0 * (-tt * 40.0).exp();
                let v = (std::f64::consts::TAU * freq * tt).sin() * (-tt * 30.0).exp() * accent;
                s[(start + k) * 2] += v as f32;
                s[(start + k) * 2 + 1] += v as f32;
            }
            for k in 0..(0.01 * sr as f64) as usize {
                let idx = start + (period * 0.5 * sr as f64) as usize + k;
                if idx >= n {
                    break;
                }
                let noise = ((idx as f32 * 12.9898).sin() * 43_758.547).fract() - 0.5;
                let env = (-(k as f32) / (0.002 * sr as f32)).exp();
                s[idx * 2] += noise * env * 0.3;
                s[idx * 2 + 1] += noise * env * 0.3;
            }
            t += period;
            beat += 1;
        }
        TrackAudio::new(sr, s)
    }

    #[test]
    fn detects_tempo_and_phase_of_synthetic_kicks() {
        for (bpm, offset) in [(128.0, 0.31), (122.0, 0.05), (174.0, 0.2), (95.5, 0.4)] {
            let track = click_track(bpm, offset, 40.0, 44_100);
            let a = analyze_beatgrid(&track, BpmRange::default()).unwrap();
            assert!((a.grid.bpm - bpm).abs() < 0.05, "bpm {bpm}: got {}", a.grid.bpm);
            let period = 60.0 / bpm;
            let err = (a.grid.first_beat_secs - offset).rem_euclid(period);
            let err = err.min(period - err);
            assert!(err < 0.008, "bpm {bpm}: phase err {err}");
        }
    }
}
