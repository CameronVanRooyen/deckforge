use std::f32::consts::{SQRT_2, TAU};

use super::util::{flush, OnePole};
use crate::svf::Svf;

const BUTTERWORTH_Q: f32 = 1.0 / SQRT_2;
const BUTTERWORTH4_Q: [f32; 2] = [0.5412, 1.3066];

/// Butterworth high-pass of 2 poles (`STAGES` = 1) or 4 poles (`STAGES` = 2) with re-tunable cutoff.
#[derive(Debug, Clone, Copy)]
pub struct HighPass<const STAGES: usize> {
    stages: [Svf; STAGES],
}

impl<const STAGES: usize> Default for HighPass<STAGES> {
    fn default() -> Self {
        Self { stages: [Svf::default(); STAGES] }
    }
}

impl<const STAGES: usize> HighPass<STAGES> {
    pub fn set(&mut self, hz: f32, sample_rate: f32) {
        for (s, q) in self.stages.iter_mut().zip(stage_q::<STAGES>()) {
            s.set(hz, q, sample_rate);
        }
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        self.stages.iter_mut().fold(x, |v, s| s.process(v).high)
    }

    pub fn reset(&mut self) {
        self.stages.iter_mut().for_each(Svf::reset);
    }
}

/// Butterworth low-pass of 2 poles (`STAGES` = 1) or 4 poles (`STAGES` = 2) with re-tunable cutoff.
#[derive(Debug, Clone, Copy)]
pub struct LowPass<const STAGES: usize> {
    stages: [Svf; STAGES],
}

impl<const STAGES: usize> Default for LowPass<STAGES> {
    fn default() -> Self {
        Self { stages: [Svf::default(); STAGES] }
    }
}

impl<const STAGES: usize> LowPass<STAGES> {
    pub fn set(&mut self, hz: f32, sample_rate: f32) {
        for (s, q) in self.stages.iter_mut().zip(stage_q::<STAGES>()) {
            s.set(hz, q, sample_rate);
        }
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        self.stages.iter_mut().fold(x, |v, s| s.process(v).low)
    }

    pub fn reset(&mut self) {
        self.stages.iter_mut().for_each(Svf::reset);
    }
}

fn stage_q<const STAGES: usize>() -> impl Iterator<Item = f32> {
    (0..STAGES).map(|i| if STAGES == 1 { BUTTERWORTH_Q } else { BUTTERWORTH4_Q[i.min(1)] })
}

/// Second-order high shelf (RBJ cookbook), transposed direct form II.
#[derive(Debug, Clone, Copy, Default)]
pub struct HighShelf {
    b: [f32; 3],
    a: [f32; 2],
    z: [f32; 2],
}

impl HighShelf {
    pub fn new(hz: f32, gain_db: f32, sample_rate: f32) -> Self {
        let mut shelf = Self::default();
        shelf.set(hz, gain_db, sample_rate);
        shelf
    }

    /// Re-tunes the shelf without touching its filter state.
    pub fn set(&mut self, hz: f32, gain_db: f32, sample_rate: f32) {
        let a = 10f32.powf(gain_db / 40.0);
        let w = TAU * hz / sample_rate;
        let (sin, cos) = w.sin_cos();
        let alpha = sin * 0.5 * SQRT_2;
        let beta = 2.0 * a.sqrt() * alpha;
        let a0 = (a + 1.0) - (a - 1.0) * cos + beta;
        self.b = [
            a * ((a + 1.0) + (a - 1.0) * cos + beta) / a0,
            -2.0 * a * ((a - 1.0) + (a + 1.0) * cos) / a0,
            a * ((a + 1.0) + (a - 1.0) * cos - beta) / a0,
        ];
        self.a = [2.0 * ((a - 1.0) - (a + 1.0) * cos) / a0, ((a + 1.0) - (a - 1.0) * cos - beta) / a0];
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let y = self.b[0] * x + self.z[0];
        self.z[0] = flush(self.b[1] * x - self.a[0] * y + self.z[1]);
        self.z[1] = flush(self.b[2] * x - self.a[1] * y);
        y
    }

    pub fn reset(&mut self) {
        self.z = [0.0; 2];
    }
}

const KEY_HP_HZ: f32 = 150.0;
const KEY_LP_HZ: f32 = 5_000.0;
const KEY_THRESHOLD_LOW: f32 = 0.012;
const KEY_THRESHOLD_HIGH: f32 = 0.07;

/// Gain computer that ducks a wet path while the dry signal is loud, keyed from a band-limited mono sum.
#[derive(Debug, Clone, Copy)]
pub struct Ducker {
    hp: OnePole,
    lp: OnePole,
    env: f32,
    attack: f32,
    release: f32,
    floor: f32,
}

impl Ducker {
    pub fn new(sample_rate: f32, depth_db: f32, attack_ms: f32, release_ms: f32) -> Self {
        let mut hp = OnePole::default();
        hp.set_cutoff(KEY_HP_HZ, sample_rate);
        let mut lp = OnePole::default();
        lp.set_cutoff(KEY_LP_HZ, sample_rate);
        let coeff = |ms: f32| 1.0 - (-1.0 / (ms * 0.001 * sample_rate)).exp();
        Self { hp, lp, env: 0.0, attack: coeff(attack_ms), release: coeff(release_ms), floor: 10f32.powf(-depth_db / 20.0) }
    }

    pub fn reset(&mut self) {
        self.hp.reset();
        self.lp.reset();
        self.env = 0.0;
    }

