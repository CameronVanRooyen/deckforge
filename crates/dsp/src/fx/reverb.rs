use std::f32::consts::TAU;

use super::util::{flush, AllPass};

const LINES: usize = 12;
const BASE_LENGTHS_48K: [usize; LINES] = [1103, 1277, 1459, 1657, 1831, 2017, 2203, 2389, 2579, 2767, 2963, 3163];
const DIFFUSER_LENGTHS_48K: [[usize; 4]; 2] = [[113, 331, 577, 863], [127, 359, 613, 911]];
const DIFFUSER_GAIN: f32 = 0.64;
const INPUT_GAIN: f32 = 0.3;
const OUTPUT_GAIN: f32 = 0.9;
const OUTPUT_REF_RT60: f32 = 2.0;
const OUTPUT_RT60_EXPONENT: f32 = 0.4;
const MAX_MOD_MS: f32 = 1.6;
const MOD_STEP: usize = 16;
const MOD_RATES_HZ: [f32; LINES] = [0.11, 0.17, 0.23, 0.29, 0.37, 0.43, 0.13, 0.19, 0.31, 0.41, 0.47, 0.53];
const MAX_SIZE_SCALE: f32 = 1.5;
const MIN_SIZE_SCALE: f32 = 0.25;
const GLIDE_SECS: f32 = 0.06;
const GLIDE_MAX_RATE: f32 = 0.14;
const OUTPUT_FLOOR: f32 = 0.3;
const GLIDE_SNAP: f32 = 0.02;
const MAX_PREDELAY_SECS: f32 = 0.12;
const PREDELAY_SLEW: f32 = 0.1;
const HF_REFERENCE_HZ: f32 = 4_000.0;
const OUTPUT_SIGNS: [f32; LINES] = [1.0, 1.0, -1.0, 1.0, 1.0, -1.0, -1.0, -1.0, 1.0, -1.0, 1.0, 1.0];

#[derive(Debug, Clone)]
struct Line {
    buf: Vec<f32>,
    mask: usize,
    pos: usize,
}

impl Line {
    fn new(min_len: usize) -> Self {
        let cap = min_len.next_power_of_two().max(16);
        Self { buf: vec![0.0; cap], mask: cap - 1, pos: 0 }
    }

    #[inline]
    fn read(&self, delay: f32) -> f32 {
        let k = delay as usize;
        let t = delay - k as f32;
        let at = |n: usize| self.buf[self.pos.wrapping_sub(n) & self.mask];
        let (newer, a, b, older) = (at(k - 1), at(k), at(k + 1), at(k + 2));
        let w_newer = -t * (t - 1.0) * (t - 2.0) * (1.0 / 6.0);
        let w_a = (t + 1.0) * (t - 1.0) * (t - 2.0) * 0.5;
        let w_b = -(t + 1.0) * t * (t - 2.0) * 0.5;
        let w_older = (t + 1.0) * t * (t - 1.0) * (1.0 / 6.0);
        newer * w_newer + a * w_a + b * w_b + older * w_older
    }

    #[inline]
    fn write_advance(&mut self, v: f32) {
        self.buf[self.pos] = v;
        self.pos = (self.pos + 1) & self.mask;
    }

    fn clear(&mut self) {
        self.buf.fill(0.0);
        self.pos = 0;
    }
}

#[derive(Debug, Clone)]
struct PreDelay {
    buf: [Vec<f32>; 2],
    mask: usize,
    pos: usize,
    current: f32,
    target: f32,
}

impl PreDelay {
    fn new(sample_rate: f32) -> Self {
        let cap = ((MAX_PREDELAY_SECS * sample_rate) as usize + 8).next_power_of_two();
        Self { buf: [vec![0.0; cap], vec![0.0; cap]], mask: cap - 1, pos: 0, current: 1.0, target: 1.0 }
    }

    #[inline]
    fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        self.current += (self.target - self.current).clamp(-PREDELAY_SLEW, PREDELAY_SLEW);
        self.buf[0][self.pos] = l;
        self.buf[1][self.pos] = r;
        let k = self.current as usize;
        let f = self.current - k as f32;
        let tap = |b: &[f32]| {
            let a = b[self.pos.wrapping_sub(k) & self.mask];
            let c = b[self.pos.wrapping_sub(k + 1) & self.mask];
            a + (c - a) * f
        };
        let out = (tap(&self.buf[0]), tap(&self.buf[1]));
        self.pos = (self.pos + 1) & self.mask;
        out
    }

    fn clear(&mut self) {
        self.buf.iter_mut().for_each(|b| b.fill(0.0));
        self.pos = 0;
    }
}

