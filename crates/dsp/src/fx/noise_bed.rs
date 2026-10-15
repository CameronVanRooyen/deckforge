use super::filters::{chase_coeff, Ducker, HighPass, LogSlew, LowPass, PinkNoise};
use super::util::Noise;
use crate::isolator::BandSplitter;
use crate::smooth::Smoothed;

const WHITE_UNIT_GAIN: f32 = 1.732_050_8;
const REF_FLOOR: f32 = 0.04;
const REF_CEIL: f32 = 0.7;
const REF_SECS: f32 = 0.25;
const BAND_SMOOTH_MS: f32 = 10.0;
const DUCK_ATTACK_MS: f32 = 8.0;
const CUT_CHASE_SECS: f32 = 0.03;
const UPDATE: usize = 16;

/// Pink/white noise through a swept band-pass, band-weighted, ducked by and scaled to the music it rides on.
#[derive(Debug, Clone)]
pub(super) struct NoiseBed {
    sample_rate: f32,
    white: [Noise; 2],
    pink: [PinkNoise; 2],
    hp: [HighPass<2>; 2],
    lp: [LowPass<1>; 2],
    lo_cut: LogSlew,
    hi_cut: LogSlew,
    splitters: [BandSplitter; 2],
    gains: [Smoothed; 3],
    duck: Ducker,
    duck_gain: f32,
    power: f32,
    power_coeff: f32,
    chase: f32,
    tick: usize,
    target: (f32, f32),
}

impl NoiseBed {
    pub(super) fn new(sample_rate: f32, duck_db: f32, duck_release_ms: f32) -> Self {
        Self {
            sample_rate,
            white: [Noise::new(0x7F4A_7C15), Noise::new(0x94D0_49BB)],
            pink: [PinkNoise::new(0xD6E8_FEB8), PinkNoise::new(0x2545_F491)],
            hp: [HighPass::default(); 2],
            lp: [LowPass::default(); 2],
            lo_cut: LogSlew::new(1_000.0),
            hi_cut: LogSlew::new(4_000.0),
            splitters: [BandSplitter::new(sample_rate); 2],
            gains: [Smoothed::new(1.0, sample_rate, BAND_SMOOTH_MS); 3],
            duck: Ducker::new(sample_rate, duck_db, DUCK_ATTACK_MS, duck_release_ms),
            duck_gain: 1.0,
            power: 0.0,
            power_coeff: 1.0 - (-1.0 / (REF_SECS * sample_rate)).exp(),
            chase: chase_coeff(sample_rate / UPDATE as f32, CUT_CHASE_SECS),
            tick: 0,
            target: (1_000.0, 4_000.0),
        }
    }

    pub(super) fn reset(&mut self) {
        self.hp.iter_mut().for_each(HighPass::reset);
        self.lp.iter_mut().for_each(LowPass::reset);
        self.splitters = [BandSplitter::new(self.sample_rate); 2];
        self.duck.reset();
        self.duck_gain = 1.0;
        self.power = 0.0;
        self.tick = 0;
        self.lo_cut.reset(self.target.0);
        self.hi_cut.reset(self.target.1);
        self.apply_cuts();
    }

    pub(super) fn set_band_gains(&mut self, gains: [f32; 3]) {
        for (g, t) in self.gains.iter_mut().zip(gains) {
            g.set(t);
        }
    }

    pub(super) fn settle_band_gains(&mut self, gains: [f32; 3]) {
        for (g, t) in self.gains.iter_mut().zip(gains) {
            g.reset(t);
        }
    }

    pub(super) fn set_band(&mut self, lo_hz: f32, hi_hz: f32) {
        self.target = (lo_hz, hi_hz);
    }

    fn apply_cuts(&mut self) {
        let (lo, hi) = (self.lo_cut.chase(self.target.0, 1.0), self.hi_cut.chase(self.target.1, 1.0));
        for ch in 0..2 {
            self.hp[ch].set(lo, self.sample_rate);
            self.lp[ch].set(hi, self.sample_rate);
        }
    }

    pub(super) fn track(&mut self, xl: f32, xr: f32) {
        self.power += (0.5 * (xl * xl + xr * xr) - self.power) * self.power_coeff;
        self.duck_gain = self.duck.process(xl, xr);
    }

    /// RMS of the music the noise sits on, clamped so silence and clipping both stay sensible.
    pub(super) fn reference(&self) -> f32 {
        self.power.sqrt().clamp(REF_FLOOR, REF_CEIL)
    }

    pub(super) fn duck_gain(&self) -> f32 {
        self.duck_gain
    }

    #[inline]
    pub(super) fn next(&mut self) -> [f32; 2] {
        if self.tick.is_multiple_of(UPDATE) {
            let (lo, hi) = (self.lo_cut.chase(self.target.0, self.chase), self.hi_cut.chase(self.target.1, self.chase));
            for ch in 0..2 {
                self.hp[ch].set(lo, self.sample_rate);
                self.lp[ch].set(hi, self.sample_rate);
            }
        }
        self.tick = self.tick.wrapping_add(1);
        let gains = [self.gains[0].next(), self.gains[1].next(), self.gains[2].next()];
        let mut out = [0.0; 2];
        for (ch, o) in out.iter_mut().enumerate() {
            let source = std::f32::consts::FRAC_1_SQRT_2 * (WHITE_UNIT_GAIN * self.white[ch].next() + self.pink[ch].next());
            let band = self.lp[ch].process(self.hp[ch].process(source));
            let bands = self.splitters[ch].split(band);
            *o = bands[0] * gains[0] + bands[1] * gains[1] + bands[2] * gains[2];
        }
        out
    }
}
