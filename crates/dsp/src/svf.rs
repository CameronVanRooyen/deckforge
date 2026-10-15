use std::f32::consts::PI;

/// Topology-preserving-transform state variable filter; stable under fast modulation.
#[derive(Debug, Clone, Copy, Default)]
pub struct Svf {
    a1: f32,
    a2: f32,
    a3: f32,
    k: f32,
    ic1: f32,
    ic2: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct SvfOut {
    pub low: f32,
    pub band: f32,
    pub high: f32,
}

impl Svf {
    pub fn new(cutoff: f32, q: f32, sample_rate: f32) -> Self {
        let mut f = Self::default();
        f.set(cutoff, q, sample_rate);
        f
    }

    pub fn set(&mut self, cutoff: f32, q: f32, sample_rate: f32) {
        let fc = cutoff.clamp(5.0, sample_rate * 0.49);
        self.set_prewarped((PI * fc / sample_rate).tan(), 1.0 / q);
    }

    /// Analog prototype `wc² / (s² + damping·wc·s + wc²)` through an unwarped bilinear transform: `cutoff` is the analog
    /// corner, so it lands lower in the digital response and may exceed Nyquist.
    pub fn set_analog(&mut self, cutoff: f32, damping: f32, sample_rate: f32) {
        self.set_prewarped(PI * cutoff.max(1.0) / sample_rate, damping);
    }

    /// `g` is `tan(π·fc/sr)` (or any analog `ωc·T/2`); `damping` is `1/Q`.
    pub fn set_prewarped(&mut self, g: f32, damping: f32) {
        self.k = damping;
        self.a1 = 1.0 / (1.0 + g * (g + self.k));
        self.a2 = g * self.a1;
        self.a3 = g * self.a2;
    }

    pub fn reset(&mut self) {
        self.ic1 = 0.0;
        self.ic2 = 0.0;
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> SvfOut {
        let v3 = x - self.ic2;
        let v1 = self.a1 * self.ic1 + self.a2 * v3;
        let v2 = self.ic2 + self.a2 * self.ic1 + self.a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        SvfOut { low: v2, band: v1, high: x - self.k * v1 - v2 }
    }

    #[inline]
    pub fn allpass(&mut self, x: f32) -> f32 {
        let k = self.k;
        let o = self.process(x);
        x - 2.0 * k * o.band
    }
}
