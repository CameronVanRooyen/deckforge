use deckforge_common::TrackAudio;

use super::*;
use crate::interp::read_cubic;

const MAX_SWELL_SECS: f32 = 6.0;
const MAX_SOURCE_SECS: f32 = 3.0;
const FADE_IN_SECS: f32 = 0.01;
const FADE_OUT_SECS: f32 = 0.0015;
const IN_HP_HZ: f32 = 90.0;
const PEAK_SEARCH_SECS: f32 = 0.3;
const PEAK_WINDOW_SECS: f32 = 0.01;
const PEAK_LIFT_SECS: f32 = 0.12;
const PEAK_MARGIN: f32 = 1.25;
const PEAK_MAX_LIFT: f32 = 3.0;
const PEAK_STEP: f32 = 1.04;
const WET_GAIN: f32 = 1.0;
const LEVEL_SCALE: f32 = 2.0;
const RT60_MIN: f32 = 0.6;
const RT60_MAX: f32 = 8.0;
const RT60_BASE: f32 = 0.9;
const RT60_SPAN: f32 = 1.1;
const BAND_SMOOTH_MS: f32 = 10.0;
const BEAT_OPTIONS: [f32; 3] = [1.0, 2.0, 4.0];

/// Asks the engine to place the swell: how many beats long it is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BackwashRequest {
    pub beats: f64,
}

/// Where the swell lands and which source audio it is built from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BackwashArm {
    pub frames_to_landing: f64,
    pub landing_secs: f64,
    pub source_start: f64,
    pub source_step: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Idle,
    Pending,
    Rendering,
    Ready,
}

#[derive(Debug, Clone)]
pub(super) struct BackwashState {
    phase: Phase,
    requested: bool,
    swell: [Vec<f32>; 2],
    in_hp: [HighPass<2>; 2],
    splitters: [BandSplitter; 2],
    band_gain: [Smoothed; 3],
    n_tail: usize,
    n_end: usize,
    envelope: Vec<f32>,
    n_source: usize,
    progress: usize,
    source_start: f64,
    source_step: f64,
    remaining: f64,
    landing_secs: f64,
    play_from: Option<f64>,
    sample_rate: f32,
    fade_in_frames: f64,
    fade_out_frames: f64,
}

impl BackwashState {
    pub(super) fn new(sample_rate: f32) -> Self {
        let capacity = (MAX_SWELL_SECS * sample_rate) as usize + 16;
        Self {
            phase: Phase::Idle,
            requested: false,
            swell: [vec![0.0; capacity], vec![0.0; capacity]],
            in_hp: [HighPass::default(); 2],
            splitters: [BandSplitter::new(sample_rate); 2],
            band_gain: [Smoothed::new(1.0, sample_rate, BAND_SMOOTH_MS); 3],
            n_tail: 0,
            n_end: 0,
            envelope: vec![0.0; capacity / ((PEAK_WINDOW_SECS * sample_rate) as usize).max(8) + 2],
            n_source: 0,
            progress: 0,
            source_start: 0.0,
            source_step: 1.0,
            remaining: 0.0,
            landing_secs: 0.0,
            play_from: None,
            sample_rate,
            fade_in_frames: (FADE_IN_SECS * sample_rate) as f64,
            fade_out_frames: (FADE_OUT_SECS * sample_rate) as f64,
        }
    }

    pub(super) fn reset(&mut self) {
        self.phase = Phase::Idle;
        self.requested = false;
        self.play_from = None;
        self.in_hp.iter_mut().for_each(HighPass::reset);
    }

    pub(super) fn start(&mut self) {
        self.phase = Phase::Pending;
        self.requested = false;
        self.play_from = None;
    }

    fn window_energy(&self, from: usize, window: usize) -> f32 {
        (from..from + window).map(|i| self.swell[0][i].powi(2) + self.swell[1][i].powi(2)).sum()
    }

