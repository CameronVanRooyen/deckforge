use std::f32::consts::{PI, TAU};

use deckforge_common::{color_fx_params, ColorFxKind, COLOR_FX_MAX_PARAMS};

use super::filters::{chase_coeff, Ducker, HighPass, HighShelf, LogSlew, LowPass};
use super::pitch::PitchShifter;
use super::reverb::FdnReverb;
use super::space::SpaceCore;
use super::ring::StereoRing;
use super::util::{Lfo, Noise, OnePole, SwapGate};
use crate::limiter::soft_limit;
use crate::smooth::Smoothed;
use crate::svf::Svf;

const DEAD_ZONE: f32 = 0.03;
const MAX_ECHO_SECS: f32 = 1.8;
const AMOUNT_SMOOTH_MS: f32 = 12.0;
const SIGNED_SMOOTH_MS: f32 = 20.0;
const SLEW: f32 = 0.35;
const WOW_HZ: f32 = 0.7;
const WOW_DEPTH_MS: f32 = 0.25;
const SPACE_RT60_MIN: f32 = 3.5;
const SPACE_RT60_SPAN: f32 = 6.0;
const SPACE_TAIL_SECS: f64 = 6.0;
const SPACE_LAYER_SMOOTH_MS: f32 = 60.0;
const SPACE_LAYER_SEED: f32 = 1.0;
const SPACE_RETURN_FLOOR: f32 = 0.35;
const SPACE_RETURN_SPAN: f32 = 0.65;
const SPACE_DARK_TRIM: f32 = 0.65;
const SPACE_RELEASE_HZ: f32 = 0.1;
const SPACE_BRIGHT_HF_RATIO: f32 = 0.85;
const SPACE_DARK_HF_RATIO: f32 = 0.6;
const SPACE_SHIMMER_REFERENCE: f32 = 50.0;
const SPACE_BRIGHT_OUT_HP_HZ: f32 = 400.0;
const SPACE_BRIGHT_OUT_LP_HZ: f32 = 15_000.0;
const SPACE_DARK_OUT_HP_HZ: f32 = 200.0;
const SPACE_DARK_OUT_LP_HZ: f32 = 5_000.0;
const SPACE_DARK_OUT_LP_DROP_HZ: f32 = 1_500.0;
const SPACE_SHELF_HZ: f32 = 6_000.0;
const SPACE_SHELF_DB: f32 = 4.0;
const SPACE_HISS_ONSET: f32 = 0.3;
const SPACE_HISS_RIGHT_PEAK: f32 = 0.75;
const SPACE_HISS_LEFT_PEAK: f32 = 0.69;
const SPACE_HISS_RIGHT_HZ: [f32; 2] = [5_000.0, 9_000.0];
const SPACE_HISS_LEFT_HZ: [f32; 2] = [2_000.0, 3_200.0];
const SPACE_HISS_RIGHT_Q: f32 = 0.8;
const SPACE_HISS_LEFT_Q: f32 = 0.7;
const SPACE_HISS_DIRECT: f32 = 0.35;
const SPACE_HISS_HP_HZ: f32 = 1_200.0;
const SPACE_HISS_REFERENCE_HZ: f32 = 6_000.0;
const SHIMMER_RATIO: f32 = 2.0;
const SHIMMER_WINDOW_SECS: f32 = 0.04;
const SHIMMER_HP_HZ: f32 = 800.0;
const SHIMMER_LP_HZ: f32 = 11_000.0;
const SHIMMER_LOOP_GAIN: f32 = 0.45;
const SHIMMER_ATTACK_MS: f32 = 60.0;
const SHIMMER_RELEASE_MS: f32 = 1_200.0;
const CUTOFF_CHASE_SECS: f32 = 0.06;
const NOISE_TAIL_SECS: f64 = 1.5;
const NOISE_MIN_HZ: f32 = 133.0;
const NOISE_MAX_HZ: f32 = 20_000.0;
const NOISE_Q: f32 = 2.0;
const NOISE_PARAM_GAIN: f32 = 1.059_253_7;
const NOISE_PEAK: f32 = 1.133;
const NOISE_GAIN_REF_HZ: f32 = 400.0;
const NOISE_DUCK_DB: f32 = 2.0;
const SPACE_DUCK_DB: f32 = 3.5;
const NOISE_LEVEL_RELEASE_SECS: f32 = 0.45;
const NOISE_LEVEL_ATTACK_SECS: f32 = 0.006;
const NOISE_CUTOFF_RELEASE_SECS: f32 = 0.5;
const NOISE_REF_FLOOR: f32 = 0.04;
const NOISE_REF_CEIL: f32 = 0.7;
const NOISE_REF_SECS: f32 = 0.25;
const NOISE_TABLE: usize = 48;
const WHITE_UNIT_GAIN: f32 = 1.732_050_8;
const ECHO_TAIL_SECS: f64 = 12.0;
const RETURN_GAIN: f32 = 0.9;
const SUBBLOCK: usize = 32;
const DUCK_ATTACK_MS: f32 = 8.0;
const DUCK_RELEASE_MS: f32 = 200.0;

/// Band-pass centre and RMS gain of the Noise effect for a signed knob position (right rises from the lows, left falls from the highs).
fn noise_colour(signed: f32) -> (f32, f32) {
    let a = signed.abs().clamp(0.0, 1.0);
    let ratio = NOISE_MAX_HZ / NOISE_MIN_HZ;
    let (hz, gain) = if signed >= 0.0 {
        let onset = 1.0 - (-a / 0.05).exp();
        (NOISE_MIN_HZ * ratio.powf(a), 0.9 * onset * (0.1 + 0.9 * (1.0 - a).powf(0.8)))
    } else {
        (NOISE_MAX_HZ / ratio.powf(a), 1.39 * a.powf(1.3))
    };
    (hz, gain * (hz / NOISE_GAIN_REF_HZ).powf(0.25))
}

fn noise_table_position(hz: f32) -> f32 {
    ((hz / NOISE_MIN_HZ).ln() / (NOISE_MAX_HZ / NOISE_MIN_HZ).ln()).clamp(0.0, 1.0)
}

/// Gain that makes white noise through the band-pass at each table centre unit RMS.
fn noise_compensation(sample_rate: f32) -> [f32; NOISE_TABLE] {
    const POINTS: usize = 768;
    let nyquist = 0.5 * sample_rate;
    std::array::from_fn(|k| {
        let fc = NOISE_MIN_HZ * (NOISE_MAX_HZ / NOISE_MIN_HZ).powf(k as f32 / (NOISE_TABLE - 1) as f32);
        let g = (PI * fc.min(0.49 * sample_rate) / sample_rate).tan();
        let mut power = 0.0f64;
        let mut prev_f = 0.0f32;
        for j in 0..POINTS {
            let f = nyquist * ((j as f32 + 1.0) / POINTS as f32).powi(2);
            let x = (PI * f / sample_rate).tan() / g;
            let bp = x * x / ((1.0 - x * x).powi(2) + x * x / (NOISE_Q * NOISE_Q));
            power += (bp * (f - prev_f) / nyquist) as f64;
            prev_f = f;
        }
        1.0 / (power as f32).sqrt().max(1e-6)
    })
}

fn table_at(table: &[f32; NOISE_TABLE], u: f32) -> f32 {
    let p = u.clamp(0.0, 1.0) * (NOISE_TABLE - 1) as f32;
    let i = (p as usize).min(NOISE_TABLE - 2);
    table[i] + (table[i + 1] - table[i]) * (p - i as f32)
}

/// Noise LEVEL setting (0..100) as a gain: linear up to a little over unity at the top.
fn noise_level_gain(level: f32) -> f32 {
    level.clamp(0.0, 100.0) * 0.01 * NOISE_PARAM_GAIN
}

/// Log-spaced position between two frequencies for a 0..1 knob amount.
fn log_between(range: [f32; 2], amount: f32) -> f32 {
    range[0] * (range[1] / range[0]).powf(amount.clamp(0.0, 1.0))
}