/// Twelve-line modulated feedback delay network: Householder mixing, frequency-dependent decay, pre-delay, four-stage input diffusion.
/// Resizing glides every line length, so the circulating tail Doppler-shifts instead of crossfading.
#[derive(Debug, Clone)]
pub struct FdnReverb {
    sample_rate: f32,
    lines: [Line; LINES],
    base_lengths: [f32; LINES],
    lengths: [f32; LINES],
    targets: [f32; LINES],
    size_snap: bool,
    size_moving: bool,
    gains: [f32; LINES],
    poles: [f32; LINES],
    damp_state: [f32; LINES],
    rt60: f32,
    hf_ratio: f32,
    mod_phase: [f32; LINES],
    mod_phase_inc: [f32; LINES],
    mod_offset: [f32; LINES],
    mod_slope: [f32; LINES],
    mod_depth: f32,
    mod_tick: usize,
    diffusers: [[AllPass; 4]; 2],
    predelay: PreDelay,
    out_gain: f32,
    out_floor: f32,
    glide_rate: f32,
    ramp_left: usize,
    ramp_step: [f32; LINES],
    fixed_out_gain: Option<f32>,
    width: f32,
}

impl FdnReverb {
    pub fn new(sample_rate: f32) -> Self {
        let scale = sample_rate / 48_000.0;
        let lengths = BASE_LENGTHS_48K.map(|l| (l as f32 * scale).round().max(64.0));
        let max_mod = MAX_MOD_MS * sample_rate / 1000.0;
        let mut r = Self {
            sample_rate,
            lines: lengths.map(|l| Line::new((l * MAX_SIZE_SCALE) as usize + max_mod as usize + 8)),
            base_lengths: lengths,
            lengths,
            targets: lengths,
            size_snap: false,
            size_moving: false,
            gains: [0.0; LINES],
            poles: [0.0; LINES],
            damp_state: [0.0; LINES],
            rt60: 0.0,
            hf_ratio: 1.0,
            mod_phase: std::array::from_fn(|i| i as f32 / LINES as f32),
            mod_phase_inc: MOD_RATES_HZ.map(|hz| hz * MOD_STEP as f32 / sample_rate),
            mod_offset: [0.0; LINES],
            mod_slope: [0.0; LINES],
            mod_depth: 0.45 * sample_rate / 1000.0,
            mod_tick: 0,
            diffusers: DIFFUSER_LENGTHS_48K
                .map(|side| side.map(|l| AllPass::new((l as f32 * scale) as usize, DIFFUSER_GAIN))),
            predelay: PreDelay::new(sample_rate),
            out_gain: OUTPUT_GAIN,
            out_floor: OUTPUT_FLOOR,
            glide_rate: GLIDE_MAX_RATE,
            ramp_left: 0,
            ramp_step: [0.0; LINES],
            fixed_out_gain: None,
            width: 1.0,
        };
        r.set_rt60(2.0);
        r
    }

    pub fn set_rt60(&mut self, seconds: f32) {
        let rt = seconds.max(0.05);
        if rt != self.rt60 {
            self.rt60 = rt;
            self.update_decay();
        }
    }

    /// Decay time at 4 kHz as a fraction of the low-frequency decay time; 1.0 is a flat decay.
    pub fn set_hf_ratio(&mut self, ratio: f32) {
        let ratio = ratio.clamp(0.1, 1.0);
        if ratio != self.hf_ratio {
            self.hf_ratio = ratio;
            self.update_decay();
        }
    }

    fn update_decay(&mut self) {
        let rt = self.rt60;
        let cos_ref = (TAU * HF_REFERENCE_HZ / self.sample_rate).cos();
        for i in 0..LINES {
            let g = (-6.907_755 * self.lengths[i] / (rt * self.sample_rate)).exp();
            self.gains[i] = g;
            self.poles[i] = if self.hf_ratio >= 1.0 {
                0.0
            } else {
                let r2 = g.powf(1.0 / self.hf_ratio - 1.0).powi(2);
                let b = (1.0 - r2 * cos_ref) / (1.0 - r2).max(1e-6);
                (b - (b * b - 1.0).max(0.0).sqrt()).clamp(0.0, 0.98)
            };
        }
        self.out_gain = self.fixed_out_gain.unwrap_or_else(|| OUTPUT_GAIN * (OUTPUT_REF_RT60 / rt).powf(OUTPUT_RT60_EXPONENT).clamp(self.out_floor, 1.8));
    }

