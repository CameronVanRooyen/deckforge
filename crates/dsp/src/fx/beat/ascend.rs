use super::pulse::pulse_gain;
use super::*;
use crate::fx::noise_bed::NoiseBed;

pub(super) const TAIL_SECS: f64 = 1.8;
const LENGTH_BARS: [f64; 4] = [4.0, 8.0, 16.0, 32.0];
const HP_MIN_HZ: f32 = 20.0;
const HP_MAX_HZ: f32 = 600.0;
const NOISE_LO_HZ: f32 = 400.0;
const NOISE_HI_HZ: f32 = 7_000.0;
const NOISE_PEAK: f32 = 0.9;
const NOISE_DUCK_DB: f32 = 6.0;
const FEEDBACK_MIN: f32 = 0.40;
const FEEDBACK_MAX: f32 = 0.88;
const RELEASE_FEEDBACK: f32 = 0.5;
const ECHO_RETURN_MIN: f32 = 0.10;
const ECHO_RETURN_MAX: f32 = 0.50;
const REVERB_SEND_MIN: f32 = 0.10;
const REVERB_SEND_MAX: f32 = 0.70;
const REVERB_WET_GAIN: f32 = 1.2;
const REVERB_RT60: f32 = 2.6;
const RELEASE_RT60: f32 = 1.2;
const TREMOLO_MAX: f32 = 0.8;
const INTENSITY: f32 = 1.4;
const STAGE_THRESHOLDS: [f32; 4] = [0.0, 0.5, 0.75, 0.875];
const STAGE_BEATS: [f32; 4] = [1.0, 0.5, 0.25, 0.125];
const UPDATE: usize = 16;
const RAMP_CHASE_SECS: f32 = 0.02;
const RELEASE_CHASE_SECS: f32 = 0.006;
const SMOOTH_MS: f32 = 12.0;
const TREMOLO_ATTACK_SECS: f32 = 0.002;

/// What the engine needs to place the ramp end: how long and on which kind of boundary.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AscendRequest {
    pub length_bars: f64,
    pub land_on_phrase: bool,
}

/// Where the engine decided the ramp ends: frames from now, the track time, bars of ramp and the bar number.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AscendLanding {
    pub frames: f64,
    pub secs: f64,
    pub bars: f64,
    pub landing_bar: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Idle,
    Pending,
    Ramp,
    Hold,
    Released,
}

#[derive(Debug, Clone)]
pub(super) struct AscendState {
    phase: Phase,
    length_bars: f64,
    land_on_phrase: bool,
    requested: bool,
    total: f64,
    remaining: f64,
    bars: f64,
    landing_secs: Option<f64>,
    landing_bar: Option<u32>,
    release_requested: bool,
    release_now: bool,
    hp: [HighPass<2>; 2],
    hp_cut: LogSlew,
    noise: NoiseBed,
    noise_gain: Smoothed,
    echo_return: Smoothed,
    reverb_send: Smoothed,
    feedback: Smoothed,
    depth: Smoothed,
    gain: Smoothed,
    stage: usize,
}

impl AscendState {
    pub(super) fn new(sample_rate: f32) -> Self {
        Self {
            phase: Phase::Idle,
            length_bars: 16.0,
            land_on_phrase: true,
            requested: false,
            total: 1.0,
            remaining: 1.0,
            bars: 0.0,
            landing_secs: None,
            landing_bar: None,
            release_requested: false,
            release_now: false,
            hp: [HighPass::default(); 2],
            hp_cut: LogSlew::new(HP_MIN_HZ),
            noise: NoiseBed::new(sample_rate, NOISE_DUCK_DB, DUCK_RELEASE_MS),
            noise_gain: Smoothed::new(0.0, sample_rate, SMOOTH_MS),
            echo_return: Smoothed::new(0.0, sample_rate, SMOOTH_MS),
            reverb_send: Smoothed::new(0.0, sample_rate, SMOOTH_MS),
            feedback: Smoothed::new(FEEDBACK_MIN, sample_rate, SMOOTH_MS),
            depth: Smoothed::new(0.0, sample_rate, SMOOTH_MS),
            gain: Smoothed::new(1.0, sample_rate, 0.8),
            stage: 0,
        }
    }

