use std::f32::consts::{FRAC_1_SQRT_2, FRAC_PI_2, PI};

const KNOB_STEPS: f32 = 255.0;
const DEAD_ZONE: (usize, usize) = (124, 131);
const STEP_SECS: f32 = 0.0006667;
const FADE_SECS: f32 = 0.000_667;
const STAGE_MAX_SECS: f32 = 0.1;
const COMB_MAX_SECS: f32 = 1.0;
const STAGE_FORWARD: f32 = 1.0;
const STAGE_BACKWARD: f32 = -0.8;
const COMB_MS: [f32; 2] = [197.7917, 314.7917];
const LEFT_STAGE_MS: [f32; 4] = [27.7324, 42.5397, 73.6054, 38.0726];
const RIGHT_STAGE_MS: [f32; 4] = [24.9167, 38.2292, 66.1146, 34.1875];
const WET_GAIN: f32 = 2.0;
const FB_FLOOR: [f32; 2] = [0.255_049_82, 0.113_656_36];
const FB_SPAN_LEFT: [f32; 2] = [0.663_101_1, 0.759_264];
const FB_SPAN_RIGHT: [f32; 2] = [0.678_916_6, 0.783_317_27];
const LEFT_BAND: (f32, f32, f32) = (1_000.0, 200.0, 0.5);
const RIGHT_BAND: (f32, f32, f32) = (7_000.0, 3_000.0, 0.7);
const COMB_LPF_LEFT: [(f32, f32); 3] = [(44_100.0, 19_000.0), (48_000.0, 19_000.0), (96_000.0, 15_000.0)];
const COMB_LPF_RIGHT: [(f32, f32); 3] = [(44_100.0, 15_000.0), (48_000.0, 18_000.0), (96_000.0, 23_000.0)];

fn interpolate(points: &[(f32, f32); 3], x: f32) -> f32 {
    if x <= points[0].0 {
        return points[0].1;
    }
    for w in points.windows(2) {
        if x <= w[1].0 {
            return w[0].1 + (w[1].1 - w[0].1) * (x - w[0].0) / (w[1].0 - w[0].0);
        }
    }
    points[2].1
}

/// All tabulated Space parameters for one knob index.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpaceParams {
    pub input_gain: f32,
    pub mono_gain: f32,
    pub input_lpf_hz: f32,
    pub input_hpf_hz: f32,
    pub stage_ms: [f32; 4],
    pub feedback: [f32; 2],
    pub comb_lpf_hz: [f32; 2],
    pub right_half: bool,
}

