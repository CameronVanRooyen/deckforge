use std::f32::consts::FRAC_PI_2;

use super::ring::StereoRing;

const DIP_SECS: f32 = 0.003;
const TOLERANCE_FRACTION: f32 = 0.004;
const TOLERANCE_SECS: f32 = 0.001;
const ALIGN_SECS: f32 = 0.012;
const ALIGN_RANGE: i32 = 64;
const ALIGN_STRIDE: usize = 4;
const ALIGN_MIN_WINDOW: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Steady,
    Out,
    In,
}

/// Delay-time follower that never bends pitch: a change fades the read out, jumps to a phase-aligned tap, and fades back in.
#[derive(Debug, Clone)]
pub struct TapFade {
    dip: f32,
    tolerance_min: f32,
    align_window: f32,
    base: f32,
    offset: f32,
    offset_r: f32,
    next_base: f32,
    next_offset: f32,
    next_offset_r: f32,
    ratio: f32,
    next_ratio: f32,
    ratio_target: f32,
    progress: f32,
    phase: Phase,
    window: Vec<f32>,
}

impl TapFade {
    pub fn new(sample_rate: f32) -> Self {
        let align_window = ALIGN_SECS * sample_rate;
        Self {
            dip: DIP_SECS * sample_rate,
            tolerance_min: TOLERANCE_SECS * sample_rate,
            align_window,
            base: 0.0,
            offset: 0.0,
            offset_r: 0.0,
            next_base: 0.0,
            next_offset: 0.0,
            next_offset_r: 0.0,
            ratio: 1.0,
            next_ratio: 1.0,
            ratio_target: 1.0,
            progress: 0.0,
            phase: Phase::Steady,
            window: vec![0.0; align_window as usize / ALIGN_STRIDE + 2],
        }
    }

    pub fn snap(&mut self, delay: f32) {
        self.base = delay;
        self.offset = 0.0;
        self.offset_r = 0.0;
        self.ratio = self.ratio_target;
        self.phase = Phase::Steady;
    }

    /// Delay the first tap currently reads.
    pub fn delay(&self) -> f32 {
        self.base + self.offset
    }

    pub fn target_delay(&self) -> f32 {
        if self.phase == Phase::Out { self.next_base } else { self.base }
    }

    /// Right-channel delay as a multiple of the left; applied with the next dip.
    pub fn set_right_ratio(&mut self, ratio: f32) {
        self.ratio_target = ratio.clamp(0.1, 4.0);
    }

    /// Starts a dip towards `target` once it has drifted past the tolerance and `may_start` allows it.
    #[inline]
    pub fn follow(&mut self, target: f32, may_start: bool, ring: &StereoRing) {
        if self.phase != Phase::Steady || !may_start {
            return;
        }
        let tolerance = (TOLERANCE_FRACTION * self.base.max(1.0)).max(self.tolerance_min);
        if (target - self.base).abs() > tolerance || self.ratio_target != self.ratio {
            self.start_dip(target, ring);
        }
    }

    fn start_dip(&mut self, target: f32, ring: &StereoRing) {
        let from = self.base + self.offset;
        let moved = (target - self.base).abs() > 1.0 || self.ratio_target != self.ratio;
        self.next_offset = if moved { self.best_offset(ring, false, from, target) } else { 0.0 };
        self.next_offset_r = if moved { self.best_offset(ring, true, self.base * self.ratio + self.offset_r, target * self.ratio_target) } else { 0.0 };
        self.next_base = target;
        self.next_ratio = self.ratio_target;
        self.progress = 0.0;
        self.phase = Phase::Out;
    }

    /// Integer shift of the new tap, within a millisecond or so, that makes it most similar to the old tap.
    fn best_offset(&mut self, ring: &StereoRing, right: bool, from: f32, to: f32) -> f32 {
        let reach = ALIGN_RANGE as f32 + 2.0;
        let room = (from.min(to) - reach).floor();
        let span = (self.align_window.min(room.max(0.0)) as usize) / ALIGN_STRIDE;
        if span * ALIGN_STRIDE < ALIGN_MIN_WINDOW {
            return 0.0;
        }
        let (from_i, to_i) = (from.round() as i64, to.round() as i64);
        for (i, w) in self.window[..span].iter_mut().enumerate() {
            *w = ring.ago(right, (from_i - (i * ALIGN_STRIDE) as i64).max(1) as usize);
        }
        let mut best = (f32::MIN, 0);
        for shift in -ALIGN_RANGE..=ALIGN_RANGE {
            let (mut dot, mut energy) = (0.0f32, 1e-9f32);
            for (i, w) in self.window[..span].iter().enumerate() {
                let b = ring.ago(right, (to_i + shift as i64 - (i * ALIGN_STRIDE) as i64).max(1) as usize);
                dot += w * b;
                energy += b * b;
            }
            let score = dot / energy.sqrt();
            if score > best.0 {
                best = (score, shift);
            }
        }
        best.1 as f32 + (to_i as f32 - to)
    }

