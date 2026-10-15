use std::f64::consts::TAU;

use super::filters::{HighPass, LowPass};
use super::util::Noise;
use crate::svf::Svf;

pub const SR: f32 = 48_000.0;

pub fn white(len: usize, seed: u32, amp: f32) -> Vec<f32> {
    let mut n = Noise::new(seed);
    (0..len).map(|_| n.next() * amp).collect()
}

pub fn db(ratio: f32) -> f32 {
    20.0 * ratio.max(1e-9).log10()
}

/// Amplitude of the component at `hz` over the whole slice (single-bin DFT).
pub fn tone_amplitude(x: &[f32], hz: f32) -> f32 {
    let w = TAU * hz as f64 / SR as f64;
    let (mut c, mut q) = (0.0f64, 0.0f64);
    for (i, v) in x.iter().enumerate() {
        let p = w * i as f64;
        c += *v as f64 * p.cos();
        q += *v as f64 * p.sin();
    }
    (2.0 * (c * c + q * q).sqrt() / x.len() as f64) as f32
}

/// Per-chunk frequency deviation from `nominal_hz` in cents, from the phase advance between consecutive chunks.
pub fn cents_deviation(x: &[f32], nominal_hz: f32, chunk: usize) -> Vec<f32> {
    let w = TAU * nominal_hz as f64 / SR as f64;
    let phases: Vec<f64> = x
        .chunks_exact(chunk)
        .enumerate()
        .map(|(k, c)| {
            let (mut re, mut im) = (0.0f64, 0.0f64);
            for (i, v) in c.iter().enumerate() {
                let p = w * (k * chunk + i) as f64;
                re += *v as f64 * p.cos();
                im -= *v as f64 * p.sin();
            }
            im.atan2(re)
        })
        .collect();
    phases
        .windows(2)
        .map(|p| {
            let mut d = p[1] - p[0];
            d -= TAU * (d / TAU).round();
            let hz_offset = d / (TAU * chunk as f64 / SR as f64);
            (1200.0 * (1.0 + hz_offset / nominal_hz as f64).log2()) as f32
        })
        .collect()
}

pub fn max_step(x: &[f32]) -> f32 {
    x.windows(2).fold(0.0, |m, w| m.max((w[1] - w[0]).abs()))
}

/// Mean frequency from interpolated upward zero crossings.
pub fn crossing_hz(x: &[f32]) -> f32 {
    let mut times = Vec::new();
    for (i, w) in x.windows(2).enumerate() {
        if w[0] < 0.0 && w[1] >= 0.0 {
            times.push(i as f32 + w[0].abs() / (w[1] - w[0]));
        }
    }
    if times.len() < 3 {
        return 0.0;
    }
    (times.len() - 1) as f32 * SR / (times[times.len() - 1] - times[0])
}

/// Mean power per hertz between `lo` and `hi`, from the energy of a band-passed copy.
pub fn band_psd(x: &[f32], lo: f32, hi: f32) -> f32 {
    let (mut hp, mut lp) = (HighPass::<2>::default(), LowPass::<2>::default());
    hp.set(lo, SR);
    lp.set(hi, SR);
    let power = x.iter().map(|v| lp.process(hp.process(*v)).powi(2)).sum::<f32>() / x.len() as f32;
    power / (hi - lo)
}

/// Reverberation time at `centre_hz` from the Schroeder integral of an impulse response (T20 doubled-up to T60).
pub fn rt60_at(response: &[f32], centre_hz: f32) -> f32 {
    let (mut first, mut second) = (Svf::new(centre_hz, 1.0, SR), Svf::new(centre_hz, 1.0, SR));
    let band: Vec<f32> = response.iter().map(|v| second.process(first.process(*v).band).band).collect();
    let mut edc = vec![0.0f64; band.len()];
    let mut acc = 0.0;
    for i in (0..band.len()).rev() {
        acc += (band[i] as f64).powi(2);
        edc[i] = acc;
    }
    let at = |level_db: f64| edc.iter().position(|v| 10.0 * (v / edc[0]).log10() < level_db).unwrap_or(edc.len()) as f32 / SR;
    3.0 * (at(-25.0) - at(-5.0))
}

pub fn semitones(from_hz: f32, to_hz: f32) -> f32 {
    12.0 * (to_hz / from_hz).log2()
}

/// Power-weighted mean frequency between `lo` and `hi` of a Hann-windowed slice.
pub fn centroid_hz(x: &[f32], lo: f32, hi: f32) -> f32 {
    let n = x.len() as f64;
    let windowed: Vec<f32> = x.iter().enumerate().map(|(i, v)| v * (0.5 - 0.5 * (TAU * i as f64 / n).cos()) as f32).collect();
    let (mut weighted, mut total) = (0.0f64, 0.0f64);
    let mut hz = lo;
    while hz <= hi {
        let a = tone_amplitude(&windowed, hz) as f64;
        weighted += a * a * (hz as f64).ln();
        total += a * a;
        hz *= 1.004;
    }
    (weighted / total.max(1e-30)).exp() as f32
}
