use std::sync::Arc;

use deckforge_engine::{secondary_channel, SecondaryRenderer, SecondaryShared, SecondarySource, SecondaryTap};

const TONE_HZ: f64 = 1_000.0;
const AMPLITUDE: f64 = 0.5;
const UNITY_LEVEL: f32 = 0.8;
const RING: usize = 16_384;

struct Setup {
    in_rate: u32,
    out_rate: u32,
    consumer_ppm: f64,
    producer_block: usize,
    consumer_block: usize,
    secs: f64,
    converged_by: f64,
    jitter_ms: f64,
}

#[derive(Default)]
struct Outcome {
    underruns_at_convergence: u64,
    underruns_total: u64,
    worst_fill_error_ms: f32,
    transient_fill_error_ms: f32,
    min_queued_ever: usize,
    max_fill_ms: f32,
    min_queued: usize,
    max_step: f64,
    tone_hz_real_time: f64,
    final_drift_ppm: f32,
    target_ms: f32,
}

struct ToneSource {
    phase: f64,
    step: f64,
}

impl ToneSource {
    fn new(rate: u32) -> Self {
        Self { phase: 0.0, step: std::f64::consts::TAU * TONE_HZ / rate as f64 }
    }

    fn push_block(&mut self, tap: &mut SecondaryTap, frames: usize, rate: u32) {
        self.push_block_at(tap, frames, rate, None);
    }

    fn push_block_at(&mut self, tap: &mut SecondaryTap, frames: usize, rate: u32, now_nanos: Option<u64>) {
        for _ in 0..frames {
            let s = (self.phase.sin() * AMPLITUDE) as f32;
            tap.push([s, s]);
            self.phase += self.step;
            if self.phase > std::f64::consts::TAU {
                self.phase -= std::f64::consts::TAU;
            }
        }
        match now_nanos {
            Some(now) => tap.end_block_at(frames, rate, now),
            None => tap.end_block(frames, rate),
        }
    }
}

/// Estimates tone frequency from upward zero crossings with linear interpolation.
#[derive(Default)]
struct CrossingCounter {
    prev: f32,
    index: u64,
    first: Option<f64>,
    last: f64,
    count: u64,
}

impl CrossingCounter {
    fn feed(&mut self, x: f32) {
        if self.prev <= 0.0 && x > 0.0 {
            let pos = self.index as f64 - 1.0 + (-self.prev / (x - self.prev)) as f64;
            self.first.get_or_insert(pos);
            self.last = pos;
            self.count += 1;
        }
        self.prev = x;
        self.index += 1;
    }

    fn cycles_per_sample(&self) -> f64 {
        (self.count - 1) as f64 / (self.last - self.first.unwrap())
    }
}

struct Lcg(u64);

impl Lcg {
    fn symmetric(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
    }
}

fn simulate(setup: &Setup) -> Outcome {
    let shared = SecondaryShared::new(SecondarySource::Master, UNITY_LEVEL);
    let (mut tap, mut renderer) = secondary_channel(shared.clone(), setup.in_rate, setup.out_rate, RING);
    let mut source = ToneSource::new(setup.in_rate);
    let producer_period = setup.producer_block as f64 / setup.in_rate as f64;
    let actual_out_rate = setup.out_rate as f64 * (1.0 + setup.consumer_ppm * 1e-6);
    let consumer_period = setup.consumer_block as f64 / actual_out_rate;
    let (mut next_producer, mut next_consumer) = (0.0, 0.0);
    let mut rng = Lcg(0x5eed);
    let jitter = |rng: &mut Lcg| rng.symmetric() * setup.jitter_ms * 1e-3;
    let (mut jp, mut jc) = (0.0, 0.0);
    let mut buf = vec![0.0f32; setup.consumer_block * 2];
    let mut out = Outcome { min_queued: usize::MAX, min_queued_ever: usize::MAX, ..Default::default() };
    let mut counter = CrossingCounter::default();
    let measure_from = setup.secs - 30.0;
    let mut prev = 0.0f32;
    let mut have_prev = false;
    let mut converged_recorded = false;

    while next_consumer < setup.secs {
        if next_producer + jp <= next_consumer + jc {
            let stamp = ((next_producer + jp).max(0.0) * 1e9) as u64;
            source.push_block_at(&mut tap, setup.producer_block, setup.in_rate, Some(stamp));
            next_producer += producer_period;
            jp = jitter(&mut rng);
            continue;
        }
        let t = next_consumer;
        let stamp = ((t + jc).max(0.0) * 1e9) as u64;
        let queued = tap.queued();
        renderer.render_at(&mut buf, stamp);
        next_consumer += consumer_period;
        jc = jitter(&mut rng);
        let stats = shared.stats();
        out.target_ms = stats.target_ms;
        if t >= 1.0 {
            out.transient_fill_error_ms = out.transient_fill_error_ms.max((stats.fill_ms - stats.target_ms).abs());
            out.min_queued_ever = out.min_queued_ever.min(queued);
        }
        if t >= setup.converged_by {
            if !converged_recorded {
                converged_recorded = true;
                out.underruns_at_convergence = stats.underruns;
            }
            out.worst_fill_error_ms = out.worst_fill_error_ms.max((stats.fill_ms - stats.target_ms).abs());
            out.max_fill_ms = out.max_fill_ms.max(stats.fill_ms);
            out.min_queued = out.min_queued.min(queued);
            for frame in buf.as_chunks::<2>().0 {
                if have_prev {
                    out.max_step = out.max_step.max((frame[0] - prev).abs() as f64);
                }
                prev = frame[0];
                have_prev = true;
            }
        }
        if t >= measure_from {
            buf.as_chunks::<2>().0.iter().for_each(|f| counter.feed(f[0]));
        }
    }
    let stats = shared.stats();
    out.underruns_total = stats.underruns;
    out.final_drift_ppm = stats.drift_ppm;
    out.tone_hz_real_time = counter.cycles_per_sample() * actual_out_rate;
    out
}

