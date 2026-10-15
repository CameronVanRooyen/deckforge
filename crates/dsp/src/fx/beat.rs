use deckforge_common::{beat_fx_params, BeatFxKind, FxBands, FxTrigger, BEAT_FX_MAX_PARAMS};

mod ascend;
mod backwash;
mod dub;
mod halo;
mod pulse;
mod spiral;

#[cfg(test)]
mod testing;

pub use ascend::{AscendLanding, AscendRequest};
pub use backwash::{BackwashArm, BackwashRequest};

use super::filters::{chase_coeff, Ducker, HighPass, HighShelf, LogSlew, LowPass, PinkNoise};
use super::mix::{beat_mix, gated_dry};
use super::pitch::PitchShifter;
use super::reverb::FdnReverb;
use super::sweep_filter::SweepFilter;
use super::ring::StereoRing;
use super::taps::TapFade;
use super::util::{flush, Lfo, Noise, OnePole, SwapGate};
use crate::isolator::BandSplitter;
use crate::limiter::soft_limit;
use crate::smooth::Smoothed;
use crate::svf::Svf;

/// Longest delay the beat-synced effects can use; longer divisions are halved until they fit.
pub const MAX_DELAY_SECS: f32 = 8.0;

const LEVEL_SMOOTH_MS: f32 = 20.0;
const SEND_SMOOTH_MS: f32 = 4.0;
const DECLICK_TAIL_SECS: f64 = 0.04;
const ECHO_WET_GAIN: f32 = 1.0;
const ECHO_FEEDBACK_MAX: f32 = 0.993;
const ECHO_FEEDBACK_SMOOTH_MS: f32 = 3.3;
const ECHO_FREEZE_SMOOTH_MS: f32 = 6.0;
const ECHO_DC_BLOCK_HZ: f32 = 20.0;
const LOOP_HP_HZ: f32 = 200.0;
const LOOP_SATURATION_DRIVE: f32 = 1.4;
const RIGHT_OFFSET_MS: f32 = 0.5;
const PING_PONG_RATIOS: [f32; 8] = [0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0];
const BAND_GAIN_SMOOTH_MS: f32 = 10.0;
const BAND_BLEND_SMOOTH_MS: f32 = 8.0;
const BAND_BLEND_OUT_SECS: f64 = 0.1;
const BAND_SCRATCH: usize = 2_048;
const TAIL_FLOOR: f64 = 1e-3;
const MAX_TAIL_SECS: f64 = 60.0;
const SHIFT_WINDOW_SECS: f32 = 0.04;
const SEMITONE_STEP: f32 = 1.0;
const REVERB_CHUNK: usize = 32;
const REVERB_MIN_RT60: f32 = 1.0;
const REVERB_MAX_RT60: f32 = 11.5;
const REVERB_HF_RATIO: f32 = 0.55;
const REVERB_IN_HP_MIN_HZ: f32 = 250.0;
const REVERB_IN_HP_SPAN_HZ: f32 = 100.0;
const REVERB_OUT_HP_MIN_HZ: f32 = 150.0;
const REVERB_OUT_HP_MAX_HZ: f32 = 1_000.0;
const REVERB_SHELF_HZ: f32 = 5_000.0;
const REVERB_SHELF_MAX_DB: f32 = 5.0;
const REVERB_TONE_CHASE_SECS: f32 = 0.05;
const REVERB_SIZE_MIN: f32 = 0.85;
const REVERB_SIZE_LOG_SPAN: f32 = 0.40;
const REVERB_SIZE_CURVE: f32 = 0.5;
const ROOM_SCALE_MIN: f32 = 0.7;
const ROOM_SCALE_LOG_SPAN: f32 = 0.75;
const TENSION_DECAY_POINTS: [(f32, f32); 11] =
    [(1.0, 1.5), (10.0, 1.6), (20.0, 1.95), (30.0, 3.4), (40.0, 5.6), (50.0, 8.7), (60.0, 13.8), (70.0, 22.8), (80.0, 38.0), (90.0, 63.0), (100.0, 100.0)];
const ROOM_SIZE_CURVE: f32 = 0.171;
const ROOM_SIZE_SLOPE: f32 = 0.023_06;
const ROOM_SIZE_OFFSET: f32 = 1.01;
const ROOM_SIZE_FLOOR: f32 = 0.2;
const ROOM_LINE_RATIO: f32 = 0.6808;
const ROOM_STEP_MAX_MS: f32 = 40.0;
const ROOM_STEP_MIN_MS: f32 = 20.0;
const ROOM_STEP_MS_PER_PERCENT: f32 = 0.6667;
const ROOM_STEP_SLEW_MS: f32 = 0.6667;
const DECAY_QUARTIC: [f32; 5] = [2.872_803e-6, -3.339_133e-4, 0.016_808_31, -0.225_531_4, 1.740_863];
const DECAY_MIN_RT60: f32 = 0.1;
const CLASSIC_OUTPUT_GAIN: f32 = 0.47;
const CLASSIC_LONG_DECAY_TRIM: f32 = 0.5;
const REVERB_VOICING_PARAM: usize = 3;
const BEND_FULL_RATE: f32 = 0.14;
const BEND_MIN_RATE: f32 = 0.004;
const REVERB_OUTPUT_FLOOR: f32 = 0.1;
const CLASSIC_IN_HP_HZ: f32 = 80.0;
const CLASSIC_IN_LP_HZ: f32 = 14_000.0;
const CLASSIC_OUT_HP_HZ: f32 = 80.0;
const CLASSIC_WET_GAIN: f32 = 0.72;
const TENSION_WET_GAIN: f32 = 1.0;
const REVERB_SWELL_DB: f32 = 2.0;
const WASH_PEAK: f32 = 2.6;
const WASH_SIZE_FLOOR: f32 = 0.3;
const WASH_HP_MIN_HZ: f32 = 2_000.0;
const WASH_HP_MAX_HZ: f32 = 4_500.0;
const WASH_DUCK_DB: f32 = 6.0;
const WASH_DUCK_RELEASE_MS: f32 = 300.0;
const WASH_REF_FLOOR: f32 = 0.04;
const WASH_REF_CEIL: f32 = 0.7;
const WASH_REF_SECS: f32 = 0.25;
const WASH_BAND_SMOOTH_MS: f32 = 10.0;
const WHITE_UNIT_GAIN: f32 = 1.732_050_8;
const REVERB_DUCK_DB: f32 = 4.5;
const DUCK_ATTACK_MS: f32 = 8.0;
const DUCK_RELEASE_MS: f32 = 200.0;
const REVERB_RT60_SMOOTH_MS: f32 = 160.0;
const PERCENT_MIN: f32 = 1.0;
const PERCENT_MAX: f32 = 100.0;
const FLANGER_MIN_MS: f32 = 0.4;
const FLANGER_RANGE_MS: f32 = 4.0;
const FLANGER_FEEDBACK: f32 = 0.55;
const PHASER_STAGES: usize = 6;
const PHASER_FEEDBACK: f32 = 0.5;
const PHASER_MIN_HZ: f32 = 300.0;
const PHASER_MAX_HZ: f32 = 3_500.0;
const SWEEP_MIN_HZ: f32 = 180.0;
const SWEEP_MAX_HZ: f32 = 12_000.0;
const SWEEP_Q: f32 = 1.4;
const SWEEP_Q_MIN: f32 = 0.7;
const SWEEP_Q_MAX: f32 = 2.5;
const COEFF_UPDATE: usize = 16;
const ROLL_CROSSFADE: usize = 64;
const GATE_SMOOTH_SECS: f32 = 0.0015;
const OUTPUT_GUARD: f32 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RollPhase {
    Armed,
    Recording,
    Looping,
}

#[derive(Clone, Copy)]
struct EchoStyle {
    right_offset_ms: f32,
    ping_pong: bool,
    shift_ratio: Option<f32>,
}

/// Echo FEEDBACK knob (0..100): 0 to 0.7 up to the centre, then on to just under unity at the far end.
pub(crate) fn echo_feedback_for_knob(knob: f32) -> f32 {
    let v = knob.clamp(0.0, 100.0);
    if v <= 50.0 { 0.7 * v / 50.0 } else { 0.7 + (ECHO_FEEDBACK_MAX - 0.7) * (v - 50.0) / 50.0 }
}

fn echo_style(kind: BeatFxKind) -> EchoStyle {
    let base = EchoStyle { right_offset_ms: RIGHT_OFFSET_MS, ping_pong: false, shift_ratio: None };
    let semitones = |st: f32| (st / 12.0).exp2();
    match kind {
        BeatFxKind::PingPong => EchoStyle { right_offset_ms: 0.0, ping_pong: true, ..base },
        BeatFxKind::UpEcho => EchoStyle { shift_ratio: Some(semitones(SEMITONE_STEP)), ..base },
        BeatFxKind::DownEcho => EchoStyle { shift_ratio: Some(semitones(-SEMITONE_STEP)), ..base },
        _ => base,
    }
}

/// Where the target deck's bar lines fall, in output frames, as measured at the start of a block.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transport {
    pub frames_per_bar: f64,
    pub frames_to_downbeat: f64,
}

/// A bar line within this many frames counts as reached, so rounding in the measured distance cannot skip it.
const BOUNDARY_SLACK: f64 = 0.5;

#[derive(Debug, Clone, Copy)]
struct BarClock {
    to_next: f64,
    per_bar: f64,
}

impl BarClock {
    fn advance(&mut self, frames: usize) {
        self.to_next -= frames as f64;
        while self.to_next <= BOUNDARY_SLACK {
            self.to_next += self.per_bar;
        }
    }
}

/// Maps a 0..100 knob to `lo`..`mid`..`hi`, landing exactly on `mid` at 50.
fn centered(v: f32, lo: f32, mid: f32, hi: f32) -> f32 {
    if v <= 50.0 { mid - (mid - lo) * ((50.0 - v) / 50.0) } else { mid + (hi - mid) * ((v - 50.0) / 50.0) }
}

/// The single Beat FX unit. All buffers are allocated in `new`; `process` never allocates.
pub struct BeatFx {
    sample_rate: f32,
    ring: StereoRing,
    reverb: FdnReverb,
    rt60: Smoothed,
    rt60_applied: f32,
    room_step: i32,
    room_step_ms: f32,
    room_step_left: usize,
    classic_lp: [OnePole; 2],
    classic_hp: [OnePole; 2],
    percent: f32,
    kind: BeatFxKind,
    active: BeatFxKind,
    beats: f32,
    bpm: f32,
    enabled: bool,
    idle: bool,
    tail_left: f64,
    level: Smoothed,
    send: Smoothed,
    swap: SwapGate,
    delay_snap: bool,
    taps: TapFade,
    grid16: f32,
    echo_fb: Smoothed,
    echo_feed: Smoothed,
    echo_freeze_held: bool,
    echo_filter: SweepFilter,
    loop_lp: [LowPass<1>; 2],
    loop_hp: [HighPass<1>; 2],
    reverb_in_hp: [HighPass<2>; 2],
    reverb_out_hp: [HighPass<1>; 2],
    reverb_shelf: [HighShelf; 2],
    shelf_db: f32,
    out_cut: LogSlew,
    wash_hp: [HighPass<2>; 2],
    wash_cut: LogSlew,
    wash_white: [Noise; 2],
    wash_pink: [PinkNoise; 2],
    wash_splitters: [BandSplitter; 2],
    wash_band_gain: [Smoothed; 3],
    wash_duck: Ducker,
    wash_ref: f32,
    pub(crate) wash_scale: f32,
    pub(crate) wash_duck_scale: f32,
    bar_phase: f64,
    duck: Ducker,
    band_sets: [FxBands; BeatFxKind::ALL.len()],
    params: [[f32; BEAT_FX_MAX_PARAMS]; BeatFxKind::ALL.len()],
    bar_clock: BarClock,
    has_transport: bool,
    pulse: pulse::PulseState,
    spiral: spiral::SpiralState,
    dub: dub::DubState,
    halo: halo::HaloState,
    ascend: ascend::AscendState,
    backwash: backwash::BackwashState,
    band_gain: [Smoothed; 3],
    band_mix: Smoothed,
    flushing: bool,
    splitters: [BandSplitter; 2],
    band_dry: [Vec<f32>; 2],
    band_raw: [Vec<f32>; 2],
    #[cfg(test)]
    raw_core: bool,
    shifters: [PitchShifter; 2],
    lfo: Lfo,
    phaser_state: [[f32; PHASER_STAGES]; 2],
    phaser_feedback: [f32; 2],
    sweep: [Svf; 2],
    gate: f32,
    roll_phase: RollPhase,
    roll_start: usize,
    roll_len: usize,
    roll_count: usize,
    roll_pos: usize,
    roll_gate: Smoothed,
}

/// Reverb line modulation depth: backed off as the decay lengthens so long tails do not detune.
fn reverb_modulation_ms(position: f32) -> f32 {
    0.6 - 0.25 * position
}

/// Delay-line scale of the Beat FX reverb for a 1..100 % setting; bigger rooms have longer lines.
fn reverb_size_for_percent(percent: f32) -> f32 {
    REVERB_SIZE_MIN * (REVERB_SIZE_LOG_SPAN * percent_position(percent).powf(REVERB_SIZE_CURVE)).exp()
}

fn percent_position(percent: f32) -> f32 {
    ((percent - PERCENT_MIN) / (PERCENT_MAX - PERCENT_MIN)).clamp(0.0, 1.0)
}

/// Delay-line scale of the Reverb room for an integer ROOM SIZE step (1..100): rekordbox's size law, rescaled to our line lengths.
fn reverb_room_scale(step: i32) -> f32 {
    let r = step.clamp(1, 100) as f32;
    ROOM_LINE_RATIO * (ROOM_SIZE_CURVE * (ROOM_SIZE_SLOPE * r + ROOM_SIZE_OFFSET).powi(2) + ROOM_SIZE_FLOOR)
}

/// Delay-line scale of the Tension voicing's room for a 1..100 % size.
fn tension_room_scale(percent: f32) -> f32 {
    ROOM_SCALE_MIN * (ROOM_SCALE_LOG_SPAN * percent_position(percent).powf(REVERB_SIZE_CURVE)).exp()
}

/// Tension voicing RT60: the rekordbox curve with its short-end dip smoothed into a monotonic rise.
fn tension_decay_rt60(decay: f32) -> f32 {
    let points = TENSION_DECAY_POINTS;
    let d = decay.clamp(points[0].0, points[points.len() - 1].0);
    let i = points.iter().rposition(|(p, _)| *p <= d).unwrap_or(0).min(points.len() - 2);
    let ((p0, t0), (p1, t1)) = (points[i], points[i + 1]);
    (t0.ln() + (t1.ln() - t0.ln()) * (d - p0) / (p1 - p0)).exp()
}

/// RT60 of the Reverb for its DECAY setting in the chosen voicing.
fn voiced_decay_rt60(classic: bool, decay: f32) -> f32 {
    if classic { reverb_decay_rt60(decay) } else { tension_decay_rt60(decay) }
}

/// Length of one ROOM SIZE step in ms: faster the further the knob has to travel, slewed 0.67 ms per step, never below 20 ms.
fn room_step_ms(previous: f32, current: i32, target: i32) -> f32 {
    let goal = (ROOM_STEP_MAX_MS - ROOM_STEP_MS_PER_PERCENT * (current - target).abs() as f32).max(ROOM_STEP_MIN_MS);
    (previous + (goal - previous).clamp(-ROOM_STEP_SLEW_MS, ROOM_STEP_SLEW_MS)).max(ROOM_STEP_MIN_MS)
}

/// RT60 in seconds for the Reverb DECAY setting (1..100): rekordbox's quartic, 0.85 s at its shortest near 8, 8.7 s at the centre, about 100 s at the top.
pub fn reverb_decay_rt60(decay: f32) -> f32 {
    let p = decay.clamp(PERCENT_MIN, PERCENT_MAX);
    DECAY_QUARTIC.iter().fold(0.0, |acc, c| acc * p + c).max(DECAY_MIN_RT60)
}

/// Input trim of the classic voicing: full on the left half of DECAY, halving towards the top so the long tails do not swamp the mix.
fn classic_decay_trim(decay: f32) -> f32 {
    1.0 - CLASSIC_LONG_DECAY_TRIM * ((decay - 50.0) / 50.0).clamp(0.0, 1.0)
}

/// Fastest size glide, in samples per sample, for the Bend setting (0..100).
fn reverb_bend_rate(bend: f32) -> f32 {
    let amount = bend.clamp(0.0, 100.0) * 0.01;
    (BEND_FULL_RATE * amount * amount).max(BEND_MIN_RATE)
}

/// Decay time of the Beat FX reverb for a 1..100 % setting: 1 s at the bottom, about 10 s at the top.
pub fn reverb_rt60_for_percent(percent: f32) -> f32 {
    REVERB_MIN_RT60 * (REVERB_MAX_RT60 / REVERB_MIN_RT60).powf(percent_position(percent))
}

