use std::sync::Arc;

use deckforge_common::{Beatgrid, BeatFxKind, ColorFxKind, FxBands, FxTarget, TrackAudio, HOT_CUE_SLOTS};
use deckforge_engine::{create_engine, Command, EngineHandle, LoadRequest, MixerParam, RtEngine, Routing};

const SR: u32 = 48_000;
const BLOCK: usize = 512;
const CH: usize = 4;
const A_HZ: f64 = 220.0;
const B_HZ: f64 = 1_250.0;

fn tone(freq: f64, secs: f64) -> Arc<TrackAudio> {
    let n = (secs * SR as f64) as usize;
    let samples = (0..n)
        .flat_map(|i| {
            let s = ((std::f64::consts::TAU * freq * i as f64 / SR as f64).sin() * 0.5) as f32;
            [s, s]
        })
        .collect();
    Arc::new(TrackAudio::new(SR, samples))
}

fn load(h: &EngineHandle, deck: usize, track: Arc<TrackAudio>, grid: Option<Beatgrid>) {
    let request = Box::new(LoadRequest { track, token: deck as u64 + 1, grid, cue_secs: Some(0.0), hot_cues: [None; HOT_CUE_SLOTS] });
    assert!(h.send(Command::Load { deck, request }));
}

fn rig(grids: [Option<Beatgrid>; 2]) -> (EngineHandle, RtEngine) {
    let (h, mut rt) = create_engine(SR);
    rt.reconfigure(SR, Routing::for_channel_count(CH));
    load(&h, 0, tone(A_HZ, 60.0), grids[0]);
    load(&h, 1, tone(B_HZ, 60.0), grids[1]);
    for channel in 0..2 {
        h.send(Command::Mixer { channel, param: MixerParam::Fader(1.0) });
    }
    h.send(Command::MasterLevel(0.8));
    h.send(Command::Play { deck: 0 });
    h.send(Command::Play { deck: 1 });
    (h, rt)
}

fn run(rt: &mut RtEngine, secs: f64) -> Vec<f32> {
    let blocks = ((secs * SR as f64) as usize).div_ceil(BLOCK);
    let mut all = Vec::with_capacity(blocks * BLOCK * CH);
    let mut buf = vec![0.0; BLOCK * CH];
    for _ in 0..blocks {
        rt.process(&mut buf, CH);
        all.extend_from_slice(&buf);
    }
    all
}

fn channel(out: &[f32], ch: usize) -> Vec<f32> {
    out.chunks(CH).map(|f| f[ch]).collect()
}

fn rms(v: &[f32]) -> f32 {
    (v.iter().map(|x| x * x).sum::<f32>() / v.len().max(1) as f32).sqrt()
}

fn max_diff(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).fold(0.0, |m, (x, y)| m.max((x - y).abs()))
}

fn max_step(v: &[f32]) -> f32 {
    v.windows(2).fold(0.0, |m, w| m.max((w[1] - w[0]).abs()))
}

fn configure_trans(h: &EngineHandle, target: FxTarget) {
    h.send(Command::BeatFxKind(BeatFxKind::Trans));
    h.send(Command::BeatFxTarget(target));
    h.send(Command::BeatFxBeats(0.5));
    h.send(Command::BeatFxLevel(1.0));
}

const WARMUP: f64 = 0.6;

fn baseline_and_fx(target: FxTarget, setup: impl Fn(&EngineHandle)) -> (Vec<f32>, Vec<f32>) {
    let (h0, mut rt0) = rig([None, None]);
    setup(&h0);
    let _ = run(&mut rt0, WARMUP);
    let baseline = run(&mut rt0, 1.5);

    let (h1, mut rt1) = rig([None, None]);
    setup(&h1);
    configure_trans(&h1, target);
    h1.send(Command::BeatFxEnabled(true));
    let _ = run(&mut rt1, WARMUP);
    let with_fx = run(&mut rt1, 1.5);
    (baseline, with_fx)
}

