/// Dual-tap crossfaded delay-line pitch shifter; fixed buffer, no allocation after `new`.
#[derive(Debug, Clone)]
pub struct PitchShifter {
    buf: Vec<f32>,
    mask: usize,
    pos: usize,
    window: f32,
    phase: f32,
}

impl PitchShifter {
    pub fn new(window_samples: usize) -> Self {
        let cap = (window_samples * 2 + 8).next_power_of_two();
        Self { buf: vec![0.0; cap], mask: cap - 1, pos: 0, window: window_samples as f32, phase: 0.0 }
    }

    pub fn latency(&self) -> f32 {
        self.window * 0.5 + 1.0
    }

    pub fn reset(&mut self) {
        self.buf.fill(0.0);
        self.phase = 0.0;
    }

    #[inline]
    fn tap(&self, delay: f32) -> f32 {
        let k = delay as usize;
        let f = delay - k as f32;
        let a = self.buf[self.pos.wrapping_sub(k) & self.mask];
        let b = self.buf[self.pos.wrapping_sub(k + 1) & self.mask];
        a + (b - a) * f
    }

    #[inline]
    pub fn process(&mut self, x: f32, ratio: f32) -> f32 {
        self.buf[self.pos] = x;
        self.pos = (self.pos + 1) & self.mask;
        let p0 = self.phase;
        let p1 = if p0 >= 0.5 { p0 - 0.5 } else { p0 + 0.5 };
        let w0 = (std::f32::consts::PI * p0).sin().powi(2);
        let w1 = 1.0 - w0;
        let out = self.tap(1.0 + p0 * self.window) * w0 + self.tap(1.0 + p1 * self.window) * w1;
        self.phase += (1.0 - ratio) / self.window;
        self.phase -= self.phase.floor();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    fn zero_crossing_hz(x: &[f32], sr: f32) -> f32 {
        let crossings = x.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
        crossings as f32 * sr / x.len() as f32
    }

    #[test]
    fn shifts_a_sine_by_the_requested_ratio() {
        let sr = 48_000.0;
        for ratio in [0.8909f32, 1.0, 1.1225, 1.5] {
            let mut shifter = PitchShifter::new(1_920);
            let out: Vec<f32> =
                (0..48_000).map(|i| shifter.process(0.5 * (TAU * 1_000.0 * i as f32 / sr).sin(), ratio)).collect();
            let hz = zero_crossing_hz(&out[9_600..], sr);
            assert!((hz / 1_000.0 - ratio).abs() < 0.03, "ratio {ratio}: measured {hz} Hz");
            assert!(out.iter().all(|v| v.is_finite() && v.abs() < 0.6));
        }
    }
}