impl SpaceParams {
    pub fn at(idx: usize, sample_rate: f32) -> Self {
        let idx = idx.min(255);
        let right_half = idx >= DEAD_ZONE.0;
        let i = idx as f32;
        let input_gain = if idx < DEAD_ZONE.0 {
            ((DEAD_ZONE.0 - idx) as f32 / 31.0).min(1.0)
        } else if idx <= DEAD_ZONE.1 {
            0.0
        } else {
            ((idx - DEAD_ZONE.1) as f32 / 31.0).min(1.0)
        };
        let (lpf, hpf, mono) = if right_half { RIGHT_BAND } else { LEFT_BAND };
        let feedback = [0, 1].map(|c| {
            if idx <= DEAD_ZONE.1 && idx >= DEAD_ZONE.0 {
                FB_FLOOR[c]
            } else if idx < DEAD_ZONE.0 {
                FB_FLOOR[c] + FB_SPAN_LEFT[c] * (1.0 - (i / 124.0).powi(2))
            } else {
                FB_FLOOR[c] + FB_SPAN_RIGHT[c] * (1.0 - ((255.0 - i) / 124.0).powi(2))
            }
        });
        let (left_lpf, right_lpf) = (interpolate(&COMB_LPF_LEFT, sample_rate), interpolate(&COMB_LPF_RIGHT, sample_rate));
        Self {
            input_gain,
            mono_gain: mono,
            input_lpf_hz: lpf,
            input_hpf_hz: hpf,
            stage_ms: if right_half { RIGHT_STAGE_MS } else { LEFT_STAGE_MS },
            feedback,
            comb_lpf_hz: if right_half { [right_lpf; 2] } else { [left_lpf; 2] },
            right_half,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct Biquad {
    b: [f32; 3],
    a: [f32; 2],
    x: [f32; 2],
    y: [f32; 2],
}

impl Biquad {
    fn butterworth(&mut self, hz: f32, sample_rate: f32, high: bool) {
        let t = (PI * hz.clamp(5.0, 0.5 * sample_rate) / sample_rate).tan();
        let q = FRAC_1_SQRT_2;
        let n = 1.0 + t / q + t * t;
        let b0 = if high { 1.0 / n } else { t * t / n };
        self.b = if high { [b0, -2.0 * b0, b0] } else { [b0, 2.0 * b0, b0] };
        self.a = [(2.0 * t * t - 2.0) / n, (1.0 - t / q + t * t) / n];
    }

    #[inline]
    fn process(&mut self, x: f32) -> f32 {
        let y = self.b[0] * x + self.b[1] * self.x[0] + self.b[2] * self.x[1] - self.a[0] * self.y[0] - self.a[1] * self.y[1];
        self.x = [x, self.x[0]];
        self.y = [super::util::flush(y), self.y[0]];
        y
    }

    fn reset(&mut self) {
        self.x = [0.0; 2];
        self.y = [0.0; 2];
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct OnePoleLp {
    b: f32,
    a1: f32,
    x1: f32,
    y1: f32,
}

impl OnePoleLp {
    fn set(&mut self, hz: f32, sample_rate: f32) {
        let t = (PI * hz.clamp(5.0, 0.5 * sample_rate) / sample_rate).tan();
        self.b = t / (1.0 + t);
        self.a1 = (t - 1.0) / (1.0 + t);
    }

    fn clear(&mut self) {
        self.x1 = 0.0;
        self.y1 = 0.0;
    }

    #[inline]
    fn process(&mut self, x: f32) -> f32 {
        let y = super::util::flush(self.b * (x + self.x1) - self.a1 * self.y1);
        self.x1 = x;
        self.y1 = y;
        y
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fade {
    Steady,
    Out,
    In,
}

/// Delay line whose length changes by fading the read out, jumping, and fading back in.
#[derive(Debug, Clone)]
struct DipDelay {
    buf: Vec<f32>,
    mask: usize,
    pos: usize,
    delay: usize,
    pending: usize,
    fade: Fade,
    progress: usize,
    fade_len: usize,
}

impl DipDelay {
    fn new(max_samples: usize, delay: usize, fade_len: usize) -> Self {
        let cap = (max_samples + 8).next_power_of_two();
        Self { buf: vec![0.0; cap], mask: cap - 1, pos: 0, delay, pending: delay, fade: Fade::Steady, progress: 0, fade_len: fade_len.max(1) }
    }

    fn set_delay(&mut self, delay: usize) {
        let delay = delay.clamp(1, self.mask - 1);
        if delay == self.pending {
            return;
        }
        self.pending = delay;
        if self.fade == Fade::Steady && delay != self.delay {
            self.fade = Fade::Out;
            self.progress = 0;
        }
    }

    fn snap(&mut self, delay: usize) {
        self.delay = delay.clamp(1, self.mask - 1);
        self.pending = self.delay;
        self.fade = Fade::Steady;
    }

    fn reset(&mut self) {
        self.buf.fill(0.0);
        self.pos = 0;
        self.fade = Fade::Steady;
        self.delay = self.pending;
    }

    /// Reads the sample written `delay` frames ago, through the fade gain.
    #[inline]
    fn read(&mut self) -> f32 {
        let raw = self.buf[self.pos.wrapping_sub(self.delay) & self.mask];
        let gain = match self.fade {
            Fade::Steady => 1.0,
            Fade::Out => (FRAC_PI_2 * self.progress as f32 / self.fade_len as f32).cos().powi(2),
            Fade::In => (FRAC_PI_2 * self.progress as f32 / self.fade_len as f32).sin().powi(2),
        };
        if self.fade != Fade::Steady {
            self.progress += 1;
            if self.progress >= self.fade_len {
                self.progress = 0;
                if self.fade == Fade::Out {
                    self.delay = self.pending;
                    self.fade = Fade::In;
                } else {
                    self.fade = Fade::Steady;
                }
            }
        }
        raw * gain
    }

    #[inline]
    fn write(&mut self, v: f32) {
        self.buf[self.pos] = v;
        self.pos = (self.pos + 1) & self.mask;
    }
}

#[derive(Debug, Clone)]
struct Stage(DipDelay);

impl Stage {
    #[inline]
    fn process(&mut self, x: f32) -> f32 {
        let d = self.0.read();
        let w = super::util::flush(x + STAGE_BACKWARD * d);
        self.0.write(w);
        STAGE_FORWARD * w + d
    }
}

#[derive(Debug, Clone)]
struct Comb {
    delay: DipDelay,
    lpf: OnePoleLp,
    feedback: f32,
}

impl Comb {
    #[inline]
    fn process(&mut self, x: f32) -> f32 {
        let v = x + self.feedback * self.delay.read();
        let y = self.lpf.process(v);
        self.delay.write(y);
        y
    }
}

/// Stereo notch-comb reverb driven by a single 0..255 knob index that glides one step at a time.
#[derive(Debug, Clone)]
pub struct SpaceCore {
    sample_rate: f32,
    step_samples: usize,
    step_left: usize,
    applied: usize,
    target: usize,
    applied_right: bool,
    gain: f32,
    gain_target: f32,
    gain_inc: f32,
    lpf: Biquad,
    hpf: Biquad,
    stages: [Stage; 4],
    combs: [Comb; 2],
}

impl SpaceCore {
    pub fn new(sample_rate: f32) -> Self {
        let samples = |ms: f32| (ms * 0.001 * sample_rate) as usize;
        let fade = (FADE_SECS * sample_rate) as usize;
        let stage_max = (STAGE_MAX_SECS * sample_rate) as usize;
        let comb_max = (COMB_MAX_SECS * sample_rate) as usize;
        let mut core = Self {
            sample_rate,
            step_samples: ((STEP_SECS * sample_rate) as usize).max(1),
            step_left: 0,
            applied: 127,
            target: 127,
            applied_right: false,
            gain: 0.0,
            gain_target: 0.0,
            gain_inc: 0.0,
            lpf: Biquad::default(),
            hpf: Biquad::default(),
            stages: LEFT_STAGE_MS.map(|ms| Stage(DipDelay::new(stage_max, samples(ms), fade))),
            combs: COMB_MS.map(|ms| Comb { delay: DipDelay::new(comb_max, samples(ms), fade), lpf: OnePoleLp::default(), feedback: 0.0 }),
        };
        core.apply_all(127, true);
        core
    }

    pub fn reset(&mut self) {
        self.lpf.reset();
        self.hpf.reset();
        for s in &mut self.stages {
            s.0.reset();
        }
        for c in &mut self.combs {
            c.delay.reset();
            c.lpf.clear();
        }
        self.step_left = 0;
        self.gain = 0.0;
        self.apply_all(self.applied, true);
    }

    /// Knob position in -1..1; the applied index follows it one step per step period.
    pub fn set_knob(&mut self, value: f32) {
        let knob = 0.5 + 0.5 * value.clamp(-1.0, 1.0);
        self.target = ((KNOB_STEPS * knob) as usize).min(255);
    }

    pub fn applied_index(&self) -> usize {
        self.applied
    }

    pub fn params(&self) -> SpaceParams {
        SpaceParams::at(self.applied, self.sample_rate)
    }

    fn apply_all(&mut self, idx: usize, force: bool) {
        let p = SpaceParams::at(idx, self.sample_rate);
        self.gain_target = p.input_gain * p.mono_gain;
        self.gain_inc = (self.gain_target - self.gain) / self.step_samples as f32;
        for (c, fb) in self.combs.iter_mut().zip(p.feedback) {
            c.feedback = fb;
        }
        if force || p.right_half != self.applied_right {
            self.applied_right = p.right_half;
            self.lpf.butterworth(p.input_lpf_hz, self.sample_rate, false);
            self.hpf.butterworth(p.input_hpf_hz, self.sample_rate, true);
            for (stage, ms) in self.stages.iter_mut().zip(p.stage_ms) {
                let samples = (ms * 0.001 * self.sample_rate) as usize;
                if force { stage.0.snap(samples) } else { stage.0.set_delay(samples) }
            }
            for (comb, hz) in self.combs.iter_mut().zip(p.comb_lpf_hz) {
                comb.lpf.set(hz, self.sample_rate);
            }
        }
    }

    /// Processes one frame and returns the wet signal, already scaled; add it to the untouched dry.
    #[inline]
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        if self.step_left == 0 {
            if self.applied != self.target {
                let next = if self.target > self.applied { self.applied + 1 } else { self.applied - 1 };
                self.apply_all(next, false);
                self.applied = next;
            }
            self.step_left = self.step_samples;
        }
        self.step_left -= 1;
        self.gain += self.gain_inc;
        if self.step_left == 0 {
            self.gain = self.gain_target;
            self.gain_inc = 0.0;
        }
        let mono = self.gain * l + self.gain * r;
        let m0 = self.hpf.process(self.lpf.process(mono));
        let a1 = self.stages[0].process(m0);
        let a2 = self.stages[1].process(a1);
        let m3 = self.stages[2].process(a2);
        let m4 = self.stages[3].process(m3);
        let wet_l = self.combs[0].process(m4 - m0);
        let wet_r = self.combs[1].process(m3 - m0);
        (WET_GAIN * wet_l, WET_GAIN * wet_r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    struct Vector {
        idx: usize,
        in_g: f32,
        mono: f32,
        lpf: f32,
        hpf: f32,
        ap1: f32,
        fb_l: f32,
        fb_r: f32,
        comb: f32,
    }

    fn close(a: f32, b: f32, tol: f32) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn tables_match_the_specification_at_the_test_vector_indices() {
        let vectors = [
            Vector { idx: 0, in_g: 1.0, mono: 0.5, lpf: 1000.0, hpf: 200.0, ap1: 27.7324, fb_l: 0.9182, fb_r: 0.8729, comb: 19000.0 },
            Vector { idx: 63, in_g: 1.0, mono: 0.5, lpf: 1000.0, hpf: 200.0, ap1: 27.7324, fb_l: 0.747, fb_r: 0.6769, comb: 19000.0 },
            Vector { idx: 95, in_g: 0.9355, mono: 0.5, lpf: 1000.0, hpf: 200.0, ap1: 27.7324, fb_l: 0.5289, fb_r: 0.4273, comb: 19000.0 },
            Vector { idx: 127, in_g: 0.0, mono: 0.7, lpf: 7000.0, hpf: 3000.0, ap1: 24.9167, fb_l: 0.255, fb_r: 0.1137, comb: 18000.0 },
            Vector { idx: 159, in_g: 0.9032, mono: 0.7, lpf: 7000.0, hpf: 3000.0, ap1: 24.9167, fb_l: 0.527, fb_r: 0.4275, comb: 18000.0 },
            Vector { idx: 191, in_g: 1.0, mono: 0.7, lpf: 7000.0, hpf: 3000.0, ap1: 24.9167, fb_l: 0.7531, fb_r: 0.6883, comb: 18000.0 },
            Vector { idx: 255, in_g: 1.0, mono: 0.7, lpf: 7000.0, hpf: 3000.0, ap1: 24.9167, fb_l: 0.934, fb_r: 0.897, comb: 18000.0 },
        ];
        for v in vectors {
            let p = SpaceParams::at(v.idx, SR);
            let idx = v.idx;
            assert!(close(p.input_gain, v.in_g, 5e-4), "idx {idx} inG {}", p.input_gain);
            assert_eq!((p.mono_gain, p.input_lpf_hz, p.input_hpf_hz), (v.mono, v.lpf, v.hpf), "idx {idx}");
            assert!(close(p.stage_ms[0], v.ap1, 1e-3), "idx {idx} AP1 {}", p.stage_ms[0]);
            assert!(close(p.feedback[0], v.fb_l, 5e-4) && close(p.feedback[1], v.fb_r, 5e-4), "idx {idx} fb {:?}", p.feedback);
            assert_eq!(p.comb_lpf_hz, [v.comb; 2], "idx {idx}");
        }
        let right = SpaceParams::at(200, SR).stage_ms;
        for (r, l) in right.iter().zip(SpaceParams::at(10, SR).stage_ms) {
            assert!(close(*r / l, 0.8985, 3e-3), "{r} vs {l}");
        }
        assert!(close(SpaceParams::at(255, SR).stage_ms[2], 66.1146, 2e-3) && close(SpaceParams::at(0, SR).stage_ms[3], 38.0726, 1e-3));
    }

    #[test]
    fn comb_cutoffs_follow_the_sample_rate_and_interpolate_between_the_verified_rates() {
        assert_eq!(SpaceParams::at(0, 44_100.0).comb_lpf_hz[0], 19_000.0);
        assert_eq!(SpaceParams::at(0, 96_000.0).comb_lpf_hz[0], 15_000.0);
        assert_eq!(SpaceParams::at(255, 44_100.0).comb_lpf_hz[0], 15_000.0);
        assert_eq!(SpaceParams::at(255, 96_000.0).comb_lpf_hz[0], 23_000.0);
        let mid = SpaceParams::at(255, 72_000.0).comb_lpf_hz[0];
        assert!(close(mid, 20_500.0, 1.0), "{mid}");
        let odd = SpaceParams::at(0, 88_200.0).comb_lpf_hz[0];
        assert!(odd < 19_000.0 && odd > 15_000.0, "{odd}");
    }

    #[test]
    fn the_dead_zone_mutes_the_input_and_holds_the_minimum_feedback() {
        for idx in 124..=131 {
            let p = SpaceParams::at(idx, SR);
            assert_eq!(p.input_gain, 0.0, "idx {idx}");
            assert!(close(p.feedback[0], 0.255_05, 1e-4) && close(p.feedback[1], 0.113_66, 1e-4));
        }
        assert!(SpaceParams::at(123, SR).input_gain > 0.0 && SpaceParams::at(132, SR).input_gain > 0.0);
    }

    fn render(core: &mut SpaceCore, x: &[f32]) -> Vec<f32> {
        x.iter().map(|v| core.process(*v, *v).0).collect()
    }

    #[test]
    fn the_index_glides_one_step_per_period_and_a_full_sweep_takes_about_170_ms() {
        let mut core = SpaceCore::new(SR);
        core.set_knob(1.0);
        let mut at = Vec::new();
        for n in 0..12_000 {
            core.process(0.0, 0.0);
            if n == 31 || n == 63 || n == 6_400 {
                at.push(core.applied_index());
            }
        }
        assert!(at[0] <= 129 && at[1] <= 131 && at[1] > at[0], "{at:?}");
        assert!(at[2] > 190, "{at:?}");
        assert_eq!(core.applied_index(), 255, "reached the end within 170 ms plus the start offset");
    }

    #[test]
    fn the_wet_is_silent_until_the_first_stage_echo() {
        let mut core = SpaceCore::new(SR);
        core.set_knob(0.6);
        for _ in 0..4_000 {
            core.process(0.0, 0.0);
        }
        let mut x = vec![0.0; 6_000];
        x[100] = 0.5;
        let wet = render(&mut core, &x);
        let early: f32 = wet[100..1_100].iter().map(|v| v.abs()).sum();
        let later: f32 = wet[1_200..2_600].iter().map(|v| v.abs()).sum();
        assert!(early < 1e-3, "nothing before the first echo: {early}");
        assert!(later > 0.01, "the stage echoes arrive about 25 ms later: {later}");
    }

    #[test]
    fn the_tail_rings_through_the_dead_zone_and_dies_in_about_a_second() {
        let mut core = SpaceCore::new(SR);
        core.set_knob(1.0);
        let burst: Vec<f32> = (0..9_600).map(|i| 0.5 * (std::f32::consts::TAU * 4_000.0 * i as f32 / SR).sin()).collect();
        render(&mut core, &burst);
        core.set_knob(0.0);
        let tail = render(&mut core, &vec![0.0; 48_000 * 6]);
        let level = |from: usize| (tail[from..from + 4_800].iter().map(|v| v * v).sum::<f32>() / 4_800.0).sqrt();
        eprintln!("space tail after the knob returns: {:.4} at 0.3 s, {:.5} at 2 s, {:.7} at 5 s", level(14_400), level(96_000), level(240_000));
        assert!(level(14_400) > 1e-3, "still ringing");
        assert!(level(96_000) < 0.1 * level(14_400) && level(240_000) < 1e-4 * level(14_400).max(1.0), "and it dies away");
    }

    #[test]
    fn the_core_stays_finite_and_bounded_for_a_minute_of_full_scale_noise() {
        let mut core = SpaceCore::new(SR);
        let mut n = super::super::util::Noise::new(3);
        let mut worst = 0.0f32;
        for i in 0..48_000 * 60 {
            if i % 24_000 == 0 {
                core.set_knob([1.0, -1.0, 0.0, 0.7, -0.3][(i / 24_000) % 5]);
            }
            let x = n.next();
            let (l, r) = core.process(x, x);
            assert!(l.is_finite() && r.is_finite());
            worst = worst.max(l.abs()).max(r.abs());
        }
        assert!(worst < 30.0, "peak {worst}");
    }
}
