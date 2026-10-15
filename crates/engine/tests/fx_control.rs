use std::sync::Arc;

use deckforge_common::{BeatFxKind, Beatgrid, FxTarget, PadVoice, ReleaseFxKind, SmartCfxPreset, TrackAudio, HOT_CUE_SLOTS};
use deckforge_engine::{create_engine, Command, EngineHandle, LoadRequest, MixerParam, RtEngine, Routing};

const SR: u32 = 48_000;
const BLOCK: usize = 512;
const CH: usize = 4;
const A_HZ: f64 = 220.0;
const B_HZ: f64 = 1_250.0;
const GRID: Beatgrid = Beatgrid { bpm: 120.0, first_beat_secs: 0.0 };

fn tone(freq: f64, secs: f64) -> Arc<TrackAudio> {
    let n = (secs * SR as f64) as usize;
    let samples = (0..n)
        .flat_map(|i| {
            let s = ((std::f64::consts::TAU * freq * i as f64 / SR as f64).sin() * 0.25) as f32;
            [s, s]
        })
        .collect();
    Arc::new(TrackAudio::new(SR, samples))
}

fn rig() -> (EngineHandle, RtEngine) {
    let (h, mut rt) = create_engine(SR);
    rt.reconfigure(SR, Routing::for_channel_count(CH));
    for (deck, hz) in [(0, A_HZ), (1, B_HZ)] {
        let request = Box::new(LoadRequest { track: tone(hz, 30.0), token: deck as u64 + 1, grid: Some(GRID), cue_secs: Some(0.0), hot_cues: [None; HOT_CUE_SLOTS] });
        h.send(Command::Load { deck, request });
        h.send(Command::Mixer { channel: deck, param: MixerParam::Fader(1.0) });
        h.send(Command::Play { deck });
    }
    h.send(Command::MasterLevel(0.8));
    (h, rt)
}

fn run(rt: &mut RtEngine, secs: f64) -> Vec<f32> {
    let blocks = ((secs * SR as f64) as usize).div_ceil(BLOCK);
    let mut all = Vec::with_capacity(blocks * BLOCK);
    let mut buf = vec![0.0; BLOCK * CH];
    for _ in 0..blocks {
        rt.process(&mut buf, CH);
        all.extend(buf.chunks(CH).map(|f| f[0]));
    }
    all
}

fn amplitude(x: &[f32], hz: f64) -> f32 {
    let w = std::f64::consts::TAU * hz / SR as f64;
    let (mut c, mut q) = (0.0f64, 0.0f64);
    for (i, v) in x.iter().enumerate() {
        c += *v as f64 * (w * i as f64).cos();
        q += *v as f64 * (w * i as f64).sin();
    }
    (2.0 * (c * c + q * q).sqrt() / x.len() as f64) as f32
}

fn trans() -> PadVoice {
    PadVoice { kind: BeatFxKind::Trans, beats: 0.25, percent: 50.0, level: 1.0, param0: None }
}

#[test]
fn a_pad_voice_acts_on_its_own_deck_only() {
    let (_h0, mut rt0) = rig();
    let _ = run(&mut rt0, 0.5);
    let dry = run(&mut rt0, 1.0);

    let (h, mut rt) = rig();
    let _ = run(&mut rt, 0.4);
    h.send(Command::PadFx { deck: 0, voice: Some(trans()) });
    let _ = run(&mut rt, 0.1);
    let wet = run(&mut rt, 1.0);
    let snap = h.snapshot();
    assert_eq!(snap.pad_fx[0].voice.map(|v| v.kind), Some(BeatFxKind::Trans));
    assert!(snap.pad_fx[0].sounding && snap.pad_fx[1].voice.is_none());
    assert!(amplitude(&wet, A_HZ) < amplitude(&dry, A_HZ) * 0.8, "deck A is gated");
    let (b_dry, b_wet) = (amplitude(&dry, B_HZ), amplitude(&wet, B_HZ));
    assert!((b_wet - b_dry).abs() < b_dry * 0.02, "deck B untouched: {b_dry} vs {b_wet}");
    assert!(!snap.beat_fx.enabled, "the Beat FX unit is not involved");

    h.send(Command::PadFx { deck: 0, voice: None });
    let _ = run(&mut rt, 0.3);
    let after = run(&mut rt, 0.5);
    assert!(h.snapshot().pad_fx[0].voice.is_none());
    assert!(amplitude(&after, A_HZ) > amplitude(&dry, A_HZ) * 0.95, "released pad lets the deck through again");
}