impl BeatFx {
    pub fn new(sample_rate: f32) -> Self {
        let ring = StereoRing::new((MAX_DELAY_SECS * sample_rate) as usize + 8);
        let mut fx = Self {
            sample_rate,
            ring,
            reverb: FdnReverb::new(sample_rate),
            rt60: Smoothed::new(reverb_rt60_for_percent(50.0), sample_rate / REVERB_CHUNK as f32, REVERB_RT60_SMOOTH_MS),
            rt60_applied: 0.0,
            room_step: 0,
            room_step_ms: ROOM_STEP_MAX_MS,
            room_step_left: 0,
            classic_lp: [OnePole::default(); 2],
            classic_hp: [OnePole::default(); 2],
            percent: 50.0,
            kind: BeatFxKind::Echo,
            active: BeatFxKind::Echo,
            beats: 1.0,
            bpm: 120.0,
            enabled: false,
            idle: true,
            tail_left: 0.0,
            level: Smoothed::new(0.5, sample_rate, LEVEL_SMOOTH_MS),
            send: Smoothed::new(0.0, sample_rate, SEND_SMOOTH_MS),
            swap: SwapGate::new(sample_rate),
            delay_snap: true,
            taps: TapFade::new(sample_rate),
            grid16: 0.0,
            echo_fb: Smoothed::new(0.5, sample_rate, ECHO_FEEDBACK_SMOOTH_MS),
            echo_feed: Smoothed::new(1.0, sample_rate, ECHO_FREEZE_SMOOTH_MS),
            echo_freeze_held: false,
            echo_filter: SweepFilter::new(sample_rate),
            loop_lp: [LowPass::default(); 2],
            loop_hp: [HighPass::default(); 2],
            reverb_in_hp: [HighPass::default(); 2],
            reverb_out_hp: [HighPass::default(); 2],
            reverb_shelf: [HighShelf::new(REVERB_SHELF_HZ, 0.0, sample_rate); 2],
            shelf_db: 0.0,
            out_cut: LogSlew::new(REVERB_OUT_HP_MIN_HZ),
            wash_hp: [HighPass::default(); 2],
            wash_cut: LogSlew::new(WASH_HP_MIN_HZ),
            wash_white: [Noise::new(0x9E37_79B1), Noise::new(0x85EB_CA6B)],
            wash_pink: [PinkNoise::new(0xC2B2_AE35), PinkNoise::new(0x27D4_EB2F)],
            wash_splitters: [BandSplitter::new(sample_rate); 2],
            wash_band_gain: [Smoothed::new(0.0, sample_rate, WASH_BAND_SMOOTH_MS); 3],
            wash_duck: Ducker::new(sample_rate, WASH_DUCK_DB, DUCK_ATTACK_MS, WASH_DUCK_RELEASE_MS),
            wash_ref: 0.0,
            wash_scale: 1.0,
            wash_duck_scale: 1.0,
            bar_phase: 0.0,
            duck: Ducker::new(sample_rate, REVERB_DUCK_DB, DUCK_ATTACK_MS, DUCK_RELEASE_MS),
            band_sets: BeatFxKind::ALL.map(BeatFxKind::default_bands),
            params: BeatFxKind::ALL.map(|kind| {
                let mut values = [0.0; BEAT_FX_MAX_PARAMS];
                for (v, spec) in values.iter_mut().zip(beat_fx_params(kind)) {
                    *v = spec.default;
                }
                values
            }),
            bar_clock: BarClock { to_next: 4.0 * 0.5 * sample_rate as f64, per_bar: 4.0 * 0.5 * sample_rate as f64 },
            has_transport: false,
            pulse: pulse::PulseState::new(sample_rate),
            spiral: spiral::SpiralState::new(sample_rate),
            dub: dub::DubState::new(sample_rate),
            halo: halo::HaloState::new(sample_rate),
            ascend: ascend::AscendState::new(sample_rate),
            backwash: backwash::BackwashState::new(sample_rate),
            band_gain: [Smoothed::new(1.0, sample_rate, BAND_GAIN_SMOOTH_MS); 3],
            band_mix: Smoothed::new(0.0, sample_rate, BAND_BLEND_SMOOTH_MS),
            flushing: false,
            splitters: [BandSplitter::new(sample_rate); 2],
            band_dry: [vec![0.0; BAND_SCRATCH], vec![0.0; BAND_SCRATCH]],
            band_raw: [vec![0.0; BAND_SCRATCH], vec![0.0; BAND_SCRATCH]],
            #[cfg(test)]
            raw_core: false,
            shifters: std::array::from_fn(|_| PitchShifter::new((SHIFT_WINDOW_SECS * sample_rate) as usize)),
            lfo: Lfo::default(),
            phaser_state: [[0.0; PHASER_STAGES]; 2],
            phaser_feedback: [0.0; 2],
            sweep: [Svf::default(); 2],
            gate: 1.0,
            roll_phase: RollPhase::Armed,
            roll_start: 0,
            roll_len: 0,
            roll_count: 0,
            roll_pos: 0,
            roll_gate: Smoothed::new(0.0, sample_rate, 3.0),
        };
        fx.reset_state();
        fx.apply_band_targets(true);
        fx
    }

    pub fn kind(&self) -> BeatFxKind {
        self.kind
    }

    pub fn beats(&self) -> f32 {
        self.beats
    }

    pub fn percent(&self) -> f32 {
        self.percent
    }

    pub fn level(&self) -> f32 {
        self.level.target()
    }

