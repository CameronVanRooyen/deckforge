use std::f32::consts::{PI, SQRT_2};

use deckforge_common::EqCurve;

use crate::smooth::Smoothed;
use crate::svf::Svf;

const BUTTERWORTH_Q: f32 = 1.0 / SQRT_2;
pub const LOW_CROSSOVER_HZ: f32 = 250.0;
pub const HIGH_CROSSOVER_HZ: f32 = 3_500.0;
pub const ISOLATOR_LOW_CROSSOVER_HZ: f32 = 284.0;
pub const ISOLATOR_HIGH_CROSSOVER_HZ: f32 = 4_649.0;

const KNOB_MAX: f32 = 255.0;
const KNOB_SMOOTH_MS: f32 = 15.0;
const COEFF_UPDATE: usize = 16;
const HIGH_SHELF_HZ: f32 = 2_200.0;
const HIGH_SHELF_DAMPING: f32 = 2.0;
const MID_PEAK_HZ: f32 = 1_000.0;
const MID_PEAK_Q: f32 = 0.11;
const LOW_SHELF_HZ: f32 = 250.0;
const LOW_SHELF_DAMPING: f32 = 1.5;
const SHELF_CUT_END: f32 = 126.0;
const SHELF_BOOST_START: f32 = 129.0;
const ISO_SQUARE_END: f32 = 9.0;
const ISO_LINEAR_START: f32 = 14.0;
const ISO_FLAT_START: f32 = 124.0;
const ISO_BOOST_START: f32 = 131.0;
const ISO_BOOST_END: f32 = 253.0;
const ISO_MAX_GAIN: f32 = 1.995_262_3;

/// MID peak gain along the knob: a slow fade-in from -26 dB, a flat detent, and a steeper climb to +6 dB.
const MID_GAIN_CURVE: [(f32, f32); 13] = [
    (0.0, 0.048), (2.0, 0.048), (7.0, 0.088), (12.0, 0.116), (20.0, 0.164), (27.0, 0.224), (43.0, 0.404),
    (72.0, 0.680), (124.0, 1.0), (131.0, 1.0), (204.0, 1.464), (237.0, 1.792), (255.0, 1.996),
];

/// One Linkwitz-Riley 4th-order crossover: two cascaded Butterworth sections per branch.
#[derive(Debug, Clone, Copy, Default)]
struct Lr4 {
    lp: [Svf; 2],
    hp: [Svf; 2],
}

impl Lr4 {
    fn new(freq: f32, sr: f32) -> Self {
        let s = Svf::new(freq, BUTTERWORTH_Q, sr);
        Self { lp: [s; 2], hp: [s; 2] }
    }

