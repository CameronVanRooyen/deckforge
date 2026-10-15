use super::*;
use crate::svf::Svf;

const MIN_DELAY: f32 = 44.1;
const MIN_TIME_SECS: f32 = 0.005;
const MAX_TIME_SECS: f32 = 3.99;
const CHUNK: usize = 30;
const GROW_SPAN_SECS: f32 = 4.0;
const GROW_TOP_SPEED: f32 = 0.21;
const GROW_SPEED_RANGE: f32 = 0.20;
const GROW_LAG_RATE: f32 = 0.5;
const SHRINK_SPAN_SECS: f32 = 2.0;
const SHRINK_TOP_SPEED: f32 = 0.41157;
const SHRINK_SPEED_RANGE: f32 = 0.40157;
const SHRINK_LAG_RATE: f32 = 3.3333;
const LAND_SPEED: f32 = 0.01;
const LAND_FRACTION: f32 = 0.001;
const MAX_SPEED_UP: f32 = 0.8;
const MAX_SPEED_DOWN: f32 = -2.0;
const PRE_LP_HZ: f32 = 12_000.0;
const POST_HP_HZ: f32 = 150.0;
const POST_HP_Q: f32 = 0.3;
const POST_LP_HZ: f32 = 12_100.0;
const POST_LP_Q: f32 = 0.7;
const OUT_LP_HZ: f32 = 10_000.0;
const OUT_LP_Q: f32 = 0.3;
const FILTER_Q: f32 = std::f32::consts::FRAC_1_SQRT_2;
const CLIP_FRACTION: f32 = 0.6;
const PEAK_RELEASE_SECS: f32 = 3.0;
const OUTPUT_DEPTH_TRIM: f32 = 0.501;
const PITCH_SMOOTH_MS: f32 = 40.0;
const PITCH_BYPASS: f32 = 0.5;
const FEEDBACK_SMOOTH_MS: f32 = 10.0;

/// Pitch ratio for a PITCH setting in percent: minus 100 is an octave down, plus 100 an octave up.
pub(super) fn spiral_pitch_ratio(percent: f32) -> f32 {
    let p = percent.clamp(-100.0, 100.0);
    1.0 + if p >= 0.0 { 0.01 * p } else { 0.005 * p }
}

/// Delay position that moves with a velocity instead of jumping, so a time change bends the echo's pitch like a tape machine.
#[derive(Debug, Clone)]
pub(super) struct TapeGlide {
    sample_rate: f32,
    delay: f32,
    target: f32,
    velocity: f32,
    accel: f32,
    chunk_left: usize,
}

impl TapeGlide {
    pub(super) fn new(sample_rate: f32) -> Self {
        Self { sample_rate, delay: MIN_DELAY, target: MIN_DELAY, velocity: 0.0, accel: 0.0, chunk_left: 0 }
    }

    pub(super) fn snap(&mut self, delay: f32) {
        self.delay = delay.max(MIN_DELAY);
        self.target = self.delay;
        self.velocity = 0.0;
        self.accel = 0.0;
        self.chunk_left = 0;
    }

    pub(super) fn set_target(&mut self, delay: f32) {
        self.target = delay.max(MIN_DELAY);
    }

    pub(super) fn delay(&self) -> f32 {
        self.delay
    }

    pub(super) fn target(&self) -> f32 {
        self.target
    }

    #[cfg(test)]
    pub(super) fn velocity(&self) -> f32 {
        self.velocity
    }

    #[cfg(test)]
    pub(super) fn is_moving(&self) -> bool {
        self.velocity != 0.0 || self.target != self.delay
    }

    fn plan(&mut self) {
        let sr = self.sample_rate;
        let gap = self.target - self.delay;
        if gap.abs() <= 1e-7 {
            self.velocity = 0.0;
            self.accel = 0.0;
            self.delay = self.target;
            return;
        }
        if gap > 0.0 {
            let r = gap.min(GROW_SPAN_SECS * sr) / (GROW_SPAN_SECS * sr);
            let wanted = GROW_TOP_SPEED - GROW_SPEED_RANGE * (1.0 - r).powi(3);
            self.accel = (wanted - self.velocity) * GROW_LAG_RATE / sr;
            if self.velocity > LAND_SPEED && gap < LAND_FRACTION * sr {
                self.accel = (LAND_SPEED - self.velocity) / gap;
            }
        } else {
            let r = (-gap).min(SHRINK_SPAN_SECS * sr) / (SHRINK_SPAN_SECS * sr);
            let wanted = -(SHRINK_TOP_SPEED - SHRINK_SPEED_RANGE * (1.0 - r).powi(3));
            self.accel = (wanted - self.velocity) * SHRINK_LAG_RATE / sr;
        }
    }