#[test]
fn channel_fx_on_a_leaves_channel_b_untouched() {
    let setup = |h: &EngineHandle| {
        h.send(Command::Mixer { channel: 0, param: MixerParam::Fader(0.0) });
    };
    let (baseline, with_fx) = baseline_and_fx(FxTarget::A, setup);
    assert!(rms(&channel(&baseline, 0)) > 0.1, "B audible");
    assert!(max_diff(&baseline, &with_fx) < 1e-6, "FX on A cannot change B: {}", max_diff(&baseline, &with_fx));

    let setup_b = |h: &EngineHandle| {
        h.send(Command::Mixer { channel: 1, param: MixerParam::Fader(0.0) });
    };
    let (baseline, with_fx) = baseline_and_fx(FxTarget::A, setup_b);
    assert!(max_diff(&baseline, &with_fx) > 0.1, "FX on A changes A");
}

#[test]
fn channel_fx_on_b_leaves_channel_a_untouched() {
    let setup = |h: &EngineHandle| {
        h.send(Command::Mixer { channel: 1, param: MixerParam::Fader(0.0) });
    };
    let (baseline, with_fx) = baseline_and_fx(FxTarget::B, setup);
    assert!(max_diff(&baseline, &with_fx) < 1e-6);
}

#[test]
fn master_target_affects_master_but_not_the_cue_bus() {
    let setup = |h: &EngineHandle| {
        h.send(Command::Mixer { channel: 0, param: MixerParam::Pfl(true) });
        h.send(Command::HeadphoneMix(0.0));
    };
    let (baseline, with_fx) = baseline_and_fx(FxTarget::Master, setup);
    let (base_cue, fx_cue) = (channel(&baseline, 2), channel(&with_fx, 2));
    assert!(rms(&base_cue) > 0.1, "cue audible");
    assert!(max_diff(&base_cue, &fx_cue) < 1e-6, "cue bus untouched by a master FX");
    assert!(max_diff(&channel(&baseline, 0), &channel(&with_fx, 0)) > 0.1, "master gated");
}

#[test]
fn channel_fx_is_audible_in_pfl() {
    let setup = |h: &EngineHandle| {
        h.send(Command::Mixer { channel: 0, param: MixerParam::Pfl(true) });
        h.send(Command::HeadphoneMix(0.0));
    };
    let (baseline, with_fx) = baseline_and_fx(FxTarget::A, setup);
    let (base_cue, fx_cue) = (channel(&baseline, 2), channel(&with_fx, 2));
    assert!(max_diff(&base_cue, &fx_cue) > 0.1, "PFL hears the FX");
    let quiet = fx_cue.chunks(480).map(rms).fold(f32::MAX, f32::min);
    assert!(quiet < 0.02, "gated sections in the headphones: {quiet}");
}

#[test]
fn channel_fx_is_pre_fader() {
    let (h, mut rt) = rig([None, None]);
    configure_trans(&h, FxTarget::A);
    h.send(Command::BeatFxEnabled(true));
    h.send(Command::Mixer { channel: 0, param: MixerParam::Fader(0.0) });
    h.send(Command::Mixer { channel: 1, param: MixerParam::Fader(0.0) });
    h.send(Command::Mixer { channel: 0, param: MixerParam::Pfl(true) });
    h.send(Command::HeadphoneMix(0.0));
    let out = run(&mut rt, 1.0);
    assert!(rms(&channel(&out, 2)[SR as usize / 2..]) > 0.02, "FX signal reaches PFL with fader down");
    assert!(rms(&channel(&out, 0)[SR as usize / 2..]) < 1e-4, "fader still controls master");
}

const REVERB_HISS_ALLOWANCE: f32 = 0.1;