fn assert_healthy(setup: &Setup, out: &Outcome) {
    let name = format!("{}->{} @ {:+} ppm", setup.in_rate, setup.out_rate, setup.consumer_ppm);
    assert_eq!(out.underruns_total, 0, "{name}: underruns");
    assert_eq!(out.underruns_at_convergence, out.underruns_total, "{name}: underrun after convergence");
    assert!(out.worst_fill_error_ms < 3.0, "{name}: fill strayed {:.2} ms from the {:.1} ms target", out.worst_fill_error_ms, out.target_ms);
    assert!(out.max_fill_ms < out.target_ms * 2.0, "{name}: fill grew to {:.1} ms", out.max_fill_ms);
    assert!(out.min_queued > 0 && out.min_queued_ever > 0, "{name}: ring ran dry");
    assert!(out.transient_fill_error_ms < 8.0, "{name}: acquisition overshoot {:.2} ms", out.transient_fill_error_ms);
    let bound = AMPLITUDE * std::f64::consts::TAU * TONE_HZ / setup.out_rate as f64 * 1.03;
    assert!(out.max_step < bound, "{name}: discontinuity, step {:.5} > {:.5}", out.max_step, bound);
    let error_ppm = (out.tone_hz_real_time / TONE_HZ - 1.0).abs() * 1e6;
    assert!(error_ppm < 100.0, "{name}: tone {:.4} Hz, {:.0} ppm off", out.tone_hz_real_time, error_ppm);
    assert!(out.final_drift_ppm.abs() <= 1000.0, "{name}: correction beyond clamp");
}

fn run(in_rate: u32, out_rate: u32, ppm: f64, producer_block: usize, consumer_block: usize) {
    run_with_jitter(in_rate, out_rate, ppm, producer_block, consumer_block, 0.0);
}

fn run_with_jitter(in_rate: u32, out_rate: u32, ppm: f64, producer_block: usize, consumer_block: usize, jitter_ms: f64) {
    let setup = Setup {
        in_rate,
        out_rate,
        consumer_ppm: ppm,
        producer_block,
        consumer_block,
        secs: 660.0,
        converged_by: 120.0,
        jitter_ms,
    };
    let out = simulate(&setup);
    println!(
        "{in_rate}->{out_rate} {ppm:+} ppm: target {:.1} ms, worst fill error {:.2} ms, max fill {:.1} ms, min queued {} (ever {}), acquisition peak {:.2} ms, step {:.5}, tone {:.4} Hz, final drift {:.1} ppm, underruns {}",
        out.target_ms, out.worst_fill_error_ms, out.max_fill_ms, out.min_queued, out.min_queued_ever, out.transient_fill_error_ms, out.max_step, out.tone_hz_real_time, out.final_drift_ppm, out.underruns_total
    );
    assert_healthy(&setup, &out);
    assert!((out.final_drift_ppm as f64 + ppm).abs() < 60.0 + jitter_ms * 100.0, "drift estimate {} vs {}", out.final_drift_ppm, -ppm);
}

#[test]
fn consumer_clock_300_ppm_fast_converges_at_48k() {
    run(48_000, 48_000, 300.0, 256, 256);
}

#[test]
fn consumer_clock_300_ppm_slow_converges_at_48k() {
    run(48_000, 48_000, -300.0, 256, 512);
}

#[test]
fn rate_mismatch_48k_to_44_1k_converges_with_drift() {
    run(48_000, 44_100, 300.0, 256, 512);
}

#[test]
fn rate_mismatch_48k_to_44_1k_converges_without_drift() {
    run(48_000, 44_100, 0.0, 512, 441);
}

#[test]
fn large_device_buffers_still_hold_without_underruns() {
    run(48_000, 44_100, -200.0, 256, 1_024);
}