    /// A fixed output trim instead of the decay-dependent one; `None` restores the compensation.
    pub fn set_fixed_output_gain(&mut self, gain: Option<f32>) {
        if gain != self.fixed_out_gain {
            self.fixed_out_gain = gain;
            self.update_decay();
        }
    }

    /// Lowest output trim the long-decay compensation may apply.
    pub fn set_output_floor(&mut self, floor: f32) {
        let floor = floor.clamp(0.02, 1.0);
        if floor != self.out_floor {
            self.out_floor = floor;
            self.update_decay();
        }
    }

    /// Fastest the line lengths may move, in samples per sample; sets how far a size change bends the tail's pitch.
    pub fn set_glide_rate(&mut self, rate: f32) {
        self.glide_rate = rate.clamp(0.002, GLIDE_MAX_RATE);
    }

    /// Scales every line length (1.0 is the reference room); changes glide so the tail bends in pitch.
    pub fn set_size(&mut self, scale: f32) {
        self.ramp_left = 0;
        let scale = scale.clamp(MIN_SIZE_SCALE, MAX_SIZE_SCALE);
        for (t, b) in self.targets.iter_mut().zip(self.base_lengths) {
            *t = b * scale;
        }
        if self.size_snap {
            self.lengths = self.targets;
            self.size_snap = false;
            self.update_decay();
        }
    }

    /// Moves every line length to `scale` in a straight line over `samples` (zero or a fresh reset jumps there).
    pub fn ramp_size(&mut self, scale: f32, samples: usize) {
        let scale = scale.clamp(MIN_SIZE_SCALE, MAX_SIZE_SCALE);
        for (t, b) in self.targets.iter_mut().zip(self.base_lengths) {
            *t = b * scale;
        }
        if samples == 0 || self.size_snap {
            self.lengths = self.targets;
            self.ramp_left = 0;
            self.size_snap = false;
            self.update_decay();
            return;
        }
        for i in 0..LINES {
            self.ramp_step[i] = (self.targets[i] - self.lengths[i]) / samples as f32;
        }
        self.ramp_left = samples;
    }

    pub fn set_predelay_ms(&mut self, ms: f32) {
        let max = MAX_PREDELAY_SECS * self.sample_rate - 4.0;
        self.predelay.target = (ms * 0.001 * self.sample_rate).clamp(1.0, max);
    }

    pub fn set_modulation_ms(&mut self, ms: f32) {
        self.mod_depth = ms.clamp(0.0, MAX_MOD_MS) * self.sample_rate / 1000.0;
    }

    /// 0 collapses the wet signal to mono, 1 is full stereo spread.
    pub fn set_width(&mut self, width: f32) {
        self.width = width.clamp(0.0, 1.0);
    }

    pub fn reset(&mut self) {
        for l in &mut self.lines {
            l.clear();
        }
        for side in &mut self.diffusers {
            side.iter_mut().for_each(AllPass::reset);
        }
        self.predelay.clear();
        self.predelay.current = self.predelay.target;
        self.damp_state = [0.0; LINES];
        self.mod_offset = [0.0; LINES];
        self.mod_slope = [0.0; LINES];
        self.mod_tick = 0;
        self.lengths = self.targets;
        self.ramp_left = 0;
        self.size_snap = true;
        self.size_moving = false;
        self.update_decay();
    }

    fn advance_modulation(&mut self) {
        for i in 0..LINES {
            self.mod_phase[i] += self.mod_phase_inc[i];
            self.mod_phase[i] -= self.mod_phase[i].floor();
            let target = self.mod_depth * (TAU * self.mod_phase[i]).sin();
            self.mod_slope[i] = (target - self.mod_offset[i]) / MOD_STEP as f32;
        }
    }