    /// Cuts the swell where its diffusion build-up peaks and lifts the last 120 ms just enough that the
    /// loudest 10 ms of the whole swell is the one before the landing.
    fn settle_end(&mut self) -> usize {
        let window = ((PEAK_WINDOW_SECS * self.sample_rate) as usize).max(8);
        let search = ((PEAK_SEARCH_SECS * self.sample_rate) as usize).min(self.n_tail);
        let (mut best, mut end) = (0.0f32, self.n_tail);
        let mut at = self.n_tail - search;
        while at + window <= self.n_tail {
            let energy = self.window_energy(at, window);
            if energy > best {
                best = energy;
                end = at + window;
            }
            at += window / 2;
        }
        let end = end.max(2 * window).min(self.n_tail);
        let windows = (end / window).min(self.envelope.len());
        for k in 0..windows {
            self.envelope[k] = self.window_energy(end - (k + 1) * window, window);
        }
        let ramp = ((PEAK_LIFT_SECS * self.sample_rate) as usize).min(end);
        let ramp_windows = (ramp / window).max(1);
        let shape = |k: usize| {
            if k >= ramp_windows {
                return 0.0;
            }
            let t = 1.0 - (k as f32 + 0.5) / ramp_windows as f32;
            t * t * (3.0 - 2.0 * t)
        };
        let mut lift = 1.0f32;
        while lift < PEAK_MAX_LIFT {
            let gain = |k: usize| 1.0 + (lift - 1.0) * shape(k);
            let others = (1..windows).map(|k| self.envelope[k] * gain(k).powi(2)).fold(0.0f32, f32::max);
            if self.envelope[0] * lift * lift >= PEAK_MARGIN * others {
                break;
            }
            lift *= PEAK_STEP;
        }
        if lift > 1.0 {
            for k in 0..ramp {
                let t = k as f32 / ramp as f32;
                let g = 1.0 + (lift - 1.0) * t * t * (3.0 - 2.0 * t);
                self.swell[0][end - ramp + k] *= g;
                self.swell[1][end - ramp + k] *= g;
            }
        }
        end
    }

    pub(super) fn rearm(&mut self, enabled: bool) {
        if enabled {
            self.start();
        }
    }
}

fn nearest_beats(beats: f32) -> f32 {
    BEAT_OPTIONS.iter().copied().min_by(|a, b| (a / beats).ln().abs().total_cmp(&(b / beats).ln().abs())).unwrap_or(2.0)
}

impl BeatFx {
    pub fn backwash_request(&mut self) -> Option<BackwashRequest> {
        let b = &mut self.backwash;
        if b.phase == Phase::Pending && !b.requested {
            b.requested = true;
            return Some(BackwashRequest { beats: nearest_beats(self.beats) as f64 });
        }
        None
    }

    /// Gives up on the pending swell, e.g. when the target deck is not playing or has no grid.
    pub fn backwash_cancel(&mut self) {
        self.backwash.phase = Phase::Idle;
    }

    /// Starts building the swell: the audio from the landing point on, reversed, through the reverb, reversed again.
    pub fn backwash_arm(&mut self, arm: BackwashArm) {
        if self.backwash.phase != Phase::Pending || arm.frames_to_landing < 1.0 {
            return;
        }
        let sr = self.sample_rate;
        let beats = nearest_beats(self.beats);
        let n_tail = ((beats * self.beat_secs() * sr) as usize).clamp(64, (MAX_SWELL_SECS * sr) as usize);
        let n_source = n_tail.min((MAX_SOURCE_SECS * sr) as usize);
        let size = self.param(0);
        let rt60 = ((n_tail as f32 / sr) * (RT60_BASE + RT60_SPAN * percent_position(size))).clamp(RT60_MIN, RT60_MAX);
        self.reverb.reset();
        self.reverb.set_size(reverb_size_for_percent(size));
        self.reverb.set_predelay_ms(1.0);
        self.reverb.set_hf_ratio(0.7);
        self.reverb.set_modulation_ms(reverb_modulation_ms(percent_position(size)));
        self.reverb.set_rt60(rt60);
        let b = &mut self.backwash;
        for hp in &mut b.in_hp {
            hp.reset();
            hp.set(IN_HP_HZ, sr);
        }
        b.n_tail = n_tail;
        b.n_source = n_source;
        b.progress = 0;
        b.source_start = arm.source_start;
        b.source_step = arm.source_step;
        b.remaining = arm.frames_to_landing;
        b.landing_secs = arm.landing_secs;
        b.play_from = None;
        b.phase = Phase::Rendering;
    }