/// Per-channel Sound Color FX: everything except the bipolar filter, which stays in `DjFilter`.
pub struct ColorFx {
    sample_rate: f32,
    kind: ColorFxKind,
    active: ColorFxKind,
    swap: SwapGate,
    value: f32,
    amount: Smoothed,
    signed: Smoothed,
    held: f32,
    held_dark: bool,
    bpm: f32,
    idle: bool,
    tail_left: f64,
    reverb: FdnReverb,
    rt60_applied: f32,
    tone_hp: [OnePole; 2],
    space: SpaceCore,
    layer_gain: Smoothed,
    space_out_hp: [HighPass<1>; 2],
    space_out_lp: [LowPass<1>; 2],
    space_shelf: [HighShelf; 2],
    space_cuts: [LogSlew; 2],
    shimmer: [PitchShifter; 2],
    shimmer_hp: [HighPass<1>; 2],
    shimmer_lp: [LowPass<1>; 2],
    shimmer_prev: [f32; 2],
    shimmer_gain: Smoothed,
    pub(crate) shimmer_scale: f32,
    space_trim: f32,
    ring: StereoRing,
    delay_cur: f32,
    delay_snap: bool,
    feedback_lp: [OnePole; 2],
    wow: Lfo,
    crush_phase: f32,
    crush_hold: [f32; 2],
    noise: [Noise; 2],
    noise_bp: [Svf; 2],
    noise_comp: [f32; NOISE_TABLE],
    noise_level: f32,
    noise_cut: LogSlew,
    hiss_hp: [HighPass<1>; 2],
    hiss_bp: [Svf; 2],
    hiss_cut: LogSlew,
    pub(crate) space_hiss_scale: f32,
    pub(crate) noise_duck_scale: f32,
    duck: Ducker,
    noise_duck: Ducker,
    ref_power: f32,
    params: [[f32; COLOR_FX_MAX_PARAMS]; ColorFxKind::ALL.len()],
}

fn amount_of(value: f32) -> f32 {
    ((value.abs() - DEAD_ZONE) / (1.0 - DEAD_ZONE)).clamp(0.0, 1.0)
}

fn signed_amount_of(value: f32) -> f32 {
    amount_of(value).copysign(value)
}

impl ColorFx {
    pub fn new(sample_rate: f32, seed: u32) -> Self {
        let mut fx = Self {
            sample_rate,
            kind: ColorFxKind::Filter,
            active: ColorFxKind::Filter,
            swap: SwapGate::new(sample_rate),
            value: 0.0,
            amount: Smoothed::new(0.0, sample_rate, AMOUNT_SMOOTH_MS),
            signed: Smoothed::new(0.0, sample_rate, SIGNED_SMOOTH_MS),
            held: 0.0,
            held_dark: false,
            bpm: 120.0,
            idle: true,
            tail_left: 0.0,
            reverb: FdnReverb::new(sample_rate),
            rt60_applied: 0.0,
            tone_hp: [OnePole::default(); 2],
            space: SpaceCore::new(sample_rate),
            layer_gain: Smoothed::new(0.0, sample_rate, SPACE_LAYER_SMOOTH_MS),
            space_out_hp: [HighPass::default(); 2],
            space_out_lp: [LowPass::default(); 2],
            space_shelf: [HighShelf::new(SPACE_SHELF_HZ, SPACE_SHELF_DB, sample_rate); 2],
            space_cuts: [LogSlew::new(SPACE_BRIGHT_OUT_HP_HZ), LogSlew::new(SPACE_BRIGHT_OUT_LP_HZ)],
            shimmer: std::array::from_fn(|_| PitchShifter::new((SHIMMER_WINDOW_SECS * sample_rate) as usize)),
            shimmer_hp: [HighPass::default(); 2],
            shimmer_lp: [LowPass::default(); 2],
            shimmer_prev: [0.0; 2],
            shimmer_gain: Smoothed::new(0.0, sample_rate, SHIMMER_ATTACK_MS),
            shimmer_scale: 1.0,
            space_trim: 1.0,
            ring: StereoRing::new((MAX_ECHO_SECS * sample_rate) as usize + 8),
            delay_cur: 0.0,
            delay_snap: true,
            feedback_lp: [OnePole::default(); 2],
            wow: Lfo::default(),
            crush_phase: 0.0,
            crush_hold: [0.0; 2],
            noise: [Noise::new(seed.wrapping_mul(2_654_435_761) | 1), Noise::new(seed.wrapping_mul(40_503) ^ 0x9E37_79B9)],
            noise_bp: [Svf::default(); 2],
            noise_comp: noise_compensation(sample_rate),
            noise_level: 0.0,
            noise_cut: LogSlew::new(NOISE_MIN_HZ),
            hiss_hp: [HighPass::default(); 2],
            hiss_bp: [Svf::default(); 2],
            hiss_cut: LogSlew::new(SPACE_HISS_RIGHT_HZ[0]),
            space_hiss_scale: 1.0,
            noise_duck_scale: 1.0,
            duck: Ducker::new(sample_rate, SPACE_DUCK_DB, DUCK_ATTACK_MS, DUCK_RELEASE_MS),
            noise_duck: Ducker::new(sample_rate, NOISE_DUCK_DB, DUCK_ATTACK_MS, DUCK_RELEASE_MS),
            ref_power: 0.0,
            params: ColorFxKind::ALL.map(|kind| {
                let mut values = [0.0; COLOR_FX_MAX_PARAMS];
                for (v, spec) in values.iter_mut().zip(color_fx_params(kind)) {
                    *v = spec.default;
                }
                values
            }),
        };
        for ch in 0..2 {
            fx.space_shelf[ch] = HighShelf::new(SPACE_SHELF_HZ, SPACE_SHELF_DB, sample_rate);
            fx.shimmer_hp[ch].set(SHIMMER_HP_HZ, sample_rate);
            fx.shimmer_lp[ch].set(SHIMMER_LP_HZ, sample_rate);
            fx.hiss_hp[ch].set(SPACE_HISS_HP_HZ, sample_rate);
        }
        fx.reverb.set_predelay_ms(25.0);
        fx.reverb.set_modulation_ms(0.6);
        fx
    }

    pub fn kind(&self) -> ColorFxKind {
        self.kind
    }

    /// Sets one sub-parameter of a kind; out-of-range values clamp, non-finite ones are ignored.
    pub fn set_param(&mut self, kind: ColorFxKind, index: usize, value: f32) {
        if let Some(v) = color_fx_params(kind).get(index).and_then(|spec| spec.sanitize(value)) {
            self.params[kind.slot()][index] = v;
        }
    }

    pub fn params(&self, kind: ColorFxKind) -> [f32; COLOR_FX_MAX_PARAMS] {
        self.params[kind.slot()]
    }

    pub fn set_bpm(&mut self, bpm: f32) {
        self.bpm = bpm.clamp(30.0, 400.0);
    }

    pub fn set_value(&mut self, value: f32) {
        self.value = value.clamp(-1.0, 1.0);
        let a = amount_of(self.value);
        let signed = signed_amount_of(self.value);
        self.amount.set(a);
        self.signed.set(signed);
        if self.idle && a > 0.0 {
            self.idle = false;
            self.amount.reset(0.0);
            self.amount.set(a);
            self.signed.reset(0.0);
            self.signed.set(signed);
        }
        if a > 0.0 {
            self.tail_left = self.tail_samples();
        }
    }

    pub fn set_kind(&mut self, kind: ColorFxKind) {
        self.kind = kind;
        if self.idle {
            self.active = kind;
            self.reset_state();
        } else if kind != self.active || self.swap.is_swapping() {
            self.swap.begin();
        }
    }

    fn tail_samples(&self) -> f64 {
        let secs = match self.active {
            ColorFxKind::Space => SPACE_TAIL_SECS,
            ColorFxKind::DubEcho => ECHO_TAIL_SECS,
            ColorFxKind::Noise => NOISE_TAIL_SECS,
            _ => 0.05,
        };
        secs * self.sample_rate as f64
    }