    #[inline]
    pub fn process(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        let (pl, pr) = self.predelay.process(in_l, in_r);
        let dl = self.diffusers[0].iter_mut().fold(pl, |x, d| d.process(x)) * INPUT_GAIN;
        let dr = self.diffusers[1].iter_mut().fold(pr, |x, d| d.process(x)) * INPUT_GAIN;

        if self.mod_tick == 0 {
            self.advance_modulation();
            if self.size_moving {
                self.size_moving = false;
                self.update_decay();
            }
        }
        self.mod_tick = (self.mod_tick + 1) % MOD_STEP;

        let mut damped = [0f32; LINES];
        let mut fed = [0f32; LINES];
        let mut sum = 0.0;
        let glide = 1.0 / (GLIDE_SECS * self.sample_rate);
        for i in 0..LINES {
            self.mod_offset[i] += self.mod_slope[i];
            let gap = self.targets[i] - self.lengths[i];
            if self.ramp_left > 0 {
                self.lengths[i] += self.ramp_step[i];
                self.size_moving = true;
            } else if gap != 0.0 {
                self.size_moving = true;
                self.lengths[i] = if gap.abs() < GLIDE_SNAP { self.targets[i] } else { self.lengths[i] + (gap * glide).clamp(-self.glide_rate, self.glide_rate) };
            }
            let y = self.lines[i].read(self.lengths[i] + self.mod_offset[i]);
            let (s, p) = (&mut self.damp_state[i], self.poles[i]);
            *s = flush(p * *s + (1.0 - p) * y);
            damped[i] = *s;
            fed[i] = *s * self.gains[i];
            sum += fed[i];
        }
        if self.ramp_left > 0 {
            self.ramp_left -= 1;
            if self.ramp_left == 0 {
                self.lengths = self.targets;
            }
        }
        let mean2 = sum * (2.0 / LINES as f32);
        for (i, (line, f)) in self.lines.iter_mut().zip(fed).enumerate() {
            let inject = if i % 2 == 0 { dl } else { dr };
            line.write_advance(flush(f - mean2 + inject));
        }
        let (mut l, mut r) = (0.0, 0.0);
        for i in 0..LINES {
            let v = damped[i] * OUTPUT_SIGNS[i];
            if i % 2 == 0 { l += v } else { r += v }
        }
        let (l, r) = (l * self.out_gain, r * self.out_gain);
        let mid = 0.5 * (l + r);
        let side = 0.5 * (l - r) * self.width;
        (mid + side, mid - side)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn energy(v: &[f32]) -> f32 {
        v.iter().map(|x| x * x).sum::<f32>() / v.len().max(1) as f32
    }

    #[test]
    fn tail_decays_and_stays_finite() {
        let mut rev = FdnReverb::new(48_000.0);
        rev.set_rt60(1.0);
        let mut out = Vec::new();
        for n in 0..48_000 * 4 {
            let x = if n < 480 { 1.0 } else { 0.0 };
            let (l, r) = rev.process(x, x);
            assert!(l.is_finite() && r.is_finite());
            out.push(l);
        }
        let early = energy(&out[4_800..14_400]);
        let late = energy(&out[96_000..105_600]);
        assert!(early > 0.0);
        assert!(late < early * 0.001, "early {early} late {late}");
    }

    #[test]
    fn rt60_is_about_right() {
        let mut rev = FdnReverb::new(48_000.0);
        rev.set_rt60(1.5);
        rev.set_hf_ratio(1.0);
        let mut e = Vec::new();
        for n in 0..48_000 * 4 {
            let x = if n < 2_000 { ((n * 7919) % 1000) as f32 / 500.0 - 1.0 } else { 0.0 };
            let (l, r) = rev.process(x, x);
            e.push(l * l + r * r);
        }
        let win = |s: usize| e[s..s + 2_400].iter().sum::<f32>();
        let db = 10.0 * (win(2_400 + 48_000) / win(2_400 + 4_800)).log10();
        let expected = -60.0 * (48_000.0 - 4_800.0) / (1.5 * 48_000.0);
        assert!((db - expected).abs() < 8.0, "decay {db} dB vs expected {expected}");
    }

    #[test]
    fn output_is_wide_and_predelay_holds_the_onset_back() {
        let mut rev = FdnReverb::new(48_000.0);
        rev.set_predelay_ms(30.0);
        for _ in 0..4_000 {
            rev.process(0.0, 0.0);
        }
        let mut first = None;
        let (mut el, mut er, mut cross) = (0.0f32, 0.0f32, 0.0f32);
        for n in 0..24_000 {
            let (l, r) = rev.process(if n == 0 { 1.0 } else { 0.0 }, if n == 0 { 1.0 } else { 0.0 });
            if first.is_none() && (l.abs() > 1e-4 || r.abs() > 1e-4) {
                first = Some(n);
            }
            el += l * l;
            er += r * r;
            cross += l * r;
        }
        assert!(first.unwrap() >= 1_300, "onset at {first:?}");
        let corr = cross / (el * er).sqrt();
        assert!(corr.abs() < 0.35, "decorrelated channels: {corr}");
    }
}