#[test]
fn kind_switch_while_enabled_does_not_jump() {
    let (h0, mut rt0) = rig([None, None]);
    h0.send(Command::Mixer { channel: 1, param: MixerParam::Fader(0.0) });
    let _ = run(&mut rt0, 2.0);
    let floor = max_step(&channel(&run(&mut rt0, 1.0), 0));

    let (h, mut rt) = rig([None, None]);
    h.send(Command::Mixer { channel: 1, param: MixerParam::Fader(0.0) });
    h.send(Command::BeatFxKind(BeatFxKind::Echo));
    h.send(Command::BeatFxTarget(FxTarget::Master));
    h.send(Command::BeatFxBeats(0.5));
    h.send(Command::BeatFxLevel(0.8));
    h.send(Command::BeatFxEnabled(true));
    let _ = run(&mut rt, 1.0);
    let mut excess = f32::MIN;
    for kind in [BeatFxKind::Reverb, BeatFxKind::Flanger, BeatFxKind::PingPong, BeatFxKind::Phaser, BeatFxKind::Delay] {
        h.send(Command::BeatFxKind(kind));
        let seg = channel(&run(&mut rt, 1.0), 0);
        let allowance = if kind == BeatFxKind::Reverb { REVERB_HISS_ALLOWANCE } else { 0.04 };
        excess = excess.max(max_step(&seg) - floor - allowance);
    }
    assert!(excess < 0.0, "worst step exceeds the clean signal's by {excess}");
}

#[test]
fn changing_target_while_enabled_does_not_jump() {
    let (h, mut rt) = rig([None, None]);
    h.send(Command::Mixer { channel: 1, param: MixerParam::Fader(0.0) });
    h.send(Command::BeatFxKind(BeatFxKind::Echo));
    h.send(Command::BeatFxBeats(0.5));
    h.send(Command::BeatFxLevel(0.8));
    h.send(Command::BeatFxTarget(FxTarget::Master));
    h.send(Command::BeatFxEnabled(true));
    let _ = run(&mut rt, 1.0);
    let clean = {
        let (h0, mut rt0) = rig([None, None]);
        h0.send(Command::Mixer { channel: 1, param: MixerParam::Fader(0.0) });
        let _ = run(&mut rt0, 1.0);
        max_step(&channel(&run(&mut rt0, 1.0), 0))
    };
    let mut worst: f32 = 0.0;
    for target in [FxTarget::A, FxTarget::B, FxTarget::Master] {
        h.send(Command::BeatFxTarget(target));
        worst = worst.max(max_step(&channel(&run(&mut rt, 1.0), 0)));
    }
    assert!(worst < clean + 0.04, "worst step {worst} vs clean {clean}");
    assert_eq!(h.snapshot().beat_fx.target, FxTarget::Master);
}

#[test]
fn echo_tail_survives_disable_on_the_master_bus() {
    let (h, mut rt) = rig([None, None]);
    h.send(Command::BeatFxKind(BeatFxKind::Echo));
    h.send(Command::BeatFxTarget(FxTarget::Master));
    h.send(Command::BeatFxBeats(1.0));
    h.send(Command::BeatFxLevel(1.0));
    h.send(Command::BeatFxEnabled(true));
    let _ = run(&mut rt, 1.5);
    h.send(Command::Pause { deck: 0 });
    h.send(Command::Pause { deck: 1 });
    h.send(Command::BeatFxEnabled(false));
    let out = channel(&run(&mut rt, 2.0), 0);
    let ringing = rms(&out[SR as usize / 4..SR as usize]);
    assert!(ringing > 0.02, "tail rings after disable and pause: {ringing}");
    assert!(out.iter().all(|v| v.is_finite()));
}

#[test]
fn snapshot_reflects_commands() {
    let (h, mut rt) = rig([None, None]);
    let _ = run(&mut rt, 0.1);
    let s = h.snapshot();
    assert_eq!(s.beat_fx.kind, BeatFxKind::Echo);
    assert_eq!(s.beat_fx.target, FxTarget::Master);
    assert!(!s.beat_fx.enabled);
    assert_eq!(s.color_fx, ColorFxKind::Filter);
    assert_eq!(s.beat_fx.bpm, 120.0);

    h.send(Command::BeatFxKind(BeatFxKind::FilterSweep));
    h.send(Command::BeatFxTarget(FxTarget::B));
    h.send(Command::BeatFxBeats(4.0));
    h.send(Command::BeatFxLevel(0.25));
    h.send(Command::BeatFxEnabled(true));
    h.send(Command::ColorFx(ColorFxKind::Crush));
    let _ = run(&mut rt, 0.1);
    let s = h.snapshot();
    assert_eq!(s.beat_fx.kind, BeatFxKind::FilterSweep);
    assert_eq!(s.beat_fx.target, FxTarget::B);
    assert_eq!(s.beat_fx.beats, 4.0);
    assert_eq!(s.beat_fx.level, 0.25);
    assert!(s.beat_fx.enabled);
    assert_eq!(s.color_fx, ColorFxKind::Crush);
}

