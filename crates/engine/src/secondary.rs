//! Secondary output: mirrors one engine bus to a second device on its own clock.
//! The primary callback fills an SPSC ring; the secondary callback drains it through an adaptive resampler.

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Instant;

use deckforge_dsp::Smoothed;
use serde::{Deserialize, Serialize};

use crate::mixer::master_gain;

pub const DEFAULT_LEVEL: f32 = 0.8;
pub const RING_FRAMES: usize = 16_384;
const BASE_TARGET_MS: f64 = 15.0;
const MAX_DRIFT: f64 = 1e-3;
const MAX_SLEW_PER_SEC: f64 = 4e-4;
const FILL_FILTER_SECS: f64 = 0.75;
const KP: f64 = 0.36;
const KI: f64 = 0.0225;
const STARTUP_FADE_MS: f32 = 400.0;
const RESTART_FADE_MS: f32 = 8.0;
const LEVEL_SMOOTH_MS: f32 = 30.0;
const HOLD_DECAY_MS: f64 = 2.0;
const OVERFILL_FACTOR: f64 = 3.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SecondarySource {
    #[default]
    Master,
    Cue,
}

impl SecondarySource {
    fn to_u8(self) -> u8 {
        self as u8
    }

    fn from_u8(v: u8) -> Self {
        if v == Self::Cue as u8 { Self::Cue } else { Self::Master }
    }
}

fn default_source() -> SecondarySource {
    SecondarySource::Master
}

fn default_level() -> f32 {
    DEFAULT_LEVEL
}