    fn reset_state(&mut self) {
        self.reverb.reset();
        self.rt60_applied = 0.0;
        self.ring.reset();
        self.delay_snap = true;
        for p in self.tone_hp.iter_mut().chain(self.feedback_lp.iter_mut()) {
            p.reset();
        }
        self.space.reset();
        self.layer_gain.reset(0.0);
        self.space_out_hp.iter_mut().for_each(HighPass::reset);
        self.space_out_lp.iter_mut().for_each(LowPass::reset);
        self.space_shelf.iter_mut().for_each(HighShelf::reset);
        self.shimmer.iter_mut().for_each(PitchShifter::reset);
        self.shimmer_hp.iter_mut().for_each(HighPass::reset);
        self.shimmer_lp.iter_mut().for_each(LowPass::reset);
        self.shimmer_prev = [0.0; 2];
        self.shimmer_gain.reset(0.0);
        self.noise_bp.iter_mut().for_each(Svf::reset);
        self.noise_level = 0.0;
        self.noise_cut.reset(NOISE_MIN_HZ);
        self.hiss_hp.iter_mut().for_each(HighPass::reset);
        self.hiss_bp.iter_mut().for_each(Svf::reset);
        self.hiss_cut.reset(SPACE_HISS_RIGHT_HZ[0]);
        self.duck.reset();
        self.noise_duck.reset();
        self.crush_phase = 0.0;
        self.crush_hold = [0.0; 2];
        self.held = 0.0;
        self.ref_power = 0.0;
        self.signed.reset(self.signed.target());
    }