#[test]
fn beats_outside_the_allowed_set_snap_and_garbage_is_ignored() {
    let (h, mut rt) = rig([None, None]);
    h.send(Command::BeatFxBeats(0.7));
    let _ = run(&mut rt, 0.05);
    assert_eq!(h.snapshot().beat_fx.beats, 0.75);
    h.send(Command::BeatFxBeats(1000.0));
    let _ = run(&mut rt, 0.05);
    assert_eq!(h.snapshot().beat_fx.beats, 16.0);
    h.send(Command::BeatFxBeats(f64::NAN));
    h.send(Command::BeatFxBeats(-2.0));
    h.send(Command::BeatFxBeats(0.0));
    let _ = run(&mut rt, 0.05);
    assert_eq!(h.snapshot().beat_fx.beats, 16.0);
}

#[test]
fn percent_command_sets_the_snapshot_clamps_and_ignores_garbage() {
    let (h, mut rt) = rig([None, None]);
    assert_eq!(h.snapshot().beat_fx.percent, 50.0, "default");
    h.send(Command::BeatFxPercent(30.0));
    let _ = run(&mut rt, 0.05);
    assert_eq!(h.snapshot().beat_fx.percent, 30.0);
    h.send(Command::BeatFxPercent(37.5));
    let _ = run(&mut rt, 0.05);
    assert_eq!(h.snapshot().beat_fx.percent, 37.5, "fine values are kept");
    h.send(Command::BeatFxPercent(900.0));
    let _ = run(&mut rt, 0.05);
    assert_eq!(h.snapshot().beat_fx.percent, 100.0);
    h.send(Command::BeatFxPercent(-5.0));
    let _ = run(&mut rt, 0.05);
    assert_eq!(h.snapshot().beat_fx.percent, 1.0);
    h.send(Command::BeatFxPercent(f64::NAN));
    h.send(Command::BeatFxPercent(f64::INFINITY));
    let _ = run(&mut rt, 0.05);
    assert_eq!(h.snapshot().beat_fx.percent, 1.0);
}

#[test]
fn reverb_decay_lengthens_the_tail_through_the_engine() {
    let tail = |decay: f32| {
        let (h, mut rt) = rig([None, None]);
        h.send(Command::BeatFxKind(BeatFxKind::Reverb));
        h.send(Command::BeatFxTarget(FxTarget::Master));
        h.send(Command::BeatFxParam { index: 2, value: decay });
        h.send(Command::BeatFxLevel(1.0));
        h.send(Command::BeatFxEnabled(true));
        let _ = run(&mut rt, 1.0);
        h.send(Command::Pause { deck: 0 });
        h.send(Command::Pause { deck: 1 });
        h.send(Command::BeatFxEnabled(false));
        let out = channel(&run(&mut rt, 6.0), 0);
        rms(&out[SR as usize * 4..SR as usize * 5])
    };
    let (short, long) = (tail(1.0), tail(100.0));
    assert!(long > short * 20.0 && long > 1e-3, "decay 1: {short}, decay 100: {long}");
}

#[test]
fn percent_survives_a_sample_rate_rebuild() {
    let (h, mut rt) = rig([None, None]);
    h.send(Command::BeatFxPercent(70.0));
    let _ = run(&mut rt, 0.05);
    rt.reconfigure(44_100, Routing::for_channel_count(CH));
    let _ = run(&mut rt, 0.05);
    assert_eq!(h.snapshot().beat_fx.percent, 70.0);
}