    pub fn backwash_landing_secs(&self) -> Option<f64> {
        matches!(self.backwash.phase, Phase::Rendering | Phase::Ready).then_some(self.backwash.landing_secs)
    }

    pub fn sync_backwash_remaining(&mut self, frames: f64) {
        if matches!(self.backwash.phase, Phase::Rendering | Phase::Ready) {
            self.backwash.remaining = frames.max(0.0);
        }
    }

    pub fn backwash_rendering(&self) -> bool {
        self.backwash.phase == Phase::Rendering
    }

    /// Renders up to `budget` frames of the swell; call once per audio block while `backwash_rendering` is true.
    pub fn backwash_pump(&mut self, track: &TrackAudio, budget: usize) {
        if self.backwash.phase != Phase::Rendering {
            return;
        }
        let total = self.backwash.n_source + self.backwash.n_tail;
        for _ in 0..budget {
            let b = &mut self.backwash;
            let k = b.progress;
            if k >= total {
                break;
            }
            if k < b.n_source {
                let position = b.source_start + (b.n_source - 1 - k) as f64 * b.source_step;
                let (a, c) = read_cubic(track, position);
                let (a, c) = (b.in_hp[0].process(a), b.in_hp[1].process(c));
                let _ = self.reverb.process(a, c);
            } else {
                let (wl, wr) = self.reverb.process(0.0, 0.0);
                let index = b.n_tail - 1 - (k - b.n_source);
                b.swell[0][index] = wl;
                b.swell[1][index] = wr;
            }
            self.backwash.progress += 1;
        }
        let b = &mut self.backwash;
        if b.progress >= total {
            b.n_end = b.settle_end();
            let fade = (b.fade_in_frames as usize).min(b.n_end);
            for j in 0..fade {
                let t = j as f32 / fade as f32;
                let g = t * t * (3.0 - 2.0 * t);
                b.swell[0][j] *= g;
                b.swell[1][j] *= g;
            }
            b.phase = Phase::Ready;
        }
    }

