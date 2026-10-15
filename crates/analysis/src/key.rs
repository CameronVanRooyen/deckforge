use deckforge_common::TrackAudio;
use realfft::RealFftPlanner;
use serde::{Deserialize, Serialize};

const TARGET_RATE: f64 = 11_025.0;
const FFT_SIZE: usize = 8192;
const HOP: usize = 4096;
const MIN_HZ: f64 = 55.0;
const MAX_HZ: f64 = 4_000.0;
const TUNING_MIN_HZ: f64 = 110.0;
const HARMONIC_ORDERS: [u32; 2] = [3, 5];
const HARMONIC_MIN_SOURCE: f32 = 0.3;
const HARMONIC_ATTENUATION: f32 = 0.4;
const WHITEN_RADIUS: usize = 48;
const PEAK_RATIO: f32 = 3.0;

const MAJOR_PROFILE: [f64; 12] = [6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88];
const MINOR_PROFILE: [f64; 12] = [6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17];

const MAJOR_NAMES: [&str; 12] = ["C", "Db", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"];
const MINOR_NAMES: [&str; 12] = ["Cm", "C#m", "Dm", "Ebm", "Em", "Fm", "F#m", "Gm", "G#m", "Am", "Bbm", "Bm"];

/// Musical key: tonic pitch class (0 = C) and mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Key {
    pub tonic: u8,
    pub minor: bool,
}

impl Key {
    pub fn new(tonic: u8, minor: bool) -> Self {
        Self { tonic: tonic % 12, minor }
    }

    /// Camelot wheel code, e.g. "8A" for A minor and "8B" for C major.
    pub fn camelot(self) -> String {
        let major_tonic = if self.minor { (self.tonic + 3) % 12 } else { self.tonic };
        let number = (u32::from(major_tonic) * 7 % 12 + 7) % 12 + 1;
        format!("{number}{}", if self.minor { 'A' } else { 'B' })
    }