    /// Advances one sample and returns the delay to read at.
    #[inline]
    pub(super) fn next(&mut self) -> f32 {
        if self.target == self.delay && self.velocity == 0.0 {
            return self.delay;
        }
        if self.chunk_left == 0 {
            self.plan();
            self.chunk_left = CHUNK;
        }
        self.chunk_left -= 1;
        self.delay = (self.delay + self.velocity).max(MIN_DELAY);
        self.velocity = (self.velocity + self.accel).clamp(MAX_SPEED_DOWN, MAX_SPEED_UP);
        if (self.target - self.delay).abs() <= 1.0 && (self.velocity < 0.0 || self.velocity < LAND_SPEED + 0.001) {
            self.delay = self.target;
            self.velocity = 0.0;
            self.accel = 0.0;
            self.chunk_left = 0;
        }
        self.delay
    }
}

#[derive(Debug, Clone)]
pub(super) struct SpiralState {
    glide: TapeGlide,
    pre_lp: [Svf; 2],
    post_hp: [Svf; 2],
    post_lp: [Svf; 2],
    out_lp: [Svf; 2],
    peak: f32,
    peak_release: f32,
    pitch: Smoothed,
    feedback: Smoothed,
}

impl SpiralState {
    pub(super) fn new(sample_rate: f32) -> Self {
        let filter = |hz: f32, q: f32| [Svf::new(hz, q, sample_rate); 2];
        Self {
            glide: TapeGlide::new(sample_rate),
            pre_lp: filter(PRE_LP_HZ, FILTER_Q),
            post_hp: filter(POST_HP_HZ, POST_HP_Q),
            post_lp: filter(POST_LP_HZ, POST_LP_Q),
            out_lp: filter(OUT_LP_HZ, OUT_LP_Q),
            peak: 0.0,
            peak_release: (-1.0 / (PEAK_RELEASE_SECS * sample_rate)).exp(),
            pitch: Smoothed::new(1.0, sample_rate, PITCH_SMOOTH_MS),
            feedback: Smoothed::new(0.8, sample_rate, FEEDBACK_SMOOTH_MS),
        }
    }

    pub(super) fn reset(&mut self) {
        for f in self.pre_lp.iter_mut().chain(&mut self.post_hp).chain(&mut self.post_lp).chain(&mut self.out_lp) {
            f.reset();
        }
        self.peak = 0.0;
        self.pitch.reset(self.pitch.target());
        self.feedback.reset(self.feedback.target());
    }

    pub(super) fn delay(&self) -> f32 {
        self.glide.delay().max(self.glide.target())
    }
}

impl BeatFx {
    fn spiral_target(&self) -> f32 {
        let sr = self.sample_rate;
        let mut d = self.beats * self.beat_secs() * sr;
        while d > MAX_TIME_SECS * sr {
            d *= 0.5;
        }
        d.max(MIN_TIME_SECS * sr)
    }

