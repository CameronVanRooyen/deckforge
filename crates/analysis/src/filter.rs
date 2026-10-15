use std::f64::consts::{PI, SQRT_2};

use deckforge_common::TrackAudio;

/// Direct-form-I biquad from the RBJ audio-EQ cookbook.
#[derive(Debug, Clone, Copy)]
pub struct Biquad {
    b: [f64; 3],
    a: [f64; 2],
    x: [f64; 2],
    y: [f64; 2],
}

impl Biquad {
    fn from_rbj(b: [f64; 3], a0: f64, a1: f64, a2: f64) -> Self {
        Self { b: b.map(|v| v / a0), a: [a1 / a0, a2 / a0], x: [0.0; 2], y: [0.0; 2] }
    }

    fn omega(freq: f64, sr: f64) -> (f64, f64) {
        let w = 2.0 * PI * (freq / sr).min(0.49);
        (w.cos(), w.sin() / SQRT_2)
    }

    pub fn lowpass(freq: f64, sr: f64) -> Self {
        let (c, alpha) = Self::omega(freq, sr);
        let k = (1.0 - c) / 2.0;
        Self::from_rbj([k, 2.0 * k, k], 1.0 + alpha, -2.0 * c, 1.0 - alpha)
    }

    pub fn highpass(freq: f64, sr: f64) -> Self {
        let (c, alpha) = Self::omega(freq, sr);
        let k = (1.0 + c) / 2.0;
        Self::from_rbj([k, -2.0 * k, k], 1.0 + alpha, -2.0 * c, 1.0 - alpha)
    }

    #[inline]
    pub fn process(&mut self, x: f64) -> f64 {
        let y = self.b[0] * x + self.b[1] * self.x[0] + self.b[2] * self.x[1] - self.a[0] * self.y[0] - self.a[1] * self.y[1];
        self.x = [x, self.x[0]];
        self.y = [y, self.y[0]];
        y
    }
}

/// A chain of biquads applied in series (e.g. two Butterworth sections form a Linkwitz-Riley filter).
#[derive(Debug, Clone)]
pub struct Chain(Vec<Biquad>);

impl Chain {
    pub fn new(stages: impl IntoIterator<Item = Biquad>) -> Self {
        Self(stages.into_iter().collect())
    }

    pub fn lowpass(freq: f64, sr: f64, stages: usize) -> Self {
        Self::new((0..stages).map(|_| Biquad::lowpass(freq, sr)))
    }

    pub fn highpass(freq: f64, sr: f64, stages: usize) -> Self {
        Self::new((0..stages).map(|_| Biquad::highpass(freq, sr)))
    }

    pub fn bandpass(lo: f64, hi: f64, sr: f64, stages: usize) -> Self {
        Self::new((0..stages).map(|_| Biquad::highpass(lo, sr)).chain((0..stages).map(|_| Biquad::lowpass(hi, sr))))
    }

    #[inline]
    pub fn process(&mut self, x: f64) -> f64 {
        self.0.iter_mut().fold(x, |v, s| s.process(v))
    }
}

pub fn mono(track: &TrackAudio, i: usize) -> f64 {
    let (l, r) = track.frame(i);
    0.5 * f64::from(l + r)
}

/// Filters the mono mix, rectifies, smooths and decimates by `factor`; returns the envelope and its rate.
pub fn envelope(track: &TrackAudio, mut band: Chain, smooth_hz: f64, factor: usize) -> (Vec<f32>, f64) {
    let sr = f64::from(track.sample_rate);
    let factor = factor.max(1);
    let mut smooth = Chain::lowpass(smooth_hz, sr, 1);
    let mut out = Vec::with_capacity(track.frames() / factor + 1);
    for i in 0..track.frames() {
        let y = smooth.process(band.process(mono(track, i)).abs());
        if i % factor == 0 {
            out.push(y.max(0.0) as f32);
        }
    }
    (out, sr / factor as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gain(mut f: Chain, freq: f64, sr: f64) -> f64 {
        let n = (sr * 0.5) as usize;
        let mut peak = 0.0f64;
        for i in 0..n {
            let y = f.process((2.0 * PI * freq * i as f64 / sr).sin());
            if i > n / 2 {
                peak = peak.max(y.abs());
            }
        }
        peak
    }

    #[test]
    fn butterworth_sections_pass_and_reject() {
        let sr = 44_100.0;
        assert!((gain(Chain::lowpass(200.0, sr, 1), 200.0, sr) - std::f64::consts::FRAC_1_SQRT_2).abs() < 0.01);
        assert!(gain(Chain::lowpass(200.0, sr, 2), 50.0, sr) > 0.95);
        assert!(gain(Chain::lowpass(200.0, sr, 2), 2_000.0, sr) < 0.02);
        assert!(gain(Chain::highpass(2_000.0, sr, 2), 8_000.0, sr) > 0.95);
        assert!(gain(Chain::highpass(2_000.0, sr, 2), 200.0, sr) < 0.02);
        assert!((gain(Chain::lowpass(200.0, sr, 2), 200.0, sr) - 0.5).abs() < 0.01);
    }
}