#[test]
fn release_fx_cancels_the_beat_fx_and_plays_on_its_target() {
    let (h, mut rt) = rig();
    h.send(Command::BeatFxKind(BeatFxKind::Echo));
    h.send(Command::BeatFxTarget(FxTarget::A));
    h.send(Command::BeatFxEnabled(true));
    h.send(Command::ReleaseFxConfig { kind: ReleaseFxKind::VinylBrake, beats: 3.0 });
    let _ = run(&mut rt, 0.5);
    h.send(Command::ReleaseFx);
    let _ = run(&mut rt, 0.2);
    let snap = h.snapshot();
    assert!(!snap.beat_fx.enabled, "Beat FX cancelled");
    assert_eq!(snap.release_fx.kind, ReleaseFxKind::VinylBrake);
    assert_eq!(snap.release_fx.beats, 4.0, "length snaps to an offered value");
    assert_eq!(snap.release_fx.running, Some(ReleaseFxKind::VinylBrake));
    assert!(snap.release_fx.progress > 0.0 && snap.release_fx.progress < 0.2);
    let _ = run(&mut rt, 2.0);
    assert_eq!(h.snapshot().release_fx.running, None, "four beats at 120 BPM are over");
    let back = run(&mut rt, 0.5);
    assert!(amplitude(&back, A_HZ) > 0.1, "deck A plays normally again");
}

#[test]
fn pad_one_shots_end_the_voice_and_report_progress() {
    let (h, mut rt) = rig();
    let _ = run(&mut rt, 0.3);
    h.send(Command::PadFx { deck: 1, voice: Some(trans()) });
    let _ = run(&mut rt, 0.1);
    h.send(Command::PadFxRelease { deck: 1, kind: ReleaseFxKind::Backspin, beats: 1.0 });
    let _ = run(&mut rt, 0.1);
    let snap = h.snapshot();
    assert!(snap.pad_fx[1].voice.is_none());
    assert_eq!(snap.pad_fx[1].release, Some(ReleaseFxKind::Backspin));
    assert!(snap.pad_fx[1].release_progress > 0.0);
    assert_eq!(snap.pad_fx[0].release, None);
}

#[test]
fn smart_cfx_takes_over_the_cfx_knob_from_centre() {
    let (h, mut rt) = rig();
    h.send(Command::Mixer { channel: 0, param: MixerParam::Filter(-0.8) });
    let _ = run(&mut rt, 0.2);
    assert!(h.snapshot().channels[0].filter < -0.5);
    h.send(Command::SmartCfxPreset(SmartCfxPreset::CrushChop));
    h.send(Command::SmartCfx(true));
    let _ = run(&mut rt, 0.3);
    let snap = h.snapshot();
    assert!(snap.smart_cfx.enabled && snap.smart_cfx.preset == SmartCfxPreset::CrushChop);
    assert_eq!(snap.channels[0].filter, 0.0, "starts centred");
    let centred = run(&mut rt, 0.5);
    h.send(Command::Mixer { channel: 0, param: MixerParam::Filter(-1.0) });
    let _ = run(&mut rt, 0.1);
    let chopped = run(&mut rt, 0.5);
    assert!(amplitude(&chopped, A_HZ) < amplitude(&centred, A_HZ) * 0.7, "left side chops deck A");
}