    #[inline]
    fn split(&mut self, x: f32) -> (f32, f32) {
        let [lp0, lp1] = &mut self.lp;
        let l = lp1.process(lp0.process(x).low).low;
        let [hp0, hp1] = &mut self.hp;
        let h = hp1.process(hp0.process(x).high).high;
        (l, h)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct BandSplitter {
    low_x: Lr4,
    high_x: Lr4,
    low_phase: Svf,
}

impl BandSplitter {
    pub(crate) fn new(sr: f32) -> Self {
        Self::with_crossovers(LOW_CROSSOVER_HZ, HIGH_CROSSOVER_HZ, sr)
    }

    fn with_crossovers(low: f32, high: f32, sr: f32) -> Self {
        Self { low_x: Lr4::new(low, sr), high_x: Lr4::new(high, sr), low_phase: Svf::new(high, BUTTERWORTH_Q, sr) }
    }

    #[inline]
    pub(crate) fn split(&mut self, x: f32) -> [f32; 3] {
        let (low, rest) = self.low_x.split(x);
        let (mid, high) = self.high_x.split(rest);
        [self.low_phase.allpass(low), mid, high]
    }
}

/// Phase-coherent 3-band DJ isolator at rekordbox's 284 Hz / 4.649 kHz: unity band gains sum to an allpass (flat magnitude).
#[derive(Debug, Clone, Copy)]
pub struct Isolator {
    sides: [BandSplitter; 2],
}

impl Isolator {
    pub fn new(sample_rate: f32) -> Self {
        Self { sides: [BandSplitter::with_crossovers(ISOLATOR_LOW_CROSSOVER_HZ, ISOLATOR_HIGH_CROSSOVER_HZ, sample_rate); 2] }
    }

    #[inline]
    pub fn process(&mut self, l: f32, r: f32, gains: [f32; 3]) -> (f32, f32) {
        let bl = self.sides[0].split(l);
        let br = self.sides[1].split(r);
        (
            bl[0] * gains[0] + bl[1] * gains[1] + bl[2] * gains[2],
            br[0] * gains[0] + br[1] * gains[1] + br[2] * gains[2],
        )
    }
}

fn knob_step(value: f32) -> f32 {
    value.clamp(0.0, 1.0) * KNOB_MAX
}

/// DJM-900NXS2 EQ curve of the HI and LOW shelves in dB: -26 dB fully off, a dead band at the detent, +6 dB fully on.
pub fn djm_shelf_db(value: f32) -> f32 {
    let k = knob_step(value);
    if k < SHELF_CUT_END {
        0.45 - 20.0 * (1.15 - k / SHELF_CUT_END).powi(2)
    } else if k <= SHELF_BOOST_START {
        0.0
    } else {
        k / 21.0 - 43.0 / 7.0
    }
}

/// DJM-900NXS2 EQ curve of the MID peak as linear gain (0.048 = -26.4 dB to 1.996 = +6 dB) along the pot taper.
pub fn djm_mid_gain(value: f32) -> f32 {
    let k = knob_step(value);
    let i = MID_GAIN_CURVE.windows(2).position(|w| k <= w[1].0).unwrap_or(MID_GAIN_CURVE.len() - 2);
    let ((k0, g0), (k1, g1)) = (MID_GAIN_CURVE[i], MID_GAIN_CURVE[i + 1]);
    g0 + (g1 - g0) * ((k - k0) / (k1 - k0)).clamp(0.0, 1.0)
}

/// rekordbox isolator knob curve as linear gain: full kill at 0, unity through the detent, +6 dB fully on.
pub fn isolator_knob_gain(value: f32) -> f32 {
    let k = knob_step(value);
    if k <= ISO_SQUARE_END {
        0.0006 * k * k
    } else if k < ISO_LINEAR_START {
        let start = 0.0006 * ISO_SQUARE_END * ISO_SQUARE_END;
        let end = (ISO_LINEAR_START - 2.0) / 122.0;
        start + (end - start) * (k - ISO_SQUARE_END) / (ISO_LINEAR_START - ISO_SQUARE_END)
    } else if k < ISO_FLAT_START {
        (k - 2.0) / 122.0
    } else if k <= ISO_BOOST_START {
        1.0
    } else {
        1.0 + (ISO_MAX_GAIN - 1.0) * ((k - ISO_BOOST_START) / (ISO_BOOST_END - ISO_BOOST_START)).min(1.0)
    }
}

/// Band gain in dB the channel EQ applies at a knob position for the chosen curve; the MID of `Eq` is its 1 kHz peak.
pub fn eq_band_db(curve: EqCurve, band: usize, value: f32) -> f32 {
    match (curve, band) {
        (EqCurve::Eq, 1) => 20.0 * djm_mid_gain(value).log10(),
        (EqCurve::Eq, _) => djm_shelf_db(value),
        (EqCurve::Isolator, _) => 20.0 * isolator_knob_gain(value).max(1e-9).log10(),
    }
}

/// Output mix of one SVF stage: `a·high + b·band + c·low`.
#[derive(Debug, Clone, Copy, Default)]
struct Mix {
    a: f32,
    b: f32,
    c: f32,
}

/// DJM-900NXS2-style EQ: HI shelf at 2.2 kHz, a very wide MID peak at 1 kHz, LOW shelf at 250 Hz, in series.
#[derive(Debug, Clone, Copy)]
struct DjmEq {
    stages: [[Svf; 3]; 2],
    mix: [Mix; 3],
    tan: [f32; 3],
}

impl DjmEq {
    fn new(sr: f32) -> Self {
        let mut eq = Self { stages: [[Svf::default(); 3]; 2], mix: [Mix::default(); 3], tan: [HIGH_SHELF_HZ, MID_PEAK_HZ, LOW_SHELF_HZ].map(|hz| (PI * hz / sr).tan()) };
        eq.update([0.5; 3]);
        eq
    }

    fn update(&mut self, knobs: [f32; 3]) {
        let [low, mid, high] = knobs;
        let g = deckforge_common::db_to_gain(djm_shelf_db(high));
        let root = g.sqrt();
        let hi = (self.tan[0] * root.max(1.0), HIGH_SHELF_DAMPING, Mix { a: g, b: root * HIGH_SHELF_DAMPING, c: 1.0 });
        let a = djm_mid_gain(mid);
        let md = (self.tan[1], 1.0 / (MID_PEAK_Q * a.max(1.0)), Mix { a: 1.0, b: a.min(1.0) / MID_PEAK_Q, c: 1.0 });
        let g = deckforge_common::db_to_gain(djm_shelf_db(low));
        let root = g.sqrt();
        let lo = (self.tan[2] / root.max(1.0), LOW_SHELF_DAMPING, Mix { a: 1.0, b: root * LOW_SHELF_DAMPING, c: g });
        for (i, (tan, damping, mix)) in [hi, md, lo].into_iter().enumerate() {
            self.mix[i] = mix;
            for side in &mut self.stages {
                side[i].set_prewarped(tan, damping);
            }
        }
    }

    #[inline]
    fn process(&mut self, side: usize, x: f32) -> f32 {
        let mut y = x;
        for (stage, m) in self.stages[side].iter_mut().zip(self.mix) {
            let o = stage.process(y);
            y = m.a * o.high + m.b * o.band + m.c * o.low;
        }
        y
    }
}

/// Per-channel 3-band EQ shared by the FLX4 and on-screen knobs: `Eq` follows the DJM-900NXS2 EQ curve, `Isolator` kills fully.
#[derive(Debug, Clone, Copy)]
pub struct ChannelEq {
    curve: EqCurve,
    knobs: [Smoothed; 3],
    djm: DjmEq,
    isolator: Isolator,
    tick: usize,
}

impl ChannelEq {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            curve: EqCurve::default(),
            knobs: [Smoothed::new(0.5, sample_rate, KNOB_SMOOTH_MS); 3],
            djm: DjmEq::new(sample_rate),
            isolator: Isolator::new(sample_rate),
            tick: 0,
        }
    }