    /// Reads both channels through the dip; `extra` shifts the right channel.
    #[inline]
    pub fn read(&mut self, ring: &StereoRing, extra: f32) -> (f32, f32) {
        self.read_modulated(ring, extra, 0.0, 0.0)
    }

    /// Like `read`, with a per-channel delay modulation (wow and flutter) added to the tap.
    #[inline]
    pub fn read_modulated(&mut self, ring: &StereoRing, extra: f32, mod_l: f32, mod_r: f32) -> (f32, f32) {
        let a = self.base + self.offset;
        let gain = match self.phase {
            Phase::Steady => 1.0,
            Phase::Out => {
                let c = (FRAC_PI_2 * self.progress / self.dip).cos();
                c * c
            }
            Phase::In => {
                let s = (FRAC_PI_2 * self.progress / self.dip).sin();
                s * s
            }
        };
        let out = (gain * ring.read_l(a + mod_l), gain * ring.read_r(self.base * self.ratio + self.offset_r + extra + mod_r));
        match self.phase {
            Phase::Steady => {}
            Phase::Out | Phase::In => {
                self.progress += 1.0;
                if self.progress >= self.dip {
                    self.progress = 0.0;
                    if self.phase == Phase::Out {
                        self.base = self.next_base;
                        self.offset = self.next_offset;
                        self.offset_r = self.next_offset_r;
                        self.ratio = self.next_ratio;
                        self.phase = Phase::In;
                    } else {
                        self.phase = Phase::Steady;
                    }
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    fn ring_with_sine(len: usize) -> StereoRing {
        let mut ring = StereoRing::new(len);
        for i in 0..len {
            let v = (std::f32::consts::TAU * 1_000.0 * i as f32 / SR).sin();
            ring.write(v, v);
        }
        ring
    }

    #[test]
    fn a_change_dips_to_silence_then_returns_at_the_new_delay() {
        let ring = ring_with_sine(20_000);
        let mut taps = TapFade::new(SR);
        taps.snap(6_000.0);
        taps.follow(9_000.0, true, &ring);
        let mut gains = Vec::new();
        let mut peak_during_dip: f32 = 1.0;
        for _ in 0..(2.0 * DIP_SECS * SR) as usize + 4 {
            let (l, _) = taps.read(&ring, 0.0);
            gains.push(l.abs());
            peak_during_dip = peak_during_dip.min(l.abs());
        }
        assert!(peak_during_dip < 0.05, "reaches silence: {peak_during_dip}");
        assert!((taps.delay() - 9_000.0).abs() <= ALIGN_RANGE as f32 + 1.0, "{}", taps.delay());
        assert_eq!(taps.target_delay(), taps.base);
    }

    #[test]
    fn small_drifts_are_left_alone_and_nothing_creeps() {
        let ring = ring_with_sine(20_000);
        let mut taps = TapFade::new(SR);
        taps.snap(6_000.0);
        for _ in 0..5_000 {
            taps.follow(6_020.0, true, &ring);
            let _ = taps.read(&ring, 0.0);
        }
        assert_eq!(taps.delay(), 6_000.0);
    }

    #[test]
    fn right_ratio_scales_the_right_tap_after_a_dip() {
        let mut ring = StereoRing::new(20_000);
        for n in 0..12_000 {
            let impulse = if n == 100 { 1.0 } else { 0.0 };
            ring.write(impulse, impulse);
        }
        let mut taps = TapFade::new(SR);
        taps.snap(2_000.0);
        taps.set_right_ratio(0.5);
        taps.follow(2_000.0, true, &ring);
        for _ in 0..(2.0 * DIP_SECS * SR) as usize + 2 {
            let _ = taps.read(&ring, 0.0);
        }
        assert_eq!(taps.ratio, 0.5);
        assert_eq!(taps.target_delay(), 2_000.0);
    }
}
