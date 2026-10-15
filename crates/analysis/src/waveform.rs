use deckforge_common::TrackAudio;

use crate::filter::Chain;
use serde::{Deserialize, Serialize};

pub const WAVEFORM_POINTS_PER_SEC: f64 = 150.0;

/// Display heights per point, 0..255: full-band peak (square law), low/mid (linear) and high (raised cosine).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Waveform {
    pub points_per_sec: f64,
    pub peak: Vec<u8>,
    pub low: Vec<u8>,
    pub mid: Vec<u8>,
    pub high: Vec<u8>,
}

impl Waveform {
    pub fn len(&self) -> usize {
        self.peak.len()
    }

    pub fn is_empty(&self) -> bool {
        self.peak.is_empty()
    }

    /// Packs as [len:u32 LE][points_per_sec:f64 LE][peak][low][mid][high] for compact storage/transport.
    pub fn to_bytes(&self) -> Vec<u8> {
        let n = self.len();
        let mut out = Vec::with_capacity(12 + n * 4);
        out.extend_from_slice(&(n as u32).to_le_bytes());
        out.extend_from_slice(&self.points_per_sec.to_le_bytes());
        for band in [&self.peak, &self.low, &self.mid, &self.high] {
            out.extend_from_slice(band);
        }
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let n = u32::from_le_bytes(bytes.get(0..4)?.try_into().ok()?) as usize;
        let pps = f64::from_le_bytes(bytes.get(4..12)?.try_into().ok()?);
        let body = bytes.get(12..12 + n * 4)?;
        Some(Self {
            points_per_sec: pps,
            peak: body[..n].to_vec(),
            low: body[n..2 * n].to_vec(),
            mid: body[2 * n..3 * n].to_vec(),
            high: body[3 * n..].to_vec(),
        })
    }
}

const MS_PER_SEC: f64 = 1_000.0;
const STEREO_DIFFERENCE: f32 = 0.001;
const RELEASE_PER_MS: [f32; 3] = [0.99, 0.98, 0.97];
const GAIN_BINS: usize = 1_200;
const GAIN_TARGET: [f32; 3] = [0.5, 0.5, HIGH_GAIN_TARGET];
const HIGH_GAIN_TARGET: f32 = 0.3;
const GAIN_MIN: f32 = 0.8;
const GAIN_MAX: [f32; 3] = [3.0, 3.0, 5.0];
const LINEAR_SCALE: f32 = 256.0;

/// Mono mix: the average of both channels, or the louder one when they are practically equal.
fn mono(l: f32, r: f32) -> f32 {
    if (l.abs() - r.abs()).abs() >= STEREO_DIFFERENCE {
        0.5 * (l + r)
    } else if l.abs() >= r.abs() {
        l
    } else {
        r
    }
}

/// Per-millisecond absolute peaks: full band, low (400 Hz low-pass), mid (200 Hz - 2 kHz) and high (> 2 kHz).
fn millisecond_peaks(track: &TrackAudio) -> [Vec<f32>; 4] {
    let sr = f64::from(track.sample_rate);
    let mut low = Chain::lowpass(400.0, sr, 2);
    let mut mid = Chain::bandpass(200.0, 2_000.0, sr, 2);
    let mut high = Chain::highpass(2_000.0, sr, 2);
    let capacity = (track.duration_secs() * MS_PER_SEC).ceil() as usize + 1;
    let mut out: [Vec<f32>; 4] = std::array::from_fn(|_| Vec::with_capacity(capacity));
    let mut acc = [0.0f32; 4];
    let step = sr / MS_PER_SEC;
    let mut next = step;
    for i in 0..track.frames() {
        let (l, r) = track.frame(i);
        let x = mono(l, r);
        let xd = f64::from(x);
        let v = [x.abs(), low.process(xd).abs() as f32, mid.process(xd).abs() as f32, high.process(xd).abs() as f32];
        for (a, v) in acc.iter_mut().zip(v) {
            *a = a.max(v);
        }
        if (i + 1) as f64 >= next || i + 1 == track.frames() {
            for (o, a) in out.iter_mut().zip(&mut acc) {
                o.push(*a);
                *a = 0.0;
            }
            next += step;
        }
    }
    out
}

/// Instant attack, exponential release per millisecond.
fn hold(peaks: &mut [f32], release: f32) {
    let mut y = 0.0f32;
    for p in peaks.iter_mut() {
        y = if *p > y { *p } else { *p + (y - *p) * release };
        *p = y;
    }
}