#[test]
fn fx_tempo_follows_target_deck_then_master_deck_then_default() {
    let grid = |bpm| Some(Beatgrid { bpm, first_beat_secs: 0.0 });
    let (h, mut rt) = rig([None, grid(100.0)]);
    h.send(Command::BeatFxTarget(FxTarget::A));
    let _ = run(&mut rt, 0.1);
    assert_eq!(h.snapshot().beat_fx.bpm, 120.0, "no grid anywhere");

    h.send(Command::BeatFxTarget(FxTarget::B));
    let _ = run(&mut rt, 0.1);
    assert!((h.snapshot().beat_fx.bpm - 100.0).abs() < 0.01, "target deck tempo");

    h.send(Command::SetMasterDeck { deck: 1 });
    h.send(Command::BeatFxTarget(FxTarget::A));
    let _ = run(&mut rt, 0.1);
    assert!((h.snapshot().beat_fx.bpm - 100.0).abs() < 0.01, "falls back to the master deck");

    h.send(Command::BeatFxTarget(FxTarget::Master));
    let _ = run(&mut rt, 0.1);
    assert!((h.snapshot().beat_fx.bpm - 100.0).abs() < 0.01, "master target follows master deck");
}

#[test]
fn echo_delay_follows_the_synced_deck_tempo() {
    let grid = Some(Beatgrid { bpm: 150.0, first_beat_secs: 0.0 });
    let (h, mut rt) = create_engine(SR);
    rt.reconfigure(SR, Routing::for_channel_count(CH));
    let mut samples = vec![0.0f32; SR as usize * 2 * 20];
    samples[SR as usize * 2 * 3] = 0.5;
    samples[SR as usize * 2 * 3 + 1] = 0.5;
    load(&h, 0, Arc::new(TrackAudio::new(SR, samples)), grid);
    h.send(Command::Mixer { channel: 0, param: MixerParam::Fader(1.0) });
    h.send(Command::MasterLevel(0.8));
    h.send(Command::BeatFxKind(BeatFxKind::Echo));
    h.send(Command::BeatFxTarget(FxTarget::A));
    h.send(Command::BeatFxBeats(1.0));
    h.send(Command::BeatFxLevel(0.75));
    h.send(Command::BeatFxEnabled(true));
    h.send(Command::Play { deck: 0 });
    let out = channel(&run(&mut rt, 4.5), 0);
    let at = |t: f64| (t * SR as f64) as usize;
    let window = |centre: usize| out[centre - 200..centre + 200].iter().fold(0.0f32, |m, v| m.max(v.abs()));
    let impulse = window(at(3.0) + 200);
    let repeat = window(at(3.0 + 60.0 / 150.0) + 200);
    let off_beat = window(at(3.0 + 0.5 * 60.0 / 150.0) + 200);
    assert!(impulse > 0.05 && repeat > 0.05, "dry {impulse}, repeat {repeat}");
    assert!(off_beat < repeat * 0.2, "silent between repeats: {off_beat}");
}

#[test]
fn color_fx_kinds_change_the_channel_but_filter_kind_is_the_classic_filter() {
    let setup = |kind: ColorFxKind| {
        let (h, mut rt) = rig([None, None]);
        h.send(Command::Mixer { channel: 0, param: MixerParam::Fader(0.0) });
        h.send(Command::ColorFx(kind));
        h.send(Command::Mixer { channel: 1, param: MixerParam::Filter(0.9) });
        let _ = run(&mut rt, 0.8);
        channel(&run(&mut rt, 0.5), 0)
    };
    let filter = setup(ColorFxKind::Filter);
    let crush = setup(ColorFxKind::Crush);
    let noise = setup(ColorFxKind::Noise);
    assert!(max_diff(&filter, &crush) > 0.01);
    assert!(max_diff(&filter, &noise) > 0.01);
    assert!(filter.iter().chain(&crush).chain(&noise).all(|v| v.is_finite()));
}