    pub(super) fn reset(&mut self) {
        self.phase = Phase::Idle;
        self.requested = false;
        self.landing_secs = None;
        self.landing_bar = None;
        self.release_requested = false;
        self.release_now = false;
        self.hp.iter_mut().for_each(HighPass::reset);
        self.hp_cut.reset(HP_MIN_HZ);
        self.noise.reset();
        self.noise_gain.reset(0.0);
        self.echo_return.reset(0.0);
        self.reverb_send.reset(0.0);
        self.feedback.reset(FEEDBACK_MIN);
        self.depth.reset(0.0);
        self.gain.reset(1.0);
        self.stage = 0;
    }

    pub(super) fn settle_band_gains(&mut self, gains: [f32; 3]) {
        self.noise.settle_band_gains(gains);
    }

    pub(super) fn start(&mut self, length_choice: f32, land_on_phrase: bool) {
        self.phase = Phase::Pending;
        self.length_bars = LENGTH_BARS[length_choice.round().clamp(0.0, 3.0) as usize];
        self.land_on_phrase = land_on_phrase;
        self.requested = false;
        self.landing_secs = None;
        self.landing_bar = None;
        self.release_requested = false;
    }

    pub(super) fn is_running(&self) -> bool {
        matches!(self.phase, Phase::Pending | Phase::Ramp | Phase::Hold)
    }

    pub(super) fn holding(&self) -> bool {
        self.phase == Phase::Hold
    }

    pub(super) fn request_release(&mut self, now: bool) {
        self.release_requested = true;
        self.release_now = now;
    }

    fn progress(&self) -> Option<f32> {
        match self.phase {
            Phase::Pending => Some(0.0),
            Phase::Ramp => Some((1.0 - self.remaining / self.total.max(1.0)).clamp(0.0, 1.0) as f32),
            Phase::Hold => Some(1.0),
            Phase::Idle | Phase::Released => None,
        }
    }
}