fn default_channels() -> [usize; 2] {
    [0, 1]
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecondaryOutputRequest {
    pub device_id: String,
    #[serde(default = "default_source")]
    pub source: SecondarySource,
    #[serde(default = "default_level")]
    pub level: f32,
    #[serde(default = "default_channels")]
    pub channels: [usize; 2],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SecondaryState {
    Running,
    Lost,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecondaryOutputStatus {
    pub device_id: String,
    pub device_name: String,
    pub source: SecondarySource,
    pub level: f32,
    pub state: SecondaryState,
    pub sample_rate: u32,
    pub latency_ms: f32,
    pub drift_ppm: f32,
    pub underruns: u64,
    pub channels: [usize; 2],
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SecondaryStats {
    pub fill_ms: f32,
    pub target_ms: f32,
    pub latency_ms: f32,
    pub drift_ppm: f32,
    pub underruns: u64,
    pub overruns: u64,
    pub callbacks: u64,
}

/// State shared between the control thread, the primary callback and the secondary callback.
pub struct SecondaryShared {
    epoch: Instant,
    burst_nanos: AtomicU64,
    burst_frames: AtomicU32,
    source: AtomicU8,
    level: AtomicU32,
    source_rate: AtomicU32,
    producer_block: AtomicU32,
    overruns: AtomicU64,
    lost: AtomicBool,
    fill_micros: AtomicU32,
    target_micros: AtomicU32,
    block_micros: AtomicU32,
    drift_ppb: AtomicI32,
    underruns: AtomicU64,
    callbacks: AtomicU64,
}

impl SecondaryShared {
    pub fn new(source: SecondarySource, level: f32) -> Arc<Self> {
        Arc::new(Self {
            epoch: Instant::now(),
            burst_nanos: AtomicU64::new(0),
            burst_frames: AtomicU32::new(0),
            source: AtomicU8::new(source.to_u8()),
            level: AtomicU32::new(level.clamp(0.0, 1.0).to_bits()),
            source_rate: AtomicU32::new(0),
            producer_block: AtomicU32::new(0),
            overruns: AtomicU64::new(0),
            lost: AtomicBool::new(false),
            fill_micros: AtomicU32::new(0),
            target_micros: AtomicU32::new(0),
            block_micros: AtomicU32::new(0),
            drift_ppb: AtomicI32::new(0),
            underruns: AtomicU64::new(0),
            callbacks: AtomicU64::new(0),
        })
    }

    pub fn now_nanos(&self) -> u64 {
        self.epoch.elapsed().as_nanos() as u64
    }

    pub fn source(&self) -> SecondarySource {
        SecondarySource::from_u8(self.source.load(Ordering::Relaxed))
    }

    pub fn set_source(&self, source: SecondarySource) {
        self.source.store(source.to_u8(), Ordering::Relaxed);
    }

    pub fn level(&self) -> f32 {
        f32::from_bits(self.level.load(Ordering::Relaxed))
    }

    pub fn set_level(&self, level: f32) {
        self.level.store(level.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }

    pub fn mark_lost(&self) {
        self.lost.store(true, Ordering::Release);
    }

    pub fn clear_lost(&self) {
        self.lost.store(false, Ordering::Release);
    }

    pub fn take_lost(&self) -> bool {
        self.lost.swap(false, Ordering::AcqRel)
    }

    pub fn reset_stats(&self) {
        self.producer_block.store(0, Ordering::Relaxed);
        self.burst_frames.store(0, Ordering::Relaxed);
        self.overruns.store(0, Ordering::Relaxed);
        self.fill_micros.store(0, Ordering::Relaxed);
        self.target_micros.store(0, Ordering::Relaxed);
        self.block_micros.store(0, Ordering::Relaxed);
        self.drift_ppb.store(0, Ordering::Relaxed);
        self.underruns.store(0, Ordering::Relaxed);
        self.callbacks.store(0, Ordering::Relaxed);
    }

    pub fn stats(&self) -> SecondaryStats {
        let fill_ms = self.fill_micros.load(Ordering::Relaxed) as f32 / 1000.0;
        let block_ms = self.block_micros.load(Ordering::Relaxed) as f32 / 1000.0;
        SecondaryStats {
            fill_ms,
            target_ms: self.target_micros.load(Ordering::Relaxed) as f32 / 1000.0,
            latency_ms: fill_ms + block_ms,
            drift_ppm: self.drift_ppb.load(Ordering::Relaxed) as f32 / 1000.0,
            underruns: self.underruns.load(Ordering::Relaxed),
            overruns: self.overruns.load(Ordering::Relaxed),
            callbacks: self.callbacks.load(Ordering::Relaxed),
        }
    }
}

/// Producer half, owned by the primary engine; writes the selected bus after level, limiter and fade.
pub struct SecondaryTap {
    producer: rtrb::Producer<[f32; 2]>,
    shared: Arc<SecondaryShared>,
    dropped: u64,
}

impl SecondaryTap {
    #[inline]
    pub fn source(&self) -> SecondarySource {
        self.shared.source()
    }

    #[inline]
    pub fn push(&mut self, frame: [f32; 2]) {
        if self.producer.push(frame).is_err() {
            self.dropped += 1;
        }
    }

    /// Frames currently queued for the secondary output.
    pub fn queued(&self) -> usize {
        self.producer.buffer().capacity() - self.producer.slots()
    }

    pub fn end_block(&mut self, frames: usize, sample_rate: u32) {
        let now = self.shared.now_nanos();
        self.end_block_at(frames, sample_rate, now);
    }

    /// Like `end_block` with an explicit timestamp from `SecondaryShared::now_nanos`'s clock.
    pub fn end_block_at(&mut self, frames: usize, sample_rate: u32, now_nanos: u64) {
        self.shared.burst_nanos.store(now_nanos, Ordering::Release);
        self.shared.burst_frames.store(frames as u32, Ordering::Release);
        self.shared.source_rate.store(sample_rate, Ordering::Relaxed);
        self.shared.producer_block.fetch_max(frames as u32, Ordering::Relaxed);
        if self.dropped > 0 {
            self.shared.overruns.fetch_add(self.dropped, Ordering::Relaxed);
            self.dropped = 0;
        }
    }
}

/// Builds a ring between a primary engine running at `source_rate` and a device running at `out_rate`.
pub fn secondary_channel(
    shared: Arc<SecondaryShared>,
    source_rate: u32,
    out_rate: u32,
    capacity: usize,
) -> (SecondaryTap, SecondaryRenderer) {
    let (producer, consumer) = rtrb::RingBuffer::new(capacity);
    shared.source_rate.store(source_rate, Ordering::Relaxed);
    let tap = SecondaryTap { producer, shared: shared.clone(), dropped: 0 };
    (tap, SecondaryRenderer::new(consumer, shared, out_rate, capacity))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Priming,
    Playing,
}

/// Consumer half, owned by the secondary callback: adaptive cubic resampler steered by ring fill.
pub struct SecondaryRenderer {
    consumer: rtrb::Consumer<[f32; 2]>,
    shared: Arc<SecondaryShared>,
    capacity: usize,
    out_rate: f64,
    in_rate: f64,
    window: [[f32; 2]; 4],
    frac: f64,
    ratio: f64,
    ratio_step: f64,
    phase: Phase,
    started: bool,
    hold: [f32; 2],
    hold_decay: f32,
    run_gain: Smoothed,
    startup_gain: Smoothed,
    level_gain: Smoothed,
    fill: f64,
    target: f64,
    integral: f64,
    drift: f64,
    max_block: usize,
}

impl SecondaryRenderer {
    fn new(consumer: rtrb::Consumer<[f32; 2]>, shared: Arc<SecondaryShared>, out_rate: u32, capacity: usize) -> Self {
        let sr = out_rate as f32;
        let in_rate = shared.source_rate.load(Ordering::Relaxed).max(1) as f64;
        let level = master_gain(shared.level());
        let level_gain = Smoothed::new(level, sr, LEVEL_SMOOTH_MS);
        Self {
            consumer,
            shared,
            capacity,
            out_rate: out_rate as f64,
            in_rate,
            window: [[0.0; 2]; 4],
            frac: 0.0,
            ratio: in_rate / out_rate as f64,
            ratio_step: 0.0,
            phase: Phase::Priming,
            started: false,
            hold: [0.0; 2],
            hold_decay: (-1.0 / (HOLD_DECAY_MS * 0.001 * out_rate as f64)).exp() as f32,
            run_gain: Smoothed::new(0.0, sr, RESTART_FADE_MS),
            startup_gain: Smoothed::new(0.0, sr, STARTUP_FADE_MS),
            level_gain,
            fill: 0.0,
            target: 0.0,
            integral: 0.0,
            drift: 0.0,
            max_block: 0,
        }
    }

    pub fn is_playing(&self) -> bool {
        self.phase == Phase::Playing
    }

    /// Call once at the start of every device callback with the number of frames it wants.
    pub fn begin_block(&mut self, frames: usize) {
        let now = self.shared.now_nanos();
        self.begin_block_at(frames, now);
    }

    /// Like `begin_block` with an explicit timestamp from `SecondaryShared::now_nanos`'s clock.
    pub fn begin_block_at(&mut self, frames: usize, now_nanos: u64) {
        if frames == 0 {
            return;
        }
        self.shared.callbacks.fetch_add(1, Ordering::Relaxed);
        self.max_block = self.max_block.max(frames);
        self.follow_source_rate();
        self.level_gain.set(master_gain(self.shared.level()));
        let nominal = self.in_rate / self.out_rate;
        self.target = self.target_frames(nominal);
        let available = self.consumer.slots();
        let correction = self.burst_correction(now_nanos);
        let smooth_fill = available as f64 - correction;
        match self.phase {
            Phase::Priming if available as f64 >= self.target => self.start_playing(nominal, correction),
            Phase::Priming => {}
            Phase::Playing => self.steer(frames, available, smooth_fill, nominal),
        }
        self.publish(frames);
    }

    /// Next output frame; call exactly `frames` times after `begin_block`.
    #[inline]
    pub fn next_frame(&mut self) -> [f32; 2] {
        let signal = match self.phase {
            Phase::Playing => self.playing_frame(),
            Phase::Priming => self.decay_hold(),
        };
        let g = self.level_gain.next() * self.startup_gain.next();
        [signal[0] * g, signal[1] * g]
    }

    /// Renders interleaved stereo; convenience wrapper around `begin_block` and `next_frame`.
    pub fn render(&mut self, out: &mut [f32]) {
        let now = self.shared.now_nanos();
        self.render_at(out, now);
    }

    pub fn render_at(&mut self, out: &mut [f32], now_nanos: u64) {
        self.begin_block_at(out.len() / 2, now_nanos);
        for frame in out.as_chunks_mut::<2>().0 {
            *frame = self.next_frame();
        }
    }

    fn follow_source_rate(&mut self) {
        let source = self.shared.source_rate.load(Ordering::Relaxed) as f64;
        if source > 0.0 && (source - self.in_rate).abs() > 0.5 {
            self.in_rate = source;
            while self.consumer.pop().is_ok() {}
            self.phase = Phase::Priming;
            self.fill = 0.0;
        }
    }

    /// Producer writes arrive in bursts; this removes the burst sawtooth so the fill estimate does not beat against it.
    fn burst_correction(&self, now_nanos: u64) -> f64 {
        let frames = self.shared.burst_frames.load(Ordering::Acquire) as f64;
        if frames == 0.0 {
            return 0.0;
        }
        let since = now_nanos.saturating_sub(self.shared.burst_nanos.load(Ordering::Acquire)) as f64 * 1e-9;
        let phase = (since * self.in_rate / frames).clamp(0.0, 1.0);
        frames * (0.5 - phase)
    }

    fn target_frames(&self, nominal: f64) -> f64 {
        let base = BASE_TARGET_MS * 0.001 * self.in_rate;
        let blocks = 1.5 * self.max_block as f64 * nominal + 1.5 * self.shared.producer_block.load(Ordering::Relaxed) as f64;
        base.max(blocks).min(self.capacity as f64 / 4.0)
    }

    fn start_playing(&mut self, nominal: f64, correction: f64) {
        let keep = self.target + correction + self.window.len() as f64;
        while self.consumer.slots() as f64 > keep && self.consumer.pop().is_ok() {}
        for slot in &mut self.window {
            *slot = self.consumer.pop().unwrap_or_default();
        }
        self.frac = 0.0;
        self.fill = self.target;
        self.ratio = nominal * (1.0 + self.drift);
        self.ratio_step = 0.0;
        self.run_gain.reset(0.0);
        self.run_gain.set(1.0);
        if !self.started {
            self.started = true;
            self.startup_gain.set(1.0);
        }
        self.phase = Phase::Playing;
    }

    fn steer(&mut self, frames: usize, available: usize, smooth_fill: f64, nominal: f64) {
        let dt = frames as f64 / self.out_rate;
        if available as f64 > self.target * OVERFILL_FACTOR {
            while self.consumer.slots() as f64 > self.target && self.consumer.pop().is_ok() {}
            self.fill = self.target;
            self.run_gain.reset(0.0);
            self.run_gain.set(1.0);
        } else {
            let alpha = 1.0 - (-dt / FILL_FILTER_SECS).exp();
            self.fill += alpha * (smooth_fill - self.fill);
        }
        let error = (self.fill - self.target) / self.in_rate;
        self.integral = (self.integral + KI * error * dt).clamp(-MAX_DRIFT, MAX_DRIFT);
        let wanted = (KP * error + self.integral).clamp(-MAX_DRIFT, MAX_DRIFT);
        let max_step = MAX_SLEW_PER_SEC * dt;
        self.drift += (wanted - self.drift).clamp(-max_step, max_step);
        self.ratio_step = (nominal * (1.0 + self.drift) - self.ratio) / frames as f64;
    }

    fn publish(&self, frames: usize) {
        let to_micros = |frames: f64, rate: f64| (frames / rate * 1e6) as u32;
        self.shared.fill_micros.store(to_micros(self.fill, self.in_rate), Ordering::Relaxed);
        self.shared.target_micros.store(to_micros(self.target, self.in_rate), Ordering::Relaxed);
        self.shared.block_micros.store(to_micros(frames as f64, self.out_rate), Ordering::Relaxed);
        self.shared.drift_ppb.store((self.drift * 1e9) as i32, Ordering::Relaxed);
    }

    #[inline]
    fn decay_hold(&mut self) -> [f32; 2] {
        self.hold = [self.hold[0] * self.hold_decay, self.hold[1] * self.hold_decay];
        self.hold
    }

    #[inline]
    fn playing_frame(&mut self) -> [f32; 2] {
        while self.frac >= 1.0 {
            let Ok(next) = self.consumer.pop() else {
                self.phase = Phase::Priming;
                self.shared.underruns.fetch_add(1, Ordering::Relaxed);
                return self.decay_hold();
            };
            self.window = [self.window[1], self.window[2], self.window[3], next];
            self.frac -= 1.0;
        }
        let t = self.frac as f32;
        let gain = self.run_gain.next();
        let w = &self.window;
        let out = [
            cubic([w[0][0], w[1][0], w[2][0], w[3][0]], t) * gain,
            cubic([w[0][1], w[1][1], w[2][1], w[3][1]], t) * gain,
        ];
        self.hold = out;
        self.frac += self.ratio;
        self.ratio += self.ratio_step;
        out
    }
}

#[inline]
fn cubic(p: [f32; 4], t: f32) -> f32 {
    let a = -0.5 * p[0] + 1.5 * p[1] - 1.5 * p[2] + 0.5 * p[3];
    let b = p[0] - 2.5 * p[1] + 2.0 * p[2] - 0.5 * p[3];
    let c = -0.5 * p[0] + 0.5 * p[2];
    ((a * t + b) * t + c) * t + p[1]
}