    /// Adds the colour effect in place; a no-op for `Filter` and when the knob is centred with no tail left.
    pub fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        let n = l.len().min(r.len());
        if self.active == ColorFxKind::Filter && self.swap.is_swapping() {
            self.active = self.kind;
            self.reset_state();
            self.swap.finish();
        }
        if self.active == ColorFxKind::Filter || self.idle {
            self.finish_block(n);
            return;
        }
        let (l, r) = (&mut l[..n], &mut r[..n]);
        match self.active {
            ColorFxKind::Space => self.run_space(l, r),
            ColorFxKind::DubEcho => self.run_dub_echo(l, r),
            ColorFxKind::Crush => self.run_crush(l, r),
            ColorFxKind::Noise => self.run_noise(l, r),
            ColorFxKind::Filter => {}
        }
        self.finish_block(n);
    }

    fn finish_block(&mut self, n: usize) {
        if self.swap.ready() {
            self.active = self.kind;
            self.reset_state();
            self.swap.finish();
        }
        if !self.idle && self.value.abs() <= DEAD_ZONE {
            self.tail_left -= n as f64;
            if self.tail_left <= 0.0 && self.amount.is_settled() && !self.swap.is_swapping() {
                self.idle = true;
                self.reset_state();
                self.swap.reset_open();
            }
        }
    }

    fn apply_rt60(&mut self, rt60: f32) {
        if (rt60 - self.rt60_applied).abs() > 1e-3 {
            self.reverb.set_rt60(rt60);
            self.rt60_applied = rt60;
        }
    }

    fn run_space(&mut self, l: &mut [f32], r: &mut [f32]) {
        self.space.set_knob(self.value);
        let amount = self.params[ColorFxKind::Space.slot()][0] / SPACE_SHIMMER_REFERENCE;
        self.layer_gain.set(amount);
        let release = 1.0 - (-TAU * SPACE_RELEASE_HZ / self.sample_rate).exp();
        let chase = chase_coeff(self.sample_rate / SUBBLOCK as f32, CUTOFF_CHASE_SECS);
        let ref_coeff = 1.0 - (-1.0 / (NOISE_REF_SECS * self.sample_rate)).exp();
        let mut hiss_gain = 0.0f32;
        for i in 0..l.len() {
            let layered = amount > 0.0 || self.layer_gain.value() > 0.0;
            if layered && i % SUBBLOCK == 0 {
                let s = self.signed.value();
                let a = s.abs();
                let (range, q, peak) = if s >= 0.0 {
                    (SPACE_HISS_RIGHT_HZ, SPACE_HISS_RIGHT_Q, SPACE_HISS_RIGHT_PEAK)
                } else {
                    (SPACE_HISS_LEFT_HZ, SPACE_HISS_LEFT_Q, SPACE_HISS_LEFT_PEAK)
                };
                let centre = self.hiss_cut.chase(log_between(range, a), chase);
                for bp in &mut self.hiss_bp {
                    bp.set(centre, q, self.sample_rate);
                }
                let onset = ((a - SPACE_HISS_ONSET) / (1.0 - SPACE_HISS_ONSET)).max(0.0);
                let reference = self.ref_power.sqrt().clamp(NOISE_REF_FLOOR, NOISE_REF_CEIL);
                hiss_gain = reference * peak * onset * onset * (SPACE_HISS_REFERENCE_HZ / centre).sqrt() * self.space_hiss_scale;
                let h = self.held;
                self.apply_rt60(SPACE_RT60_MIN + SPACE_RT60_SPAN * h);
                let bright = !self.held_dark;
                self.reverb.set_hf_ratio(if bright { SPACE_BRIGHT_HF_RATIO } else { SPACE_DARK_HF_RATIO });
                let (out_hp, out_lp) = if bright {
                    (SPACE_BRIGHT_OUT_HP_HZ, SPACE_BRIGHT_OUT_LP_HZ)
                } else {
                    (SPACE_DARK_OUT_HP_HZ, SPACE_DARK_OUT_LP_HZ - SPACE_DARK_OUT_LP_DROP_HZ * h)
                };
                let cuts = [self.space_cuts[0].chase(out_hp, chase), self.space_cuts[1].chase(out_lp, chase)];
                for ch in 0..2 {
                    self.space_out_hp[ch].set(cuts[0], self.sample_rate);
                    self.space_out_lp[ch].set(cuts[1], self.sample_rate);
                }
                self.space_trim += (if bright { 1.0 } else { SPACE_DARK_TRIM } - self.space_trim) * chase;
                let holding = self.value.abs() > DEAD_ZONE;
                let shimmer = if bright && holding { SHIMMER_LOOP_GAIN * h * h * self.shimmer_scale } else { 0.0 };
                self.shimmer_gain.set_time(self.sample_rate, if holding { SHIMMER_ATTACK_MS } else { SHIMMER_RELEASE_MS });
                self.shimmer_gain.set(shimmer);
            }
            let s = self.signed.next();
            let a = s.abs();
            if self.value.abs() > DEAD_ZONE {
                self.held += if a > self.held { a - self.held } else { (a - self.held) * release };
                self.held_dark = self.value < 0.0;
            }
            let gate = self.swap.next();
            let (xl, xr) = (l[i], r[i]);
            let (wet_l, wet_r) = self.space.process(xl, xr);
            let (mut out_l, mut out_r) = (xl + soft_limit(wet_l) * gate, xr + soft_limit(wet_r) * gate);
            if layered {
                let layer = self.layer_gain.next();
                self.ref_power += (0.5 * (xl * xl + xr * xr) - self.ref_power) * ref_coeff;
                let duck = 1.0 + (self.duck.process(xl, xr) - 1.0) * self.noise_duck_scale;
                let shimmer = self.shimmer_gain.next();
                let back = (SPACE_RETURN_FLOOR + SPACE_RETURN_SPAN * self.held) * self.space_trim * gate * layer;
                let seeds = [wet_l * SPACE_LAYER_SEED, wet_r * SPACE_LAYER_SEED];
                let mut fed = [0.0f32; 2];
                let mut hiss = [0.0f32; 2];
                for ch in 0..2 {
                    let seed = self.shimmer_hp[ch].process(self.shimmer_prev[ch] + seeds[ch]);
                    let lifted = self.shimmer_lp[ch].process(self.shimmer[ch].process(seed, SHIMMER_RATIO));
                    let source = self.hiss_hp[ch].process(WHITE_UNIT_GAIN * self.noise[ch].next());
                    hiss[ch] = self.hiss_bp[ch].process(source).band * hiss_gain * layer * duck * gate;
                    fed[ch] = shimmer * lifted.tanh() + hiss[ch];
                }
                let (layer_l, layer_r) = self.reverb.process(fed[0], fed[1]);
                let wets = [layer_l, layer_r];
                let mut out = [0.0f32; 2];
                for ch in 0..2 {
                    let cleaned = self.space_out_lp[ch].process(self.space_out_hp[ch].process(soft_limit(wets[ch])));
                    self.shimmer_prev[ch] = cleaned;
                    out[ch] = soft_limit(self.space_shelf[ch].process(cleaned) * back);
                }
                out_l += out[0] + hiss[0] * SPACE_HISS_DIRECT;
                out_r += out[1] + hiss[1] * SPACE_HISS_DIRECT;
            }
            l[i] = out_l;
            r[i] = out_r;
        }
    }

    fn run_dub_echo(&mut self, l: &mut [f32], r: &mut [f32]) {
        let dark = self.value < 0.0;
        let max = MAX_ECHO_SECS * self.sample_rate;
        let mut target = 0.75 * 60.0 / self.bpm * self.sample_rate;
        while target > max {
            target *= 0.5;
        }
        if self.delay_snap {
            self.delay_cur = target;
            self.delay_snap = false;
        }
        let wow_inc = WOW_HZ / self.sample_rate;
        let wow_depth = WOW_DEPTH_MS * self.sample_rate / 1000.0;
        for i in 0..l.len() {
            if i % SUBBLOCK == 0 {
                let a = self.held;
                let (lp, hp) = if dark { (3_200.0 - 2_000.0 * a, 40.0) } else { (3_800.0, 60.0 + 900.0 * a) };
                for ch in 0..2 {
                    self.feedback_lp[ch].set_cutoff(lp, self.sample_rate);
                    self.tone_hp[ch].set_cutoff(hp, self.sample_rate);
                }
            }
            self.delay_cur += (target - self.delay_cur).clamp(-SLEW, SLEW);
            self.wow.advance(wow_inc);
            let a = self.amount.next();
            if self.value.abs() > DEAD_ZONE {
                self.held = a;
            }
            let fb = 0.35 + 0.4 * self.held;
            let gate = self.swap.next();
            let send = (a * 2.0).min(1.0) * gate;
            let back = RETURN_GAIN * gate;
            let (yl, yr) = self.ring.read(self.delay_cur + wow_depth * self.wow.sine_at(0.0));
            let (xl, xr) = (l[i], r[i]);
            let fl = self.tone_hp[0].hp(self.feedback_lp[0].lp(yl));
            let fr = self.tone_hp[1].hp(self.feedback_lp[1].lp(yr));
            self.ring.write(soft_limit(xl * send + fb * fl), soft_limit(xr * send + fb * fr));
            l[i] = xl + yl * back;
            r[i] = xr + yr * back;
        }
    }

    fn run_crush(&mut self, l: &mut [f32], r: &mut [f32]) {
        let mut levels = 32_768.0f32;
        let mut step = 1.0f32;
        for i in 0..l.len() {
            let amount = self.amount.next();
            if i % SUBBLOCK == 0 {
                levels = (15.0 - 11.0 * amount).exp2();
                step = 1.0 / (1.0 + 40.0 * amount * amount);
            }
            let mix = (amount * 8.0).min(1.0) * self.swap.next();
            self.crush_phase += step;
            if self.crush_phase >= 1.0 {
                self.crush_phase -= 1.0;
                self.crush_hold = [(l[i] * levels).round() / levels, (r[i] * levels).round() / levels];
            }
            l[i] += (self.crush_hold[0] - l[i]) * mix;
            r[i] += (self.crush_hold[1] - r[i]) * mix;
        }
    }

    fn run_noise(&mut self, l: &mut [f32], r: &mut [f32]) {
        let ref_coeff = 1.0 - (-1.0 / (NOISE_REF_SECS * self.sample_rate)).exp();
        let level_attack = 1.0 - (-1.0 / (NOISE_LEVEL_ATTACK_SECS * self.sample_rate)).exp();
        let level_release = 1.0 - (-1.0 / (NOISE_LEVEL_RELEASE_SECS * self.sample_rate)).exp();
        let tick_rate = self.sample_rate / SUBBLOCK as f32;
        let follow = chase_coeff(tick_rate, CUTOFF_CHASE_SECS);
        let fall = chase_coeff(tick_rate, NOISE_CUTOFF_RELEASE_SECS);
        let level_gain = noise_level_gain(self.params[ColorFxKind::Noise.slot()][0]);
        let mut target = 0.0f32;
        let mut gain = 0.0f32;
        for i in 0..l.len() {
            if i % SUBBLOCK == 0 {
                let s = self.signed.value();
                let held = self.value.abs() > DEAD_ZONE;
                let (centre, rms) = noise_colour(s);
                let cutoff = if held { self.noise_cut.chase(centre, follow) } else { self.noise_cut.chase(NOISE_MIN_HZ, fall) };
                for bp in &mut self.noise_bp {
                    bp.set(cutoff, NOISE_Q, self.sample_rate);
                }
                target = if held { rms } else { 0.0 };
                let reference = self.ref_power.sqrt().clamp(NOISE_REF_FLOOR, NOISE_REF_CEIL);
                gain = reference * NOISE_PEAK * level_gain * table_at(&self.noise_comp, noise_table_position(cutoff));
            }
            self.signed.next();
            let rate = if target > self.noise_level { level_attack } else { level_release };
            self.noise_level += (target - self.noise_level) * rate;
            let gate = self.swap.next();
            let (xl, xr) = (l[i], r[i]);
            self.ref_power += (0.5 * (xl * xl + xr * xr) - self.ref_power) * ref_coeff;
            let duck = 1.0 + (self.noise_duck.process(xl, xr) - 1.0) * self.noise_duck_scale;
            let amount = self.noise_level * gain * duck * gate;
            let hiss = [0, 1].map(|ch| self.noise_bp[ch].process(WHITE_UNIT_GAIN * self.noise[ch].next()).band * amount);
            l[i] = xl + soft_limit(hiss[0]);
            r[i] = xr + soft_limit(hiss[1]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    use super::super::testkit::{band_psd, db, tone_amplitude, white};

    const SR: f32 = 48_000.0;
    const BLOCK: usize = 256;

    fn make(kind: ColorFxKind, value: f32) -> ColorFx {
        let mut fx = ColorFx::new(SR, 1);
        fx.set_bpm(120.0);
        fx.set_kind(kind);
        fx.set_value(value);
        fx
    }

    fn run(fx: &mut ColorFx, x: &[f32]) -> Vec<f32> {
        let (mut l, mut r) = (x.to_vec(), x.to_vec());
        for (cl, cr) in l.chunks_mut(BLOCK).zip(r.chunks_mut(BLOCK)) {
            fx.process(cl, cr);
        }
        l
    }

    fn sine(len: usize, hz: f32, amp: f32) -> Vec<f32> {
        (0..len).map(|i| amp * (TAU * hz * i as f32 / SR).sin()).collect()
    }

    fn rms(v: &[f32]) -> f32 {
        (v.iter().map(|x| x * x).sum::<f32>() / v.len().max(1) as f32).sqrt()
    }

    #[test]
    fn centered_knob_is_transparent_for_every_kind() {
        let x = sine(9_600, 440.0, 0.5);
        for kind in [ColorFxKind::Filter, ColorFxKind::Space, ColorFxKind::DubEcho, ColorFxKind::Crush, ColorFxKind::Noise] {
            let mut fx = make(kind, 0.0);
            assert_eq!(run(&mut fx, &x), x, "{kind:?}");
        }
    }

    #[test]
    fn crush_quantises_to_a_coarse_set_of_levels() {
        let mut fx = make(ColorFxKind::Crush, 1.0);
        let x = sine(48_000, 220.0, 0.8);
        let out = run(&mut fx, &x);
        let tail = &out[24_000..];
        let mut distinct: Vec<i64> = tail.iter().map(|v| (v * 1e5).round() as i64).collect();
        distinct.sort_unstable();
        distinct.dedup();
        assert!(distinct.len() <= 64, "{} distinct levels", distinct.len());
        let held = tail.windows(2).filter(|w| w[0] == w[1]).count();
        assert!(held > tail.len() / 2, "sample-rate reduction holds values");
    }

    #[test]
    fn crush_amount_scales_with_distance_from_centre() {
        let x = sine(24_000, 220.0, 0.8);
        let err = |v: f32| {
            let out = run(&mut make(ColorFxKind::Crush, v), &x);
            rms(&out.iter().zip(&x).map(|(a, b)| a - b).collect::<Vec<_>>()[12_000..])
        };
        assert!(err(0.2) < err(0.6) && err(0.6) < err(1.0));
        assert!((err(-1.0) - err(1.0)).abs() < 1e-6, "symmetric around centre");
    }

    #[test]
    fn noise_is_silent_in_the_dead_zone_and_swells_on_both_sides() {
        let silence = vec![0.0; 24_000];
        let level = |v: f32| rms(&run(&mut make(ColorFxKind::Noise, v), &silence)[12_000..]);
        assert_eq!(level(0.02), 0.0);
        assert!(level(0.5) > 0.0003 && level(-0.5) > 0.0003);
        assert!(level(0.1) > level(0.05) && level(-0.6) > level(-0.2));
    }

    #[test]
    fn negative_noise_is_darker_than_positive() {
        let silence = vec![0.0; 24_000];
        let hf = |v: f32| {
            let out = run(&mut make(ColorFxKind::Noise, v), &silence);
            let diff: Vec<f32> = out.windows(2).map(|w| w[1] - w[0]).collect();
            rms(&diff[12_000..]) / rms(&out[12_000..]).max(1e-9)
        };
        assert!(hf(-0.8) < hf(0.8));
    }

    #[test]
    fn space_tail_rings_through_the_dead_zone_after_the_knob_returns_to_centre_then_dies() {
        let mut fx = make(ColorFxKind::Space, 0.8);
        let burst = sine(12_000, 3_000.0, 0.5);
        let _ = run(&mut fx, &burst);
        fx.set_value(0.0);
        let silence = vec![0.0; 48_000 * 12];
        let out = run(&mut fx, &silence);
        let early = rms(&out[4_800..24_000]);
        let at_three_seconds = rms(&out[144_000..168_000]);
        let late = rms(&out[480_000..520_000]);
        eprintln!("space tail after returning to centre: {early:.4} early, {at_three_seconds:.6} at 3 s, {late:.8} at 10 s");
        assert!(early > 1e-2, "the tail keeps ringing: {early}");
        assert!(at_three_seconds < early * 0.05, "and falls away within a few seconds");
        assert!(late < early * 1e-4);
        assert!(out.iter().all(|v| v.is_finite()));
    }

    fn rt60_seconds(value: f32) -> f32 {
        let mut fx = make(ColorFxKind::Space, value);
        let mut x = vec![0.0; 48_000 * 40];
        x[48_000] = 0.5;
        let out = run(&mut fx, &x);
        let energy: Vec<f64> = out[60_000..].iter().map(|v| (*v as f64).powi(2)).collect();
        let mut edc = vec![0.0f64; energy.len()];
        let mut acc = 0.0;
        for i in (0..energy.len()).rev() {
            acc += energy[i];
            edc[i] = acc;
        }
        let at = |db: f64| edc.iter().position(|v| 10.0 * (v / edc[0]).log10() < db).unwrap() as f32 / SR;
        2.0 * (at(-35.0) - at(-5.0))
    }

    #[test]
    fn space_decay_follows_the_feedback_curve_long_at_the_extremes_and_short_near_the_centre() {
        let times: Vec<(f32, f32)> = [-1.0, -0.5, -0.2, 0.2, 0.5, 1.0].iter().map(|v| (*v, rt60_seconds(*v))).collect();
        eprintln!("space RT60 by knob: {times:.1?}");
        for (v, t) in &times {
            assert!(t.is_finite() && *t > 0.5, "{v}: {t}");
        }
        assert!(times[0].1 > 8.0 && times[5].1 > 8.0, "the extremes ring for many seconds");
        assert!(times[0].1 > times[1].1 && times[1].1 > times[2].1, "left side shortens toward the centre");
        assert!(times[5].1 > times[4].1 && times[4].1 > times[3].1, "right side shortens toward the centre");
    }

    fn tone_gain_db(kind: ColorFxKind, value: f32, hz: f32) -> f32 {
        let x = sine(48_000 * 3, hz, 0.3);
        let out = run(&mut make(kind, value), &x);
        let (mut c, mut q) = (0.0f64, 0.0f64);
        for (i, v) in out.iter().enumerate().skip(96_000) {
            let phase = std::f64::consts::TAU * hz as f64 * i as f64 / SR as f64;
            c += *v as f64 * phase.cos();
            q += *v as f64 * phase.sin();
        }
        let amplitude = 2.0 * (c * c + q * q).sqrt() / (out.len() - 96_000) as f64;
        20.0 * (amplitude / 0.3).log10() as f32
    }

    #[test]
    fn space_adds_little_below_150_hz_and_nothing_at_all_on_the_right_half() {
        let mut worst = 0.0f32;
        for v in [-1.0, -0.8, -0.5, -0.25, -0.1] {
            for hz in [60.0, 100.0, 140.0] {
                let change = tone_gain_db(ColorFxKind::Space, v, hz);
                worst = worst.max(change.abs());
                assert!(change.abs() < 2.0, "knob {v}, {hz} Hz: {change:+.3} dB");
            }
        }
        for v in [0.1, 0.25, 0.5, 0.8, 1.0] {
            for hz in [40.0, 100.0, 140.0] {
                let change = tone_gain_db(ColorFxKind::Space, v, hz);
                assert!(change.abs() < 0.3, "knob {v}, {hz} Hz: {change:+.3} dB");
            }
        }
        eprintln!("space: worst change on the left half between 60 and 140 Hz {worst:.2} dB");
    }

    #[test]
    fn space_adds_twice_the_core_wet_to_an_untouched_dry_and_the_shimmer_layer_is_off_by_default() {
        assert_eq!(ColorFx::new(SR, 1).params(ColorFxKind::Space)[0], 0.0, "shimmer defaults to zero");
        let x = music(48_000 * 3, 0.25);
        for value in [-0.8f32, 0.6] {
            let mut core = SpaceCore::new(SR);
            core.set_knob(value);
            let expected: Vec<f32> = x
                .iter()
                .map(|v| {
                    let (wet, _) = core.process(*v, *v);
                    v + soft_limit(wet)
                })
                .collect();
            let out = run(&mut make(ColorFxKind::Space, value), &x);
            let worst = out.iter().zip(&expected).fold(0.0f32, |m, (a, b)| m.max((a - b).abs()));
            assert!(worst < 1e-5, "knob {value}: dry plus wet, nothing else: {worst}");
        }
    }

    #[test]
    fn space_dry_path_is_untouched_on_both_sides() {
        for v in [-1.0, 1.0] {
            assert!(tone_gain_db(ColorFxKind::Space, v, 8_000.0) > -0.5, "{v}: treble stays at dry level or above");
        }
        let x = sine(48_000, 5_000.0, 0.3);
        let out = run(&mut make(ColorFxKind::Space, 1.0), &x);
        assert!(out.iter().zip(&x).take(2_000).all(|(o, d)| (o - d).abs() < 0.2), "wet has not arrived yet or is small");
    }

    #[test]
    fn space_right_extreme_adds_a_shimmer_an_octave_above_the_input() {
        let x = sine(48_000 * 4, 3_000.0, 0.3);
        let octave = |scale: f32| {
            let mut fx = make(ColorFxKind::Space, 1.0);
            fx.set_param(ColorFxKind::Space, 0, 50.0);
            fx.shimmer_scale = scale;
            let out = run(&mut fx, &x);
            tone_amplitude(&out[96_000..], 6_000.0)
        };
        let (on, off) = (octave(1.0), octave(0.0));
        let rise = 20.0 * (on / off.max(1e-9)).log10();
        eprintln!("space shimmer: 6 kHz partial {on:.4} with, {off:.5} without ({rise:+.1} dB)");
        assert!(rise > 12.0 && on > 0.005, "{rise} dB, level {on}");
    }

    #[test]
    fn space_shimmer_fades_before_the_tail_ends() {
        let mut fx = make(ColorFxKind::Space, 1.0);
        fx.set_param(ColorFxKind::Space, 0, 50.0);
        let _ = run(&mut fx, &sine(48_000 * 2, 3_000.0, 0.3));
        fx.set_value(0.0);
        let out = run(&mut fx, &vec![0.0; 48_000 * 14]);
        let at = |secs: usize| {
            let w = &out[48_000 * secs..48_000 * (secs + 1)];
            tone_amplitude(w, 6_000.0) / tone_amplitude(w, 3_000.0).max(1e-9)
        };
        eprintln!("shimmer-to-fundamental ratio: 1 s {:.2}, 6 s {:.2}", at(1), at(6));
        assert!(at(6) < at(1) * 0.8 || tone_amplitude(&out[48_000 * 6..48_000 * 7], 3_000.0) < 1e-4);
    }

    #[test]
    fn space_knob_moves_and_side_flips_do_not_click() {
        let x = sine(48_000 * 4, 220.0, 0.3);
        let worst = |moves: bool| {
            let mut fx = make(ColorFxKind::Space, 0.5);
            let (mut l, mut r) = (x.clone(), x.clone());
            let (mut prev, mut worst) = (0.0f32, 0.0f32);
            for (b, (cl, cr)) in l.chunks_mut(BLOCK).zip(r.chunks_mut(BLOCK)).enumerate() {
                if moves && b % 30 == 0 {
                    fx.set_value([1.0, -1.0, 0.2, 0.9, -0.6, 0.0][(b / 30) % 6]);
                }
                fx.process(cl, cr);
                for v in cl.iter() {
                    if b > 100 {
                        worst = worst.max((v - prev).abs());
                    }
                    prev = *v;
                }
            }
            (worst, peak_of(&l))
        };
        let ((moving, level), (steady, _)) = (worst(true), worst(false));
        let sine_step = 1.5 * level * TAU * 440.0 / SR;
        eprintln!("space knob moves: worst step {moving:.4} (sine bound {sine_step:.4}), steady {steady:.4}");
        assert!(moving < sine_step.max(2.0 * steady + 0.03), "{moving} vs {sine_step}");
    }

    fn peak_of(v: &[f32]) -> f32 {
        v.iter().fold(0.0f32, |m, x| m.max(x.abs()))
    }

    fn wet_of(kind: ColorFxKind, value: f32, x: &[f32]) -> Vec<f32> {
        run(&mut make(kind, value), x).iter().zip(x).map(|(o, d)| o - d).collect()
    }

    #[test]
    fn space_left_side_is_a_darker_mid_wash_and_right_side_a_bright_one() {
        let x = white(48_000 * 4, 4, 0.4);
        let wet_left = wet_of(ColorFxKind::Space, -0.9, &x);
        let wet_right = wet_of(ColorFxKind::Space, 0.9, &x);
        let ratio = |w: &[f32]| band_psd(&w[96_000..], 5_000.0, 12_000.0) / band_psd(&w[96_000..], 300.0, 1_000.0);
        eprintln!("space wet high/mid power: left {:.4}, right {:.4}", ratio(&wet_left), ratio(&wet_right));
        assert!(ratio(&wet_left) * 10.0 < ratio(&wet_right));
        assert!(band_psd(&wet_left[96_000..], 300.0, 1_000.0) > band_psd(&wet_right[96_000..], 300.0, 1_000.0) * 10.0, "left carries the mids");
    }

    #[test]
    fn dub_echo_repeats_on_the_dotted_eighth_and_decays() {
        let mut fx = make(ColorFxKind::DubEcho, 0.9);
        let delay = (0.75 * 0.5 * SR) as usize;
        let mut x = vec![0.0; 120_000];
        x[8_000] = 0.8;
        let out = run(&mut fx, &x);
        let window = |c: usize| out[c - 40..c + 40].iter().fold(0.0f32, |m, v| m.max(v.abs()));
        let first = window(8_000 + delay);
        let second = window(8_000 + 2 * delay);
        assert!(first > 0.05, "first repeat {first}");
        assert!(second < first, "decays");
    }

    #[test]
    fn kind_switch_does_not_click() {
        let x = sine(48_000 * 2, 220.0, 0.3);
        let mut fx = make(ColorFxKind::DubEcho, 0.7);
        fx.space_hiss_scale = 0.0;
        let (mut l, mut r) = (x.clone(), x.clone());
        let mut prev = 0.0;
        let mut worst: f32 = 0.0;
        for (b, (cl, cr)) in l.chunks_mut(BLOCK).zip(r.chunks_mut(BLOCK)).enumerate() {
            if b == 100 {
                fx.set_kind(ColorFxKind::Space);
            }
            if b == 250 {
                fx.set_kind(ColorFxKind::DubEcho);
            }
            fx.process(cl, cr);
            for v in cl.iter() {
                worst = worst.max((v - prev).abs());
                prev = *v;
            }
        }
        assert!(worst < 0.05, "worst step {worst}");
        assert_eq!(fx.kind(), ColorFxKind::DubEcho);
    }

    #[test]
    fn full_scale_input_stays_bounded() {
        let loud = sine(96_000, 1_000.0, 1.0);
        for kind in [ColorFxKind::Space, ColorFxKind::DubEcho, ColorFxKind::Crush, ColorFxKind::Noise] {
            for v in [-1.0, 1.0] {
                let out = run(&mut make(kind, v), &loud);
                assert!(out.iter().all(|x| x.is_finite() && x.abs() <= 2.0), "{kind:?} {v}");
            }
        }
    }

    fn noise_only(value: f32, x: &[f32]) -> Vec<f32> {
        let mut a = ColorFx::new(SR, 11);
        let mut b = ColorFx::new(SR, 977);
        for fx in [&mut a, &mut b] {
            fx.set_kind(ColorFxKind::Noise);
            fx.set_value(value);
        }
        let (oa, ob) = (run(&mut a, x), run(&mut b, x));
        oa.iter().zip(&ob).map(|(p, q)| (p - q) * std::f32::consts::FRAC_1_SQRT_2).collect()
    }

    fn music(len: usize, amp: f32) -> Vec<f32> {
        let mut n = Noise::new(23);
        let mut z = 0.0f32;
        let lowpassed: Vec<f32> = (0..len).map(|_| { z += 0.04 * (n.next() - z); z }).collect();
        let level = rms(&lowpassed[len / 2..]);
        lowpassed.iter().map(|v| v * amp / level).collect()
    }

    #[test]
    fn noise_stays_under_the_music_and_is_loudest_on_the_dark_side_tapering_on_the_bright_side() {
        let x = music(48_000 * 6, 0.25);
        let dry = rms(&x[96_000..]);
        let level = |v: f32| db(rms(&noise_only(v, &x)[96_000..]) / dry);
        let right: Vec<f32> = [0.1, 0.2, 0.4, 0.6, 0.8, 1.0].iter().map(|v| level(*v)).collect();
        let left: Vec<f32> = [-0.1, -0.2, -0.4, -0.6, -0.8, -1.0].iter().map(|v| level(*v)).collect();
        eprintln!("noise re music, right 0.1..1.0: {right:.1?}; left -0.1..-1.0: {left:.1?}");
        for v in [-1.0, -0.8, -0.6, -0.4, -0.2, -0.05, 0.05, 0.2, 0.4, 0.6, 0.8, 1.0] {
            assert!(level(v) < -4.0, "knob {v}: {} dB", level(v));
        }
        let (left_peak, right_peak) = (left.iter().cloned().fold(f32::MIN, f32::max), right.iter().cloned().fold(f32::MIN, f32::max));
        assert!((-8.0..-4.0).contains(&left_peak), "dark side peak {left_peak} dB");
        assert!(left_peak > right_peak, "louder on the dark side: {left_peak} vs {right_peak}");
        assert!(right[5] < right_peak - 6.0, "the bright extreme tapers off: {} vs peak {right_peak}", right[5]);
    }

    #[test]
    fn noise_band_sweeps_exponentially_from_133_hz_to_20_khz_in_both_directions() {
        let right: Vec<f32> = [0.15, 0.4, 0.7, 1.0].iter().map(|v| noise_colour(*v).0).collect();
        let left: Vec<f32> = [-1.0, -0.7, -0.4, -0.15].iter().map(|v| noise_colour(*v).0).collect();
        assert!(right.windows(2).all(|w| w[1] > w[0]) && left.windows(2).all(|w| w[1] > w[0]), "{right:?} {left:?}");
        assert!((right[3] - NOISE_MAX_HZ).abs() < 1.0 && (left[0] - NOISE_MIN_HZ).abs() < 1.0);
        let (a, b) = (noise_colour(0.5).0, noise_colour(1.0).0);
        assert!((a - (NOISE_MIN_HZ * NOISE_MAX_HZ).sqrt()).abs() < 5.0, "the half-way knob sits at the geometric mean: {a}");
        assert!(b / a > 11.0 && b / a < 13.0);
        let x = music(48_000 * 5, 0.25);
        let measured: Vec<f32> = [0.2, 0.5, 0.8].iter().map(|v| spectral_centroid(&noise_only(*v, &x)[96_000..96_000 + 16_384])).collect();
        eprintln!("noise spectral centroid at the right knob 0.2/0.5/0.8: {measured:?}");
        assert!(measured.windows(2).all(|w| w[1] > w[0]), "{measured:?}");
        let measured_left: Vec<f32> = [-0.8, -0.5, -0.2].iter().map(|v| spectral_centroid(&noise_only(*v, &x)[96_000..96_000 + 16_384])).collect();
        assert!(measured_left.windows(2).all(|w| w[1] > w[0]), "{measured_left:?}");
    }

    #[test]
    fn noise_colour_gain_rises_on_the_left_and_peaks_then_tapers_on_the_right() {
        let left: Vec<f32> = [-0.2, -0.4, -0.6, -0.8, -1.0].iter().map(|v| noise_colour(*v).1).collect();
        assert!(left.windows(2).all(|w| w[1] > w[0]), "{left:?}");
        let right: Vec<f32> = [0.05, 0.1, 0.2, 0.4, 0.6, 0.8, 1.0].iter().map(|v| noise_colour(*v).1).collect();
        let peak = right.iter().cloned().fold(0.0, f32::max);
        assert!(right[0] < 0.7 * peak, "rises from silence");
        assert!(right[6] < 0.5 * peak, "tapers at the bright end: {right:?}");
        assert_eq!(noise_colour(0.0).1, 0.0);
        assert!((noise_level_gain(100.0) - 1.059_253_7).abs() < 1e-6);
        assert!((noise_level_gain(50.0) - 0.529_626_85).abs() < 1e-6);
        assert_eq!(noise_level_gain(0.0), 0.0);
    }

    #[test]
    fn noise_reaches_down_to_the_lows_on_the_left_and_up_to_the_air_on_the_right() {
        let x = music(48_000 * 5, 0.25);
        let low = noise_only(-1.0, &x);
        assert!(band_psd(&low[96_000..], 100.0, 250.0) > 100.0 * band_psd(&low[96_000..], 2_000.0, 4_000.0), "dark extreme sits in the lows");
        let high = noise_only(1.0, &x);
        assert!(band_psd(&high[96_000..], 10_000.0, 18_000.0) > 100.0 * band_psd(&high[96_000..], 100.0, 400.0), "bright extreme sits in the air");
    }

    #[test]
    fn noise_is_ducked_by_the_music() {
        let x = music(48_000 * 5, 0.25);
        let measure = |scale: f32| {
            let mut a = ColorFx::new(SR, 11);
            let mut b = ColorFx::new(SR, 977);
            for fx in [&mut a, &mut b] {
                fx.set_kind(ColorFxKind::Noise);
                fx.set_value(1.0);
                fx.noise_duck_scale = scale;
            }
            let (oa, ob) = (run(&mut a, &x), run(&mut b, &x));
            rms(&oa.iter().zip(&ob).map(|(p, q)| p - q).collect::<Vec<_>>()[96_000..])
        };
        let depth = db(measure(1.0) / measure(0.0));
        eprintln!("noise duck depth under loud music: {depth:+.2} dB");
        assert!((-3.0..=-1.0).contains(&depth), "{depth} dB");
    }

    #[test]
    fn noise_tail_falls_in_pitch_while_it_fades() {
        let x = music(48_000 * 3, 0.25);
        let mut a = ColorFx::new(SR, 11);
        let mut b = ColorFx::new(SR, 977);
        for fx in [&mut a, &mut b] {
            fx.set_kind(ColorFxKind::Noise);
            fx.set_value(1.0);
        }
        let _ = (run(&mut a, &x), run(&mut b, &x));
        for fx in [&mut a, &mut b] {
            fx.set_value(0.0);
        }
        let silence = vec![0.0; 48_000 * 4];
        let (oa, ob) = (run(&mut a, &silence), run(&mut b, &silence));
        let diff: Vec<f32> = oa.iter().zip(&ob).map(|(p, q)| p - q).collect();
        let window = |from: f32| &diff[(from * SR) as usize..(from * SR) as usize + 8_192];
        let (early, late) = (spectral_centroid(window(0.05)), spectral_centroid(window(1.4)));
        eprintln!("noise tail centroid {early:.0} Hz -> {late:.0} Hz, level {:.4} -> {:.4}", rms(window(0.05)), rms(window(1.4)));
        assert!(late < early * 0.6, "cutoff closes downward: {early} -> {late}");
        assert!(rms(window(1.4)) < rms(window(0.05)) * 0.5, "level fades");
    }

    fn spectral_centroid(v: &[f32]) -> f32 {
        let (mut weighted, mut total) = (0.0f64, 0.0f64);
        for k in 0..64 {
            let hz = 60.0 * (20_000.0f64 / 60.0).powf(k as f64 / 63.0);
            let w = std::f64::consts::TAU * hz / SR as f64;
            let (mut c, mut q) = (0.0f64, 0.0f64);
            for (i, x) in v.iter().enumerate() {
                let phase = w * i as f64;
                c += *x as f64 * phase.cos();
                q += *x as f64 * phase.sin();
            }
            let power = c * c + q * q;
            weighted += hz * power;
            total += power;
        }
        (weighted / total) as f32
    }

    #[test]
    fn noise_darkens_towards_the_left_extreme_and_brightens_towards_the_right_one() {
        let x = music(48_000 * 5, 0.25);
        let right: Vec<Vec<f32>> = [0.2, 0.5, 0.8, 1.0].iter().map(|v| noise_only(*v, &x)).collect();
        let left: Vec<Vec<f32>> = [-0.2, -0.5, -0.8, -1.0].iter().map(|v| noise_only(*v, &x)).collect();
        let bright = |v: &Vec<f32>| spectral_centroid(&v[96_000..96_000 + 16_384]);
        assert!(right.windows(2).all(|w| bright(&w[1]) > bright(&w[0])), "brighter to the right");
        assert!(left.windows(2).all(|w| bright(&w[1]) < bright(&w[0])), "darker to the left");
        assert!(bright(&right[3]) > 10.0 * bright(&left[3]));
    }

    #[test]
    fn noise_is_added_to_an_untouched_dry_signal() {
        for knob in [-1.0, -0.5, 0.5, 1.0] {
            for hz in [100.0, 1_000.0, 8_000.0] {
                let change = tone_gain_db(ColorFxKind::Noise, knob, hz);
                assert!(change.abs() < 0.5, "knob {knob}, {hz} Hz: {change:+.2} dB");
            }
        }
        let x = sine(24_000, 440.0, 0.4);
        let mut fx = make(ColorFxKind::Noise, 0.8);
        let (mut l, mut r) = (x.clone(), x.clone());
        for (cl, cr) in l.chunks_mut(BLOCK).zip(r.chunks_mut(BLOCK)) {
            fx.process(cl, cr);
        }
        let added: Vec<f32> = l.iter().zip(&x).map(|(o, d)| o - d).collect();
        assert!(rms(&added[12_000..]) > 0.001, "something was added");
        assert!(tone_amplitude(&added[12_000..], 440.0) < 0.2 * rms(&added[12_000..]), "and none of it is the programme");
    }

    #[test]
    fn noise_is_independent_in_each_channel_and_the_level_parameter_scales_it() {
        let silence = vec![0.0; 48_000];
        let render = |level: f32| {
            let mut fx = make(ColorFxKind::Noise, 0.6);
            fx.set_param(ColorFxKind::Noise, 0, level);
            let (mut l, mut r) = (silence.clone(), silence.clone());
            for (cl, cr) in l.chunks_mut(BLOCK).zip(r.chunks_mut(BLOCK)) {
                fx.process(cl, cr);
            }
            (l, r)
        };
        let (l, r) = render(50.0);
        let (l, r) = (&l[24_000..], &r[24_000..]);
        let dot: f32 = l.iter().zip(r).map(|(a, b)| a * b).sum();
        let corr = dot / (l.iter().map(|v| v * v).sum::<f32>() * r.iter().map(|v| v * v).sum::<f32>()).sqrt();
        assert!(corr.abs() < 0.1, "decorrelated channels: {corr}");
        let (quiet, default, loud) = (rms(&render(25.0).0[24_000..]), rms(&render(50.0).0[24_000..]), rms(&render(100.0).0[24_000..]));
        assert!(quiet < default && default < loud, "{quiet} {default} {loud}");
        assert_eq!(rms(&render(0.0).0[24_000..]), 0.0);
    }

    #[test]
    fn noise_tail_is_a_short_wash_after_the_knob_returns() {
        let x = music(48_000 * 3, 0.25);
        let mut fx = make(ColorFxKind::Noise, 1.0);
        let _ = run(&mut fx, &x);
        fx.set_value(0.0);
        let out = run(&mut fx, &vec![0.0; 48_000 * 6]);
        assert!(rms(&out[2_400..12_000]) > 1e-3, "reverb of the noise rings");
        assert!(rms(&out[48_000 * 5..]) < 1e-4);
    }

    #[test]
    fn space_and_noise_stay_bounded_for_thirty_seconds_of_full_scale_noise() {
        let mut n = Noise::new(5);
        let x: Vec<f32> = (0..48_000 * 30).map(|_| n.next()).collect();
        for kind in [ColorFxKind::Space, ColorFxKind::Noise] {
            for v in [-1.0, 1.0] {
                let out = run(&mut make(kind, v), &x);
                assert!(out.iter().all(|s| s.is_finite() && s.abs() <= 3.0), "{kind:?} {v}");
            }
        }
    }

    fn hiss_of(value: f32, x: &[f32]) -> Vec<f32> {
        let render = |scale: f32| {
            let mut fx = make(ColorFxKind::Space, value);
            fx.set_param(ColorFxKind::Space, 0, 50.0);
            fx.space_hiss_scale = scale;
            run(&mut fx, x)
        };
        render(1.0).iter().zip(render(0.0)).map(|(a, b)| a - b).collect()
    }

    #[test]
    fn space_high_noise_rises_with_the_knob_on_the_right() {
        let x = music(48_000 * 6, 0.25);
        let high = |v: f32| band_psd(&hiss_of(v, &x)[96_000..], 4_000.0, 16_000.0);
        let rising: Vec<f32> = [0.35, 0.5, 0.7, 0.85, 1.0].iter().map(|v| high(*v)).collect();
        eprintln!("space hiss psd above 4 kHz at knob 0.2 {:.3e}, then {rising:?}", high(0.2));
        assert!(rising.windows(2).all(|w| w[1] > w[0] * 1.2), "{rising:?}");
        assert!(high(0.2) < rising[4] * 1e-3, "near zero before 30 % of the turn");
        let centroids: Vec<f32> = [0.5, 0.75, 1.0].iter().map(|v| spectral_centroid(&hiss_of(*v, &x)[96_000..96_000 + 16_384])).collect();
        eprintln!("space hiss centroid at 0.5/0.75/1.0: {centroids:?}");
        assert!(centroids.windows(2).all(|w| w[1] > w[0]) && centroids[0] > 4_000.0, "{centroids:?}");
    }

    #[test]
    fn space_noise_stays_clear_of_the_lows() {
        let x = music(48_000 * 6, 0.25);
        for v in [-1.0, -0.6, 0.6, 1.0] {
            let hiss = hiss_of(v, &x);
            let (low, band) = (band_psd(&hiss[96_000..], 30.0, 150.0), band_psd(&hiss[96_000..], 1_500.0, 12_000.0));
            let gap = db((band / low.max(1e-30)).sqrt());
            assert!(gap > 35.0, "knob {v}: {gap:.1} dB");
        }
    }

    #[test]
    fn space_noise_peaks_about_twelve_db_under_the_music_on_the_right_and_eighteen_on_the_left() {
        let x = music(48_000 * 6, 0.25);
        let dry = rms(&x[96_000..]);
        let level = |v: f32| db(rms(&hiss_of(v, &x)[96_000..]) / dry);
        let (right, left) = (level(1.0), level(-1.0));
        eprintln!("space hiss re dry rms: right {right:.1} dB, left {left:.1} dB; right 0.7 {:.1} dB, left -0.7 {:.1} dB", level(0.7), level(-0.7));
        assert!((-14.0..=-11.0).contains(&right), "right {right} dB");
        assert!((-20.0..=-17.0).contains(&left), "left {left} dB");
        assert!(level(0.7) < right - 3.0 && level(-0.7) < left - 3.0);
    }

    #[test]
    fn space_noise_is_ducked_by_the_music() {
        let x = music(48_000 * 6, 0.25);
        let measure = |duck: f32| {
            let render = |scale: f32| {
                let mut fx = make(ColorFxKind::Space, 1.0);
                fx.set_param(ColorFxKind::Space, 0, 50.0);
                fx.space_hiss_scale = scale;
                fx.noise_duck_scale = duck;
                run(&mut fx, &x)
            };
            let hiss: Vec<f32> = render(1.0).iter().zip(render(0.0)).map(|(a, b)| a - b).collect();
            rms(&hiss[96_000..])
        };
        let depth = db(measure(1.0) / measure(0.0));
        eprintln!("space hiss duck under loud music: {depth:+.2} dB");
        assert!(depth < -2.0, "{depth} dB");
    }

    #[test]
    fn space_noise_tail_rings_with_the_reverb_after_the_knob_returns_to_centre() {
        let x = music(48_000 * 3, 0.25);
        let render = |scale: f32| {
            let mut fx = make(ColorFxKind::Space, 1.0);
            fx.set_param(ColorFxKind::Space, 0, 50.0);
            fx.space_hiss_scale = scale;
            let _ = run(&mut fx, &x);
            fx.set_value(0.0);
            run(&mut fx, &vec![0.0; 48_000 * 14])
        };
        let hiss: Vec<f32> = render(1.0).iter().zip(render(0.0)).map(|(a, b)| a - b).collect();
        let at = |secs: f32| band_psd(&hiss[(secs * SR) as usize..(secs * SR) as usize + 9_600], 4_000.0, 16_000.0).sqrt();
        eprintln!("space hiss tail above 4 kHz: {:.5} at 0.3 s, {:.5} at 3 s, {:.7} at 12 s", at(0.3), at(3.0), at(12.0));
        assert!(at(0.3) > 1e-5 && at(3.0) > 1e-7, "rings on");
        assert!(at(12.0) < at(0.3) * 0.02, "and fades with the reverb");
        assert!(hiss.iter().all(|v| v.is_finite()));
    }
}