    pub(super) fn run_spiral(&mut self, l: &mut [f32], r: &mut [f32]) {
        let target = self.spiral_target();
        if self.delay_snap {
            self.spiral.glide.snap(target);
            self.delay_snap = false;
            self.spiral.feedback.reset(self.param(0) * 0.01);
        }
        self.spiral.glide.set_target(target);
        self.spiral.feedback.set(self.param(0) * 0.01);
        let percent = self.param(1);
        let ratio = spiral_pitch_ratio(percent);
        self.spiral.pitch.set(ratio);
        let shifting = percent.abs() > PITCH_BYPASS || (self.spiral.pitch.value() - 1.0).abs() > 0.005;
        for i in 0..l.len() {
            let lvl = self.level.next();
            let send = self.send.next();
            let gate = self.swap.next();
            let fb = self.spiral.feedback.next();
            let pitch = self.spiral.pitch.next();
            let delay = self.spiral.glide.next();
            let (xl, xr) = (l[i], r[i]);
            let mono = (0.5 * (xl + xr)).abs();
            self.spiral.peak = mono.max(self.spiral.peak * self.spiral.peak_release);
            let clip = CLIP_FRACTION * self.spiral.peak;
            let inputs = [xl * send, xr * send];
            let echoes = self.ring.read(delay);
            let echoes = [echoes.0, echoes.1];
            let mut stored = [0.0f32; 2];
            for ch in 0..2 {
                let x = self.spiral.pre_lp[ch].process(inputs[ch]).low;
                let summed = (x + echoes[ch]).clamp(-clip, clip);
                let high = self.spiral.post_hp[ch].process(summed).high;
                stored[ch] = fb * self.spiral.post_lp[ch].process(high).low;
            }
            self.ring.write(soft_limit(stored[0]), soft_limit(stored[1]));
            let mut out = echoes;
            if shifting {
                out = [self.shifters[0].process(out[0], pitch), self.shifters[1].process(out[1], pitch)];
            }
            let out = [self.spiral.out_lp[0].process(out[0]).low, self.spiral.out_lp[1].process(out[1]).low];
            let depth_gain = 1.0 - OUTPUT_DEPTH_TRIM * lvl;
            let wet = (2.0 * lvl).min(1.0) * depth_gain * gate;
            l[i] = xl + soft_limit(out[0] * wet);
            r[i] = xr + soft_limit(out[1] * wet);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::*;

    const SPEC_SR: f32 = 48_000.0;
    const BEAT: f32 = 22_500.0;

    struct Run {
        settle_secs: f32,
        peak_v: f32,
        peak_secs: f32,
        v_at: Vec<f32>,
    }

    fn glide(from: f32, to: f32) -> Run {
        let mut g = TapeGlide::new(SPEC_SR);
        g.snap(from);
        g.set_target(to);
        let marks = [0.25f32, 0.5, 1.0, 2.0, 4.0, 8.0].map(|s| (s * SPEC_SR) as usize);
        let (mut peak_v, mut peak_n, mut v_at, mut settled) = (0.0f32, 0usize, Vec::new(), None);
        for n in 1..(30.0 * SPEC_SR) as usize {
            g.next();
            if g.velocity().abs() > peak_v.abs() {
                peak_v = g.velocity();
                peak_n = n;
            }
            if marks.contains(&n) {
                v_at.push(g.velocity());
            }
            if !g.is_moving() {
                settled = Some(n);
                break;
            }
        }
        Run { settle_secs: settled.unwrap_or(0) as f32 / SPEC_SR, peak_v, peak_secs: peak_n as f32 / SPEC_SR, v_at }
    }

    #[test]
    fn half_beat_to_one_beat_grows_the_delay_slowly_and_settles_in_about_ten_seconds() {
        let run = glide(BEAT / 2.0, BEAT);
        eprintln!("1/2 -> 1 beat: settles {:.2} s, peak v {:.4} at {:.2} s, v at marks {:.4?}", run.settle_secs, run.peak_v, run.peak_secs, run.v_at);
        assert!((run.settle_secs - 9.87).abs() < 1.0, "settles after {} s", run.settle_secs);
        assert!((1.0 - run.peak_v - 0.9687).abs() < 0.006, "peak ratio {}", 1.0 - run.peak_v);
        let semitones = 12.0 * (1.0 - run.peak_v).log2();
        assert!((semitones + 0.55).abs() < 0.12, "peak {semitones} st");
        assert!((run.peak_secs - 3.9).abs() < 0.8, "at {} s", run.peak_secs);
        let want = [0.0051, 0.0095, 0.0168, 0.0261, 0.0313, 0.0216];
        for (got, want) in run.v_at.iter().zip(want) {
            assert!((got - want).abs() < 0.12 * want + 0.0006, "{got} vs {want}");
        }
    }

    #[test]
    fn one_beat_to_half_beat_shrinks_fast_and_settles_in_about_four_seconds() {
        let run = glide(BEAT, BEAT / 2.0);
        eprintln!("1 -> 1/2 beat: settles {:.2} s, peak v {:.4} at {:.2} s, v at marks {:.4?}", run.settle_secs, run.peak_v, run.peak_secs, run.v_at);
        assert!((run.settle_secs - 4.12).abs() < 0.6, "settles after {} s", run.settle_secs);
        let semitones = 12.0 * (1.0 - run.peak_v).log2();
        assert!((semitones - 1.78).abs() < 0.25, "peak {semitones} st");
        assert!((run.peak_secs - 0.71).abs() < 0.3, "at {} s", run.peak_secs);
        let want = [-0.0754, -0.1030, -0.1024, -0.0602, -0.0140, 0.0];
        for (got, want) in run.v_at.iter().zip(want) {
            assert!((got - want).abs() < 0.15 * want.abs() + 0.004, "{got} vs {want}");
        }
    }

    #[test]
    fn longer_changes_bend_further_and_the_minimum_delay_is_respected() {
        let four_to_one = glide(4.0 * BEAT, BEAT);
        assert!((12.0 * (1.0 - four_to_one.peak_v).log2() - 5.5).abs() < 0.6, "4 -> 1 beat peaks at {} st", 12.0 * (1.0 - four_to_one.peak_v).log2());
        let one_to_four = glide(BEAT, 4.0 * BEAT);
        assert!((12.0 * (1.0 - one_to_four.peak_v).log2() + 2.3).abs() < 0.5);
        let mut g = TapeGlide::new(SPEC_SR);
        g.snap(100.0);
        g.set_target(1.0);
        for _ in 0..4_000 {
            assert!(g.next() >= MIN_DELAY);
        }
    }

    #[test]
    fn pitch_percent_follows_the_octave_law() {
        assert_eq!(spiral_pitch_ratio(0.0), 1.0);
        assert_eq!(spiral_pitch_ratio(100.0), 2.0);
        assert_eq!(spiral_pitch_ratio(-100.0), 0.5);
        assert!((spiral_pitch_ratio(50.0) - 1.5).abs() < 1e-6 && (spiral_pitch_ratio(-50.0) - 0.75).abs() < 1e-6);
    }

    fn spiral(beats: f32, level: f32) -> BeatFx {
        let mut fx = unit(BeatFxKind::Spiral, beats, level);
        fx.set_enabled(true);
        fx
    }

    #[test]
    fn wet_rises_twice_as_fast_as_depth_while_the_dry_is_never_reduced() {
        let level_of = |level: f32| {
            let mut fx = spiral(0.25, level);
            let x = noise(48_000 * 3, 5, 0.3);
            let (out, _) = run(&mut fx, &x, &x);
            let wet: Vec<f32> = out.iter().zip(&x).map(|(o, d)| o - d).collect();
            rms(&wet[48_000..])
        };
        let (quarter, half, full) = (level_of(0.25), level_of(0.5), level_of(1.0));
        eprintln!("spiral wet at depth 25 / 50 / 100 %: {quarter:.4} / {half:.4} / {full:.4}");
        assert!(half > 0.9 * quarter * 1.4, "wet already tops out at half depth");
        assert!(full < half, "the output gain falls by about half toward full depth");
        let mut fx = spiral(0.25, 1.0);
        let mut x = vec![0.0; 2_000];
        x[1_000] = 0.5;
        let (out, _) = run(&mut fx, &x, &x);
        assert!((out[1_000] - 0.5).abs() < 1e-3, "the dry is untouched at full depth: {}", out[1_000]);
    }

    #[test]
    fn division_change_bends_the_echo_pitch_and_a_pitch_setting_shifts_the_output_only() {
        let mut fx = spiral(1.0, 1.0);
        fx.set_param(1, 100.0);
        let x = sine(48_000 * 6, 440.0, 0.3);
        let (out, _) = run(&mut fx, &x, &x);
        let wet: Vec<f32> = out.iter().zip(&x).map(|(o, d)| o - d).collect();
        let hz = crate::fx::testkit::crossing_hz(&wet[48_000 * 3..48_000 * 5]);
        assert!((hz / 880.0 - 1.0).abs() < 0.05, "an octave up on the output: {hz} Hz");
        let mut plain = spiral(1.0, 1.0);
        let (out, _) = run(&mut plain, &x, &x);
        let wet: Vec<f32> = out.iter().zip(&x).map(|(o, d)| o - d).collect();
        let hz = crate::fx::testkit::crossing_hz(&wet[48_000 * 3..48_000 * 5]);
        assert!((hz / 440.0 - 1.0).abs() < 0.03, "no shift at 0 %: {hz} Hz");
    }

    #[test]
    fn the_loop_stays_bounded_at_maximum_feedback_through_division_changes() {
        for (name, x) in [
            ("noise", noise(48_000 * 30, 99, 1.0)),
            ("square", (0..48_000 * 30).map(|i| if (i / 55) % 2 == 0 { 1.0 } else { -1.0 }).collect()),
            ("sine", sine(48_000 * 30, 330.0, 1.0)),
        ] {
            let mut fx = spiral(0.5, 1.0);
            fx.set_param(0, 150.0);
            let (mut l, mut r) = (x.clone(), x.clone());
            for (b, (cl, cr)) in l.chunks_mut(BLOCK).zip(r.chunks_mut(BLOCK)).enumerate() {
                if b % 400 == 0 {
                    fx.set_beats([0.5, 2.0, 0.125, 16.0, 1.0][(b / 400) % 5]);
                }
                fx.process(cl, cr);
                assert!(cl.iter().chain(cr.iter()).all(|v| v.is_finite() && v.abs() <= 2.0), "{name}");
            }
        }
    }
}