    /// Feeds one dry stereo frame and returns the wet gain in `[floor, 1]`.
    #[inline]
    pub fn process(&mut self, l: f32, r: f32) -> f32 {
        let key = self.lp.lp(self.hp.hp(0.5 * (l + r))).abs();
        let rate = if key > self.env { self.attack } else { self.release };
        self.env = flush(self.env + rate * (key - self.env));
        let t = ((self.env - KEY_THRESHOLD_LOW) / (KEY_THRESHOLD_HIGH - KEY_THRESHOLD_LOW)).clamp(0.0, 1.0);
        let amount = t * t * (3.0 - 2.0 * t);
        1.0 - amount * (1.0 - self.floor)
    }
}

/// Log-domain one-pole chase for cutoffs, updated once per sub-block.
#[derive(Debug, Clone, Copy)]
pub struct LogSlew {
    ln_hz: f32,
}

impl LogSlew {
    pub fn new(hz: f32) -> Self {
        Self { ln_hz: hz.max(1.0).ln() }
    }

    pub fn reset(&mut self, hz: f32) {
        self.ln_hz = hz.max(1.0).ln();
    }

    #[inline]
    pub fn chase(&mut self, target_hz: f32, coeff: f32) -> f32 {
        self.ln_hz += (target_hz.max(1.0).ln() - self.ln_hz) * coeff;
        self.ln_hz.exp()
    }
}

/// Per-update smoothing coefficient for a time constant at a given update rate.
pub fn chase_coeff(update_rate_hz: f32, time_secs: f32) -> f32 {
    1.0 - (-1.0 / (time_secs * update_rate_hz)).exp()
}

const PINK_ROWS: usize = 7;

/// Voss-McCartney pink noise: octave-spaced white rows summed, normalised to unit RMS.
#[derive(Debug, Clone, Copy)]
pub struct PinkNoise {
    rows: [f32; PINK_ROWS],
    sum: f32,
    counter: u32,
    state: u32,
}

impl PinkNoise {
    pub fn new(seed: u32) -> Self {
        Self { rows: [0.0; PINK_ROWS], sum: 0.0, counter: 0, state: seed.max(1) }
    }

    #[inline]
    fn white(&mut self) -> f32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        (x >> 8) as f32 / 8_388_608.0 - 1.0
    }

    #[inline]
    pub fn next(&mut self) -> f32 {
        self.counter = self.counter.wrapping_add(1);
        let row = (self.counter.trailing_zeros() as usize).min(PINK_ROWS - 1);
        let fresh = self.white();
        self.sum += fresh - self.rows[row];
        self.rows[row] = fresh;
        let extra = self.white();
        (self.sum + extra) * (1.0 / 1.633)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    fn tone_gain(mut run: impl FnMut(f32) -> f32, hz: f32) -> f32 {
        let n = 24_000;
        let mut acc = 0.0;
        for i in 0..n {
            let y = run((TAU * hz * i as f32 / SR).sin());
            if i > n / 2 {
                acc += y * y;
            }
        }
        (acc / (n / 2) as f32).sqrt() * SQRT_2
    }

    #[test]
    fn four_pole_high_pass_rolls_off_at_24_db_per_octave() {
        let at = |hz| {
            let mut hp = HighPass::<2>::default();
            hp.set(1_000.0, SR);
            20.0 * tone_gain(|x| hp.process(x), hz).log10()
        };
        assert!((at(1_000.0) + 3.0).abs() < 0.5, "corner {}", at(1_000.0));
        assert!((at(500.0) + 24.0).abs() < 2.0, "octave below {}", at(500.0));
        assert!(at(8_000.0).abs() < 0.2);
    }

    #[test]
    fn high_shelf_lifts_the_top_by_the_requested_amount() {
        let mut shelf = HighShelf::new(6_000.0, 4.0, SR);
        let top = 20.0 * tone_gain(|x| shelf.process(x), 18_000.0).log10();
        let mut shelf = HighShelf::new(6_000.0, 4.0, SR);
        let bottom = 20.0 * tone_gain(|x| shelf.process(x), 200.0).log10();
        assert!((top - 4.0).abs() < 0.6, "top {top}");
        assert!(bottom.abs() < 0.1, "bottom {bottom}");
    }

    #[test]
    fn ducker_pulls_the_gain_down_to_its_depth_under_loud_input_and_releases() {
        let mut d = Ducker::new(SR, 4.5, 8.0, 200.0);
        let mut gain = 1.0;
        for i in 0..24_000 {
            gain = d.process(0.4 * (TAU * 1_000.0 * i as f32 / SR).sin(), 0.0);
        }
        assert!((20.0 * gain.log10() + 4.5).abs() < 0.3, "{} dB", 20.0 * gain.log10());
        for _ in 0..48_000 {
            gain = d.process(0.0, 0.0);
        }
        assert!(gain > 0.99);
    }

    #[test]
    fn pink_noise_carries_equal_power_per_octave() {
        let mut p = PinkNoise::new(7);
        let band = |centre: f32, p: &mut PinkNoise| {
            let mut hp = HighPass::<1>::default();
            let mut lp = LowPass::<1>::default();
            hp.set(centre / 1.41, SR);
            lp.set(centre * 1.41, SR);
            let mut acc = 0.0;
            for _ in 0..192_000 {
                let y = lp.process(hp.process(p.next()));
                acc += y * y;
            }
            10.0 * (acc / 192_000.0).log10()
        };
        let (a, b) = (band(400.0, &mut p), band(3_200.0, &mut p));
        assert!((a - b).abs() < 1.5, "{a} vs {b}");
    }
}
