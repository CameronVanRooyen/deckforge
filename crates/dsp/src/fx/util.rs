use std::f32::consts::TAU;

use crate::smooth::Smoothed;

#[inline]
pub fn flush(x: f32) -> f32 {
    if x.abs() < 1e-15 { 0.0 } else { x }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct OnePole {
    z: f32,
    a: f32,
}

impl OnePole {
    pub fn set_cutoff(&mut self, cutoff: f32, sample_rate: f32) {
        self.a = 1.0 - (-TAU * cutoff.clamp(1.0, sample_rate * 0.45) / sample_rate).exp();
    }

    #[inline]
    pub fn lp(&mut self, x: f32) -> f32 {
        self.z = flush(self.z + self.a * (x - self.z));
        self.z
    }

    #[inline]
    pub fn hp(&mut self, x: f32) -> f32 {
        x - self.lp(x)
    }

    pub fn reset(&mut self) {
        self.z = 0.0;
    }
}

/// Phase accumulator in cycles, [0, 1).
#[derive(Debug, Clone, Copy, Default)]
pub struct Lfo {
    pub phase: f32,
}

impl Lfo {
    #[inline]
    pub fn advance(&mut self, increment: f32) {
        self.phase += increment;
        self.phase -= self.phase.floor();
    }

    #[inline]
    pub fn sine_at(&self, offset: f32) -> f32 {
        (TAU * (self.phase + offset)).sin()
    }

    pub fn set_cycles(&mut self, cycles: f64) {
        self.phase = cycles.rem_euclid(1.0) as f32;
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Noise(u32);

impl Noise {
    pub fn new(seed: u32) -> Self {
        Self(seed.max(1))
    }

    /// White noise in [-1, 1).
    #[inline]
    pub fn next(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x >> 8) as f32 / 8_388_608.0 - 1.0
    }
}

/// Dips a wet path to silence, lets the owner reset its state, then ramps back.
#[derive(Debug, Clone, Copy)]
pub struct SwapGate {
    gain: Smoothed,
    swapping: bool,
}

impl SwapGate {
    pub fn new(sample_rate: f32) -> Self {
        Self { gain: Smoothed::new(1.0, sample_rate, 8.0), swapping: false }
    }

    pub fn begin(&mut self) {
        self.swapping = true;
        self.gain.set(0.0);
    }

    pub fn is_swapping(&self) -> bool {
        self.swapping
    }

    #[inline]
    pub fn next(&mut self) -> f32 {
        self.gain.next()
    }

    /// True once the dip is deep enough for the owner to swap state.
    pub fn ready(&self) -> bool {
        self.swapping && self.gain.value() < 1e-3
    }

    pub fn finish(&mut self) {
        self.swapping = false;
        self.gain.reset(0.0);
        self.gain.set(1.0);
    }

    pub fn reset_open(&mut self) {
        self.swapping = false;
        self.gain.reset(1.0);
    }
}

/// Schroeder all-pass diffuser with a fixed buffer.
#[derive(Debug, Clone)]
pub struct AllPass {
    buf: Vec<f32>,
    pos: usize,
    gain: f32,
}

impl AllPass {
    pub fn new(len: usize, gain: f32) -> Self {
        Self { buf: vec![0.0; len.max(2)], pos: 0, gain }
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let delayed = self.buf[self.pos];
        let w = flush(x + self.gain * delayed);
        self.buf[self.pos] = w;
        self.pos += 1;
        if self.pos == self.buf.len() {
            self.pos = 0;
        }
        delayed - self.gain * w
    }

    pub fn reset(&mut self) {
        self.buf.fill(0.0);
        self.pos = 0;
    }
}