fn to_points(ms: &[f32], points: usize) -> Vec<f32> {
    let per_point = MS_PER_SEC / WAVEFORM_POINTS_PER_SEC;
    (0..points)
        .map(|i| {
            let a = ((i as f64 * per_point) as usize).min(ms.len());
            let b = (((i + 1) as f64 * per_point) as usize).clamp(a + 1, ms.len().max(a + 1));
            ms.get(a..b.min(ms.len())).map_or(0.0, |s| s.iter().cloned().fold(0.0, f32::max))
        })
        .collect()
}

/// Per-track band gain that brings the band's typical loudness (mean of 1200 whole-track bin peaks) to a target.
fn band_gain(points: &[f32], band: usize) -> f32 {
    if points.is_empty() {
        return 1.0;
    }
    let bins = GAIN_BINS.min(points.len());
    let mean = (0..bins)
        .map(|b| points[b * points.len() / bins..((b + 1) * points.len() / bins).max(b * points.len() / bins + 1)].iter().cloned().fold(0.0, f32::max))
        .sum::<f32>()
        / bins as f32;
    if mean <= 1e-6 { 1.0 } else { (GAIN_TARGET[band] / mean).clamp(GAIN_MIN, GAIN_MAX[band]) }
}

fn byte(v: f32) -> u8 {
    v.round().clamp(0.0, 255.0) as u8
}

/// Display heights per point (0..255): square-law full-band peak, linear low and mid, raised-cosine high, with
/// per-band peak hold and per-track band gains.
pub fn compute_waveform(track: &TrackAudio) -> Waveform {
    let points = (track.duration_secs() * WAVEFORM_POINTS_PER_SEC).ceil() as usize;
    let [full, mut low, mut mid, mut high] = millisecond_peaks(track);
    for (band, release) in [&mut low, &mut mid, &mut high].into_iter().zip(RELEASE_PER_MS) {
        hold(band, release);
    }
    let full = to_points(&full, points);
    let bands: Vec<Vec<f32>> = [&low, &mid, &high].iter().map(|b| to_points(b, points)).collect();
    let gains: Vec<f32> = bands.iter().enumerate().map(|(i, b)| band_gain(b, i)).collect();
    let max = full.iter().cloned().fold(0.0, f32::max).max(1e-9);
    Waveform {
        points_per_sec: WAVEFORM_POINTS_PER_SEC,
        peak: full.iter().map(|p| byte(255.0 * (p / max).powi(2))).collect(),
        low: bands[0].iter().map(|p| byte(gains[0] * p * LINEAR_SCALE)).collect(),
        mid: bands[1].iter().map(|p| byte(gains[1] * p * LINEAR_SCALE)).collect(),
        high: bands[2]
            .iter()
            .map(|p| byte(127.5 * (1.0 - (std::f32::consts::PI * (gains[2] * p).min(1.0)).cos())))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_count_matches_duration_and_bands_separate() {
        let sr = 44_100;
        let n = sr as usize * 2;
        let samples: Vec<f32> = (0..n)
            .flat_map(|i| {
                let s = (std::f32::consts::TAU * 60.0 * i as f32 / sr as f32).sin() * 0.8;
                [s, s]
            })
            .collect();
        let wf = compute_waveform(&TrackAudio::new(sr, samples));
        assert!((wf.len() as f64 - 300.0).abs() <= 1.0);
        let mid_point = wf.len() / 2;
        assert!(wf.low[mid_point] > 150);
        assert!(wf.high[mid_point] < 40);
        let back = Waveform::from_bytes(&wf.to_bytes()).unwrap();
        assert_eq!(back.peak, wf.peak);
        assert_eq!(back.high, wf.high);
    }

    fn burst_then_silence(sr: u32, freq: f32) -> TrackAudio {
        let n = sr as usize * 2;
        let burst = sr as usize / 2;
        let samples: Vec<f32> = (0..n)
            .flat_map(|i| {
                let amp = if i < burst / 2 { 0.8 } else if i < burst { 0.4 } else { 0.0 };
                let s = (std::f32::consts::TAU * freq * i as f32 / sr as f32).sin() * amp;
                [s, s]
            })
            .collect();
        TrackAudio::new(sr, samples)
    }

    #[test]
    fn peaks_are_square_law_and_bands_release_slowly() {
        let wf = compute_waveform(&burst_then_silence(44_100, 100.0));
        let (loud, half) = (wf.peak[20], wf.peak[60]);
        assert!(loud > 250 && (f32::from(half) - 255.0 / 4.0).abs() < 4.0, "peak {loud} / {half}");
        let end = 75;
        let after = |ms: f64| wf.low[end + (ms * WAVEFORM_POINTS_PER_SEC / 1000.0) as usize];
        let ratio = f32::from(after(100.0)) / f32::from(wf.low[end - 2]);
        assert!((0.3..0.45).contains(&ratio), "low after 100 ms: {ratio}");
        assert_eq!(wf.peak[end + 15], 0, "the full-band peak has no release");
    }
}