#[test]
fn color_fx_is_independent_per_channel() {
    let run_with = |channel_index: usize| {
        let (h, mut rt) = rig([None, None]);
        h.send(Command::Mixer { channel: 1 - channel_index, param: MixerParam::Fader(0.0) });
        h.send(Command::ColorFx(ColorFxKind::Crush));
        h.send(Command::Mixer { channel: 0, param: MixerParam::Filter(0.9) });
        let _ = run(&mut rt, 0.6);
        channel(&run(&mut rt, 0.4), 0)
    };
    let ch0_only = run_with(0);
    let ch1_only = run_with(1);
    assert!(rms(&ch0_only) > 0.05 && rms(&ch1_only) > 0.05);
    let (h, mut rt) = rig([None, None]);
    h.send(Command::Mixer { channel: 0, param: MixerParam::Fader(0.0) });
    let _ = run(&mut rt, 0.6);
    let clean_b = channel(&run(&mut rt, 0.4), 0);
    assert!(max_diff(&ch1_only, &clean_b) < 1e-6, "channel 1 unaffected by channel 0's knob");
}

#[test]
fn color_fx_switch_while_knob_engaged_does_not_jump() {
    let (h, mut rt) = rig([None, None]);
    h.send(Command::Mixer { channel: 1, param: MixerParam::Fader(0.0) });
    h.send(Command::ColorFx(ColorFxKind::DubEcho));
    h.send(Command::Mixer { channel: 0, param: MixerParam::Filter(0.5) });
    let _ = run(&mut rt, 1.0);
    let mut worst: f32 = 0.0;
    for kind in [ColorFxKind::Space, ColorFxKind::DubEcho, ColorFxKind::Filter, ColorFxKind::Space] {
        h.send(Command::ColorFx(kind));
        worst = worst.max(max_step(&channel(&run(&mut rt, 0.8), 0)));
    }
    assert!(worst < 0.1, "worst step {worst}");
}

#[test]
fn every_fx_at_once_stays_finite_and_bounded() {
    let (h, mut rt) = rig([None, None]);
    h.send(Command::ColorFx(ColorFxKind::Space));
    h.send(Command::Mixer { channel: 0, param: MixerParam::Filter(-1.0) });
    h.send(Command::Mixer { channel: 1, param: MixerParam::Filter(1.0) });
    h.send(Command::BeatFxLevel(1.0));
    h.send(Command::BeatFxEnabled(true));
    for kind in BeatFxKind::ALL {
        for target in [FxTarget::A, FxTarget::B, FxTarget::Master] {
            h.send(Command::BeatFxTarget(target));
            h.send(Command::BeatFxKind(kind));
            let out = run(&mut rt, 0.4);
            assert!(out.iter().all(|v| v.is_finite() && v.abs() <= 1.0), "{kind:?} {target:?}");
        }
    }
}

fn tone_rig(hz: f64) -> (EngineHandle, RtEngine) {
    let (h, mut rt) = create_engine(SR);
    rt.reconfigure(SR, Routing::for_channel_count(CH));
    load(&h, 0, tone(hz, 60.0), None);
    h.send(Command::Mixer { channel: 0, param: MixerParam::Fader(1.0) });
    h.send(Command::MasterLevel(0.8));
    h.send(Command::Play { deck: 0 });
    (h, rt)
}

fn amplitude_at(x: &[f32], hz: f64) -> f64 {
    let w = std::f64::consts::TAU * hz / SR as f64;
    let (mut c, mut q) = (0.0f64, 0.0f64);
    for (i, v) in x.iter().enumerate() {
        c += *v as f64 * (w * i as f64).cos();
        q += *v as f64 * (w * i as f64).sin();
    }
    2.0 * (c * c + q * q).sqrt() / x.len() as f64
}