fn stage_for(progress: f32) -> usize {
    STAGE_THRESHOLDS.iter().rposition(|t| progress >= *t).unwrap_or(0)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

impl BeatFx {
    /// Asks the engine, once per ascent, where the ramp should end.
    pub fn ascend_request(&mut self) -> Option<AscendRequest> {
        let a = &mut self.ascend;
        if a.phase == Phase::Pending && !a.requested {
            a.requested = true;
            return Some(AscendRequest { length_bars: a.length_bars, land_on_phrase: a.land_on_phrase });
        }
        None
    }

    pub fn arm_ascend(&mut self, landing: AscendLanding) {
        let a = &mut self.ascend;
        if a.phase != Phase::Pending || landing.frames < 1.0 {
            return;
        }
        a.total = landing.frames;
        a.remaining = landing.frames;
        a.bars = landing.bars;
        a.landing_secs = Some(landing.secs);
        a.landing_bar = Some(landing.landing_bar);
        a.phase = Phase::Ramp;
        self.ascend_prepare();
    }

    /// Track time the armed ramp ends at, so the engine can re-measure the remaining frames as tempo moves.
    pub fn ascend_landing_secs(&self) -> Option<f64> {
        (self.ascend.phase == Phase::Ramp).then_some(self.ascend.landing_secs).flatten()
    }

    pub fn sync_ascend_remaining(&mut self, frames: f64) {
        if self.ascend.phase == Phase::Ramp {
            self.ascend.remaining = frames.max(0.0);
        }
    }

    pub fn ascend_progress(&self) -> Option<f32> {
        self.ascend.progress()
    }

    /// Bars in the armed ramp and the bar number it lands on, when the engine placed it on the deck's bars.
    pub fn ascend_layout(&self) -> Option<(f64, u32)> {
        let a = &self.ascend;
        (a.phase != Phase::Idle).then_some(a.landing_bar.map(|b| (a.bars, b))).flatten()
    }

    fn ascend_prepare(&mut self) {
        self.reverb.set_size(1.0);
        self.reverb.set_predelay_ms(18.0);
        self.reverb.set_hf_ratio(REVERB_HF_RATIO);
        self.reverb.set_modulation_ms(reverb_modulation_ms(0.5));
        self.reverb.set_rt60(REVERB_RT60);
        self.rt60_applied = REVERB_RT60;
        self.ascend.noise.set_band_gains(self.band_gain.map(|g| g.target()));
    }

    pub(super) fn run_ascend(&mut self, l: &mut [f32], r: &mut [f32]) {
        if self.ascend.phase == Phase::Pending {
            let per_bar = self.bar_clock.per_bar;
            self.ascend.total = (self.ascend.length_bars * per_bar).max(1.0);
            self.ascend.remaining = self.ascend.total;
            self.ascend.bars = self.ascend.length_bars;
            self.ascend.phase = Phase::Ramp;
            self.ascend_prepare();
        }
        if matches!(self.ascend.phase, Phase::Idle) {
            return;
        }
        let sr = self.sample_rate;
        let per_bar = self.bar_clock.per_bar;
        let beat_frames = per_bar / 4.0;
        let beat_secs = self.beat_secs();
        let mut to_next = self.bar_clock.to_next;
        let ramp_chase = chase_coeff(sr / UPDATE as f32, RAMP_CHASE_SECS);
        let release_chase = chase_coeff(sr / UPDATE as f32, RELEASE_CHASE_SECS);
        let max_delay = MAX_DELAY_SECS * sr;
        self.ascend.noise.set_band_gains(self.band_gain.map(|g| g.target()));
        for ch in 0..2 {
            self.loop_hp[ch].set(LOOP_HP_HZ, sr);
            self.loop_lp[ch].set(ASCEND_LOOP_LP_HZ, sr);
        }
        let right_offset = RIGHT_OFFSET_MS * sr / 1000.0;
        let grid_inc = 4.0 / (beat_secs * sr);
        if self.delay_snap {
            let d = (STAGE_BEATS[0] as f64 * beat_frames).min(max_delay as f64) as f32;
            self.taps.snap(d.max(2.0));
            self.delay_snap = false;
        }
        let mut released = self.ascend.phase == Phase::Released;
        for i in 0..l.len() {
            to_next -= 1.0;
            let boundary_bar = to_next <= BOUNDARY_SLACK;
            if boundary_bar {
                to_next += per_bar;
            }
            if self.ascend.phase == Phase::Ramp {
                self.ascend.remaining -= 1.0;
                if self.ascend.remaining <= 0.0 {
                    self.ascend.remaining = 0.0;
                    self.ascend.phase = Phase::Hold;
                }
            }
            if !released && self.ascend.release_requested && (self.ascend.release_now || boundary_bar) {
                self.ascend.phase = Phase::Released;
                released = true;
                self.send.set(0.0);
                self.reverb.set_rt60(RELEASE_RT60);
                self.rt60_applied = RELEASE_RT60;
                self.tail_left = TAIL_SECS * sr as f64;
            }
            let progress = match self.ascend.phase {
                Phase::Ramp => (1.0 - self.ascend.remaining / self.ascend.total.max(1.0)).clamp(0.0, 1.0) as f32,
                Phase::Hold => 1.0,
                _ => 0.0,
            };
            if i % UPDATE == 0 {
                let ramping = !released;
                let e = progress * progress;
                let intensity = self.level.value() * INTENSITY;
                let bars_left = (self.ascend.remaining / per_bar - 1e-6).ceil().max(0.0) as f32;
                let total_bars = (self.ascend.total / per_bar).max(1.0) as f32;
                let quantised = match self.ascend.phase {
                    Phase::Ramp => 1.0 - bars_left / total_bars,
                    Phase::Hold => 1.0,
                    _ => 0.0,
                };
                if ramping {
                    self.ascend.stage = stage_for(quantised);
                }
                let hp_target = if ramping { HP_MIN_HZ * (HP_MAX_HZ / HP_MIN_HZ).powf(e) } else { HP_MIN_HZ };
                let hp_hz = self.ascend.hp_cut.chase(hp_target, if ramping { ramp_chase } else { release_chase });
                for ch in 0..2 {
                    self.ascend.hp[ch].set(hp_hz, sr);
                }
                let centre = NOISE_LO_HZ * (NOISE_HI_HZ / NOISE_LO_HZ).powf(progress);
                self.ascend.noise.set_band(centre / 1.7, centre * 1.7);
                let on = if ramping { 1.0 } else { 0.0 };
                self.ascend.noise_gain.set(on * e * intensity);
                self.ascend.echo_return.set(lerp(ECHO_RETURN_MIN, ECHO_RETURN_MAX, e) * intensity * if ramping { 1.0 } else { 0.7 });
                self.ascend.reverb_send.set(on * lerp(REVERB_SEND_MIN, REVERB_SEND_MAX, e) * intensity);
                self.ascend.feedback.set(if ramping { lerp(FEEDBACK_MIN, FEEDBACK_MAX, e) } else { RELEASE_FEEDBACK });
                self.ascend.depth.set(on * TREMOLO_MAX * e * intensity.min(1.2));
            }
            let division = STAGE_BEATS[self.ascend.stage];
            let target = (division as f64 * beat_frames).min(max_delay as f64) as f32;
            self.grid16 += grid_inc;
            let boundary = self.grid16 >= 1.0;
            if boundary {
                self.grid16 -= 1.0;
            }
            self.taps.follow(target.max(2.0), boundary, &self.ring);
            let (yl, yr) = self.taps.read(&self.ring, right_offset);
            let send = self.send.next();
            let (xl, xr) = (l[i], r[i]);
            self.ascend.noise.track(xl, xr);
            let (hl, hr) = (self.ascend.hp[0].process(xl), self.ascend.hp[1].process(xr));
            let fb = self.ascend.feedback.next();
            let echo_return = self.ascend.echo_return.next();
            let reverb_send = self.ascend.reverb_send.next();
            let noise_amount = self.ascend.noise_gain.next();
            let depth = self.ascend.depth.next();
            let (fl, fr) = (self.loop_stage(0, yl, LOOP_SATURATION_DRIVE), self.loop_stage(1, yr, LOOP_SATURATION_DRIVE));
            self.ring.write(soft_limit(hl * send + fb * fl), soft_limit(hr * send + fb * fr));
            let (wl, wr) = self.reverb.process(self.reverb_in_hp[0].process(hl * send * reverb_send), self.reverb_in_hp[1].process(hr * send * reverb_send));
            let position = 4.0 - to_next / beat_frames;
            let phase = (position / division as f64).rem_euclid(1.0) as f32;
            self.ascend.gain.set(pulse_gain(phase, depth, TREMOLO_ATTACK_SECS / (division * beat_secs)));
            let trem = self.ascend.gain.next();
            let hiss = if noise_amount > 0.0 { noise_amount * self.ascend.noise.reference() * NOISE_PEAK * self.ascend.noise.duck_gain() * send } else { 0.0 };
            let bed = if hiss > 0.0 { self.ascend.noise.next() } else { [0.0; 2] };
            let wet_l = (yl * echo_return + soft_limit(wl) * REVERB_WET_GAIN) * trem;
            let wet_r = (yr * echo_return + soft_limit(wr) * REVERB_WET_GAIN) * trem;
            l[i] = soft_limit(hl + wet_l + bed[0] * hiss);
            r[i] = soft_limit(hr + wet_r + bed[1] * hiss);
        }
        if released && self.tail_left <= 0.0 {
            self.ascend.phase = Phase::Idle;
        }
    }
}

const ASCEND_LOOP_LP_HZ: f32 = 5_000.0;

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::*;
    use crate::fx::testkit::{band_psd, tone_amplitude};

    const PER_BAR: usize = 96_000;
    const SMALL: usize = 64;

    struct Rig {
        fx: BeatFx,
        frames: usize,
        landing: usize,
        stages: Vec<(usize, usize)>,
        progress_hit: Option<usize>,
    }

    impl Rig {
        fn new(length_choice: f32, level: f32, landing_bars: f64) -> Self {
            let mut fx = unit(BeatFxKind::Ascend, 1.0, level);
            fx.set_param(0, length_choice);
            fx.set_enabled(true);
            let request = fx.ascend_request().expect("armed on enable");
            assert!(fx.ascend_request().is_none(), "asked once");
            let landing = (landing_bars * PER_BAR as f64) as usize;
            fx.arm_ascend(AscendLanding { frames: landing as f64, secs: landing as f64 / 48_000.0, bars: landing_bars, landing_bar: 17 });
            assert!(request.length_bars > 0.0);
            Self { fx, frames: 0, landing, stages: Vec::new(), progress_hit: None }
        }

        fn step(&mut self, x: &[f32]) -> Vec<f32> {
            let to_next = PER_BAR - self.frames % PER_BAR;
            self.fx.set_transport(Some(Transport { frames_per_bar: PER_BAR as f64, frames_to_downbeat: to_next as f64 }));
            let (mut l, mut r) = (x.to_vec(), x.to_vec());
            self.fx.process(&mut l, &mut r);
            self.frames += x.len();
            if self.progress_hit.is_none() && self.fx.ascend_progress() == Some(1.0) {
                self.progress_hit = Some(self.frames);
            }
            let stage = self.fx.ascend.stage;
            if self.stages.last().is_none_or(|(_, s)| *s != stage) {
                self.stages.push((self.frames, stage));
            }
            l
        }

        fn run(&mut self, x: &[f32]) -> Vec<f32> {
            x.chunks(SMALL).flat_map(|c| self.step(c)).collect()
        }
    }

    #[test]
    fn the_ramp_ends_on_the_armed_frame_then_holds_the_peak() {
        let mut rig = Rig::new(0.0, 0.5, 4.0);
        let x = noise(PER_BAR * 6, 3, 0.3);
        let _ = rig.run(&x);
        let hit = rig.progress_hit.expect("ramp completes") as f64;
        let error_ms = (hit - rig.landing as f64) / 48.0;
        eprintln!("ramp end {error_ms:+.2} ms from the landing frame");
        assert!(error_ms.abs() <= 2.0);
        assert!(rig.fx.held() && rig.fx.ascend_progress() == Some(1.0), "holds the peak");
    }

    #[test]
    fn the_ramp_follows_a_remaining_frame_resync_so_it_still_lands_exactly() {
        let mut rig = Rig::new(0.0, 0.5, 4.0);
        let x = noise(PER_BAR * 6, 3, 0.3);
        let mut jitter = 0.0f64;
        for (b, chunk) in x.chunks(SMALL).enumerate() {
            if b % 50 == 0 {
                jitter += if b % 100 == 0 { 20.0 } else { -20.0 };
                let truth = rig.landing as f64 - rig.frames as f64 + jitter * 0.0;
                rig.fx.sync_ascend_remaining(truth);
            }
            rig.step(chunk);
        }
        let error_ms = (rig.progress_hit.unwrap() as f64 - rig.landing as f64) / 48.0;
        assert!(error_ms.abs() <= 2.0, "{error_ms}");
    }

    #[test]
    fn without_an_engine_arm_the_ramp_lasts_the_chosen_number_of_bars() {
        let mut fx = unit(BeatFxKind::Ascend, 1.0, 0.5);
        fx.set_param(0, 0.0);
        fx.set_enabled(true);
        let mut frames = 0;
        let mut hit = None;
        let x = noise(PER_BAR * 6, 1, 0.3);
        for chunk in x.chunks(SMALL) {
            let (mut l, mut r) = (chunk.to_vec(), chunk.to_vec());
            fx.process(&mut l, &mut r);
            frames += SMALL;
            if hit.is_none() && fx.ascend_progress() == Some(1.0) {
                hit = Some(frames);
            }
        }
        let error_ms = (hit.unwrap() as f64 - 4.0 * PER_BAR as f64) / 48.0;
        assert!(error_ms.abs() <= 10.0, "{error_ms}");
    }

    fn windows(frames: usize, at: f32) -> std::ops::Range<usize> {
        let centre = (frames as f32 * at) as usize;
        centre - 4_800..centre + 4_800
    }

    #[test]
    fn the_build_thins_the_dry_adds_noise_and_tightens_the_echo_together() {
        let mut rig = Rig::new(1.0, 0.5, 8.0);
        let frames = PER_BAR * 8;
        let bass = sine(frames, 100.0, 0.3);
        let x = bass;
        let out = rig.run(&x);
        let sample = |at: f32| {
            let r = windows(frames, at);
            (tone_amplitude(&out[r.clone()], 100.0) / 0.3, band_psd(&out[r], 3_000.0, 9_000.0))
        };
        let (early, mid, late) = (sample(0.1), sample(0.6), sample(0.95));
        eprintln!("100 Hz kept: {:.2} / {:.2} / {:.3}; 3-9 kHz psd {:.2e} / {:.2e} / {:.2e}", early.0, mid.0, late.0, early.1, mid.1, late.1);
        assert!(early.0 > 0.8 && late.0 < 0.1, "high-pass sweeps up over the build");
        assert!(late.1 > 20.0 * early.1.max(1e-12) && mid.1 < late.1, "noise rises");
        let stages: Vec<usize> = rig.stages.iter().map(|(_, s)| *s).collect();
        eprintln!("echo division stages {stages:?}");
        assert_eq!(stages.first(), Some(&0));
        assert_eq!(stages.last(), Some(&3), "ends on the shortest division");
        assert!(stages.windows(2).all(|w| w[1] > w[0]), "only ever shorter");
    }

    #[test]
    fn division_and_tremolo_rate_steps_land_on_bar_lines() {
        let mut rig = Rig::new(1.0, 0.5, 8.0);
        let _ = rig.run(&noise(PER_BAR * 8, 3, 0.2));
        for (frame, stage) in &rig.stages[1..] {
            let to_landing = rig.landing as i64 - *frame as i64;
            let off = to_landing.rem_euclid(PER_BAR as i64);
            let off = off.min(PER_BAR as i64 - off);
            assert!(off <= 2 * SMALL as i64 + UPDATE as i64, "stage {stage} changed {off} frames off a bar line");
        }
    }

    #[test]
    fn release_cuts_hard_on_the_next_downbeat_and_leaves_a_short_tail() {
        let mut rig = Rig::new(0.0, 0.5, 4.0);
        let bass = sine(PER_BAR * 8, 100.0, 0.3);
        let mut out = rig.run(&bass[..PER_BAR * 5 + 30_000]);
        let release_at = rig.frames;
        rig.fx.trigger(FxTrigger::Release);
        out.extend(rig.run(&bass[PER_BAR * 5 + 30_000..]));
        let downbeat = (release_at / PER_BAR + 1) * PER_BAR;
        let amp = |from: usize, len: usize| tone_amplitude(&out[from..from + len], 100.0) / 0.3;
        let before = amp(downbeat - 3_600, 2_400);
        let after = amp(downbeat + 2_400, 2_400);
        eprintln!("100 Hz: {before:.3} just before the downbeat, {after:.3} just after (release asked {:.0} ms ahead)", (downbeat - release_at) as f32 / 48.0);
        assert!(before < 0.1, "still thinned until the downbeat");
        assert!(after > 0.7, "full range back on the downbeat");
        assert!(!rig.fx.held(), "no longer holding");
        assert_eq!(rig.fx.ascend_progress(), None);
        let tail = rms(&out[downbeat + 24_000..downbeat + 48_000]);
        assert!(tail > 0.1 * 0.2, "the dry carries on");
    }

    #[test]
    fn release_now_cuts_immediately_and_the_tail_dies_out() {
        let mut fx_rig = Rig::new(0.0, 0.5, 4.0);
        fx_rig.fx.set_param(2, 1.0);
        let x = noise(PER_BAR * 5 + 12_345, 3, 0.3);
        let _ = fx_rig.run(&x);
        assert!(fx_rig.fx.held());
        let before = fx_rig.frames;
        fx_rig.fx.trigger(FxTrigger::Release);
        let silence = vec![0.0; 48_000 * 12];
        let out = fx_rig.run(&silence);
        assert!(rms(&out[2_400..24_000]) > 0.01, "echo and reverb ring out: {}", rms(&out[2_400..24_000]));
        assert!(rms(&out[48_000 * 6..]) < 1e-3, "short tail");
        assert!(fx_rig.fx.is_idle(), "idle {before}");
    }

    #[test]
    fn turning_the_effect_off_is_a_release_on_the_downbeat() {
        let mut rig = Rig::new(0.0, 0.5, 4.0);
        let bass = sine(PER_BAR * 8, 100.0, 0.3);
        let mut out = rig.run(&bass[..PER_BAR * 5 + 40_000]);
        let off_at = rig.frames;
        rig.fx.set_enabled(false);
        out.extend(rig.run(&bass[PER_BAR * 5 + 40_000..PER_BAR * 7]));
        let downbeat = (off_at / PER_BAR + 1) * PER_BAR;
        let amp = |from: usize| tone_amplitude(&out[from..from + 2_400], 100.0) / 0.3;
        assert!(amp(downbeat - 3_600) < 0.1);
        assert!(amp(downbeat + 2_400) > 0.7);
    }

    #[test]
    fn stays_under_the_limiter_at_default_level_through_the_build_and_the_hold() {
        let mut rig = Rig::new(1.0, 0.5, 8.0);
        let mut x = music(20.0, 120.0);
        let top = peak(&x);
        x.iter_mut().for_each(|v| *v *= 0.9 / top);
        let out = rig.run(&x);
        let worst = peak(&out);
        let dry_rms = rms(&x[PER_BAR * 6..PER_BAR * 8]);
        let hold_rms = rms(&out[PER_BAR * 6..PER_BAR * 8]);
        eprintln!("ascend peak {worst:.3}, loudness at the peak {:+.1} dB against the dry", db(hold_rms / dry_rms));
        let hot = out.iter().filter(|v| v.abs() > crate::limiter::LIMIT_KNEE).count() as f32 / out.len() as f32;
        eprintln!("{:.3} % of the samples reach the limiter knee", hot * 100.0);
        assert!(worst < 1.0, "peak {worst}");
        assert!(hot < 0.005, "the limiter is barely touched: {hot}");
        assert!(hold_rms < 2.0 * dry_rms, "the build gets louder by design, but not without bounds");
    }

    #[test]
    fn stays_finite_and_bounded_holding_the_peak_for_thirty_seconds_at_full_level() {
        for (name, x) in [
            ("noise", noise(48_000 * 30, 99, 1.0)),
            ("square", (0..48_000 * 30).map(|i| if (i / 55) % 2 == 0 { 1.0 } else { -1.0 }).collect()),
            ("sine", sine(48_000 * 30, 330.0, 1.0)),
        ] {
            let mut rig = Rig::new(0.0, 1.0, 4.0);
            let out = rig.run(&x);
            assert!(out.iter().all(|v| v.is_finite()), "{name}");
            assert!(peak(&out) < 4.0, "{name}: peak {}", peak(&out));
            assert!(rig.fx.held());
        }
    }

    #[test]
    fn restarting_the_effect_starts_a_fresh_ascent() {
        let mut rig = Rig::new(0.0, 0.5, 4.0);
        let _ = rig.run(&noise(PER_BAR * 5, 3, 0.3));
        assert!(rig.fx.held());
        rig.fx.set_enabled(false);
        let _ = rig.run(&vec![0.0; PER_BAR * 2]);
        rig.fx.set_enabled(true);
        assert!(rig.fx.ascend_request().is_some(), "a new ascent asks to be placed");
        assert_eq!(rig.fx.ascend_progress(), Some(0.0));
    }
}