    /// Musical notation, e.g. "Am", "F#", "Bbm".
    pub fn name(self) -> &'static str {
        let names = if self.minor { &MINOR_NAMES } else { &MAJOR_NAMES };
        names[usize::from(self.tonic)]
    }

    pub fn from_camelot(number: u8, minor: bool) -> Option<Self> {
        if !(1..=12).contains(&number) {
            return None;
        }
        let major_tonic = ((u32::from(number) + 4) % 12 * 7 % 12) as u8;
        let tonic = if minor { (major_tonic + 9) % 12 } else { major_tonic };
        Some(Self::new(tonic, minor))
    }

    /// Parses key tags in note names ("Am", "F# minor", "Dbm"), Camelot ("8A") or Open Key ("1m") notation.
    pub fn parse(tag: &str) -> Option<Self> {
        let cleaned: String = tag
            .chars()
            .filter(|c| !c.is_whitespace())
            .map(|c| match c {
                '♯' => '#',
                '♭' => 'b',
                other => other,
            })
            .collect();
        let lower = cleaned.to_ascii_lowercase();
        let lower = lower.strip_prefix("openkey").unwrap_or(&lower);
        let digits = lower.chars().take_while(char::is_ascii_digit).count();
        if (1..=2).contains(&digits) && digits < lower.len() {
            let number: u8 = lower[..digits].parse().ok()?;
            return match &lower[digits..] {
                "a" => Self::from_camelot(number, true),
                "b" => Self::from_camelot(number, false),
                "m" => Self::from_camelot((number + 6) % 12 + 1, true),
                "d" => Self::from_camelot((number + 6) % 12 + 1, false),
                _ => None,
            };
        }
        Self::parse_note(&cleaned)
    }

    fn parse_note(text: &str) -> Option<Self> {
        let mut chars = text.chars();
        let letter = chars.next()?.to_ascii_uppercase();
        let mut tonic: i32 = match letter {
            'C' => 0,
            'D' => 2,
            'E' => 4,
            'F' => 5,
            'G' => 7,
            'A' => 9,
            'B' => 11,
            _ => return None,
        };
        let mut rest = chars.as_str();
        if let Some(r) = rest.strip_prefix('#') {
            tonic += 1;
            rest = r;
        } else if let Some(r) = rest.strip_prefix('b') {
            tonic -= 1;
            rest = r;
        }
        let minor = match rest {
            "" | "M" | "maj" | "Maj" | "major" | "Major" | "dur" => false,
            "m" | "-" | "min" | "Min" | "minor" | "Minor" | "mol" => true,
            other => match other.to_ascii_lowercase().as_str() {
                "major" | "maj" => false,
                "minor" | "min" => true,
                _ => return None,
            },
        };
        Some(Self::new(tonic.rem_euclid(12) as u8, minor))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct KeyEstimate {
    pub key: Key,
    /// 0..1, margin of the best key over the strongest unrelated alternative.
    pub confidence: f32,
}

fn decimate(track: &TrackAudio) -> (Vec<f32>, f64) {
    let factor = (f64::from(track.sample_rate) / TARGET_RATE).round().max(1.0) as usize;
    let frames = track.frames();
    let weights: Vec<f32> = (0..2 * factor - 1).map(|i| 1.0 - (i as f32 - (factor - 1) as f32).abs() / factor as f32).collect();
    let norm = factor as f32 * 2.0;
    let mut out = Vec::with_capacity(frames / factor + 1);
    let mut centre = 0;
    while centre < frames {
        let first = centre as isize - (factor as isize - 1);
        let mut acc = 0.0f32;
        for (i, w) in weights.iter().enumerate() {
            let idx = first + i as isize;
            if idx >= 0 && (idx as usize) < frames {
                let (l, r) = track.frame(idx as usize);
                acc += w * (l + r);
            }
        }
        out.push(acc / norm);
        centre += factor;
    }
    (out, f64::from(track.sample_rate) / factor as f64)
}

struct Peak {
    midi: f64,
    weight: f32,
}

fn attenuate_harmonics(peaks: &mut [Peak]) {
    let original: Vec<(f64, f32)> = peaks.iter().map(|p| (p.midi, p.weight)).collect();
    for peak in peaks.iter_mut() {
        let is_harmonic = HARMONIC_ORDERS.iter().any(|&h| {
            let target = peak.midi - 12.0 * f64::from(h).log2();
            original.iter().any(|&(m, w)| (m - target).abs() < 0.4 && w >= HARMONIC_MIN_SOURCE * peak.weight)
        });
        if is_harmonic {
            peak.weight *= HARMONIC_ATTENUATION;
        }
    }
}

fn frame_peaks(mag: &[f32], bin_hz: f64, out: &mut Vec<Peak>, smooth: &mut Vec<f32>) {
    let lo = (MIN_HZ / bin_hz).floor() as usize;
    let hi = ((MAX_HZ / bin_hz).ceil() as usize).min(mag.len() - 2);
    smooth.clear();
    let mut prefix = Vec::with_capacity(mag.len() + 1);
    prefix.push(0.0f64);
    for m in mag {
        prefix.push(prefix.last().copied().unwrap_or(0.0) + f64::from(*m));
    }
    for k in 0..mag.len() {
        let a = k.saturating_sub(WHITEN_RADIUS);
        let b = (k + WHITEN_RADIUS + 1).min(mag.len());
        smooth.push(((prefix[b] - prefix[a]) / (b - a) as f64) as f32);
    }
    let floor = mag.iter().cloned().fold(0.0f32, f32::max) * 1e-4;
    for k in lo.max(1)..=hi {
        let m = mag[k];
        if m <= floor || m < mag[k - 1] || m <= mag[k + 1] {
            continue;
        }
        let ratio = m / smooth[k].max(1e-12);
        if ratio < PEAK_RATIO {
            continue;
        }
        let (a, b, c) = (mag[k - 1].max(1e-12).ln(), m.ln(), mag[k + 1].max(1e-12).ln());
        let denom = a - 2.0 * b + c;
        let delta = if denom.abs() > 1e-9 { (0.5 * (a - c) / denom).clamp(-0.5, 0.5) } else { 0.0 };
        let freq = (k as f64 + f64::from(delta)) * bin_hz;
        out.push(Peak { midi: 69.0 + 12.0 * (freq / 440.0).log2(), weight: (ratio / PEAK_RATIO).ln() + 0.5 });
    }
}

fn tuning_offset(frames: &[Vec<Peak>]) -> f64 {
    let (mut re, mut im) = (0.0f64, 0.0f64);
    let min_midi = 69.0 + 12.0 * (TUNING_MIN_HZ / 440.0).log2();
    for p in frames.iter().flatten().filter(|p| p.midi >= min_midi) {
        let angle = std::f64::consts::TAU * p.midi;
        re += f64::from(p.weight) * angle.cos();
        im += f64::from(p.weight) * angle.sin();
    }
    if re * re + im * im < 1e-9 {
        return 0.0;
    }
    im.atan2(re) / std::f64::consts::TAU
}

fn chroma(frames: &[Vec<Peak>], tuning: f64) -> [f64; 12] {
    let mut total = [0.0f64; 12];
    for frame in frames {
        let mut local = [0.0f64; 12];
        for p in frame {
            let pc = (p.midi - tuning).rem_euclid(12.0);
            let lower = pc.floor();
            let frac = pc - lower;
            let w = f64::from(p.weight);
            local[lower as usize % 12] += w * (1.0 - frac);
            local[(lower as usize + 1) % 12] += w * frac;
        }
        for (t, l) in total.iter_mut().zip(local) {
            *t += l;
        }
    }
    total
}

fn pearson(a: &[f64; 12], b: &[f64; 12]) -> f64 {
    let (ma, mb) = (a.iter().sum::<f64>() / 12.0, b.iter().sum::<f64>() / 12.0);
    let (mut num, mut da, mut db) = (0.0, 0.0, 0.0);
    for i in 0..12 {
        let (x, y) = (a[i] - ma, b[i] - mb);
        num += x * y;
        da += x * x;
        db += y * y;
    }
    if da <= 0.0 || db <= 0.0 { 0.0 } else { num / (da * db).sqrt() }
}

fn best_key(chroma: &[f64; 12]) -> Option<KeyEstimate> {
    if chroma.iter().sum::<f64>() <= 0.0 {
        return None;
    }
    let mut scores: Vec<(Key, f64)> = Vec::with_capacity(24);
    for tonic in 0..12u8 {
        for minor in [false, true] {
            let profile = if minor { &MINOR_PROFILE } else { &MAJOR_PROFILE };
            let mut rotated = [0.0; 12];
            for (i, r) in rotated.iter_mut().enumerate() {
                *r = profile[(i + 12 - usize::from(tonic)) % 12];
            }
            scores.push((Key::new(tonic, minor), pearson(chroma, &rotated)));
        }
    }
    scores.sort_by(|a, b| b.1.total_cmp(&a.1));
    let (best, best_score) = scores[0];
    let runner_up = scores
        .iter()
        .skip(1)
        .find(|(k, _)| !is_neighbour(best, *k))
        .map_or(0.0, |(_, s)| *s);
    let confidence = ((best_score - runner_up) * 4.0).clamp(0.0, 1.0) as f32;
    Some(KeyEstimate { key: best, confidence })
}

fn is_neighbour(a: Key, b: Key) -> bool {
    let (ca, cb) = (a.camelot(), b.camelot());
    let parse = |c: &str| -> (i32, bool) { (c[..c.len() - 1].parse().unwrap_or(0), c.ends_with('A')) };
    let ((na, ma), (nb, mb)) = (parse(&ca), parse(&cb));
    let dist = (na - nb).rem_euclid(12).min((nb - na).rem_euclid(12));
    (dist == 0 && ma != mb) || (dist == 1 && ma == mb)
}

/// Detects the global musical key from tonal spectral peaks, robust to tuning offsets.
pub fn detect_key(track: &TrackAudio) -> Option<KeyEstimate> {
    let (mono, rate) = decimate(track);
    if mono.len() < FFT_SIZE {
        return None;
    }
    let mut planner = RealFftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(FFT_SIZE);
    let window: Vec<f32> = (0..FFT_SIZE)
        .map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / FFT_SIZE as f32).cos())
        .collect();
    let mut input = fft.make_input_vec();
    let mut spectrum = fft.make_output_vec();
    let mut mag = vec![0f32; spectrum.len()];
    let mut smooth = Vec::new();
    let bin_hz = rate / FFT_SIZE as f64;
    let count = (mono.len() - FFT_SIZE) / HOP + 1;
    let mut frames = Vec::with_capacity(count);
    for f in 0..count {
        let start = f * HOP;
        for (k, x) in input.iter_mut().enumerate() {
            *x = mono[start + k] * window[k];
        }
        if fft.process(&mut input, &mut spectrum).is_err() {
            continue;
        }
        for (m, c) in mag.iter_mut().zip(&spectrum) {
            *m = c.norm();
        }
        let mut peaks = Vec::new();
        frame_peaks(&mag, bin_hz, &mut peaks, &mut smooth);
        attenuate_harmonics(&mut peaks);
        frames.push(peaks);
    }
    let tuning = tuning_offset(&frames);
    best_key(&chroma(&frames, tuning).map(f64::sqrt))
}