    pub fn bpm(&self) -> f32 {
        self.bpm
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn is_idle(&self) -> bool {
        self.idle
    }

    pub fn is_swapping(&self) -> bool {
        self.swap.is_swapping() || self.flushing
    }

    pub fn bands(&self) -> FxBands {
        self.band_sets[self.kind.slot()]
    }

    pub fn band_sets(&self) -> [FxBands; BeatFxKind::ALL.len()] {
        self.band_sets
    }

    pub fn restore_band_sets(&mut self, sets: [FxBands; BeatFxKind::ALL.len()]) {
        self.band_sets = sets;
        self.apply_band_targets(self.idle);
    }

    /// Sets the bands the current kind acts on; the selection is remembered per kind.
    pub fn set_bands(&mut self, bands: FxBands) {
        self.band_sets[self.kind.slot()] = bands;
        self.apply_band_targets(self.idle);
    }

    /// Current values of the selected kind's sub-parameters.
    pub fn params(&self) -> [f32; BEAT_FX_MAX_PARAMS] {
        self.params[self.kind.slot()]
    }

    pub fn param_sets(&self) -> [[f32; BEAT_FX_MAX_PARAMS]; BeatFxKind::ALL.len()] {
        self.params
    }

    pub fn restore_param_sets(&mut self, sets: [[f32; BEAT_FX_MAX_PARAMS]; BeatFxKind::ALL.len()]) {
        self.params = sets;
    }

    /// Sets one sub-parameter of the selected kind; out-of-range values clamp, non-finite ones are ignored.
    pub fn set_param(&mut self, index: usize, value: f32) {
        if let Some(v) = beat_fx_params(self.kind).get(index).and_then(|spec| spec.sanitize(value)) {
            let changed = self.params[self.kind.slot()][index] != v;
            self.params[self.kind.slot()][index] = v;
            if changed && self.kind == BeatFxKind::Reverb && index == REVERB_VOICING_PARAM && !self.idle {
                self.swap.begin();
            }
        }
    }

    fn param(&self, index: usize) -> f32 {
        self.params[self.active.slot()][index]
    }

    /// Tells the unit where the target deck's bar lines are; without it bars follow the aligned grid phase.
    pub fn set_transport(&mut self, transport: Option<Transport>) {
        match transport {
            Some(t) if t.frames_per_bar > 1.0 => {
                self.bar_clock = BarClock { to_next: t.frames_to_downbeat.max(0.0), per_bar: t.frames_per_bar };
                self.has_transport = true;
            }
            _ => self.has_transport = false,
        }
    }

    fn sync_bar_clock_to_grid(&mut self) {
        let per_bar = 4.0 * self.beat_secs() as f64 * self.sample_rate as f64;
        self.bar_clock = BarClock { to_next: (1.0 - self.bar_phase) * per_bar, per_bar };
    }

    /// True while a throw, freeze or ascent hold is engaged.
    pub fn held(&self) -> bool {
        match self.active {
            BeatFxKind::DubTape => self.dub.holding(),
            BeatFxKind::Halo => self.halo.holding(),
            BeatFxKind::Ascend => self.ascend.holding(),
            kind if kind.is_echo_family() => self.echo_freeze_held,
            _ => false,
        }
    }

    pub fn trigger(&mut self, trigger: FxTrigger) {
        match (self.kind, trigger) {
            (BeatFxKind::DubTape, FxTrigger::ThrowOn) => self.dub.throw_held = self.enabled,
            (BeatFxKind::DubTape, FxTrigger::ThrowOff) => self.dub.throw_held = false,
            (BeatFxKind::DubTape, FxTrigger::FreezeOn) => self.dub.freeze_held = self.enabled,
            (BeatFxKind::DubTape, FxTrigger::FreezeOff) => self.dub.freeze_held = false,
            (BeatFxKind::Halo, FxTrigger::FreezeOn) => self.halo.freeze_held = self.enabled,
            (BeatFxKind::Halo, FxTrigger::FreezeOff) => self.halo.freeze_held = false,
            (kind, FxTrigger::FreezeOn) if kind.is_echo_family() => self.echo_freeze_held = self.enabled,
            (kind, FxTrigger::FreezeOff) if kind.is_echo_family() => self.echo_freeze_held = false,
            (BeatFxKind::Ascend, FxTrigger::Release) => self.set_enabled(false),
            (BeatFxKind::Backwash, FxTrigger::ThrowOn) => self.backwash.rearm(self.enabled),
            _ => {}
        }
    }

    fn apply_band_targets(&mut self, immediate: bool) {
        let gains = self.band_sets[self.kind.slot()].gains();
        for (g, target) in self.band_gain.iter_mut().zip(gains) {
            g.set(target);
            if immediate {
                g.reset(target);
            }
        }
    }

    pub fn set_beats(&mut self, beats: f32) {
        self.beats = beats.max(0.0625);
    }

    pub fn set_percent(&mut self, percent: f32) {
        if percent.is_finite() {
            self.percent = percent.clamp(PERCENT_MIN, PERCENT_MAX);
        }
    }

    pub fn set_bpm(&mut self, bpm: f32) {
        self.bpm = bpm.clamp(30.0, 400.0);
    }

    pub fn set_level(&mut self, level: f32) {
        let level = level.clamp(0.0, 1.0);
        self.level.set(level);
        if self.idle {
            self.level.reset(level);
        }
    }

    pub fn set_kind(&mut self, kind: BeatFxKind) {
        self.kind = kind;
        self.apply_band_targets(self.idle);
        if self.idle {
            self.active = kind;
            self.reset_state();
        } else if kind != self.active || self.swap.is_swapping() {
            self.swap.begin();
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        if enabled == self.enabled {
            return;
        }
        self.enabled = enabled;
        if enabled {
            if self.idle {
                self.idle = false;
                self.active = self.kind;
                self.start_band_path();
            }
            self.reset_state();
            self.send.set(1.0);
            self.arm_active_kind();
        } else if self.active == BeatFxKind::Ascend && self.ascend.is_running() {
            self.ascend.request_release(self.param(2) >= 0.5);
            self.tail_left = self.bar_clock.per_bar + ascend::TAIL_SECS * self.sample_rate as f64;
        } else {
            self.send.set(0.0);
            self.tail_left = self.tail_samples();
            self.dub.release_holds();
            self.halo.release_holds();
            self.echo_freeze_held = false;
        }
    }

    fn arm_active_kind(&mut self) {
        match self.active {
            BeatFxKind::Ascend => self.ascend.start(self.param(0), self.param(1) < 0.5),
            BeatFxKind::Backwash => self.backwash.start(),
            _ => {}
        }
    }

    fn start_band_path(&mut self) {
        self.splitters = [BandSplitter::new(self.sample_rate); 2];
        self.band_mix.reset(0.0);
        self.apply_band_targets(true);
    }

    /// Aligns the LFO / gate phase and the 1/16 grid to a beat position (in beats) of the synced deck.
    pub fn align_phase(&mut self, beat_position: f64) {
        self.lfo.set_cycles(beat_position / self.beats as f64);
        self.bar_phase = (beat_position / 4.0).rem_euclid(1.0);
        self.grid16 = (beat_position * 4.0).rem_euclid(1.0) as f32;
    }

    /// Ramps the wet path out and clears all state, e.g. when the unit moves to another bus.
    pub fn flush(&mut self) {
        if !self.idle {
            self.flushing = true;
        }
    }

    fn beat_secs(&self) -> f32 {
        60.0 / self.bpm
    }

    fn delay_target(&self) -> f32 {
        let max = MAX_DELAY_SECS * self.sample_rate;
        let mut d = self.beats * self.beat_secs() * self.sample_rate;
        while d > max {
            d *= 0.5;
        }
        d.max(2.0)
    }

    fn lfo_increment(&self) -> f32 {
        1.0 / (self.beats * self.beat_secs() * self.sample_rate).max(2.0)
    }

    fn tail_samples(&self) -> f64 {
        let sr = self.sample_rate as f64;
        match self.active {
            BeatFxKind::Spiral => {
                let fb = (self.param(0) * 0.01).clamp(0.05, ECHO_FEEDBACK_MAX) as f64;
                let repeats = TAIL_FLOOR.ln() / fb.ln();
                ((self.current_delay() as f64) * (repeats + 1.0)).min(MAX_TAIL_SECS * sr)
            }
            kind if kind.is_echo_family() => {
                let fb = echo_feedback_for_knob(self.param(0)).clamp(0.05, ECHO_FEEDBACK_MAX) as f64;
                let repeats = TAIL_FLOOR.ln() / fb.ln();
                ((self.current_delay().max(self.delay_target()) as f64) * (repeats + 1.0)).min(MAX_TAIL_SECS * sr)
            }
            BeatFxKind::Reverb => (voiced_decay_rt60(self.param(3) < 0.5, self.param(2)) as f64 * 1.4).min(MAX_TAIL_SECS) * sr,
            BeatFxKind::Halo => reverb_rt60_for_percent(self.percent) as f64 * 1.4 * sr,
            BeatFxKind::PulseVerb => reverb_rt60_for_percent(self.param(0)) as f64 * 1.4 * sr,
            BeatFxKind::DubTape => {
                let fb = (self.param(0) * 0.01).min(dub::RELEASE_FEEDBACK) as f64;
                let repeats = TAIL_FLOOR.ln() / fb.max(0.05).ln();
                ((self.current_delay().max(self.delay_target()) as f64) * (repeats + 1.0)).min(MAX_TAIL_SECS * sr)
            }
            _ => DECLICK_TAIL_SECS * sr,
        }
    }

    fn current_delay(&self) -> f32 {
        if self.active == BeatFxKind::Spiral { self.spiral.delay() } else { self.taps.delay().max(self.taps.target_delay()) }
    }

    fn reset_state(&mut self) {
        self.ring.reset();
        self.reverb.reset();
        self.reverb.set_fixed_output_gain(None);
        self.rt60_applied = 0.0;
        self.room_step = 0;
        self.room_step_left = 0;
        self.classic_lp.iter_mut().chain(&mut self.classic_hp).for_each(OnePole::reset);
        self.loop_lp.iter_mut().for_each(LowPass::reset);
        self.loop_hp.iter_mut().for_each(HighPass::reset);
        self.reverb_in_hp.iter_mut().for_each(HighPass::reset);
        self.reverb_out_hp.iter_mut().for_each(HighPass::reset);
        self.reverb_shelf.iter_mut().for_each(HighShelf::reset);
        self.out_cut.reset(REVERB_OUT_HP_MIN_HZ);
        self.wash_hp.iter_mut().for_each(HighPass::reset);
        self.wash_cut.reset(WASH_HP_MIN_HZ);
        self.wash_splitters = [BandSplitter::new(self.sample_rate); 2];
        for (g, target) in self.wash_band_gain.iter_mut().zip(self.band_sets[self.kind.slot()].gains()) {
            g.reset(target);
        }
        self.wash_duck.reset();
        self.wash_ref = 0.0;
        self.duck.reset();
        self.shifters.iter_mut().for_each(PitchShifter::reset);
        self.phaser_state = [[0.0; PHASER_STAGES]; 2];
        self.phaser_feedback = [0.0; 2];
        self.sweep.iter_mut().for_each(Svf::reset);
        self.gate = 1.0;
        self.roll_phase = RollPhase::Armed;
        self.roll_gate.reset(0.0);
        self.delay_snap = true;
        self.echo_freeze_held = false;
        self.echo_feed.reset(1.0);
        self.echo_filter.reset();
        self.lfo.phase = 0.0;
        self.pulse.reset();
        self.spiral.reset();
        self.dub.reset();
        self.halo.reset();
        self.ascend.reset();
        self.backwash.reset();
        let gains = self.band_sets[self.kind.slot()].gains();
        self.pulse.noise_gains_settle(gains);
        self.ascend.settle_band_gains(gains);
        self.send.reset(0.0);
        if self.enabled {
            self.send.set(1.0);
        }
    }

    pub fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        if self.idle {
            return;
        }
        let n = left.len().min(right.len());
        let mut start = 0;
        while start < n {
            let end = (start + BAND_SCRATCH).min(n);
            self.process_chunk(&mut left[start..end], &mut right[start..end]);
            start = end;
        }
        if !self.enabled {
            self.tail_left -= n as f64;
            if self.tail_left <= 0.0 && !self.swap.is_swapping() && self.band_mix.value() < 1e-3 {
                self.idle = true;
                self.reset_state();
                self.swap.reset_open();
            }
        }
    }

    fn process_chunk(&mut self, l: &mut [f32], r: &mut [f32]) {
        #[cfg(test)]
        if self.raw_core {
            if self.flushing && !self.swap.is_swapping() {
                self.swap.begin();
            }
            self.run_core_tracked(l, r);
            return;
        }
        let closing = !self.enabled && self.tail_left < BAND_BLEND_OUT_SECS * self.sample_rate as f64;
        self.band_mix.set(if self.flushing || closing { 0.0 } else { 1.0 });
        if self.flushing && !self.swap.is_swapping() && self.band_mix.value() < 1e-3 {
            self.swap.begin();
        }
        let n = l.len();
        for i in 0..n {
            let gains = [self.band_gain[0].next(), self.band_gain[1].next(), self.band_gain[2].next()];
            for (ch, buf) in [&mut *l, &mut *r].into_iter().enumerate() {
                let x = buf[i];
                let bands = self.splitters[ch].split(x);
                let mut selected = 0.0;
                let mut dry = 0.0;
                for (b, g) in bands.iter().zip(gains) {
                    selected += b * g;
                    dry += b * (1.0 - g);
                }
                self.band_raw[ch][i] = x;
                self.band_dry[ch][i] = dry;
                buf[i] = selected;
            }
        }
        self.run_core_tracked(l, r);
        for i in 0..n {
            let mix = self.band_mix.next();
            for (ch, buf) in [&mut *l, &mut *r].into_iter().enumerate() {
                let fx = buf[i] + self.band_dry[ch][i];
                let out = self.band_raw[ch][i] + (fx - self.band_raw[ch][i]) * mix;
                buf[i] = out;
            }
        }
    }

    fn run_core_tracked(&mut self, l: &mut [f32], r: &mut [f32]) {
        if !self.has_transport {
            self.sync_bar_clock_to_grid();
        }
        self.run_core(l, r);
        self.bar_clock.advance(l.len());
        if matches!(self.active, BeatFxKind::Reverb | BeatFxKind::PulseVerb | BeatFxKind::Ascend) {
            let per_bar = 4.0 * self.beat_secs() as f64 * self.sample_rate as f64;
            self.bar_phase = (self.bar_phase + l.len() as f64 / per_bar).rem_euclid(1.0);
        }
    }

    fn run_core(&mut self, l: &mut [f32], r: &mut [f32]) {
        match self.active {
            BeatFxKind::Delay
            | BeatFxKind::Echo
            | BeatFxKind::UpEcho
            | BeatFxKind::DownEcho
            | BeatFxKind::PingPong => self.run_echo(l, r),
            BeatFxKind::Spiral => self.run_spiral(l, r),
            BeatFxKind::Reverb => self.run_reverb(l, r),
            BeatFxKind::Flanger => self.run_flanger(l, r),
            BeatFxKind::Phaser => self.run_phaser(l, r),
            BeatFxKind::Roll => self.run_roll(l, r),
            BeatFxKind::Trans => self.run_trans(l, r),
            BeatFxKind::FilterSweep => self.run_filter_sweep(l, r),
            BeatFxKind::PulseVerb => self.run_pulse_verb(l, r),
            BeatFxKind::DubTape => self.run_dub_tape(l, r),
            BeatFxKind::Halo => self.run_halo(l, r),
            BeatFxKind::Ascend => self.run_ascend(l, r),
            BeatFxKind::Backwash => self.run_backwash(l, r),
        }
        for v in l.iter_mut().chain(r.iter_mut()) {
            *v = if v.is_finite() { v.clamp(-OUTPUT_GUARD, OUTPUT_GUARD) } else { 0.0 };
        }
        if self.swap.ready() {
            self.active = self.kind;
            self.reset_state();
            self.swap.finish();
            self.flushing = false;
            if self.enabled {
                self.arm_active_kind();
            }
        }
    }

    fn run_echo(&mut self, l: &mut [f32], r: &mut [f32]) {
        let style = echo_style(self.active);
        let frozen = self.echo_freeze_held && self.enabled;
        for ch in 0..2 {
            self.loop_hp[ch].set(ECHO_DC_BLOCK_HZ, self.sample_rate);
        }
        self.echo_filter.set(self.param(1) * 0.01);
        self.echo_fb.set(if frozen { 1.0 } else { echo_feedback_for_knob(self.param(0)) });
        self.echo_feed.set(if frozen { 0.0 } else { 1.0 });
        if style.ping_pong {
            let ratio = PING_PONG_RATIOS[(self.param(2).round() as usize).min(PING_PONG_RATIOS.len() - 1)];
            self.taps.set_right_ratio(ratio);
        }
        let latency = if style.shift_ratio.is_some() { self.shifters[0].latency() } else { 0.0 };
        let target = (self.delay_target() - latency).max(2.0);
        if self.delay_snap {
            self.taps.snap(target);
            self.delay_snap = false;
            self.echo_fb.reset(self.echo_fb.target());
            self.echo_feed.reset(self.echo_feed.target());
        }
        let shift = style.shift_ratio.map(|ratio| if frozen { 1.0 } else { ratio });
        let grid_inc = 4.0 / (self.beat_secs() * self.sample_rate);
        let right_offset = style.right_offset_ms * self.sample_rate / 1000.0;
        for i in 0..l.len() {
            self.grid16 += grid_inc;
            let boundary = self.grid16 >= 1.0;
            if boundary {
                self.grid16 -= 1.0;
            }
            let lvl = self.level.next();
            let feedback = self.echo_fb.next();
            let feed = self.echo_feed.next();
            let send_raw = self.send.next();
            let gate = self.swap.next();
            let send = send_raw * feed;
            let (dry_law, wet_law) = beat_mix(lvl);
            let wet = wet_law * gate * ECHO_WET_GAIN;
            self.taps.follow(target, boundary, &self.ring);
            let (yl, yr) = self.taps.read(&self.ring, right_offset);
            let (xl, xr) = (l[i], r[i]);
            let (fl, fr) = (self.loop_hp[0].process(yl), self.loop_hp[1].process(yr));
            let (ol, or) = self.echo_filter.process(yl, yr);
            let (mut wl, mut wr) = if style.ping_pong {
                let mono = 0.5 * (xl + xr) * send;
                (mono + feedback * fr, feedback * fl)
            } else {
                (xl * send + feedback * fl, xr * send + feedback * fr)
            };
            if let Some(ratio) = shift {
                wl = self.shifters[0].process(wl, ratio);
                wr = self.shifters[1].process(wr, ratio);
            }
            self.ring.write(soft_limit(wl), soft_limit(wr));
            let dry = gated_dry(dry_law, send_raw * gate);
            l[i] = xl * dry + ol * wet;
            r[i] = xr * dry + or * wet;
        }
    }

    #[inline]
    fn loop_stage(&mut self, ch: usize, y: f32, drive: f32) -> f32 {
        let filtered = self.loop_hp[ch].process(self.loop_lp[ch].process(y));
        (drive * filtered).tanh() / drive
    }

    fn run_reverb(&mut self, l: &mut [f32], r: &mut [f32]) {
        let classic = self.param(3) < 0.5;
        let t = percent_position(self.percent);
        self.rt60.set(voiced_decay_rt60(classic, self.param(2)));
        if classic {
            self.reverb.set_fixed_output_gain(Some(CLASSIC_OUTPUT_GAIN));
        } else {
            self.room_step = 0;
            self.reverb.set_fixed_output_gain(None);
            self.reverb.set_size(tension_room_scale(self.percent));
            self.reverb.set_glide_rate(reverb_bend_rate(self.param(4)));
        }
        self.reverb.set_output_floor(REVERB_OUTPUT_FLOOR);
        let noise_mul = if classic { 0.0 } else { self.param(1) / 50.0 };
        if classic {
            self.reverb.set_predelay_ms(0.0);
            self.reverb.set_hf_ratio(1.0);
            self.reverb.set_modulation_ms(0.0);
        } else {
            self.reverb.set_predelay_ms(20.0 + 18.0 * t);
            self.reverb.set_hf_ratio((REVERB_HF_RATIO * centered(self.param(0), 0.6, 1.0, 1.5)).min(1.0));
            self.reverb.set_modulation_ms(reverb_modulation_ms(t));
        }
        let in_hp_hz = if classic { CLASSIC_IN_HP_HZ } else { REVERB_IN_HP_MIN_HZ + REVERB_IN_HP_SPAN_HZ * t };
        for ch in 0..2 {
            self.reverb_in_hp[ch].set(in_hp_hz, self.sample_rate);
            self.classic_hp[ch].set_cutoff(CLASSIC_IN_HP_HZ, self.sample_rate);
            self.classic_lp[ch].set_cutoff(CLASSIC_IN_LP_HZ, self.sample_rate);
        }
        let wet_gain = if classic { CLASSIC_WET_GAIN * classic_decay_trim(self.param(2)) } else { TENSION_WET_GAIN };
        let follow = chase_coeff(self.sample_rate / REVERB_CHUNK as f32, REVERB_TONE_CHASE_SECS);
        let ref_coeff = 1.0 - (-1.0 / (WASH_REF_SECS * self.sample_rate)).exp();
        let bar_inc = 1.0 / (4.0 * self.beat_secs() as f64 * self.sample_rate as f64);
        let wash_size = WASH_SIZE_FLOOR + (1.0 - WASH_SIZE_FLOOR) * t;
        let (mut wash_base, mut swell) = (0.0f32, 1.0f32);
        for i in 0..l.len() {
            if i % REVERB_CHUNK == 0 {
                if classic {
                    self.step_room(REVERB_CHUNK);
                }
                let rt = self.rt60.next();
                if (rt - self.rt60_applied).abs() > 1e-4 {
                    self.reverb.set_rt60(rt);
                    self.rt60_applied = rt;
                }
                let depth = self.level.value();
                let (out_target, wash_hz, shelf_db) = if classic {
                    (CLASSIC_OUT_HP_HZ, WASH_HP_MIN_HZ, 0.0)
                } else {
                    (
                        REVERB_OUT_HP_MIN_HZ * (REVERB_OUT_HP_MAX_HZ / REVERB_OUT_HP_MIN_HZ).powf(depth),
                        WASH_HP_MIN_HZ + (WASH_HP_MAX_HZ - WASH_HP_MIN_HZ) * depth,
                        REVERB_SHELF_MAX_DB * depth,
                    )
                };
                let out_hz = self.out_cut.chase(out_target, follow);
                let wash_hz = self.wash_cut.chase(wash_hz, follow);
                for ch in 0..2 {
                    self.reverb_out_hp[ch].set(out_hz, self.sample_rate);
                    self.wash_hp[ch].set(wash_hz, self.sample_rate);
                }
                if (shelf_db - self.shelf_db).abs() > 0.02 {
                    self.shelf_db = shelf_db;
                    self.reverb_shelf.iter_mut().for_each(|s| s.set(REVERB_SHELF_HZ, shelf_db, self.sample_rate));
                }
                for (g, target) in self.wash_band_gain.iter_mut().zip(self.band_gain.iter()) {
                    g.set(target.target());
                }
                let reference = self.wash_ref.sqrt().clamp(WASH_REF_FLOOR, WASH_REF_CEIL);
                wash_base = reference * WASH_PEAK * wash_size * self.wash_scale * noise_mul;
                let bar = (self.bar_phase + (i as f64) * bar_inc).rem_euclid(1.0);
                let breath = 0.5 + 0.5 * (std::f64::consts::TAU * bar).cos() as f32;
                swell = if classic { 1.0 } else { 10f32.powf(REVERB_SWELL_DB * depth * (breath - 0.5) / 20.0) };
            }
            let send = self.send.next();
            let gate = self.swap.next();
            let depth = self.level.next();
            let (dry_law, wet_law) = beat_mix(depth);
            let (xl, xr) = (l[i], r[i]);
            self.wash_ref += (0.5 * (xl * xl + xr * xr) - self.wash_ref) * ref_coeff;
            let duck = if classic { 1.0 } else { 1.0 + (self.duck.process(xl, xr) - 1.0) * dry_law };
            let wash_duck = 1.0 + (self.wash_duck.process(xl, xr) - 1.0) * self.wash_duck_scale;
            let wash = wash_base * depth * depth * send * gate * wash_duck;
            let mut input = if classic {
                [self.classic_hp[0].hp(self.classic_lp[0].lp(xl * send)), self.classic_hp[1].hp(self.classic_lp[1].lp(xr * send))]
            } else {
                [self.reverb_in_hp[0].process(xl * send), self.reverb_in_hp[1].process(xr * send)]
            };
            if wash > 0.0 {
                let gains = [self.wash_band_gain[0].next(), self.wash_band_gain[1].next(), self.wash_band_gain[2].next()];
                for (ch, v) in input.iter_mut().enumerate() {
                    let source = std::f32::consts::FRAC_1_SQRT_2 * (WHITE_UNIT_GAIN * self.wash_white[ch].next() + self.wash_pink[ch].next());
                    let bands = self.wash_splitters[ch].split(self.wash_hp[ch].process(source));
                    *v += wash * (bands[0] * gains[0] + bands[1] * gains[1] + bands[2] * gains[2]);
                }
            }
            let (wl, wr) = self.reverb.process(input[0], input[1]);
            let (wl, wr) = if classic {
                (wl, wr)
            } else {
                (self.reverb_shelf[0].process(self.reverb_out_hp[0].process(wl)), self.reverb_shelf[1].process(self.reverb_out_hp[1].process(wr)))
            };
            let wet = wet_law * wet_gain * duck * swell * gate;
            let dry = gated_dry(dry_law, send * gate);
            l[i] = xl * dry + soft_limit(wl) * wet;
            r[i] = xr * dry + soft_limit(wr) * wet;
        }
    }

    /// Walks the ROOM SIZE one integer step at a time toward the knob, each step a straight-line glide of 20..40 ms.
    fn step_room(&mut self, elapsed: usize) {
        let target = self.percent.round() as i32;
        if self.room_step == 0 {
            self.room_step = target;
            self.room_step_ms = ROOM_STEP_MAX_MS;
            self.room_step_left = 0;
            self.reverb.ramp_size(reverb_room_scale(target), 0);
            return;
        }
        self.room_step_left = self.room_step_left.saturating_sub(elapsed);
        if self.room_step_left > 0 || self.room_step == target {
            return;
        }
        self.room_step_ms = room_step_ms(self.room_step_ms, self.room_step, target);
        self.room_step += (target - self.room_step).signum();
        let samples = ((self.room_step_ms * 0.001 * self.sample_rate) as usize).max(1);
        self.reverb.ramp_size(reverb_room_scale(self.room_step), samples);
        self.room_step_left = samples;
    }

    fn run_flanger(&mut self, l: &mut [f32], r: &mut [f32]) {
        let inc = self.lfo_increment();
        let ms = self.sample_rate / 1000.0;
        let feedback = centered(self.param(0), 0.0, FLANGER_FEEDBACK, 0.85);
        for i in 0..l.len() {
            let m = 0.5 * self.level.next() * self.send.next() * self.swap.next();
            let dl = (FLANGER_MIN_MS + FLANGER_RANGE_MS * (0.5 + 0.5 * self.lfo.sine_at(0.0))) * ms;
            let dr = (FLANGER_MIN_MS + FLANGER_RANGE_MS * (0.5 + 0.5 * self.lfo.sine_at(0.25))) * ms;
            let yl = self.ring.read_l(dl);
            let yr = self.ring.read_r(dr);
            let (xl, xr) = (l[i], r[i]);
            self.ring.write(soft_limit(xl + feedback * yl), soft_limit(xr + feedback * yr));
            l[i] = xl * (1.0 - m) + yl * m;
            r[i] = xr * (1.0 - m) + yr * m;
            self.lfo.advance(inc);
        }
    }

    fn run_phaser(&mut self, l: &mut [f32], r: &mut [f32]) {
        let inc = self.lfo_increment();
        let mut coeff = [0f32; 2];
        let feedback = centered(self.param(0), 0.0, PHASER_FEEDBACK, 0.9);
        let ratio = PHASER_MAX_HZ / PHASER_MIN_HZ;
        for i in 0..l.len() {
            if i % 8 == 0 {
                for (c, offset) in coeff.iter_mut().zip([0.0, 0.25]) {
                    let fc = PHASER_MIN_HZ * ratio.powf(0.5 + 0.5 * self.lfo.sine_at(offset));
                    let t = (std::f32::consts::PI * fc / self.sample_rate).tan();
                    *c = (t - 1.0) / (t + 1.0);
                }
            }
            let m = 0.5 * self.level.next() * self.send.next() * self.swap.next();
            let x = [l[i], r[i]];
            let mut y = [0f32; 2];
            for ch in 0..2 {
                let a = coeff[ch];
                let mut u = x[ch] + feedback * self.phaser_feedback[ch];
                for z in &mut self.phaser_state[ch] {
                    let out = a * u + *z;
                    *z = flush(u - a * out);
                    u = out;
                }
                self.phaser_feedback[ch] = flush(u);
                y[ch] = u;
            }
            l[i] = x[0] * (1.0 - m) + soft_limit(y[0]) * m;
            r[i] = x[1] * (1.0 - m) + soft_limit(y[1]) * m;
            self.lfo.advance(inc);
        }
    }

    fn run_trans(&mut self, l: &mut [f32], r: &mut [f32]) {
        let inc = self.lfo_increment();
        let smooth = 1.0 - (-1.0 / (GATE_SMOOTH_SECS * self.sample_rate)).exp();
        for i in 0..l.len() {
            let depth = self.level.next() * self.send.next() * self.swap.next();
            let target = if self.lfo.phase < 0.5 { 1.0 } else { 1.0 - depth };
            self.gate += smooth * (target - self.gate);
            l[i] *= self.gate;
            r[i] *= self.gate;
            self.lfo.advance(inc);
        }
    }

    fn run_filter_sweep(&mut self, l: &mut [f32], r: &mut [f32]) {
        let inc = self.lfo_increment();
        let ratio = SWEEP_MIN_HZ / SWEEP_MAX_HZ;
        let q = centered(self.param(0), SWEEP_Q_MIN, SWEEP_Q, SWEEP_Q_MAX);
        for i in 0..l.len() {
            let lvl = self.level.next();
            if i % COEFF_UPDATE == 0 {
                let u = 0.5 + 0.5 * self.lfo.sine_at(0.0);
                let reach = 0.35 + 0.65 * lvl;
                let fc = SWEEP_MAX_HZ * ratio.powf(u * reach);
                for s in &mut self.sweep {
                    s.set(fc, q, self.sample_rate);
                }
            }
            let m = lvl * self.send.next() * self.swap.next();
            let (xl, xr) = (l[i], r[i]);
            let wl = self.sweep[0].process(xl).low;
            let wr = self.sweep[1].process(xr).low;
            l[i] = xl * (1.0 - m) + wl * m;
            r[i] = xr * (1.0 - m) + wr * m;
            self.lfo.advance(inc);
        }
    }

    fn run_roll(&mut self, l: &mut [f32], r: &mut [f32]) {
        if self.roll_phase == RollPhase::Armed {
            self.roll_len = (self.delay_target().round() as usize).max(ROLL_CROSSFADE * 2);
            self.roll_start = self.ring.position();
            self.roll_count = 0;
            self.roll_pos = 0;
            self.roll_phase = RollPhase::Recording;
        }
        for i in 0..l.len() {
            let amount = self.level.next() * self.send.next() * self.swap.next();
            let gate = self.roll_gate.next();
            let (xl, xr) = (l[i], r[i]);
            if self.roll_phase == RollPhase::Recording {
                self.ring.write(xl, xr);
                self.roll_count += 1;
                if self.roll_count >= self.roll_len + ROLL_CROSSFADE {
                    self.roll_phase = RollPhase::Looping;
                    self.roll_gate.set(1.0);
                }
                continue;
            }
            let (a, b) = self.ring.peek(self.roll_start + self.roll_pos);
            let (ll, lr) = if self.roll_pos < ROLL_CROSSFADE {
                let w = self.roll_pos as f32 / ROLL_CROSSFADE as f32;
                let (c, d) = self.ring.peek(self.roll_start + self.roll_len + self.roll_pos);
                (a * w + c * (1.0 - w), b * w + d * (1.0 - w))
            } else {
                (a, b)
            };
            self.roll_pos += 1;
            if self.roll_pos >= self.roll_len {
                self.roll_pos = 0;
            }
            let m = amount * gate;
            l[i] = xl * (1.0 - m) + ll * m;
            r[i] = xr * (1.0 - m) + lr * m;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    use super::super::testkit::{band_psd, cents_deviation, crossing_hz, db, max_step, centroid_hz, rt60_at, semitones, tone_amplitude};

    const SR: f32 = 48_000.0;
    const BLOCK: usize = 256;
    const IMPULSE: f32 = 0.5;

    fn unit(kind: BeatFxKind, beats: f32, level: f32) -> BeatFx {
        let mut fx = BeatFx::new(SR);
        fx.raw_core = true;
        fx.set_bpm(120.0);
        fx.set_kind(kind);
        fx.set_beats(beats);
        fx.set_level(level);
        fx
    }

    fn first_repeat_gain(_kind: BeatFxKind, level: f32) -> f32 {
        beat_mix(level).1 * ECHO_WET_GAIN
    }

    fn run(fx: &mut BeatFx, l: &[f32], r: &[f32]) -> (Vec<f32>, Vec<f32>) {
        let (mut ol, mut or) = (l.to_vec(), r.to_vec());
        for (cl, cr) in ol.chunks_mut(BLOCK).zip(or.chunks_mut(BLOCK)) {
            fx.process(cl, cr);
        }
        (ol, or)
    }

    fn impulse(len: usize, at: usize) -> Vec<f32> {
        let mut v = vec![0.0; len];
        v[at] = IMPULSE;
        v
    }

    fn rms(v: &[f32]) -> f32 {
        (v.iter().map(|x| x * x).sum::<f32>() / v.len().max(1) as f32).sqrt()
    }

    fn peak(v: &[f32]) -> f32 {
        v.iter().fold(0.0, |m, x| m.max(x.abs()))
    }

    fn noise(len: usize, seed: u32) -> Vec<f32> {
        let mut n = super::super::util::Noise::new(seed);
        (0..len).map(|_| n.next()).collect()
    }

    fn sine(len: usize, hz: f32, amp: f32) -> Vec<f32> {
        (0..len).map(|i| amp * (TAU * hz * i as f32 / SR).sin()).collect()
    }

    #[test]
    fn echo_repeat_lands_at_beat_offset_and_decays() {
        let mut fx = unit(BeatFxKind::Echo, 0.5, 1.0);
        fx.set_enabled(true);
        let delay = 12_000;
        let at = 4_000;
        let x = impulse(80_000, at);
        let (out, _) = run(&mut fx, &x, &x);
        let first = first_repeat_gain(BeatFxKind::Echo, 1.0) * IMPULSE;
        assert!((out[at + delay] - first).abs() < 1e-3, "first repeat {} want {first}", out[at + delay]);
        assert!(out[at + delay - 1].abs() < 1e-6 && out[at + delay + 1].abs() < 1e-6);
        let second = out[at + 2 * delay];
        assert!(second > 0.0 && second < out[at + delay], "second {second}");
        let third = out[at + 3 * delay];
        assert!(third < second);
    }

    #[test]
    fn delay_second_repeat_is_softer_than_the_first() {
        let mut fx = unit(BeatFxKind::Delay, 1.0, 1.0);
        fx.set_enabled(true);
        let delay = 24_000;
        let x = impulse(120_000, 4_000);
        let (out, _) = run(&mut fx, &x, &x);
        assert!((out[4_000 + delay] - first_repeat_gain(BeatFxKind::Delay, 1.0) * IMPULSE).abs() < 1e-3);
        let second = out[4_000 + 2 * delay];
        assert!(second > 0.1 * out[4_000 + delay] && second < 0.9 * out[4_000 + delay], "second {second}");
    }

    #[test]
    fn echo_tail_rings_after_disable_then_dies() {
        let mut fx = unit(BeatFxKind::Echo, 0.25, 1.0);
        fx.set_enabled(true);
        let x = noise(24_000, 3);
        let _ = run(&mut fx, &x, &x);
        fx.set_enabled(false);
        let silence = vec![0.0; 48_000 * 10];
        let (out, _) = run(&mut fx, &silence, &silence);
        let early = rms(&out[600..6_000]);
        let later = rms(&out[36_000..42_000]);
        assert!(early > 0.01, "tail audible after disable: {early}");
        assert!(later < early, "decays");
        assert!(out.iter().all(|v| v.is_finite()));
        assert!(fx.is_idle(), "unit goes idle once the tail is gone");
    }

    fn zero_crossing_hz(x: &[f32]) -> f32 {
        let crossings = x.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
        crossings as f32 * SR / x.len() as f32
    }

    fn repeat_pitches(kind: BeatFxKind) -> Vec<f32> {
        let mut fx = unit(kind, 2.0, 1.0);
        fx.set_enabled(true);
        let (start, burst, delay) = (4_000, 6_000, 48_000);
        let mut x = vec![0.0; start + 4 * delay];
        for (i, v) in x[start..start + burst].iter_mut().enumerate() {
            *v = 0.5 * (TAU * 1_000.0 * i as f32 / SR).sin();
        }
        let (out, _) = run(&mut fx, &x, &x);
        (1..=3).map(|k| zero_crossing_hz(&out[start + k * delay + 1_500..start + k * delay + 4_500])).collect()
    }

    #[test]
    fn up_echo_repeats_rise_one_semitone_each() {
        let hz = repeat_pitches(BeatFxKind::UpEcho);
        let step = 2f32.powf(1.0 / 12.0);
        for (k, f) in hz.iter().enumerate() {
            let want = 1_000.0 * step.powi(k as i32 + 1);
            assert!((f / want - 1.0).abs() < 0.05, "repeat {}: {f} Hz, want about {want}", k + 1);
        }
        assert!(hz[0] < hz[1] && hz[1] < hz[2]);
    }

    #[test]
    fn down_echo_repeats_fall_one_semitone_each() {
        let hz = repeat_pitches(BeatFxKind::DownEcho);
        let step = 2f32.powf(-1.0 / 12.0);
        for (k, f) in hz.iter().enumerate() {
            let want = 1_000.0 * step.powi(k as i32 + 1);
            assert!((f / want - 1.0).abs() < 0.05, "repeat {}: {f} Hz, want about {want}", k + 1);
        }
        assert!(hz[0] > hz[1] && hz[1] > hz[2]);
    }

    #[test]
    fn up_down_and_spiral_tails_ring_after_disable_then_die() {
        for kind in [BeatFxKind::UpEcho, BeatFxKind::DownEcho, BeatFxKind::Spiral] {
            let mut fx = unit(kind, 0.25, 0.5);
            fx.set_enabled(true);
            let x = noise(24_000, 3);
            let _ = run(&mut fx, &x, &x);
            fx.set_enabled(false);
            let silence = vec![0.0; 48_000 * 8];
            let (out, _) = run(&mut fx, &silence, &silence);
            let early = rms(&out[600..9_000]);
            let late = rms(&out[330_000..350_000]);
            assert!(early > 0.01, "{kind:?} tail audible: {early}");
            assert!(late < early * 0.5, "{kind:?} decays: {early} -> {late}");
            assert!(out.iter().all(|v| v.is_finite()));
            assert!(fx.is_idle(), "{kind:?} goes idle after the tail");
        }
    }

    #[test]
    fn spiral_stays_bounded_at_max_feedback_for_thirty_seconds() {
        for (name, x) in [
            ("noise", noise(48_000 * 30, 99)),
            ("square", (0..48_000 * 30).map(|i| if (i / 55) % 2 == 0 { 1.0 } else { -1.0 }).collect()),
            ("sine", sine(48_000 * 30, 330.0, 1.0)),
        ] {
            let mut fx = unit(BeatFxKind::Spiral, 0.5, 1.0);
            fx.set_enabled(true);
            let mut worst: f32 = 0.0;
            let (mut l, mut r) = (x.clone(), x.clone());
            for (b, (cl, cr)) in l.chunks_mut(BLOCK).zip(r.chunks_mut(BLOCK)).enumerate() {
                if b % 400 == 0 {
                    fx.set_beats([0.5, 2.0, 0.125, 16.0, 1.0][(b / 400) % 5]);
                }
                fx.process(cl, cr);
                for v in cl.iter().chain(cr.iter()) {
                    assert!(v.is_finite(), "{name}");
                    worst = worst.max(v.abs());
                }
            }
            assert!(worst <= 2.0, "{name}: peak {worst}");
        }
    }

    #[test]
    fn spiral_beats_change_glides_the_pitch_without_clicking() {
        let mut fx = unit(BeatFxKind::Spiral, 0.5, 0.6);
        fx.set_enabled(true);
        let x = sine(48_000 * 12, 220.0, 0.3);
        let (mut l, mut r) = (x.clone(), x.clone());
        let change_block = 48_000 * 4 / BLOCK;
        let mut worst_step: f32 = 0.0;
        let mut prev = None;
        for (b, (cl, cr)) in l.chunks_mut(BLOCK).zip(r.chunks_mut(BLOCK)).enumerate() {
            if b == change_block {
                fx.set_beats(1.0);
            }
            fx.process(cl, cr);
            if b > change_block - 40 {
                for v in cl.iter() {
                    worst_step = worst_step.max(prev.map_or(0.0, |p: f32| (v - p).abs()));
                    prev = Some(*v);
                }
            }
        }
        let wet: Vec<f32> = l.iter().zip(&x).map(|(o, d)| o - d).collect();
        let at = 48_000 * 4;
        let before = crossing_hz(&wet[at - 48_000..at - 4_000]);
        let during = crossing_hz(&wet[at + 3 * 48_000..at + 4 * 48_000]);
        eprintln!("spiral 1/2 -> 1 beat: {before:.1} Hz -> {during:.1} Hz, step {worst_step:.4}");
        assert!((0.95..0.985).contains(&(during / before)), "the echo bends down while the delay lengthens: {before} -> {during} Hz");
        assert!(worst_step < 0.08, "no click: {worst_step}");
    }

    #[test]
    fn ping_pong_alternates_channels() {
        let mut fx = unit(BeatFxKind::PingPong, 0.25, 1.0);
        fx.set_param(2, 3.0);
        fx.set_enabled(true);
        let delay = 6_000;
        let x = impulse(60_000, 4_000);
        let (l, r) = run(&mut fx, &x, &x);
        let first = first_repeat_gain(BeatFxKind::PingPong, 1.0) * IMPULSE;
        let near = |v: &[f32], n: usize| peak(&v[4_000 + n * delay - 60..4_000 + n * delay + 60]);
        assert!(near(&l, 1) > 0.9 * first && near(&r, 1) < 0.05 * near(&l, 1), "first repeat on the left only");
        assert!(near(&r, 2) > 0.05 * first && near(&l, 2) < 0.05 * near(&r, 2), "second repeat on the right only");
        assert!(near(&l, 3) > 0.01 * first);
    }

    #[test]
    fn ping_pong_r_ratio_scales_the_right_repeat_time() {
        let delay = 6_000;
        for (choice, ratio) in PING_PONG_RATIOS.iter().enumerate() {
            let mut fx = unit(BeatFxKind::PingPong, 0.25, 1.0);
            fx.set_param(2, choice as f32);
            fx.set_enabled(true);
            let x = impulse(80_000, 4_000);
            let (_, r) = run(&mut fx, &x, &x);
            let want = 4_000 + (delay as f32 * (1.0 + ratio)) as usize;
            let (mut best, mut at) = (0.0f32, 0);
            for (i, v) in r.iter().enumerate().skip(4_000 + delay + 200) {
                if v.abs() > best {
                    best = v.abs();
                    at = i;
                }
                if i > want + 3_000 {
                    break;
                }
            }
            assert!((at as i64 - want as i64).abs() < 8, "ratio {ratio}: first right repeat at {at}, want {want}");
        }
        assert_eq!(beat_fx_params(BeatFxKind::PingPong)[2].default, 2.0, "centre ratio 0.75");
    }

    #[test]
    fn ping_pong_ratio_change_while_running_does_not_click() {
        let x = house_programme(24);
        let steady = {
            let mut fx = banded_unit(BeatFxKind::PingPong, 0.5, 0.8);
            fx.set_bpm(125.0);
            fx.set_enabled(true);
            max_step(&run(&mut fx, &x, &x).0[SR as usize * 2..])
        };
        let mut fx = banded_unit(BeatFxKind::PingPong, 0.5, 0.8);
        fx.set_bpm(125.0);
        fx.set_enabled(true);
        let (mut l, mut r) = (x.clone(), x.clone());
        for (b, (cl, cr)) in l.chunks_mut(BLOCK).zip(r.chunks_mut(BLOCK)).enumerate() {
            if b % 150 == 149 {
                fx.set_param(2, [0.0, 5.0, 2.0, 7.0, 3.0][(b / 150) % 5]);
            }
            fx.process(cl, cr);
        }
        let moving = max_step(&l[SR as usize * 2..]);
        eprintln!("ping pong ratio changes: step {moving:.4} vs {steady:.4} without");
        assert!(moving < 1.3 * steady, "{moving} vs {steady}");
    }

    #[test]
    fn echo_division_changes_dip_cleanly_on_sustained_music() {
        let x = pad_programme(48);
        let render = |sweep: bool| {
            let mut fx = banded_unit(BeatFxKind::Echo, 0.5, 0.8);
            fx.set_bpm(125.0);
            fx.set_enabled(true);
            let (mut l, mut r) = (x.clone(), x.clone());
            for (b, (cl, cr)) in l.chunks_mut(BLOCK).zip(r.chunks_mut(BLOCK)).enumerate() {
                if sweep && b % 200 == 199 {
                    fx.set_beats([0.25, 0.75, 0.125, 1.0, 0.5][(b / 200) % 5]);
                }
                fx.process(cl, cr);
            }
            max_step(&l[SR as usize * 2..])
        };
        let (moving, steady) = (render(true), render(false));
        eprintln!("echo division changes on sustained music: max step {moving:.4} vs {steady:.4} without");
        assert!(moving < 1.3 * steady, "{moving} vs {steady}");
    }

    #[test]
    fn echo_freeze_holds_the_loop_flat_and_closes_the_input() {
        let mut z = 0.0f32;
        let lead: Vec<f32> = noise(48_000 * 2, 33).into_iter().map(|v| { z += 0.3 * (v - z); z }).collect();
        let tail = noise(48_000 * 38, 91);
        let x: Vec<f32> = lead.iter().cloned().chain(tail.iter().map(|v| v * 0.3)).collect();
        let mut fx = unit(BeatFxKind::Echo, 0.25, 0.75);
        fx.set_enabled(true);
        let (mut l, mut r) = (x.clone(), x.clone());
        for (b, (cl, cr)) in l.chunks_mut(BLOCK).zip(r.chunks_mut(BLOCK)).enumerate() {
            if b == 188 {
                fx.trigger(FxTrigger::FreezeOn);
            }
            fx.process(cl, cr);
        }
        assert!(fx.held());
        let wet: Vec<f32> = l.iter().zip(&x).map(|(o, d)| o - d).collect();
        let (early, late) = (rms(&wet[48_000 * 6..48_000 * 12]), rms(&wet[48_000 * 30..48_000 * 40]));
        eprintln!("echo freeze: {early:.4} at 6-12 s, {late:.4} at 30-40 s");
        assert!(late > 0.01 && db(late / early).abs() < 1.5, "holds level: {:+.2} dB", db(late / early));
        assert!(l.iter().all(|v| v.is_finite()) && peak(&l) < 2.0);
        fx.trigger(FxTrigger::FreezeOff);
        assert!(!fx.held());
        let silence = vec![0.0; 48_000 * 20];
        let (after, _) = run(&mut fx, &silence, &silence);
        assert!(rms(&after[48_000 * 15..]) < 0.05 * late, "releasing the freeze lets the repeats decay");
    }

    #[test]
    fn echo_filter_is_clean_at_the_centre_closes_a_low_pass_below_it_and_opens_a_high_pass_above_it() {
        let repeat = |knob: f32, n: usize| {
            let mut fx = unit(BeatFxKind::Echo, 0.25, 1.0);
            fx.set_param(1, knob);
            fx.set_param(0, 70.0);
            fx.set_enabled(true);
            let x: Vec<f32> = noise(6_000, 3).into_iter().map(|v| v * 0.5).chain(vec![0.0; 48_000 * 2]).collect();
            let (out, _) = run(&mut fx, &x, &x);
            let start = 6_000 * (n + 1);
            out[start..start + 4_096].to_vec()
        };
        let tilt = |v: &[f32]| db((band_psd(v, 2_000.0, 6_000.0) / band_psd(v, 100.0, 300.0)).sqrt());
        let (centre, dark, bright) = (tilt(&repeat(50.0, 1)), tilt(&repeat(0.0, 1)), tilt(&repeat(100.0, 1)));
        eprintln!("echo repeat high-over-low tilt: filter 0 {dark:.1} dB, centre {centre:.1} dB, filter 100 {bright:.1} dB");
        assert!(dark < centre - 20.0, "the low-pass closes below the centre");
        assert!(bright > centre + 10.0, "the high-pass opens above the centre");
        let (first, fifth) = (tilt(&repeat(50.0, 1)), tilt(&repeat(50.0, 5)));
        assert!((first - fifth).abs() < 1.5, "the filter sits outside the loop, so repeats do not keep darkening: {first} vs {fifth}");
        assert_eq!(beat_fx_params(BeatFxKind::Echo)[1].default, 50.0);
    }

    #[test]
    fn reverb_tail_decays_and_stays_finite() {
        let mut fx = unit(BeatFxKind::Reverb, 1.0, 1.0);
        fx.set_enabled(true);
        let x = noise(24_000, 9);
        let _ = run(&mut fx, &x, &x);
        fx.set_enabled(false);
        let silence = vec![0.0; 48_000 * 6];
        let (out, _) = run(&mut fx, &silence, &silence);
        assert!(out.iter().all(|v| v.is_finite()));
        let early = rms(&out[1_000..13_000]);
        let late = rms(&out[200_000..212_000]);
        assert!(early > 0.01);
        assert!(late < early * 0.2, "early {early} late {late}");
    }

    fn reverb_unit(percent: f32, level: f32) -> BeatFx {
        let mut fx = unit(BeatFxKind::Reverb, 1.0, level);
        fx.set_param(REVERB_VOICING_PARAM, 1.0);
        fx.set_percent(percent);
        fx.set_enabled(true);
        fx
    }

    fn classic_unit(percent: f32, level: f32) -> BeatFx {
        let mut fx = unit(BeatFxKind::Reverb, 1.0, level);
        fx.set_percent(percent);
        fx.set_enabled(true);
        fx
    }

    fn rt60_seconds(decay: f32, size: f32) -> f32 {
        let mut fx = classic_unit(size, 0.3);
        fx.set_param(2, decay);
        let secs = ((reverb_decay_rt60(decay) * 1.3) as usize).clamp(8, 160);
        let x = impulse(48_000 * secs, 100);
        let (out, _) = run(&mut fx, &x, &x);
        let energy: Vec<f64> = out[3_000..].iter().map(|v| (*v as f64).powi(2)).collect();
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
    fn reverb_decay_curve_is_short_and_flat_then_climbs_to_a_near_endless_wash() {
        assert!((tension_decay_rt60(1.0) - 1.5).abs() < 0.05);
        assert!((tension_decay_rt60(50.0) - 8.7).abs() < 0.3);
        assert!(tension_decay_rt60(100.0) >= 60.0);
        let curve: Vec<f32> = (1..=100).map(|p| tension_decay_rt60(p as f32)).collect();
        assert!(curve.windows(2).all(|w| w[1] >= w[0]), "monotonic");
        assert!(curve[19] < 2.5 && curve[49] > 3.0 * curve[19], "flat on the left, steep on the right");
    }

    #[test]
    fn classic_decay_follows_the_decoded_quartic_including_its_short_end_dip() {
        for (p, t) in [(1.0, 1.53), (8.0, 0.85), (10.0, 0.86), (20.0, 1.74), (50.0, 8.70), (80.0, 38.0), (100.0, 100.6)] {
            assert!((reverb_decay_rt60(p) / t - 1.0).abs() < 0.02, "decay {p}: {} s vs {t} s", reverb_decay_rt60(p));
        }
        assert!(reverb_decay_rt60(8.0) < reverb_decay_rt60(1.0));
        assert_eq!(classic_decay_trim(30.0), 1.0);
        assert!((classic_decay_trim(75.0) - 0.75).abs() < 1e-6 && (classic_decay_trim(100.0) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn classic_room_follows_the_size_law_one_step_at_a_time() {
        let ratio = reverb_room_scale(100) / reverb_room_scale(1);
        assert!((ratio - 2.080 / 0.382).abs() < 0.02, "size range {ratio}");
        assert!((reverb_room_scale(50) / reverb_room_scale(25) - 1.0 / 0.630).abs() < 0.01);
        assert_eq!(room_step_ms(40.0, 10, 11), 39.3333);
        assert!((room_step_ms(40.0, 10, 60) - 39.3333).abs() < 1e-4, "slews 0.67 ms per step");
        let mut ms = 40.0;
        for _ in 0..60 {
            ms = room_step_ms(ms, 10, 100);
        }
        assert_eq!(ms, 20.0);
        let mut fx = classic_unit(10.0, 1.0);
        let x = noise(4_800, 3);
        let _ = run(&mut fx, &x, &x);
        assert_eq!(fx.room_step, 10);
        fx.set_percent(60.0);
        let x = noise(48_000, 4);
        let _ = run(&mut fx, &x, &x);
        assert!(fx.room_step > 10 && fx.room_step < 60, "a 50-step move takes over a second: at {}", fx.room_step);
        let x = noise(96_000, 5);
        let _ = run(&mut fx, &x, &x);
        assert_eq!(fx.room_step, 60);
    }

    #[test]
    fn reverb_decay_parameter_sets_the_measured_rt60() {
        let measured: Vec<(f32, f32)> = [1.0, 30.0, 50.0, 70.0].iter().map(|d| (*d, rt60_seconds(*d, 50.0))).collect();
        eprintln!("reverb RT60 by decay: {measured:?}");
        for (decay, got) in &measured {
            let want = reverb_decay_rt60(*decay);
            assert!((got / want - 1.0).abs() < 0.3, "decay {decay}: {got} s vs {want} s");
        }
    }

    #[test]
    fn reverb_size_sets_the_room_and_leaves_the_decay_time_alone() {
        let (small, large) = (rt60_seconds(30.0, 1.0), rt60_seconds(30.0, 100.0));
        eprintln!("reverb decay 30: RT60 {small:.2} s in the smallest room, {large:.2} s in the largest");
        assert!((small / large - 1.0).abs() < 0.25, "{small} vs {large}");
        let response = |size: f32| {
            let mut fx = classic_unit(size, 0.5);
            let x = impulse(24_000, 100);
            run(&mut fx, &x, &x).0
        };
        let (a, b) = (response(1.0)[4_800..].to_vec(), response(100.0)[4_800..].to_vec());
        let dot: f32 = a.iter().zip(&b).map(|(p, q)| p * q).sum();
        let corr = dot / (a.iter().map(|v| v * v).sum::<f32>() * b.iter().map(|v| v * v).sum::<f32>()).sqrt();
        assert!(corr.abs() < 0.5, "the room geometry changes with size: correlation {corr}");
    }

    #[test]
    fn reverb_decay_tail_is_audible_for_seconds_after_disable() {
        let tail_db = |decay: f32, secs: f32| {
            let mut fx = reverb_unit(50.0, 0.75);
            fx.set_param(2, decay);
            let x = noise(24_000, 9);
            let _ = run(&mut fx, &x, &x);
            fx.set_enabled(false);
            let silence = vec![0.0; 48_000 * 14];
            let (out, _) = run(&mut fx, &silence, &silence);
            let at = (secs * SR) as usize;
            20.0 * (rms(&out[at..at + 4_800]) / rms(&x)).log10()
        };
        assert!(tail_db(100.0, 6.0) > -45.0, "full decay still rings at 6 s: {}", tail_db(100.0, 6.0));
        assert!(tail_db(100.0, 6.0) > tail_db(30.0, 6.0) + 10.0);
        assert!(tail_db(1.0, 3.0) < -50.0, "the shortest decay is short: {}", tail_db(1.0, 3.0));
    }

    #[test]
    fn mix_law_holds_the_dry_to_three_quarters_then_removes_it_in_the_selected_bands() {
        for (kind, name) in [(BeatFxKind::Reverb, "reverb"), (BeatFxKind::Echo, "echo"), (BeatFxKind::PingPong, "ping pong")] {
            for (level, dry) in [(0.0f32, 1.0f32), (0.5, 1.0), (0.75, 1.0), (0.875, 0.5), (1.0, 0.0)] {
                let mut fx = unit(kind, 0.5, level);
                fx.set_enabled(true);
                let x = impulse(2_000, 1_000);
                let (out, _) = run(&mut fx, &x, &x);
                let direct = out[1_000] / IMPULSE;
                assert!((direct - dry).abs() < 0.02, "{name} depth {level}: the direct sound is {direct}, want {dry}");
            }
        }
    }

    #[test]
    fn mix_law_leaves_unselected_bands_dry_at_full_depth() {
        let held_back = |hz: f32| {
            let x = sine(48_000 * 2, hz, 0.3);
            let mut fx = banded_unit(BeatFxKind::Echo, 4.0, 1.0);
            fx.set_enabled(true);
            let (out, _) = run(&mut fx, &x, &x);
            (db(tone_amplitude(&out[24_000..90_000], hz) / tone_amplitude(&x[24_000..90_000], hz)), rms(&out[24_000..90_000]))
        };
        let (bass, _) = held_back(60.0);
        assert!(bass.abs() < 0.5, "the bass stays dry at full depth: {bass:+.2} dB");
        let (_, mids) = held_back(2_000.0);
        assert!(mids < 0.02, "the selected bands lose their direct sound before the first repeat: {mids}");
    }

    #[test]
    fn classic_voicing_is_the_default_bright_unmodulated_and_preless_while_tension_adds_its_extras() {
        assert_eq!(beat_fx_params(BeatFxKind::Reverb)[3].default, 0.0);
        assert_eq!(beat_fx_params(BeatFxKind::Reverb)[3].options, &["Classic", "Tension"]);
        let onset = |tension: bool| {
            let mut fx = if tension { reverb_unit(50.0, 0.5) } else { classic_unit(50.0, 0.5) };
            fx.wash_scale = 0.0;
            let x = impulse(60_000, 48_000);
            let (out, _) = run(&mut fx, &x, &x);
            out.iter().enumerate().skip(48_001).find(|(_, v)| v.abs() > 1e-4).map(|(i, _)| i - 48_000).unwrap_or(usize::MAX)
        };
        let (classic, tension) = (onset(false), onset(true));
        eprintln!("first wet sample after the impulse: classic {classic}, tension {tension}");
        assert!(tension > classic + 900, "no pre-delay in classic, about 20-30 ms in tension");
        let ratio = |tension: bool| {
            let mut fx = if tension { reverb_unit(50.0, 1.0) } else { classic_unit(50.0, 1.0) };
            fx.wash_scale = 0.0;
            let (out, _) = run(&mut fx, &impulse(48_000 * 20, 100), &impulse(48_000 * 20, 100));
            rt60_at(&out[3_000..], 5_000.0) / rt60_at(&out[3_000..], 500.0)
        };
        let (c, t) = (ratio(false), ratio(true));
        eprintln!("RT60 at 5 kHz over 500 Hz: classic {c:.2}, tension {t:.2}");
        assert!(c > 0.85 && t < 0.75, "classic is undamped, tension darkens as it ages");
        let render = |noise_param: f32, start_beat: f64| {
            let mut fx = classic_unit(60.0, 0.9);
            fx.set_param(1, noise_param);
            fx.align_phase(start_beat);
            let x = noise(48_000 * 2, 5).into_iter().map(|v| v * 0.3).collect::<Vec<_>>();
            run(&mut fx, &x, &x).0
        };
        assert_eq!(render(0.0, 0.0), render(100.0, 0.0), "classic has no noise wash");
        assert_eq!(render(50.0, 0.0), render(50.0, 2.0), "classic has no bar breathing");
    }

    #[test]
    fn reverb_voicing_switch_while_running_does_not_click() {
        let x = pad_programme(16);
        let render = |switch: bool| {
            let mut fx = banded_unit(BeatFxKind::Reverb, 1.0, 0.7);
            fx.set_enabled(true);
            let (mut l, mut r) = (x.clone(), x.clone());
            let mut worst = 0.0f32;
            let mut prev = 0.0f32;
            for (b, (cl, cr)) in l.chunks_mut(BLOCK).zip(r.chunks_mut(BLOCK)).enumerate() {
                if switch && b == 300 {
                    fx.set_param(3, 1.0);
                }
                if switch && b == 600 {
                    fx.set_param(3, 0.0);
                }
                fx.process(cl, cr);
                if b > 100 {
                    for v in cl.iter() {
                        worst = worst.max((v - prev).abs());
                        prev = *v;
                    }
                }
            }
            worst
        };
        let (moving, steady) = (render(true), render(false));
        eprintln!("voicing switches: step {moving:.4} vs {steady:.4}");
        assert!(moving < 1.5 * steady, "{moving} vs {steady}");
    }

    #[test]
    fn classic_reverb_at_maximum_decay_stays_bounded_for_a_minute_of_noise() {
        let x = noise(48_000 * 60, 17);
        let mut fx = classic_unit(100.0, 1.0);
        fx.set_param(2, 100.0);
        let (mut l, mut r) = (x.clone(), x.clone());
        let mut worst = 0.0f32;
        for (cl, cr) in l.chunks_mut(BLOCK).zip(r.chunks_mut(BLOCK)) {
            fx.process(cl, cr);
            for v in cl.iter().chain(cr.iter()) {
                assert!(v.is_finite());
                worst = worst.max(v.abs());
            }
        }
        assert!(worst <= 2.0, "peak {worst}");
    }

    #[test]
    fn reverb_percent_changes_do_not_click() {
        let x = sine(48_000 * 4, 220.0, 0.3);
        let worst = |sweep: bool| {
            let mut fx = reverb_unit(40.0, 0.8);
            fx.wash_scale = 0.0;
            let (mut l, mut r) = (x.clone(), x.clone());
            let (mut prev, mut worst) = (0.0f32, 0.0f32);
            for (b, (cl, cr)) in l.chunks_mut(BLOCK).zip(r.chunks_mut(BLOCK)).enumerate() {
                if sweep && b % 40 == 0 {
                    fx.set_percent([1.0, 100.0, 20.0, 80.0, 5.0][(b / 40) % 5]);
                }
                fx.process(cl, cr);
                if b > 100 {
                    for v in cl.iter() {
                        worst = worst.max((v - prev).abs());
                        prev = *v;
                    }
                }
            }
            worst
        };
        assert!(worst(true) < worst(false) * 1.5 + 0.01, "{} vs {}", worst(true), worst(false));
    }

    #[test]
    fn reverb_stays_bounded_for_thirty_seconds_of_full_scale_noise_at_maximum() {
        let x = noise(48_000 * 30, 77);
        let mut fx = reverb_unit(100.0, 1.0);
        let (mut l, mut r) = (x.clone(), x.clone());
        let mut worst = 0.0f32;
        for (cl, cr) in l.chunks_mut(BLOCK).zip(r.chunks_mut(BLOCK)) {
            fx.process(cl, cr);
            for v in cl.iter().chain(cr.iter()) {
                assert!(v.is_finite());
                worst = worst.max(v.abs());
            }
        }
        assert!(worst <= 2.0, "peak {worst}");
    }

    #[test]
    fn percent_is_clamped_and_only_reshapes_the_reverb() {
        let mut fx = unit(BeatFxKind::Echo, 1.0, 1.0);
        fx.set_percent(500.0);
        assert_eq!(fx.percent(), 100.0);
        fx.set_percent(-3.0);
        assert_eq!(fx.percent(), 1.0);
        fx.set_percent(f32::NAN);
        assert_eq!(fx.percent(), 1.0);
        let render = |percent: f32| {
            let mut fx = unit(BeatFxKind::Echo, 0.5, 0.8);
            fx.set_percent(percent);
            fx.set_enabled(true);
            run(&mut fx, &noise(24_000, 4), &noise(24_000, 4)).0
        };
        assert_eq!(render(10.0), render(90.0));
    }

    fn burst_noise() -> Vec<f32> {
        let mut z = 0.0f32;
        noise(12_000, 3).iter().map(|v| { z += 0.2 * (v - z); z * 2.2 }).collect()
    }

    fn echo_tail_db(kind: BeatFxKind, level: f32, knob: f32, secs: f32) -> f32 {
        let mut fx = unit(kind, 1.0, level);
        fx.set_bpm(125.0);
        fx.set_param(0, knob);
        fx.set_enabled(true);
        let burst = burst_noise();
        let mut x = vec![0.0; 48_000 * 45];
        x[1_000..13_000].copy_from_slice(&burst);
        let (out, _) = run(&mut fx, &x, &x);
        let window = |end: usize| rms(&out[end - 12_000..end]);
        let first_repeat = window(13_000 + 24_000);
        20.0 * (window(13_000 + (secs * SR) as usize + 6_000) / first_repeat).log10()
    }

    #[test]
    fn echo_feedback_knob_sets_the_tail_and_a_high_setting_rings_for_ages() {
        let at_six = echo_tail_db(BeatFxKind::Echo, 1.0, 80.0, 6.0);
        assert!(at_six > -30.0, "6 s: {at_six} dB re first repeat");
        let at_ten = echo_tail_db(BeatFxKind::Echo, 1.0, 80.0, 10.0);
        assert!(at_ten > -45.0, "10 s: {at_ten} dB");
        let at_thirty = echo_tail_db(BeatFxKind::Echo, 1.0, 80.0, 30.0);
        assert!(at_thirty < -60.0, "30 s: {at_thirty} dB");
        let default = beat_fx_params(BeatFxKind::Echo)[0].default;
        let controlled = echo_tail_db(BeatFxKind::Echo, 1.0, default, 4.0);
        assert!(controlled < -25.0, "the default feedback is long but controlled: {controlled} dB at 4 s");
    }

    #[test]
    fn echo_level_is_a_pure_mix_control_and_the_feedback_knob_alone_sets_the_repeat_count() {
        for kind in [BeatFxKind::Echo, BeatFxKind::PingPong, BeatFxKind::Delay, BeatFxKind::UpEcho, BeatFxKind::DownEcho] {
            let (soft, full) = (echo_tail_db(kind, 0.5, 70.0, 5.0), echo_tail_db(kind, 1.0, 70.0, 5.0));
            assert!((soft - full).abs() < 1.0, "{kind:?}: the tail shape does not follow the level ({soft} vs {full} dB)");
            let (short, long) = (echo_tail_db(kind, 1.0, 30.0, 5.0), echo_tail_db(kind, 1.0, 90.0, 5.0));
            assert!(long > short + 20.0, "{kind:?}: feedback sets the tail ({short} -> {long} dB)");
        }
    }

    #[test]
    fn echo_feedback_knob_runs_zero_to_point_seven_at_the_centre_then_to_just_under_one() {
        let law: Vec<f32> = [0.0, 25.0, 50.0, 75.0, 100.0].iter().map(|k| echo_feedback_for_knob(*k)).collect();
        assert_eq!(law[0], 0.0);
        assert!((law[1] - 0.35).abs() < 1e-6 && (law[2] - 0.7).abs() < 1e-6);
        assert!(law[3] > 0.8 && law[4] > 0.985 && law[4] < 1.0, "{law:?}");
        let default = echo_feedback_for_knob(beat_fx_params(BeatFxKind::Echo)[0].default);
        assert!((0.55..=0.6).contains(&default), "default feedback {default}");
    }

    #[test]
    fn effects_keep_the_affected_bands_within_two_db_of_the_dry_programme_at_any_depth() {
        let x = house_programme(64);
        let window = SR as usize * 12..;
        let dry = hp_rms(&x[window.clone()]);
        let kinds = [
            (BeatFxKind::Echo, false),
            (BeatFxKind::Delay, false),
            (BeatFxKind::PingPong, false),
            (BeatFxKind::UpEcho, false),
            (BeatFxKind::DownEcho, false),
            (BeatFxKind::Spiral, false),
            (BeatFxKind::Reverb, false),
            (BeatFxKind::Reverb, true),
            (BeatFxKind::PulseVerb, false),
            (BeatFxKind::Halo, false),
            (BeatFxKind::DubTape, false),
        ];
        for (kind, tension) in kinds {
            for level in [0.25, 0.5, 0.75, 1.0] {
                let mut fx = banded_unit(kind, 1.0, level);
                fx.set_bpm(125.0);
                if tension {
                    fx.set_param(REVERB_VOICING_PARAM, 1.0);
                }
                fx.set_enabled(true);
                let (out, _) = run(&mut fx, &x, &x);
                let change = 20.0 * (hp_rms(&out[window.clone()]) / dry).log10();
                eprintln!("{kind:?}{} level {level}: {change:+.2} dB", if tension { " (tension)" } else { "" });
                let ceiling = if matches!(kind, BeatFxKind::Echo | BeatFxKind::Delay | BeatFxKind::PingPong | BeatFxKind::UpEcho | BeatFxKind::DownEcho) { 3.0 } else { 2.0 };
                assert!((-4.5..=ceiling).contains(&change), "{kind:?} tension {tension} level {level}: {change:+.2} dB");
            }
        }
    }

    #[test]
    fn echo_family_stays_bounded_for_thirty_seconds_at_full_depth_and_maximum_feedback() {
        for kind in [BeatFxKind::Delay, BeatFxKind::Echo, BeatFxKind::UpEcho, BeatFxKind::DownEcho, BeatFxKind::PingPong, BeatFxKind::Spiral] {
            for knob in [42.0, 100.0] {
                let x = noise(48_000 * 30, 41);
                let mut fx = unit(kind, 0.25, 1.0);
                fx.set_param(0, knob);
                fx.set_enabled(true);
                let (out, _) = run(&mut fx, &x, &x);
                assert!(out.iter().all(|v| v.is_finite() && v.abs() <= 2.0), "{kind:?} feedback {knob}: peak {}", peak(&out));
            }
        }
    }

    #[test]
    fn roll_repeats_the_captured_slice() {
        let mut fx = unit(BeatFxKind::Roll, 0.25, 1.0);
        fx.set_enabled(true);
        let slice = 6_000;
        let x = noise(60_000, 21);
        let (out, _) = run(&mut fx, &x, &x);
        let looped_from = slice + ROLL_CROSSFADE;
        let settle = 1_500;
        for pass in 1..4 {
            let base = looped_from + (pass - 1) * slice;
            for k in ROLL_CROSSFADE..slice - 1 {
                if base + k < looped_from + settle {
                    continue;
                }
                let got = out[base + k];
                let want = x[k];
                assert!((got - want).abs() < 1e-3, "pass {pass} k {k}: {got} vs {want}");
            }
        }
    }

    #[test]
    fn roll_stops_immediately_when_disabled() {
        let mut fx = unit(BeatFxKind::Roll, 0.25, 1.0);
        fx.set_enabled(true);
        let x = noise(60_000, 5);
        let _ = run(&mut fx, &x, &x);
        fx.set_enabled(false);
        let silence = vec![0.0; 9_600];
        let (out, _) = run(&mut fx, &silence, &silence);
        assert!(rms(&out[2_400..]) < 1e-4, "no loop after de-click ramp");
    }

    #[test]
    fn trans_gates_at_the_beat_rate() {
        let mut fx = unit(BeatFxKind::Trans, 1.0, 1.0);
        fx.set_enabled(true);
        let period = 24_000;
        let x = vec![1.0; period * 4];
        let (out, _) = run(&mut fx, &x, &x);
        for p in 1..3 {
            let on = &out[p * period + 2_000..p * period + period / 2 - 2_000];
            let off = &out[p * period + period / 2 + 2_000..(p + 1) * period - 2_000];
            assert!(on.iter().all(|v| *v > 0.99), "open half");
            assert!(off.iter().all(|v| v.abs() < 0.01), "closed half");
        }
    }

    #[test]
    fn align_phase_places_the_gate_on_the_deck_grid() {
        let mut fx = unit(BeatFxKind::Trans, 1.0, 1.0);
        fx.set_enabled(true);
        fx.align_phase(0.5);
        let x = vec![1.0; 24_000 * 2];
        let (out, _) = run(&mut fx, &x, &x);
        assert!(out[4_000..10_000].iter().all(|v| v.abs() < 0.01), "starts half a period in: closed");
        assert!(out[16_000..22_000].iter().all(|v| *v > 0.99), "then open");
    }

    #[test]
    fn trans_level_sets_gate_depth() {
        let mut fx = unit(BeatFxKind::Trans, 1.0, 0.5);
        fx.set_enabled(true);
        let x = vec![1.0; 24_000 * 3];
        let (out, _) = run(&mut fx, &x, &x);
        let closed = &out[24_000 + 14_000..24_000 + 22_000];
        assert!(closed.iter().all(|v| (v - 0.5).abs() < 0.01));
    }

    #[test]
    fn modulation_effects_stay_bounded() {
        for kind in [BeatFxKind::Flanger, BeatFxKind::Phaser, BeatFxKind::FilterSweep] {
            let mut fx = unit(kind, 0.5, 1.0);
            fx.set_enabled(true);
            let x = sine(96_000, 700.0, 1.0);
            let (out, _) = run(&mut fx, &x, &x);
            assert!(out.iter().all(|v| v.is_finite()));
            assert!(peak(&out) < 2.0, "{kind:?} peak {}", peak(&out));
            assert!(rms(&out[24_000..]) > 0.05);
        }
    }

    #[test]
    fn flanger_modulates_the_spectrum() {
        let mut fx = unit(BeatFxKind::Flanger, 2.0, 1.0);
        fx.set_enabled(true);
        let x = noise(48_000 * 2, 77);
        let (out, _) = run(&mut fx, &x, &x);
        let diff: Vec<f32> = out.iter().zip(&x).map(|(a, b)| a - b).collect();
        assert!(rms(&diff) > 0.1);
    }

    #[test]
    fn non_tail_effects_stop_after_declick() {
        for kind in [BeatFxKind::Flanger, BeatFxKind::Phaser, BeatFxKind::Trans, BeatFxKind::FilterSweep] {
            let mut fx = unit(kind, 0.5, 1.0);
            fx.set_enabled(true);
            let x = sine(24_000, 500.0, 0.5);
            let _ = run(&mut fx, &x, &x);
            fx.set_enabled(false);
            let probe = sine(24_000, 500.0, 0.5);
            let (out, _) = run(&mut fx, &probe, &probe);
            let tail: Vec<f32> = out[4_800..].iter().zip(&probe[4_800..]).map(|(a, b)| a - b).collect();
            assert!(rms(&tail) < 1e-4, "{kind:?} leaves {}", rms(&tail));
        }
    }

    #[test]
    fn full_scale_input_never_exceeds_six_db_over() {
        for kind in BeatFxKind::ALL {
            for (name, x) in [
                ("sine on delay period", sine(96_000, 1_000.0, 1.0)),
                ("noise", noise(96_000, 11).iter().map(|v| v * 1.0).collect()),
                ("square", (0..96_000).map(|i| if (i / 40) % 2 == 0 { 1.0 } else { -1.0 }).collect()),
            ] {
                let mut fx = unit(kind, 0.5, 1.0);
                fx.set_enabled(true);
                let (out, _) = run(&mut fx, &x, &x);
                assert!(out.iter().all(|v| v.is_finite()), "{kind:?} {name}");
                assert!(peak(&out) <= 2.0, "{kind:?} {name} peak {}", peak(&out));
            }
        }
    }

    #[test]
    fn kind_switch_while_enabled_does_not_click() {
        let x = sine(48_000 * 3, 220.0, 0.3);
        for (from, to) in [
            (BeatFxKind::Echo, BeatFxKind::Reverb),
            (BeatFxKind::Flanger, BeatFxKind::Trans),
            (BeatFxKind::PingPong, BeatFxKind::Roll),
            (BeatFxKind::UpEcho, BeatFxKind::Spiral),
            (BeatFxKind::Spiral, BeatFxKind::DownEcho),
            (BeatFxKind::Phaser, BeatFxKind::Delay),
        ] {
            let mut fx = unit(from, 0.5, 1.0);
            fx.wash_scale = 0.0;
            fx.set_enabled(true);
            let (mut l, mut r) = (x.clone(), x.clone());
            let mut worst: f32 = 0.0;
            let mut prev = 0.0;
            for (b, (cl, cr)) in l.chunks_mut(BLOCK).zip(r.chunks_mut(BLOCK)).enumerate() {
                if b == 200 {
                    fx.set_kind(to);
                }
                fx.process(cl, cr);
                for v in cl.iter() {
                    worst = worst.max((v - prev).abs());
                    prev = *v;
                }
            }
            eprintln!("{from:?}->{to:?} worst step {worst}");
            let sine_step_at_peak = 1.5 * peak(&l) * TAU * 220.0 / SR;
            assert!(worst < sine_step_at_peak.max(0.03), "{from:?}->{to:?} worst step {worst} vs {sine_step_at_peak}");
            assert_eq!(fx.kind(), to);
        }
    }

    #[test]
    fn disabled_unit_is_bit_exact_passthrough() {
        let mut fx = unit(BeatFxKind::Reverb, 1.0, 1.0);
        let x = noise(4_096, 8);
        let (out, _) = run(&mut fx, &x, &x);
        assert_eq!(out, x);
    }

    #[test]
    fn tempo_change_retimes_the_echo() {
        let mut fx = unit(BeatFxKind::Echo, 1.0, 1.0);
        fx.set_bpm(150.0);
        fx.set_enabled(true);
        let delay = 19_200;
        let x = impulse(60_000, 4_000);
        let (out, _) = run(&mut fx, &x, &x);
        assert!((out[4_000 + delay] - first_repeat_gain(BeatFxKind::Echo, 1.0) * IMPULSE).abs() < 1e-3);
    }

    #[test]
    fn long_divisions_fold_into_the_delay_cap() {
        let mut fx = unit(BeatFxKind::Echo, 16.0, 1.0);
        fx.set_bpm(60.0);
        assert!(fx.delay_target() <= MAX_DELAY_SECS * SR);
        assert!((fx.delay_target() - 8.0 * SR).abs() < 1.0);
    }

    fn banded_unit(kind: BeatFxKind, beats: f32, level: f32) -> BeatFx {
        let mut fx = unit(kind, beats, level);
        fx.raw_core = false;
        fx
    }

    const DIVISIONS: [f32; 5] = [0.25, 0.5, 0.75, 1.0, 0.25];

    fn division_sweep(kind: BeatFxKind, level: f32) -> (Vec<f32>, usize) {
        let secs_each = 2.0;
        let seg = (secs_each * SR) as usize;
        let x = sine(seg * DIVISIONS.len(), 1_000.0, 0.3);
        let mut fx = banded_unit(kind, DIVISIONS[0], level);
        fx.set_bpm(127.3);
        fx.set_enabled(true);
        let (mut l, mut r) = (x.clone(), x.clone());
        for (b, (cl, cr)) in l.chunks_mut(BLOCK).zip(r.chunks_mut(BLOCK)).enumerate() {
            let start = b * BLOCK;
            if start.is_multiple_of(seg) && start > 0 {
                fx.set_beats(DIVISIONS[start / seg]);
            }
            fx.process(cl, cr);
        }
        (l, seg)
    }

    #[test]
    fn echo_family_keeps_a_sine_within_five_cents_across_division_changes() {
        for kind in [BeatFxKind::Echo, BeatFxKind::Delay, BeatFxKind::PingPong] {
            let (out, seg) = division_sweep(kind, 0.9);
            let settled = &out[seg / 2..];
            let cents = cents_deviation(settled, 1_000.0, 480);
            let worst = cents.iter().fold(0.0f32, |m, c| m.max(c.abs()));
            let sine_step_at_peak = peak(settled) * TAU * 1_000.0 / SR;
            let step = max_step(settled);
            eprintln!("{kind:?}: worst {worst:.2} cents, max step {step:.4} (sine at same peak {sine_step_at_peak:.4})");
            assert!(worst < 5.0, "{kind:?}: {worst} cents");
            assert!(step < 1.25 * sine_step_at_peak, "{kind:?}: step {step} vs {sine_step_at_peak}");
        }
    }

    fn hp_rms(v: &[f32]) -> f32 {
        let mut hp = HighPass::<2>::default();
        hp.set(300.0, SR);
        let y: Vec<f32> = v.iter().map(|s| hp.process(*s)).collect();
        rms(&y[4_800..])
    }

    fn pad_programme(beats: usize) -> Vec<f32> {
        let beat = (60.0 / 125.0 * SR) as usize;
        let mut grit = super::super::util::Noise::new(7);
        (0..beat * beats)
            .map(|i| {
                let secs = i as f32 / SR;
                let swell = 0.6 + 0.4 * (TAU * 0.5 * secs).sin();
                let chord: f32 = [330.0f32, 392.0, 494.0, 660.0, 988.0, 1_320.0].iter().map(|f| (TAU * f * secs).sin()).sum();
                0.07 * swell * chord + 0.01 * grit.next()
            })
            .collect()
    }

    fn house_programme(beats: usize) -> Vec<f32> {
        let beat = (60.0 / 125.0 * SR) as usize;
        let mut hats = super::super::util::Noise::new(29);
        let mut x = vec![0.0f32; beat * beats];
        for (i, v) in x.iter_mut().enumerate() {
            let t = (i % beat) as f32 / SR;
            let kick = (TAU * (50.0 + 90.0 * (-t * 40.0).exp()) * t).sin() * (-t * 9.0).exp() * 0.6;
            let off = ((i + beat / 2) % beat) as f32 / SR;
            let hat = hats.next() * (-off * 45.0).exp() * 0.3;
            let bass = 0.2 * (TAU * 55.0 * i as f32 / SR).sin();
            let secs = i as f32 / SR;
            let stab = if (i / (beat / 2)).is_multiple_of(3) { (-t * 6.0).exp() } else { 0.0 };
            let chord: f32 = [220.0f32, 261.6, 329.6, 440.0, 659.3, 880.0].iter().map(|f| (TAU * f * secs).sin()).sum();
            *v = kick + hat + bass + 0.07 * chord * stab + 0.03 * (TAU * 1_320.0 * secs).sin();
        }
        x
    }

    #[test]
    fn up_and_down_echo_repeats_stay_on_their_semitone_grid_through_division_changes() {
        for (kind, sign) in [(BeatFxKind::UpEcho, 1.0f32), (BeatFxKind::DownEcho, -1.0)] {
            let mut fx = banded_unit(kind, 0.5, 1.0);
            fx.set_enabled(true);
            let burst = 9_600;
            let mut x = vec![0.0; 48_000 * 6];
            x[4_000..4_000 + burst].copy_from_slice(&sine(burst, 1_000.0, 0.5));
            let (mut l, mut r) = (x.clone(), x.clone());
            for (b, (cl, cr)) in l.chunks_mut(BLOCK).zip(r.chunks_mut(BLOCK)).enumerate() {
                if b == 140 {
                    fx.set_beats(0.75);
                }
                if b == 300 {
                    fx.set_beats(0.5);
                }
                fx.process(cl, cr);
            }
            let wet: Vec<f32> = l.iter().zip(&x).map(|(o, d)| o - d).collect();
            let window = 480;
            let env: Vec<f32> = wet.chunks(window).map(rms).collect();
            let mut found = Vec::new();
            let mut i = (4_000 + burst) / window + 2;
            while i + 6 < env.len() && found.len() < 4 {
                if env[i] > 0.02 {
                    let end = (i..env.len()).find(|j| env[*j] < 0.01).unwrap_or(env.len());
                    if end - i >= 8 {
                        let core = &wet[(i + 3) * window..(end - 3) * window];
                        found.push(crossing_hz(core));
                    }
                    i = end + 1;
                } else {
                    i += 1;
                }
            }
            assert!(found.len() >= 3, "{kind:?}: only {} repeats found", found.len());
            for (k, hz) in found.iter().take(3).enumerate() {
                let want = 1_000.0 * (sign * (k as f32 + 1.0) / 12.0).exp2();
                let cents = 1200.0 * (hz / want).log2();
                eprintln!("{kind:?} repeat {}: {hz:.1} Hz want {want:.1} ({cents:+.1} cents)", k + 1);
                assert!(cents.abs() < 5.0, "{kind:?} repeat {}: {cents:+.1} cents", k + 1);
            }
        }
    }

    #[test]
    fn spiral_is_exempt_and_still_glides_through_the_same_sweep() {
        let (out, seg) = division_sweep(BeatFxKind::Spiral, 0.6);
        let worst = cents_deviation(&out[seg / 2..], 1_000.0, 480).iter().fold(0.0f32, |m, c| m.max(c.abs()));
        assert!(worst > 5.0, "spiral keeps its tape glide: {worst} cents");
    }

    #[test]
    fn division_change_is_quantised_to_a_sixteenth_of_the_grid() {
        let mut fx = banded_unit(BeatFxKind::Echo, 0.5, 1.0);
        fx.set_enabled(true);
        fx.align_phase(0.0);
        let sixteenth = (SR * 0.5 / 4.0) as usize;
        let x = impulse(sixteenth * 40, 1_000);
        let (mut l, mut r) = (x.clone(), x.clone());
        let mut changed_at = None;
        for (b, (cl, cr)) in l.chunks_mut(BLOCK).zip(r.chunks_mut(BLOCK)).enumerate() {
            if b * BLOCK > sixteenth * 8 + 700 && changed_at.is_none() {
                fx.set_beats(1.0);
                changed_at = Some(b * BLOCK);
            }
            fx.process(cl, cr);
        }
        let waited = fx.taps.delay();
        assert!((waited - 48_000.0 * 0.5).abs() < 80.0, "delay settled on the new division: {waited}");
        assert!(changed_at.is_some());
    }

    fn amplitude_change_db(kind: BeatFxKind, hz: f32) -> f32 {
        let x = sine(48_000 * 3, hz, 0.3);
        let mut fx = banded_unit(kind, 0.5, 1.0);
        fx.set_enabled(true);
        let (out, _) = run(&mut fx, &x, &x);
        db(tone_amplitude(&out[96_000..], hz) / tone_amplitude(&x[96_000..], hz))
    }

    #[test]
    fn low_band_off_keeps_the_bass_at_dry_level_under_echo_and_reverb() {
        for kind in [BeatFxKind::Echo, BeatFxKind::Delay, BeatFxKind::PingPong, BeatFxKind::UpEcho, BeatFxKind::Reverb] {
            assert!(!kind.default_bands().low, "{kind:?} defaults to LOW off");
            for hz in [45.0, 60.0, 100.0] {
                let change = amplitude_change_db(kind, hz);
                eprintln!("{kind:?} {hz} Hz: {change:+.3} dB");
                assert!(change.abs() < 0.5, "{kind:?} {hz} Hz: {change:+.2} dB");
            }
        }
    }

    #[test]
    fn all_bands_on_with_the_effect_silent_is_transparent_within_a_tenth_of_a_db() {
        for kind in [BeatFxKind::Echo, BeatFxKind::Reverb, BeatFxKind::Trans, BeatFxKind::Flanger] {
            for hz in [40.0, 100.0, 250.0, 1_000.0, 3_500.0, 10_000.0] {
                let x = sine(48_000 * 2, hz, 0.3);
                let mut fx = banded_unit(kind, 1.0, 0.0);
                fx.set_bands(FxBands::ALL);
                fx.set_enabled(true);
                let (out, _) = run(&mut fx, &x, &x);
                let change = db(rms(&out[48_000..]) / rms(&x[48_000..]));
                assert!(change.abs() < 0.1, "{kind:?} {hz} Hz: {change:+.3} dB");
            }
        }
        let mut idle = banded_unit(BeatFxKind::Echo, 1.0, 1.0);
        idle.set_bands(FxBands::ALL);
        let x = noise(4_096, 8);
        assert_eq!(run(&mut idle, &x, &x).0, x);
    }

    #[test]
    fn band_selection_is_remembered_per_kind_and_defaults_follow_the_kind() {
        let mut fx = BeatFx::new(SR);
        assert_eq!(fx.bands(), FxBands { low: false, mid: true, high: true });
        fx.set_bands(FxBands { low: true, mid: false, high: true });
        fx.set_kind(BeatFxKind::Trans);
        assert_eq!(fx.bands(), FxBands::ALL);
        fx.set_bands(FxBands { low: true, mid: true, high: false });
        fx.set_kind(BeatFxKind::Echo);
        assert_eq!(fx.bands(), FxBands { low: true, mid: false, high: true });
        fx.set_kind(BeatFxKind::Trans);
        assert_eq!(fx.bands(), FxBands { low: true, mid: true, high: false });
        let sets = fx.band_sets();
        let mut other = BeatFx::new(SR);
        other.restore_band_sets(sets);
        other.set_kind(BeatFxKind::Echo);
        assert_eq!(other.bands(), FxBands { low: true, mid: false, high: true });
    }

    #[test]
    fn toggling_bands_and_enabling_the_effect_do_not_click() {
        let x = sine(48_000 * 4, 180.0, 0.4);
        let mut fx = banded_unit(BeatFxKind::Echo, 0.5, 0.8);
        let (mut l, mut r) = (x.clone(), x.clone());
        let sine_step = 0.4 * TAU * 180.0 / SR;
        let mut worst = 0.0f32;
        let mut prev = 0.0;
        for (b, (cl, cr)) in l.chunks_mut(BLOCK).zip(r.chunks_mut(BLOCK)).enumerate() {
            match b {
                60 => fx.set_enabled(true),
                300 => fx.set_bands(FxBands::ALL),
                420 => fx.set_bands(FxBands { low: true, mid: false, high: false }),
                540 => fx.set_bands(FxBands { low: false, mid: true, high: true }),
                _ => {}
            }
            fx.process(cl, cr);
            for v in cl.iter() {
                worst = worst.max((v - prev).abs());
                prev = *v;
            }
        }
        assert!(worst < 3.0 * sine_step, "worst step {worst} vs sine {sine_step}");
    }

    #[test]
    fn the_unit_hands_back_to_a_bit_exact_bypass_after_its_tail_with_banded_processing() {
        let mut fx = banded_unit(BeatFxKind::Echo, 0.25, 0.6);
        fx.set_enabled(true);
        let burst = noise(12_000, 5);
        let _ = run(&mut fx, &burst, &burst);
        fx.set_enabled(false);
        let silence = vec![0.0; 48_000 * 12];
        let _ = run(&mut fx, &silence, &silence);
        assert!(fx.is_idle());
        let x = sine(9_600, 300.0, 0.4);
        let (out, _) = run(&mut fx, &x, &x);
        assert_eq!(out, x);
    }

    fn reverb_wet(percent: f32, x: &[f32]) -> Vec<f32> {
        let mut fx = reverb_unit(percent, 1.0);
        let (out, _) = run(&mut fx, x, x);
        out.iter().zip(x).map(|(o, d)| o - beat_mix(1.0).0 * d).collect()
    }

    #[test]
    fn reverb_wet_is_high_passed_well_below_the_midband() {
        let x = noise(48_000 * 8, 12);
        let wet = reverb_wet(60.0, &x);
        let tail = &wet[48_000 * 2..];
        let low = band_psd(tail, 40.0, 150.0);
        let mid = band_psd(tail, 500.0, 2_000.0);
        let gap = db((mid / low).sqrt());
        eprintln!("reverb wet: below 150 Hz is {gap:.1} dB under the midband");
        assert!(gap >= 9.0, "{gap} dB");
    }

    #[test]
    fn reverb_high_frequencies_decay_faster_than_the_lows() {
        for percent in [30.0, 60.0] {
            let mut fx = reverb_unit(percent, 1.0);
            fx.wash_scale = 0.0;
            let x = impulse(48_000 * 30, 100);
            let (out, _) = run(&mut fx, &x, &x);
            let response = &out[3_000..];
            let (low, high) = (rt60_at(response, 500.0), rt60_at(response, 5_000.0));
            eprintln!("reverb {percent}%: RT60 {low:.2} s at 500 Hz, {high:.2} s at 5 kHz (ratio {:.2})", high / low);
            assert!(high < 0.75 * low, "{percent}%: {high} vs {low}");
        }
    }

    #[test]
    fn reverb_is_ducked_while_the_dry_signal_is_dense_and_blooms_in_the_gaps() {
        let x = noise(48_000 * 6, 31).iter().map(|v| v * 0.6).collect::<Vec<_>>();
        let mut ducked = reverb_unit(60.0, 0.75);
        let mut free = reverb_unit(60.0, 0.75);
        free.duck = Ducker::new(SR, 0.0, DUCK_ATTACK_MS, DUCK_RELEASE_MS);
        let dry = 1.0;
        let wet = |fx: &mut BeatFx| -> Vec<f32> {
            let (out, _) = run(fx, &x, &x);
            out.iter().zip(&x).map(|(o, d)| o - dry * d).collect()
        };
        let (wet_ducked, wet_free) = (wet(&mut ducked), wet(&mut free));
        let depth = db(rms(&wet_ducked[48_000 * 3..]) / rms(&wet_free[48_000 * 3..]));
        eprintln!("duck depth during dense input: {depth:+.2} dB");
        assert!((-6.0..=-3.0).contains(&depth), "{depth} dB");
        let silence = vec![0.0; 48_000];
        let (gap_out, _) = run(&mut ducked, &silence, &silence);
        let early = rms(&gap_out[0..2_400]);
        let later = rms(&gap_out[19_200..24_000]);
        assert!(later > 0.0 && early > 0.0);
    }

    #[test]
    fn reverb_modulation_backs_off_as_the_tail_lengthens() {
        assert!(reverb_modulation_ms(0.0) > reverb_modulation_ms(1.0));
        assert!((0.25..=0.65).contains(&reverb_modulation_ms(0.0)) && (0.25..=0.65).contains(&reverb_modulation_ms(1.0)));
    }

    fn tail_shift_with_bend(from: f32, to: f32, ride_secs: f32, measure_at: f32, bend: Option<f32>) -> f32 {
        let hz = 3_000.0;
        let lead = 48_000 * 2;
        let total = lead + 48_000 * 3;
        let x: Vec<f32> = (0..total).map(|i| if i < lead { 0.3 * (TAU * hz * i as f32 / SR).sin() } else { 0.0 }).collect();
        let mut fx = reverb_unit(from, 0.5);
        if let Some(bend) = bend {
            fx.set_param(4, bend);
        }
        fx.wash_scale = 0.0;
        let (mut l, mut r) = (x.clone(), x.clone());
        for (b, (cl, cr)) in l.chunks_mut(BLOCK).zip(r.chunks_mut(BLOCK)).enumerate() {
            let t = (b * BLOCK) as f32 / SR - lead as f32 / SR;
            if t >= 0.0 {
                let progress = if ride_secs <= 0.0 { 1.0 } else { (t / ride_secs).min(1.0) };
                fx.set_percent(from + (to - from) * progress);
            }
            fx.process(cl, cr);
        }
        let at = lead + (measure_at * SR) as usize;
        let heard = centroid_hz(&l[at..at + 8_192], hz * 0.5, hz * 1.6);
        semitones(hz, heard)
    }

    fn tail_shift(from: f32, to: f32, ride_secs: f32, measure_at: f32) -> f32 {
        tail_shift_with_bend(from, to, ride_secs, measure_at, None)
    }

    #[test]
    fn reverb_growing_bends_the_tail_down_and_shrinking_bends_it_up_by_a_semitone_or_two() {
        let grow = tail_shift(1.0, 100.0, 0.0, 0.5);
        let shrink = tail_shift(100.0, 1.0, 0.0, 0.5);
        eprintln!("reverb fast full-range step at the default bend: grow {grow:+.2} st, shrink {shrink:+.2} st");
        assert!((-3.5..=-1.0).contains(&grow), "growing the room: {grow} st");
        assert!((0.5..=2.5).contains(&shrink), "shrinking the room: {shrink} st");
        let steps: Vec<String> = [(1.0, 10.0), (10.0, 20.0), (30.0, 40.0), (50.0, 60.0), (90.0, 100.0)]
            .iter()
            .map(|(a, b)| format!("{a}->{b} {:+.2}", tail_shift(*a, *b, 0.0, 0.5)))
            .collect();
        eprintln!("reverb one-step bends (st): {}", steps.join(", "));
        assert!(tail_shift(10.0, 20.0, 0.0, 0.5) < -0.5 && tail_shift(60.0, 50.0, 0.0, 0.5) > 0.2);
    }

    #[test]
    fn reverb_bend_parameter_scales_the_glide_depth() {
        let shift = |bend: f32| tail_shift_with_bend(1.0, 100.0, 0.0, 0.5, Some(bend)).abs();
        let (none, default, full) = (shift(0.0), shift(20.0), shift(100.0));
        eprintln!("reverb bend 0 / 20 / 100: {none:.2} / {default:.2} / {full:.2} st on a full-range step");
        assert!(none < 0.8 * default && default < 0.5 * full, "{none} {default} {full}");
        assert!(full > 5.0, "full bend is the big swoop: {full}");
        assert!(beat_fx_params(BeatFxKind::Reverb)[4].default == 20.0);
    }

    #[test]
    fn reverb_slow_percent_ride_only_drifts() {
        let ride = tail_shift(30.0, 60.0, 10.0, 1.0);
        let step = tail_shift(30.0, 60.0, 0.0, 1.0);
        eprintln!("reverb 30->60: slow 10 s ride {ride:+.2} st, instant step {step:+.2} st");
        assert!(ride.abs() < 0.5, "slow ride {ride} st");
        assert!(step < -1.0 && step.abs() > 3.0 * ride.abs(), "step {step} st vs ride {ride} st");
    }

    #[test]
    fn reverb_size_glide_stays_smooth_and_bounded() {
        let lead = 48_000 * 2;
        let x: Vec<f32> = (0..lead + 48_000 * 5).map(|i| if i < lead { 0.3 * (TAU * 3_000.0 * i as f32 / SR).sin() } else { 0.0 }).collect();
        let roughness = |jumps: bool| {
            let mut fx = reverb_unit(50.0, 0.8);
            fx.wash_scale = 0.0;
            let (mut l, mut r) = (x.clone(), x.clone());
            for (b, (cl, cr)) in l.chunks_mut(BLOCK).zip(r.chunks_mut(BLOCK)).enumerate() {
                let t = (b * BLOCK) as f32 / SR - 2.0;
                if jumps && t >= 0.0 && b % 150 == 0 {
                    fx.set_percent([1.0, 100.0, 5.0, 90.0, 1.0, 100.0][(b / 150) % 6]);
                }
                fx.process(cl, cr);
            }
            let tail = &l[lead + 2_400..lead + 48_000 * 3];
            (max_step(tail) / peak(tail), peak(tail))
        };
        let ((moving, level), (still, _)) = (roughness(true), roughness(false));
        eprintln!("reverb size glide: relative sample step {moving:.3} vs {still:.3} without, tail peak {level:.2}");
        assert!(moving < still * 1.3 + 0.01, "{moving} vs {still}");
        assert!(level < 1.0);
    }

    fn music(len: usize, amp: f32) -> Vec<f32> {
        let mut n = super::super::util::Noise::new(23);
        let mut z = 0.0f32;
        let lowpassed: Vec<f32> = (0..len).map(|_| { z += 0.04 * (n.next() - z); z }).collect();
        let level = rms(&lowpassed[len / 2..]);
        lowpassed.iter().map(|v| v * amp / level).collect()
    }

    fn wash_of(percent: f32, level: f32, x: &[f32]) -> Vec<f32> {
        let render = |scale: f32| {
            let mut fx = reverb_unit(percent, level);
            fx.wash_scale = scale;
            run(&mut fx, x, x).0
        };
        render(1.0).iter().zip(render(0.0)).map(|(a, b)| a - b).collect()
    }

    #[test]
    fn reverb_noise_wash_rises_with_depth_and_size() {
        let x = music(48_000 * 6, 0.25);
        let hiss = |percent: f32, level: f32| band_psd(&wash_of(percent, level, &x)[96_000..], 2_000.0, 16_000.0);
        let by_depth: Vec<f32> = [0.2, 0.4, 0.6, 0.8, 1.0].iter().map(|l| hiss(70.0, *l)).collect();
        let by_size: Vec<f32> = [10.0, 40.0, 70.0, 100.0].iter().map(|p| hiss(*p, 0.8)).collect();
        eprintln!("reverb wash psd above 2 kHz by depth {by_depth:?}, by size {by_size:?}");
        assert!(by_depth.windows(2).all(|w| w[1] > w[0] * 1.3), "{by_depth:?}");
        assert!(by_size.windows(2).all(|w| w[1] > w[0]), "{by_size:?}");
        assert!(by_depth[4] > by_depth[0] * 30.0, "level^2 slope");
    }

    #[test]
    fn reverb_noise_wash_stays_well_under_the_music() {
        let x = music(48_000 * 6, 0.25);
        let dry = rms(&x[96_000..]);
        let full = db(rms(&wash_of(100.0, 1.0, &x)[96_000..]) / dry);
        let typical = db(rms(&wash_of(50.0, 0.75, &x)[96_000..]) / dry);
        eprintln!("reverb wash re dry rms: 100 % at full depth {full:.1} dB, 50 % at 75 % depth {typical:.1} dB");
        assert!((-8.0..-2.0).contains(&full), "{full} dB");
        assert!(typical < full - 4.0);
    }

    #[test]
    fn reverb_noise_wash_is_ducked_by_dense_music() {
        let x = music(48_000 * 6, 0.25);
        let measure = |duck: f32| {
            let render = |scale: f32| {
                let mut fx = reverb_unit(70.0, 1.0);
                fx.wash_scale = scale;
                fx.wash_duck_scale = duck;
                run(&mut fx, &x, &x).0
            };
            let wash: Vec<f32> = render(1.0).iter().zip(render(0.0)).map(|(a, b)| a - b).collect();
            rms(&wash[96_000..])
        };
        let depth = db(measure(1.0) / measure(0.0));
        eprintln!("reverb wash duck under dense music: {depth:+.2} dB");
        assert!(depth <= -3.0, "{depth} dB");
    }

    #[test]
    fn reverb_wash_rings_out_with_the_tail_after_the_effect_is_switched_off() {
        let x = music(48_000 * 3, 0.25);
        let render = |scale: f32| {
            let mut fx = reverb_unit(70.0, 1.0);
            fx.wash_scale = scale;
            let _ = run(&mut fx, &x, &x);
            fx.set_enabled(false);
            let silence = vec![0.0; 48_000 * 10];
            run(&mut fx, &silence, &silence).0
        };
        let (with, without) = (render(1.0), render(0.0));
        let wash: Vec<f32> = with.iter().zip(&without).map(|(a, b)| a - b).collect();
        let at = |secs: f32| rms(&wash[(secs * SR) as usize..(secs * SR) as usize + 4_800]);
        eprintln!("reverb wash after off: {:.5} at 0.3 s, {:.5} at 3 s, {:.6} at 9 s", at(0.3), at(3.0), at(9.0));
        assert!(at(0.3) > 1e-3, "still there right after off: {}", at(0.3));
        assert!(at(3.0) < at(0.3) * 0.5 && at(9.0) < at(3.0) * 0.5, "decays with the tail");
    }

    #[test]
    fn reverb_gets_thinner_and_brighter_as_depth_rises() {
        let x = noise(48_000 * 6, 15);
        let ratio = |level: f32| {
            let mut fx = reverb_unit(60.0, level);
            fx.wash_scale = 0.0;
            let (out, _) = run(&mut fx, &x, &x);
            let dry = beat_mix(level).0;
            let wet: Vec<f32> = out.iter().zip(&x).map(|(o, d)| o - dry * d).collect();
            band_psd(&wet[96_000..], 4_000.0, 12_000.0) / band_psd(&wet[96_000..], 300.0, 1_000.0)
        };
        let tilt: Vec<f32> = [0.2, 0.5, 0.8, 1.0].iter().map(|l| ratio(*l)).collect();
        eprintln!("reverb wet high/mid power by depth: {tilt:?}");
        assert!(tilt.windows(2).all(|w| w[1] > w[0] * 1.4), "{tilt:?}");
    }

    #[test]
    fn reverb_breathes_gently_with_the_bar() {
        let x = noise(48_000 * 5, 21);
        let wet_rms = |start_beat: f64, level: f32| {
            let mut fx = reverb_unit(60.0, level);
            fx.wash_scale = 0.0;
            fx.set_enabled(true);
            fx.align_phase(start_beat);
            let (out, _) = run(&mut fx, &x, &x);
            let dry = beat_mix(level).0;
            let wet: Vec<f32> = out.iter().zip(&x).map(|(o, d)| o - dry * d).collect();
            rms(&wet[96_000..96_000 + 4_800])
        };
        let swing = db(wet_rms(0.0, 1.0) / wet_rms(2.0, 1.0));
        let faint = db(wet_rms(0.0, 0.3) / wet_rms(2.0, 0.3));
        eprintln!("reverb bar breathing at full depth {swing:+.2} dB peak to trough, at 30 % depth {faint:+.2} dB");
        assert!((1.4..=2.2).contains(&swing), "{swing} dB");
        assert!(faint.abs() < swing.abs() * 0.5 && faint > 0.1);
    }

    #[test]
    fn reverb_with_only_the_high_band_selected_leaves_everything_below_three_khz_dry() {
        let high_only = FxBands { low: false, mid: false, high: true };
        for hz in [100.0, 500.0, 1_000.0, 1_500.0] {
            let x = sine(48_000 * 3, hz, 0.3);
            let mut fx = banded_unit(BeatFxKind::Reverb, 1.0, 0.9);
            fx.set_bands(high_only);
            fx.set_enabled(true);
            let (out, _) = run(&mut fx, &x, &x);
            let change = db(tone_amplitude(&out[96_000..], hz) / tone_amplitude(&x[96_000..], hz));
            assert!(change.abs() < 0.5, "{hz} Hz: {change:+.2} dB");
        }
        let x = noise(48_000 * 4, 41);
        let mut fx = banded_unit(BeatFxKind::Reverb, 1.0, 0.9);
        fx.set_bands(high_only);
        fx.set_enabled(true);
        let (out, _) = run(&mut fx, &x, &x);
        for (lo, hi) in [(150.0, 600.0), (600.0, 1_200.0), (1_200.0, 2_000.0)] {
            let change = db((band_psd(&out[96_000..], lo, hi) / band_psd(&x[96_000..], lo, hi)).sqrt());
            assert!(change.abs() < 0.5, "{lo}-{hi} Hz noise: {change:+.2} dB");
        }
        let silence = vec![0.0; 48_000 * 2];
        let (tail, _) = run(&mut fx, &silence, &silence);
        let (below, above) = (band_psd(&tail[4_800..48_000], 150.0, 1_200.0), band_psd(&tail[4_800..48_000], 4_500.0, 12_000.0));
        let gap = db((above / below.max(1e-30)).sqrt());
        eprintln!("reverb high-only: tail and wash sit {gap:.1} dB above the 150-1200 Hz band, rms {:.4}", rms(&tail[4_800..48_000]));
        assert!(rms(&tail[4_800..48_000]) > 1e-3 && gap > 25.0, "gap {gap} dB");
        let wash_only = |scale: f32| {
            let mut fx = banded_unit(BeatFxKind::Reverb, 1.0, 0.9);
            fx.set_param(REVERB_VOICING_PARAM, 1.0);
            fx.set_bands(high_only);
            fx.wash_scale = scale;
            fx.set_enabled(true);
            run(&mut fx, &silence, &silence).0
        };
        let wash: Vec<f32> = wash_only(1.0).iter().zip(wash_only(0.0)).map(|(a, b)| a - b).collect();
        let (above, below) = (band_psd(&wash[24_000..], 4_500.0, 12_000.0), band_psd(&wash[24_000..], 150.0, 1_200.0));
        eprintln!("reverb high-only wash: {:.1} dB above the 150-1200 Hz band, rms {:.4}", db((above / below.max(1e-30)).sqrt()), rms(&wash[24_000..]));
        assert!(rms(&wash[24_000..]) > 1e-3 && above > below * 300.0);
    }

    #[test]
    fn sub_parameters_persist_per_kind_clamp_and_ignore_garbage() {
        let mut fx = BeatFx::new(SR);
        fx.set_kind(BeatFxKind::Echo);
        assert_eq!(&fx.params()[..2], &[42.0, 50.0]);
        fx.set_param(0, 20.0);
        fx.set_param(1, 400.0);
        fx.set_param(0, f32::NAN);
        fx.set_param(7, 3.0);
        assert_eq!(&fx.params()[..2], &[20.0, 100.0]);
        fx.set_kind(BeatFxKind::DubTape);
        assert_eq!(fx.params()[0], 45.0);
        fx.set_param(0, 999.0);
        assert_eq!(fx.params()[0], 105.0);
        fx.set_kind(BeatFxKind::Echo);
        assert_eq!(&fx.params()[..2], &[20.0, 100.0], "the echo values survive a visit to another kind");
        fx.set_kind(BeatFxKind::PulseVerb);
        fx.set_param(3, 0.7);
        assert_eq!(fx.params()[3], 1.0, "choices snap to an option");
        fx.set_kind(BeatFxKind::Reverb);
        assert_eq!(&fx.params()[..5], &[50.0, 50.0, 50.0, 0.0, 20.0], "classic voicing, decay 50, bend 20 by default");
    }

    #[test]
    fn echo_feedback_parameter_sets_how_long_the_repeats_last() {
        let tail = |feedback: f32| {
            let mut fx = unit(BeatFxKind::Echo, 0.5, 0.6);
            fx.set_param(0, feedback);
            fx.set_enabled(true);
            let x: Vec<f32> = noise(12_000, 3).into_iter().map(|v| v * 0.5).chain(vec![0.0; 48_000 * 6]).collect();
            let (out, _) = run(&mut fx, &x, &x);
            rms(&out[48_000 * 2..48_000 * 3])
        };
        let (low, mid, high) = (tail(0.0), tail(50.0), tail(100.0));
        eprintln!("echo tail at feedback 0/50/100: {low:.5} / {mid:.5} / {high:.5}");
        assert!(low < mid && mid < high);
        assert!(high > 5.0 * low);
    }

    #[test]
    fn reverb_noise_parameter_scales_the_wash_and_zero_removes_it() {
        let hiss = |noise_param: f32| {
            let mut fx = unit(BeatFxKind::Reverb, 1.0, 1.0);
            fx.set_param(REVERB_VOICING_PARAM, 1.0);
            fx.set_percent(70.0);
            fx.set_param(1, noise_param);
            fx.set_enabled(true);
            let x: Vec<f32> = noise(48_000 * 2, 4).into_iter().map(|v| v * 0.2).chain(vec![0.0; 48_000 * 3]).collect();
            let (out, _) = run(&mut fx, &x, &x);
            rms(&out[48_000 * 3..48_000 * 4])
        };
        let (none, default, lots) = (hiss(0.0), hiss(50.0), hiss(100.0));
        eprintln!("reverb wash at noise 0/50/100: {none:.5} / {default:.5} / {lots:.5}");
        assert!(none < default && default < lots);
    }

    #[test]
    fn reverb_tone_parameter_moves_the_tail_brightness() {
        let tail = |tone: f32| {
            let mut fx = unit(BeatFxKind::Reverb, 1.0, 0.8);
            fx.set_param(REVERB_VOICING_PARAM, 1.0);
            fx.set_percent(60.0);
            fx.wash_scale = 0.0;
            fx.set_param(0, tone);
            fx.set_enabled(true);
            let x: Vec<f32> = noise(12_000, 4).into_iter().map(|v| v * 0.4).chain(vec![0.0; 48_000 * 4]).collect();
            let (out, _) = run(&mut fx, &x, &x);
            centroid_hz(&out[48_000 * 2..48_000 * 3], 300.0, 15_000.0)
        };
        let (dark, bright) = (tail(0.0), tail(100.0));
        eprintln!("reverb tail centroid dark/bright: {dark:.0} / {bright:.0} Hz");
        assert!(dark < bright);
    }

    #[test]
    fn modulation_feedback_and_resonance_parameters_reshape_the_effect_without_breaking_it() {
        for (kind, low, high) in [(BeatFxKind::Flanger, 0.0, 100.0), (BeatFxKind::Phaser, 0.0, 100.0), (BeatFxKind::FilterSweep, 0.0, 100.0)] {
            let mut outs = Vec::new();
            for value in [low, 50.0, high] {
                let mut fx = unit(kind, 0.5, 1.0);
                fx.set_param(0, value);
                fx.set_enabled(true);
                let x = sine(96_000, 700.0, 1.0);
                let (out, _) = run(&mut fx, &x, &x);
                assert!(out.iter().all(|v| v.is_finite()) && peak(&out) < 3.0, "{kind:?} at {value}");
                outs.push(out);
            }
            let diff = |a: &[f32], b: &[f32]| a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f32>();
            assert!(diff(&outs[0], &outs[2]) > 1.0, "{kind:?} responds to its parameter");
        }
    }
}