fn worst_cents(x: &[f32], nominal_hz: f64, chunk: usize) -> f64 {
    let w = std::f64::consts::TAU * nominal_hz / SR as f64;
    let phases: Vec<f64> = x
        .chunks_exact(chunk)
        .enumerate()
        .map(|(k, c)| {
            let (mut re, mut im) = (0.0f64, 0.0f64);
            for (i, v) in c.iter().enumerate() {
                let p = w * (k * chunk + i) as f64;
                re += *v as f64 * p.cos();
                im -= *v as f64 * p.sin();
            }
            im.atan2(re)
        })
        .collect();
    phases
        .windows(2)
        .map(|p| {
            let mut d = p[1] - p[0];
            d -= std::f64::consts::TAU * (d / std::f64::consts::TAU).round();
            let hz_offset = d / (std::f64::consts::TAU * chunk as f64 / SR as f64);
            (1200.0 * (1.0 + hz_offset / nominal_hz).log2()).abs()
        })
        .fold(0.0, f64::max)
}

#[test]
fn echo_division_changes_through_the_engine_keep_a_1khz_tone_within_five_cents() {
    for kind in [BeatFxKind::Echo, BeatFxKind::Delay, BeatFxKind::PingPong, BeatFxKind::UpEcho, BeatFxKind::DownEcho] {
        let (h, mut rt) = tone_rig(1_000.0);
        h.send(Command::BeatFxKind(kind));
        h.send(Command::BeatFxTarget(FxTarget::Master));
        h.send(Command::BeatFxBeats(0.25));
        h.send(Command::BeatFxLevel(0.9));
        h.send(Command::BeatFxEnabled(true));
        let mut all = Vec::new();
        for beats in [0.25, 0.5, 0.75, 1.0, 0.25] {
            h.send(Command::BeatFxBeats(beats));
            all.extend(channel(&run(&mut rt, 2.0), 0));
        }
        let settled = &all[SR as usize / 2..];
        let step_floor = std::f32::consts::TAU * 1_000.0 / SR as f32;
        let peak = settled.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        if matches!(kind, BeatFxKind::UpEcho | BeatFxKind::DownEcho) {
            assert!(max_step(settled) < 1.6 * step_floor * peak, "{kind:?} step {}", max_step(settled));
            continue;
        }
        let cents = worst_cents(settled, 1_000.0, 480);
        eprintln!("engine {kind:?}: worst {cents:.2} cents");
        assert!(cents < 5.0, "{kind:?}: {cents} cents");
        assert!(max_step(settled) < 1.25 * step_floor * peak, "{kind:?} step {} vs {}", max_step(settled), step_floor * peak);
    }
}

#[test]
fn echo_with_low_band_off_leaves_the_bass_at_dry_level_and_all_bands_on_is_transparent() {
    for hz in [60.0, 100.0] {
        let measure = |fx_on: bool, bands: Option<FxBands>| {
            let (h, mut rt) = tone_rig(hz);
            h.send(Command::BeatFxKind(BeatFxKind::Echo));
            h.send(Command::BeatFxTarget(FxTarget::Master));
            h.send(Command::BeatFxBeats(0.5));
            h.send(Command::BeatFxLevel(if fx_on { 1.0 } else { 0.0 }));
            if let Some(b) = bands {
                h.send(Command::BeatFxBands(b));
            }
            h.send(Command::BeatFxEnabled(true));
            let out = channel(&run(&mut rt, 3.0), 0);
            amplitude_at(&out[SR as usize..], hz)
        };
        let dry = {
            let (_h, mut rt) = tone_rig(hz);
            amplitude_at(&channel(&run(&mut rt, 3.0), 0)[SR as usize..], hz)
        };
        let db = |v: f64| 20.0 * (v / dry).log10();
        let low_off = db(measure(true, None));
        let silent_all_on = db(measure(false, Some(FxBands::ALL)));
        eprintln!("engine {hz} Hz: LOW off {low_off:+.3} dB, all bands silent {silent_all_on:+.3} dB");
        assert!(low_off.abs() < 0.5, "{hz} Hz with LOW off: {low_off:+.3} dB");
        assert!(silent_all_on.abs() < 0.1, "{hz} Hz, all bands on, effect silent: {silent_all_on:+.3} dB");
    }
}