    pub(super) fn run_backwash(&mut self, l: &mut [f32], r: &mut [f32]) {
        let phase = self.backwash.phase;
        if !matches!(phase, Phase::Rendering | Phase::Ready) {
            return;
        }
        for (g, t) in self.backwash.band_gain.iter_mut().zip(self.band_gain.iter()) {
            g.set(t.target());
        }
        for i in 0..l.len() {
            let send = self.send.next();
            let gate = self.swap.next();
            let lvl = self.level.next();
            let gains = [self.backwash.band_gain[0].next(), self.backwash.band_gain[1].next(), self.backwash.band_gain[2].next()];
            let b = &mut self.backwash;
            if !matches!(b.phase, Phase::Rendering | Phase::Ready) {
                continue;
            }
            b.remaining -= 1.0;
            if b.remaining <= 0.0 {
                b.phase = Phase::Idle;
                continue;
            }
            if b.phase != Phase::Ready || b.remaining > b.n_end as f64 {
                continue;
            }
            let index = b.n_end as f64 - b.remaining;
            let from = *b.play_from.get_or_insert(index);
            let whole = index as usize;
            let frac = (index - whole as f64) as f32;
            let next = (whole + 1).min(b.n_end - 1);
            let mut env = 1.0f32;
            if from > 0.5 {
                let t = ((index - from) / b.fade_in_frames).clamp(0.0, 1.0) as f32;
                env *= t * t * (3.0 - 2.0 * t);
            }
            if b.remaining < b.fade_out_frames {
                env *= (b.remaining / b.fade_out_frames) as f32;
            }
            let scale = env * WET_GAIN * (lvl * LEVEL_SCALE) * send * gate;
            let (sl, sr_) = (b.swell[0][whole] + (b.swell[0][next] - b.swell[0][whole]) * frac, b.swell[1][whole] + (b.swell[1][next] - b.swell[1][whole]) * frac);
            let weigh = |bands: [f32; 3]| bands[0] * gains[0] + bands[1] * gains[1] + bands[2] * gains[2];
            let wl = weigh(b.splitters[0].split(sl * scale));
            let wr = weigh(b.splitters[1].split(sr_ * scale));
            l[i] += soft_limit(wl);
            r[i] += soft_limit(wr);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::*;
    use crate::alloc_guard::allocations_in;
    use crate::fx::testkit::band_psd;

    fn track_of(secs: f32) -> TrackAudio {
        let mono = music(secs, 120.0);
        TrackAudio::new(48_000, mono.iter().flat_map(|v| [*v, *v]).collect())
    }

    fn armed(beats: f32, landing: usize) -> BeatFx {
        let mut fx = unit(BeatFxKind::Backwash, beats, 0.5);
        fx.set_enabled(true);
        let request = fx.backwash_request().expect("asks once on enable");
        assert_eq!(request.beats, beats as f64);
        assert!(fx.backwash_request().is_none());
        fx.backwash_arm(BackwashArm { frames_to_landing: landing as f64, landing_secs: landing as f64 / 48_000.0, source_start: landing as f64, source_step: 1.0 });
        fx
    }

    fn play(fx: &mut BeatFx, track: &TrackAudio, frames: usize, budget: usize) -> Vec<f32> {
        let silence = vec![0.0f32; BLOCK];
        let mut out = Vec::with_capacity(frames);
        for _ in 0..frames / BLOCK {
            fx.backwash_pump(track, budget);
            let (mut l, mut r) = (silence.clone(), silence.clone());
            fx.process(&mut l, &mut r);
            out.extend_from_slice(&l);
        }
        out
    }

    /// End of the loudest 10 ms window: where the swell's envelope reaches its maximum.
    fn envelope_peak_at(out: &[f32]) -> usize {
        let window = 480;
        let mut best = (0.0f32, 0usize);
        let mut at = 0;
        while at + window <= out.len() {
            let energy: f32 = out[at..at + window].iter().map(|v| v * v).sum();
            if energy > best.0 {
                best = (energy, at + window);
            }
            at += 96;
        }
        best.1
    }

    #[test]
    fn the_swell_peaks_within_ten_milliseconds_of_the_landing_downbeat() {
        let track = track_of(20.0);
        for (beats, landing) in [(1.0f32, 60_000usize), (2.0, 96_000), (4.0, 192_000)] {
            let landing = landing.max(((beats * 24_000.0) as usize) + 12_000);
            let mut fx = armed(beats, landing);
            let out = play(&mut fx, &track, landing + 4_800, 16 * BLOCK);
            let at = envelope_peak_at(&out);
            let error_ms = (at as f32 - landing as f32) / 48.0;
            eprintln!("backwash {beats} beats: peak {error_ms:+.2} ms from the landing, level {:.3}", peak(&out));
            assert!(error_ms.abs() <= 10.0 && at <= landing, "{beats} beats: {error_ms} ms");
            assert!(peak(&out[landing..]) < 1e-6, "silent after the landing");
        }
    }

    #[test]
    fn the_swell_builds_up_to_the_downbeat() {
        let track = track_of(20.0);
        let mut fx = armed(4.0, 96_000);
        let out = play(&mut fx, &track, 96_000, 16 * BLOCK);
        let windows: Vec<f32> = (0..4).map(|k| rms(&out[k * 24_000..(k + 1) * 24_000])).collect();
        eprintln!("backwash rms per beat: {windows:?}");
        assert!(windows.windows(2).all(|w| w[1] > w[0]), "rises every beat");
        assert!(windows[3] > 4.0 * windows[0]);
        assert!(peak(&out) > 0.05 && peak(&out) < 1.0, "audible but under the music: {}", peak(&out));
    }

    #[test]
    fn a_swell_armed_late_still_peaks_on_the_downbeat_with_a_gentle_start() {
        let track = track_of(20.0);
        let mut fx = armed(4.0, 30_000);
        let out = play(&mut fx, &track, 36_000, 64 * BLOCK);
        let at = envelope_peak_at(&out);
        assert!(((at as f32 - 30_000.0) / 48.0).abs() <= 10.0);
        assert!(peak(&out[..200]) < 0.01 || peak(&out[..200]) < 0.2 * peak(&out), "no click at the late start");
    }

    #[test]
    fn a_swell_that_cannot_finish_in_time_is_dropped_quietly() {
        let track = track_of(20.0);
        let mut fx = armed(4.0, 3_000);
        let out = play(&mut fx, &track, 12_000, BLOCK / 4);
        assert!(peak(&out) < 1e-6);
        assert!(!fx.backwash_rendering());
    }

    #[test]
    fn the_landing_can_be_remeasured_each_block() {
        let track = track_of(20.0);
        let mut fx = armed(2.0, 96_000);
        let silence = vec![0.0f32; BLOCK];
        let mut out = Vec::new();
        for b in 0..(96_000 + 9_600) / BLOCK {
            fx.backwash_pump(&track, 16 * BLOCK);
            if b == 200 {
                fx.sync_backwash_remaining(96_000.0 - (b * BLOCK) as f64 + 480.0);
            }
            let (mut l, mut r) = (silence.clone(), silence.clone());
            fx.process(&mut l, &mut r);
            out.extend_from_slice(&l);
        }
        let at = envelope_peak_at(&out);
        let error_ms = (at as f32 - 96_480.0) / 48.0;
        assert!(error_ms.abs() <= 10.0, "follows the resync: {error_ms}");
    }

    #[test]
    fn rendering_and_playing_never_allocate() {
        let track = track_of(20.0);
        let mut fx = armed(2.0, 96_000);
        let (mut l, mut r) = (vec![0.0f32; BLOCK], vec![0.0f32; BLOCK]);
        let (_, allocations) = allocations_in(|| {
            for b in 0..(96_000 + 4_800) / BLOCK {
                fx.backwash_pump(&track, 16 * BLOCK);
                l.fill(0.1 * (b % 7) as f32);
                r.fill(0.0);
                fx.process(&mut l, &mut r);
            }
        });
        assert_eq!(allocations, 0);
    }

    #[test]
    fn the_low_band_selection_keeps_the_bass_out_of_the_swell() {
        let track = track_of(20.0);
        let low = |bands: FxBands| {
            let mut fx = banded(BeatFxKind::Backwash, 4.0, 0.8);
            fx.set_bands(bands);
            fx.set_enabled(true);
            let _ = fx.backwash_request();
            fx.backwash_arm(BackwashArm { frames_to_landing: 192_000.0, landing_secs: 4.0, source_start: 192_000.0, source_step: 1.0 });
            let out = play(&mut fx, &track, 192_000, 16 * BLOCK);
            band_psd(&out[96_000..], 40.0, 200.0)
        };
        let (with, without) = (low(FxBands::ALL), low(FxBands { low: false, mid: true, high: true }));
        eprintln!("swell below 200 Hz: {:.1} dB lower with LOW off", db((with / without).sqrt()));
        assert!(without < with * 0.02, "LOW off removes at least 17 dB of bass");
    }

    #[test]
    fn disabling_the_effect_fades_the_swell_out_without_a_click() {
        let track = track_of(20.0);
        let mut fx = armed(2.0, 96_000);
        let mut out = play(&mut fx, &track, 80_000, 16 * BLOCK);
        fx.set_enabled(false);
        out.extend(play(&mut fx, &track, 16_000, 16 * BLOCK));
        assert!(max_step_of(&out[80_000..]) < 0.05);
        assert!(peak(&out[84_000..]) < 1e-5);
    }

    fn max_step_of(x: &[f32]) -> f32 {
        x.windows(2).fold(0.0, |m, w| m.max((w[1] - w[0]).abs()))
    }

    #[test]
    fn it_stays_finite_for_thirty_seconds_of_repeated_swells_over_loud_material() {
        let mono = noise(48_000 * 30, 7, 1.0);
        let track = TrackAudio::new(48_000, mono.iter().flat_map(|v| [*v, *v]).collect());
        let mut fx = unit(BeatFxKind::Backwash, 4.0, 1.0);
        fx.set_param(0, 100.0);
        fx.set_enabled(true);
        let mut all = Vec::new();
        for round in 0..6 {
            fx.trigger(FxTrigger::ThrowOn);
            let _ = fx.backwash_request();
            let landing = 48_000 * 4;
            fx.backwash_arm(BackwashArm { frames_to_landing: landing as f64, landing_secs: 4.0, source_start: (round * landing) as f64, source_step: 1.0 });
            all.extend(play(&mut fx, &track, landing + 480, 16 * BLOCK));
        }
        assert!(all.iter().all(|v| v.is_finite()));
        assert!(peak(&all) < 2.0, "peak {}", peak(&all));
    }
}