    pub fn curve(&self) -> EqCurve {
        self.curve
    }

    pub fn set_curve(&mut self, curve: EqCurve) {
        self.curve = curve;
        self.tick = 0;
    }

    /// Knob 0..1 per band (low, mid, high); 0.5 is the detent.
    pub fn set_knob(&mut self, band: usize, value: f32) {
        self.knobs[band].set(value.clamp(0.0, 1.0));
    }

    #[inline]
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        let k = [self.knobs[0].next(), self.knobs[1].next(), self.knobs[2].next()];
        match self.curve {
            EqCurve::Eq => {
                if self.tick == 0 {
                    self.djm.update(k);
                }
                self.tick = (self.tick + 1) % COEFF_UPDATE;
                (self.djm.process(0, l), self.djm.process(1, r))
            }
            EqCurve::Isolator => self.isolator.process(l, r, k.map(isolator_knob_gain)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    fn rms_through(freq: f32, gains: [f32; 3]) -> f32 {
        let sr = 48_000.0;
        let mut iso = Isolator::new(sr);
        let n = 48_000;
        let mut acc = 0.0;
        let mut count = 0;
        for i in 0..n {
            let x = (TAU * freq * i as f32 / sr).sin();
            let (y, _) = iso.process(x, x, gains);
            if i > n / 2 {
                acc += y * y;
                count += 1;
            }
        }
        (acc / count as f32).sqrt() * SQRT_2
    }

    fn eq_db(curve: EqCurve, knobs: [f32; 3], freq: f32) -> f32 {
        let sr = 48_000.0;
        let mut eq = ChannelEq::new(sr);
        eq.set_curve(curve);
        for (band, v) in knobs.into_iter().enumerate() {
            eq.knobs[band].reset(v);
        }
        let n = 48_000;
        let (mut acc, mut count) = (0.0, 0);
        for i in 0..n {
            let x = (TAU * freq * i as f32 / sr).sin();
            let (y, _) = eq.process(x, x);
            if i > n / 2 {
                acc += y * y;
                count += 1;
            }
        }
        20.0 * ((acc / count as f32).sqrt() * SQRT_2).log10()
    }

    fn at_clock(hour: f32) -> f32 {
        0.5 + (hour - 12.0) / 10.0
    }

    #[test]
    fn unity_gains_are_flat() {
        for f in [40.0, 284.0, 1_000.0, 4_649.0, 10_000.0] {
            let g = rms_through(f, [1.0, 1.0, 1.0]);
            assert!((g - 1.0).abs() < 0.02, "{f} Hz -> {g}");
        }
    }

    #[test]
    fn killing_a_band_removes_it() {
        assert!(rms_through(60.0, [0.0, 1.0, 1.0]) < 0.08);
        assert!(rms_through(1_200.0, [1.0, 0.0, 1.0]) < 0.12);
        assert!(rms_through(14_000.0, [1.0, 1.0, 0.0]) < 0.08);
        assert!(rms_through(1_200.0, [0.0, 1.0, 0.0]) > 0.85);
    }

    #[test]
    fn djm_curve_is_gentle_near_the_detent_and_reaches_minus_26() {
        let cases = [(0.0, -26.0, -26.38), (at_clock(9.0), -10.66, -6.30), (at_clock(10.0), -5.44, -2.93), (at_clock(11.0), -1.87, -1.23), (0.5, 0.0, 0.0), (1.0, 6.0, 6.0)];
        for (v, shelf, mid) in cases {
            assert!((djm_shelf_db(v) - shelf).abs() < 0.05, "shelf at {v}: {}", djm_shelf_db(v));
            assert!((eq_band_db(EqCurve::Eq, 1, v) - mid).abs() < 0.1, "mid at {v}: {}", eq_band_db(EqCurve::Eq, 1, v));
        }
        let mut prev = f32::MIN;
        for i in 0..=100 {
            let db = djm_shelf_db(i as f32 / 100.0) + eq_band_db(EqCurve::Eq, 1, i as f32 / 100.0);
            assert!(db >= prev - 1e-4);
            prev = db;
        }
    }

    #[test]
    fn isolator_curve_kills_fully_and_tapers_gently() {
        let cases = [(at_clock(9.0), -7.92), (at_clock(10.0), -4.28), (at_clock(11.0), -1.73), (0.5, 0.0), (1.0, 6.0)];
        for (v, want) in cases {
            assert!((eq_band_db(EqCurve::Isolator, 0, v) - want).abs() < 0.1, "isolator at {v}: {}", eq_band_db(EqCurve::Isolator, 0, v));
        }
        assert_eq!(isolator_knob_gain(0.0), 0.0);
        assert!((isolator_knob_gain(10.0 / 255.0) - 0.0591).abs() < 0.002);
        let mut prev = -1.0;
        for i in 0..=1000 {
            let g = isolator_knob_gain(i as f32 / 1000.0);
            assert!(g >= prev);
            prev = g;
        }
    }

    #[test]
    fn both_curves_are_flat_at_the_detent() {
        for curve in [EqCurve::Eq, EqCurve::Isolator] {
            for f in [40.0, 250.0, 1_000.0, 2_200.0, 4_649.0, 12_000.0] {
                let db = eq_db(curve, [0.5; 3], f);
                assert!(db.abs() < 0.05, "{curve:?} {f} Hz: {db} dB");
            }
        }
    }

    #[test]
    fn djm_eq_cuts_where_the_mixer_spec_measures() {
        assert!((eq_db(EqCurve::Eq, [0.0, 0.5, 0.5], 20.0) + 26.0).abs() < 1.0);
        assert!((eq_db(EqCurve::Eq, [0.5, 0.0, 0.5], 1_000.0) + 26.4).abs() < 1.0);
        assert!((eq_db(EqCurve::Eq, [0.5, 0.5, 0.0], 20_000.0) + 26.0).abs() < 1.5);
        assert!((eq_db(EqCurve::Eq, [1.0, 0.5, 0.5], 20.0) - 6.0).abs() < 0.5);
        assert!(eq_db(EqCurve::Eq, [0.5, 0.5, 0.0], 100.0) > -1.0, "HI cut leaves the bass");
        assert!(eq_db(EqCurve::Eq, [0.0, 0.5, 0.5], 8_000.0) > -1.0, "LOW cut leaves the highs");
        let small_mid = eq_db(EqCurve::Eq, [0.5, at_clock(11.0), 0.5], 1_000.0);
        assert!((small_mid + 1.23).abs() < 0.2, "an hour of MID is a gentle dip: {small_mid}");
    }

    #[test]
    fn isolator_mode_kills_a_band() {
        assert!(eq_db(EqCurve::Isolator, [0.0, 0.5, 0.5], 60.0) < -20.0);
        assert!(eq_db(EqCurve::Isolator, [0.5, 0.0, 0.5], 1_200.0) < -18.0);
    }
}