#[test]
fn band_selection_reaches_the_snapshot_is_kept_per_kind_and_survives_a_rate_change() {
    let (h, mut rt) = rig([None, None]);
    let _ = run(&mut rt, 0.05);
    assert_eq!(h.snapshot().beat_fx.bands, FxBands { low: false, mid: true, high: true }, "echo starts LOW off");
    h.send(Command::BeatFxBands(FxBands { low: true, mid: false, high: true }));
    let _ = run(&mut rt, 0.05);
    assert_eq!(h.snapshot().beat_fx.bands, FxBands { low: true, mid: false, high: true });
    h.send(Command::BeatFxKind(BeatFxKind::Trans));
    let _ = run(&mut rt, 0.05);
    assert_eq!(h.snapshot().beat_fx.bands, FxBands::ALL, "trans acts on every band by default");
    h.send(Command::BeatFxKind(BeatFxKind::Echo));
    let _ = run(&mut rt, 0.05);
    assert_eq!(h.snapshot().beat_fx.bands, FxBands { low: true, mid: false, high: true }, "echo restores its own set");
    rt.reconfigure(44_100, Routing::for_channel_count(CH));
    let _ = run(&mut rt, 0.05);
    assert_eq!(h.snapshot().beat_fx.bands, FxBands { low: true, mid: false, high: true });
}

#[test]
fn color_fx_parameters_reach_the_snapshot_clamp_survive_kind_changes_and_a_rate_rebuild() {
    let (h, mut rt) = rig([None, None]);
    let _ = run(&mut rt, 0.05);
    assert_eq!(h.snapshot().color_fx_params, [0.0, 0.0], "the filter kind has no sub-parameters");
    h.send(Command::ColorFx(ColorFxKind::Space));
    let _ = run(&mut rt, 0.05);
    assert_eq!(h.snapshot().color_fx_params[0], 0.0, "shimmer starts at zero");
    h.send(Command::ColorFxParam { index: 0, value: 65.0 });
    h.send(Command::ColorFxParam { index: 1, value: 30.0 });
    h.send(Command::ColorFxParam { index: 0, value: f32::NAN });
    let _ = run(&mut rt, 0.05);
    assert_eq!(h.snapshot().color_fx_params, [65.0, 0.0], "only the first slot exists for space");
    h.send(Command::ColorFxParam { index: 0, value: 400.0 });
    let _ = run(&mut rt, 0.05);
    assert_eq!(h.snapshot().color_fx_params[0], 100.0);
    h.send(Command::ColorFxParam { index: 0, value: 65.0 });
    h.send(Command::ColorFx(ColorFxKind::Noise));
    let _ = run(&mut rt, 0.05);
    assert_eq!(h.snapshot().color_fx_params[0], 50.0, "noise keeps its own level");
    h.send(Command::ColorFx(ColorFxKind::Space));
    rt.reconfigure(44_100, Routing::for_channel_count(CH));
    let _ = run(&mut rt, 0.05);
    assert_eq!(h.snapshot().color_fx_params[0], 65.0, "values come back with the kind and survive a rate rebuild");
}

#[test]
fn echo_freeze_reaches_the_snapshot_and_releases_when_the_unit_is_switched_off() {
    let (h, mut rt) = rig([None, None]);
    h.send(Command::BeatFxKind(BeatFxKind::Echo));
    h.send(Command::BeatFxEnabled(true));
    h.send(Command::BeatFxTrigger(deckforge_common::FxTrigger::FreezeOn));
    let _ = run(&mut rt, 0.2);
    assert!(h.snapshot().beat_fx.held, "the freeze is held");
    h.send(Command::BeatFxEnabled(false));
    let _ = run(&mut rt, 0.2);
    assert!(!h.snapshot().beat_fx.held, "switching off drops the hold");
    h.send(Command::BeatFxKind(BeatFxKind::Spiral));
    h.send(Command::BeatFxEnabled(true));
    h.send(Command::BeatFxTrigger(deckforge_common::FxTrigger::FreezeOn));
    let _ = run(&mut rt, 0.2);
    assert!(!h.snapshot().beat_fx.held, "spiral has no freeze");
}