#[test]
fn callback_timing_jitter_does_not_upset_the_loop() {
    run_with_jitter(48_000, 44_100, 300.0, 256, 512, 1.0);
    run_with_jitter(48_000, 48_000, -300.0, 256, 256, 1.0);
}

#[test]
fn upsampling_44_1k_to_48k_with_drift() {
    run(44_100, 48_000, 150.0, 256, 256);
}

struct Rig {
    tap: SecondaryTap,
    renderer: SecondaryRenderer,
    shared: Arc<SecondaryShared>,
    source: ToneSource,
    rate: u32,
    last: f32,
    owed: f64,
}

impl Rig {
    fn new(rate: u32, level: f32) -> Self {
        let shared = SecondaryShared::new(SecondarySource::Master, level);
        let (tap, renderer) = secondary_channel(shared.clone(), rate, 48_000, RING);
        Self { tap, renderer, shared, source: ToneSource::new(rate), rate, last: 0.0, owed: 0.0 }
    }

    fn switch_source_rate(&mut self, rate: u32) {
        self.source = ToneSource::new(rate);
        self.rate = rate;
    }

    /// Runs 512-frame consumer iterations, each fed one device period of source audio, and returns the largest sample step and the block peaks.
    fn run(&mut self, blocks: usize, feed: bool) -> (f64, Vec<f32>) {
        let mut buf = vec![0.0f32; 1024];
        let mut max_step = 0.0f64;
        let mut peaks = Vec::with_capacity(blocks);
        for _ in 0..blocks {
            if feed {
                self.owed += 512.0 * self.rate as f64 / 48_000.0;
                let frames = self.owed as usize;
                self.owed -= frames as f64;
                self.source.push_block(&mut self.tap, frames, self.rate);
            }
            self.renderer.render(&mut buf);
            for frame in buf.as_chunks::<2>().0 {
                max_step = max_step.max((frame[0] - self.last).abs() as f64);
                self.last = frame[0];
            }
            peaks.push(buf.iter().fold(0.0f32, |m, v| m.max(v.abs())));
        }
        (max_step, peaks)
    }
}

fn step_bound() -> f64 {
    AMPLITUDE * std::f64::consts::TAU * TONE_HZ / 48_000.0 * 1.03
}

#[test]
fn underrun_fades_out_counts_once_and_recovers() {
    let mut rig = Rig::new(48_000, UNITY_LEVEL);
    rig.run(400, true);
    assert_eq!(rig.shared.stats().underruns, 0);
    assert!(rig.renderer.is_playing());

    let (stalled_step, _) = rig.run(40, false);
    assert_eq!(rig.shared.stats().underruns, 1);
    assert!(!rig.renderer.is_playing());
    assert!(rig.last.abs() < 1e-3, "output must have faded to silence, got {}", rig.last);

    let (resumed_step, _) = rig.run(400, true);
    assert!(rig.renderer.is_playing());
    assert_eq!(rig.shared.stats().underruns, 1);
    assert!(stalled_step < step_bound() && resumed_step < step_bound(), "steps {stalled_step} {resumed_step}");
}

#[test]
fn primary_rate_change_flushes_and_resumes_with_the_new_rate() {
    let mut rig = Rig::new(48_000, UNITY_LEVEL);
    rig.run(300, true);

    rig.switch_source_rate(44_100);
    let (step, _) = rig.run(600, true);
    assert!(rig.renderer.is_playing());
    assert_eq!(rig.shared.stats().underruns, 0);
    assert!(step < step_bound(), "step {step} vs {}", step_bound());
}

#[test]
fn startup_is_silent_then_fades_in_over_hundreds_of_milliseconds() {
    let mut rig = Rig::new(48_000, UNITY_LEVEL);
    let (_, peaks) = rig.run(200, true);
    assert!(peaks[0] < 1e-6, "first block must be silent");
    let full = peaks.iter().position(|&p| p > 0.9 * AMPLITUDE as f32).expect("reaches full level");
    let fade_ms = full as f64 * 512.0 / 48.0;
    assert!(fade_ms > 300.0, "fade-in finished after only {fade_ms} ms");
    assert!(peaks[..full].windows(2).all(|w| w[1] >= w[0] * 0.9), "fade-in must rise steadily");
}

#[test]
fn level_scales_the_secondary_output_with_the_master_taper() {
    let measure = |level: f32| {
        let mut rig = Rig::new(48_000, level);
        let (_, peaks) = rig.run(800, true);
        peaks[700..].iter().fold(0.0f32, |m, p| m.max(*p)) as f64
    };
    let unity = measure(0.8);
    assert!((unity - AMPLITUDE).abs() < 0.01, "unity level should pass the bus unchanged, got {unity}");
    let half = measure(0.4);
    assert!((half / unity - 0.25).abs() < 0.01, "0.4 maps to a quarter amplitude, got {}", half / unity);
    assert_eq!(measure(0.0), 0.0);
}
