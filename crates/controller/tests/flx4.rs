use std::time::Instant;

use deckforge_common::{Band, BeatFxKind, FxTarget, HotCue, PadVoice, ReleaseFxKind, SmartCfxPreset, TempoRange};
use deckforge_controller::flx4::{vu_value, PadMode, KEEPALIVE_SYSEX};
use deckforge_controller::{Action, AppAction, ControllerMapping, Flx4, FxControlState, FxHudControl, FxKnobTarget};
use deckforge_engine::{Command, EngineSnapshot, JogMode, MixerParam};

fn snap() -> EngineSnapshot {
    let mut s = EngineSnapshot::default();
    for d in &mut s.decks {
        d.loaded = true;
        d.fader_tempo_ratio = 1.0;
        d.tempo_range = TempoRange::Ten;
    }
    s
}

fn engine(actions: Vec<Action>) -> Vec<Command> {
    actions
        .into_iter()
        .filter_map(|a| match a {
            Action::Engine(c) => Some(c),
            Action::App(_) => None,
        })
        .collect()
}

fn one(f: &mut Flx4, msg: &[u8], s: &EngineSnapshot) -> Command {
    let mut cmds = engine(f.on_input(msg, s));
    assert_eq!(cmds.len(), 1, "{msg:02x?} -> {cmds:?}");
    cmds.remove(0)
}

/// Press and release, as the hardware always does; returns the press's single command.
fn tap(f: &mut Flx4, msg: &[u8], s: &EngineSnapshot) -> Command {
    let cmd = one(f, msg, s);
    assert!(engine(f.on_input(&[msg[0], msg[1], 0x00], s)).is_empty(), "{msg:02x?} release");
    cmd
}

#[test]
fn transport_buttons_map_to_the_right_deck() {
    let mut f = Flx4::new();
    let s = snap();
    assert!(matches!(one(&mut f, &[0x90, 0x0B, 0x7F], &s), Command::TogglePlay { deck: 0 }));
    assert!(matches!(one(&mut f, &[0x91, 0x0B, 0x7F], &s), Command::TogglePlay { deck: 1 }));
    assert!(engine(f.on_input(&[0x91, 0x0B, 0x00], &s)).is_empty(), "release is ignored");
    assert!(matches!(one(&mut f, &[0x90, 0x0C, 0x7F], &s), Command::CueDown { deck: 0 }));
    assert!(matches!(one(&mut f, &[0x90, 0x0C, 0x00], &s), Command::CueUp { deck: 0 }));
    assert!(matches!(one(&mut f, &[0x80, 0x0C, 0x40], &s), Command::CueUp { deck: 0 }), "note-off form");
    assert!(matches!(one(&mut f, &[0x91, 0x48, 0x7F], &s), Command::SeekSecs { deck: 1, .. }));
}

#[test]
fn sync_toggles_from_engine_state_and_shift_cycles_range() {
    let mut f = Flx4::new();
    let mut s = snap();
    assert!(matches!(one(&mut f, &[0x91, 0x58, 0x7F], &s), Command::Sync { deck: 1, enabled: true }));
    s.decks[1].sync_enabled = true;
    assert!(matches!(one(&mut f, &[0x91, 0x58, 0x7F], &s), Command::Sync { deck: 1, enabled: false }));
    assert!(matches!(
        one(&mut f, &[0x90, 0x60, 0x7F], &s),
        Command::SetTempoRange { deck: 0, range: TempoRange::Sixteen }
    ));
    assert!(matches!(one(&mut f, &[0x90, 0x5C, 0x7F], &s), Command::SetMasterDeck { deck: 0 }));
}

#[test]
fn jog_relative_encoding_and_touch_selects_scratch() {
    let mut f = Flx4::new();
    let s = snap();
    assert!(matches!(one(&mut f, &[0xB0, 0x22, 0x41], &s), Command::Jog { deck: 0, ticks: 1, mode: JogMode::Bend }));
    assert!(matches!(one(&mut f, &[0xB0, 0x22, 0x3D], &s), Command::Jog { deck: 0, ticks: -3, .. }));
    assert!(matches!(one(&mut f, &[0x91, 0x36, 0x7F], &s), Command::JogTouch { deck: 1, touched: true }));
    assert!(matches!(one(&mut f, &[0xB1, 0x22, 0x45], &s), Command::Jog { deck: 1, ticks: 5, mode: JogMode::Scratch }));
    assert!(matches!(one(&mut f, &[0xB1, 0x21, 0x3F], &s), Command::Jog { deck: 1, ticks: -1, mode: JogMode::Bend }), "edge always bends");
    assert!(matches!(one(&mut f, &[0x91, 0x36, 0x00], &s), Command::JogTouch { deck: 1, touched: false }));
    assert!(matches!(one(&mut f, &[0xB1, 0x22, 0x45], &s), Command::Jog { mode: JogMode::Bend, .. }));
    assert!(matches!(one(&mut f, &[0xB0, 0x29, 0x50], &s), Command::Jog { deck: 0, ticks: 16, mode: JogMode::FastSearch }));
}

#[test]
fn fourteen_bit_mixer_controls() {
    let mut f = Flx4::new();
    let s = snap();
    let fader = |c: Command| match c {
        Command::Mixer { channel, param: MixerParam::Fader(v) } => (channel, v),
        other => panic!("{other:?}"),
    };
    let (ch, v) = fader(one(&mut f, &[0xB1, 0x13, 0x7F], &s));
    assert_eq!(ch, 1);
    assert!((v - 16_256.0 / 16_383.0).abs() < 1e-4, "MSB alone");
    let (_, v) = fader(one(&mut f, &[0xB1, 0x33, 0x7F], &s));
    assert!((v - 1.0).abs() < 1e-6, "MSB+LSB full scale");
    let (_, v) = fader(one(&mut f, &[0xB1, 0x13, 0x00], &s));
    assert!(v.abs() < 1e-6);

    assert!(matches!(one(&mut f, &[0xB0, 0x0F, 0x00], &s), Command::Mixer { channel: 0, param: MixerParam::Eq(Band::Low, _) }));
    assert!(matches!(one(&mut f, &[0xB0, 0x07, 0x40], &s), Command::Mixer { param: MixerParam::Eq(Band::High, _), .. }));
    assert!(matches!(one(&mut f, &[0xB0, 0x0B, 0x40], &s), Command::Mixer { param: MixerParam::Eq(Band::Mid, _), .. }));
    assert!(matches!(one(&mut f, &[0xB0, 0x04, 0x40], &s), Command::Mixer { param: MixerParam::Trim(_), .. }));
    match one(&mut f, &[0xB6, 0x1F, 0x00], &s) {
        Command::Crossfader(v) => assert_eq!(v, 0.0),
        other => panic!("{other:?}"),
    }
    match one(&mut f, &[0xB6, 0x18, 0x00], &s) {
        Command::Mixer { channel: 1, param: MixerParam::Filter(v) } => assert!((v + 1.0).abs() < 1e-6),
        other => panic!("{other:?}"),
    }
}

#[test]
fn tempo_fader_soft_takeover() {
    let mut f = Flx4::new();
    let mut s = snap();
    s.decks[0].fader_tempo_ratio = 1.05;
    assert!(engine(f.on_input(&[0xB0, 0x00, 0x00], &s)).is_empty(), "far from engine value: ignored");
    let cmd = one(&mut f, &[0xB0, 0x00, 0x60], &s);
    match cmd {
        Command::SetPitchFader { deck: 0, position } => assert!((position - 0.5).abs() < 0.01),
        other => panic!("{other:?}"),
    }
    match one(&mut f, &[0xB0, 0x00, 0x00], &s) {
        Command::SetPitchFader { position, .. } => assert!((position + 1.0).abs() < 1e-6, "engaged: follows"),
        other => panic!("{other:?}"),
    }
    s.decks[0].sync_enabled = true;
    assert!(engine(f.on_input(&[0xB0, 0x00, 0x40], &s)).is_empty(), "synced deck ignores fader");
}

#[test]
fn pads_follow_mode_and_shift_layers() {
    let mut f = Flx4::new();
    let s = snap();
    assert!(matches!(one(&mut f, &[0x97, 0x02, 0x7F], &s), Command::HotCue { deck: 0, slot: 2, pressed: true }));
    assert!(matches!(one(&mut f, &[0x97, 0x02, 0x00], &s), Command::HotCue { deck: 0, slot: 2, pressed: false }));
    assert!(matches!(one(&mut f, &[0x9A, 0x07, 0x7F], &s), Command::HotCueDelete { deck: 1, slot: 7 }));
    assert!(engine(f.on_input(&[0x90, 0x20, 0x7F], &s)).is_empty());
    assert_eq!(f.pad_mode(0), PadMode::BeatJump);
    match one(&mut f, &[0x97, 0x23, 0x7F], &s) {
        Command::BeatJump { deck: 0, beats } => assert_eq!(beats, 2.0),
        other => panic!("{other:?}"),
    }
    match one(&mut f, &[0x99, 0x64, 0x7F], &s) {
        Command::BeatLoop { deck: 1, beats } => assert_eq!(beats, 4.0),
        other => panic!("{other:?}"),
    }
    assert_eq!(f.pad_mode(1), PadMode::BeatLoop);
}

#[test]
fn browser_and_load_produce_app_actions() {
    let mut f = Flx4::new();
    let s = snap();
    let app = |a: Vec<Action>| match a.as_slice() {
        [Action::App(x)] => x.clone(),
        other => panic!("{other:?}"),
    };
    assert_eq!(app(f.on_input(&[0xB6, 0x40, 0x01], &s)), AppAction::BrowseScroll { delta: 1 });
    assert_eq!(app(f.on_input(&[0xB6, 0x40, 0x7F], &s)), AppAction::BrowseScroll { delta: -1 });
    assert_eq!(app(f.on_input(&[0xB6, 0x64, 0x7E], &s)), AppAction::BrowseScroll { delta: -20 });
    assert_eq!(app(f.on_input(&[0x96, 0x47, 0x7F], &s)), AppAction::LoadSelected { deck: 1 });
    assert_eq!(app(f.on_input(&[0x96, 0x41, 0x7F], &s)), AppAction::BrowseToggleFocus);
}

#[test]
fn monitor_knobs_are_display_only_by_default() {
    let mut f = Flx4::new();
    let s = snap();
    let actions = f.on_input(&[0xB6, 0x0C, 0x40], &s);
    assert!(engine(actions.clone()).is_empty());
    assert!(matches!(actions.as_slice(), [Action::App(AppAction::HardwareKnob { name: "headphoneMix", .. })]));
    f.software_monitor_knobs = true;
    assert!(matches!(engine(f.on_input(&[0xB6, 0x08, 0x40], &s)).as_slice(), [Command::MasterLevel(_)]));
}

#[test]
fn led_feedback_reflects_engine_state_and_only_sends_changes() {
    let mut f = Flx4::new();
    let init = f.on_connect();
    assert!(init.contains(&vec![0x90, 0x17, 0x7F]) && init.contains(&vec![0x91, 0x17, 0x7F]), "vinyl mode on");
    assert!(init.contains(&KEEPALIVE_SYSEX.to_vec()));
    let mut s = snap();
    s.decks[0].playing = true;
    s.decks[0].track_token = 5;
    s.decks[1].sync_enabled = true;
    s.decks[0].hot_cues[3] = Some(HotCue { position_secs: 1.0, color: 0 });
    s.channels[1].pfl = true;
    let now = Instant::now();
    let out = f.feedback(&s, now);
    assert!(out.contains(&vec![0x90, 0x0B, 0x7F]) && out.contains(&vec![0x90, 0x0E, 0x7F]), "play LED + shifted twin");
    assert!(out.contains(&vec![0x91, 0x58, 0x7F]), "sync LED");
    assert!(out.contains(&vec![0x97, 0x03, 0x7F]) && out.contains(&vec![0x98, 0x03, 0x7F]), "hot cue pad");
    assert!(out.contains(&vec![0x97, 0x00, 0x00]), "empty hot cue pad off");
    assert!(out.contains(&vec![0x91, 0x54, 0x7F]), "headphone cue LED");
    assert!(out.contains(&vec![0x90, 0x1B, 0x7F]) && out.contains(&vec![0x90, 0x20, 0x00]), "pad mode LEDs");
    assert!(out.contains(&vec![0x9F, 0x00, 0x7F]), "loaded illumination");
    let again = f.feedback(&s, now);
    assert!(again.iter().all(|m| m[0] != 0x9F && m[1] != 0x58), "unchanged LEDs not resent: {again:02x?}");
    let off = f.on_disconnect();
    assert!(off.iter().all(|m| m[2] == 0) && off.contains(&vec![0x91, 0x58, 0x00]));
}

#[test]
fn vu_meter_thresholds() {
    assert_eq!(vu_value(0.0), 0);
    assert!(vu_value(0.063) >= 0x26 && vu_value(0.063) <= 0x40, "-24 dB in first green");
    assert!(vu_value(1.0) >= 0x77, "0 dB lights red");
    assert!(vu_value(0.5) >= 0x57 && vu_value(0.5) < 0x77, "-6 dB in orange");
}

#[test]
fn beat_fx_select_cycles_kind_and_shift_reverses() {
    let mut f = Flx4::new();
    let mut s = snap();
    s.beat_fx.kind = BeatFxKind::Echo;
    assert!(matches!(tap(&mut f, &[0x94, 0x63, 0x7F], &s), Command::BeatFxKind(BeatFxKind::UpEcho)));
    assert!(matches!(tap(&mut f, &[0x94, 0x64, 0x7F], &s), Command::BeatFxKind(BeatFxKind::Delay)));
    assert!(engine(f.on_input(&[0x94, 0x63, 0x00], &s)).is_empty(), "release ignored");
    s.beat_fx.kind = BeatFxKind::Backwash;
    assert!(matches!(tap(&mut f, &[0x94, 0x63, 0x7F], &s), Command::BeatFxKind(BeatFxKind::Delay)), "wraps forward");
    s.beat_fx.kind = BeatFxKind::Delay;
    assert!(matches!(tap(&mut f, &[0x94, 0x64, 0x7F], &s), Command::BeatFxKind(BeatFxKind::Backwash)), "wraps backward");
}

#[test]
fn beat_fx_select_walks_the_full_cycle_in_contract_order() {
    let mut f = Flx4::new();
    let mut s = snap();
    s.beat_fx.kind = BeatFxKind::Delay;
    let mut seen = vec![s.beat_fx.kind];
    for _ in 0..BeatFxKind::ALL.len() - 1 {
        let Command::BeatFxKind(next) = tap(&mut f, &[0x94, 0x63, 0x7F], &s) else { panic!("not a kind command") };
        s.beat_fx.kind = next;
        seen.push(next);
    }
    use BeatFxKind::*;
    assert_eq!(
        seen,
        [Delay, Echo, UpEcho, DownEcho, PingPong, Spiral, Reverb, Flanger, Phaser, Roll, Trans, FilterSweep, PulseVerb, DubTape, Halo, Ascend, Backwash]
    );
}

#[test]
fn beat_left_right_step_through_allowed_divisions_and_clamp() {
    let mut f = Flx4::new();
    let mut s = snap();
    s.beat_fx.beats = 1.0;
    assert!(matches!(tap(&mut f, &[0x94, 0x4A, 0x7F], &s), Command::BeatFxBeats(b) if b == 0.75));
    assert!(matches!(tap(&mut f, &[0x94, 0x4B, 0x7F], &s), Command::BeatFxBeats(b) if b == 2.0));
    s.beat_fx.beats = 0.125;
    assert!(matches!(tap(&mut f, &[0x94, 0x4A, 0x7F], &s), Command::BeatFxBeats(b) if b == 0.125), "clamped at the low end");
    s.beat_fx.beats = 16.0;
    assert!(matches!(tap(&mut f, &[0x94, 0x4B, 0x7F], &s), Command::BeatFxBeats(b) if b == 16.0), "clamped at the high end");
    assert!(engine(f.on_input(&[0x94, 0x4B, 0x00], &s)).is_empty());
}

#[test]
fn beat_left_right_adjust_the_primary_time_parameter_of_each_new_kind() {
    let mut f = Flx4::new();
    let mut s = snap();

    s.beat_fx.kind = BeatFxKind::PulseVerb;
    s.beat_fx.beats = 1.0;
    assert!(matches!(tap(&mut f, &[0x94, 0x4A, 0x7F], &s), Command::BeatFxBeats(b) if b == 0.75), "tremolo division");
    s.beat_fx.beats = 4.0;
    assert!(matches!(tap(&mut f, &[0x94, 0x4B, 0x7F], &s), Command::BeatFxBeats(b) if b == 4.0), "Pulse Verb tops out at 4");

    s.beat_fx.kind = BeatFxKind::DubTape;
    s.beat_fx.beats = 0.75;
    assert!(matches!(tap(&mut f, &[0x94, 0x4B, 0x7F], &s), Command::BeatFxBeats(b) if b == 1.0), "delay time");

    s.beat_fx.kind = BeatFxKind::Backwash;
    s.beat_fx.beats = 2.0;
    assert!(matches!(tap(&mut f, &[0x94, 0x4A, 0x7F], &s), Command::BeatFxBeats(b) if b == 1.0), "swell length");
    assert!(matches!(tap(&mut f, &[0x94, 0x4B, 0x7F], &s), Command::BeatFxBeats(b) if b == 4.0));
    s.beat_fx.beats = 4.0;
    assert!(matches!(tap(&mut f, &[0x94, 0x4B, 0x7F], &s), Command::BeatFxBeats(b) if b == 4.0), "clamped at four beats");

    s.beat_fx.kind = BeatFxKind::Halo;
    s.beat_fx.percent = 50.0;
    assert!(matches!(tap(&mut f, &[0x94, 0x4B, 0x7F], &s), Command::BeatFxPercent(p) if p == 60.0), "Halo size steps the percent");

    s.beat_fx.kind = BeatFxKind::Ascend;
    s.beat_fx.params[0] = 2.0;
    assert!(matches!(tap(&mut f, &[0x94, 0x4A, 0x7F], &s), Command::BeatFxParam { index: 0, value } if value == 1.0), "16 bars down to 8");
    assert!(matches!(tap(&mut f, &[0x94, 0x4B, 0x7F], &s), Command::BeatFxParam { index: 0, value } if value == 3.0), "16 bars up to 32");
    s.beat_fx.params[0] = 3.0;
    assert!(matches!(tap(&mut f, &[0x94, 0x4B, 0x7F], &s), Command::BeatFxParam { index: 0, value } if value == 3.0), "clamped at 32 bars");
    s.beat_fx.params[0] = 0.0;
    assert!(matches!(tap(&mut f, &[0x94, 0x4A, 0x7F], &s), Command::BeatFxParam { index: 0, value } if value == 0.0), "clamped at 4 bars");
    assert!(engine(f.on_input(&[0x94, 0x4B, 0x00], &s)).is_empty(), "release ignored");
}

#[test]
fn beat_left_right_step_the_percent_when_the_kind_is_reverb() {
    let mut f = Flx4::new();
    let mut s = snap();
    s.beat_fx.kind = BeatFxKind::Reverb;
    s.beat_fx.percent = 50.0;
    assert!(matches!(tap(&mut f, &[0x94, 0x4A, 0x7F], &s), Command::BeatFxPercent(p) if p == 40.0));
    assert!(matches!(tap(&mut f, &[0x94, 0x4B, 0x7F], &s), Command::BeatFxPercent(p) if p == 60.0));
    s.beat_fx.percent = 37.0;
    assert!(matches!(tap(&mut f, &[0x94, 0x4A, 0x7F], &s), Command::BeatFxPercent(p) if p == 30.0), "fine value snaps down to a preset");
    assert!(matches!(tap(&mut f, &[0x94, 0x4B, 0x7F], &s), Command::BeatFxPercent(p) if p == 40.0));
    s.beat_fx.percent = 1.0;
    assert!(matches!(tap(&mut f, &[0x94, 0x4A, 0x7F], &s), Command::BeatFxPercent(p) if p == 1.0), "clamped at the low end");
    assert!(matches!(tap(&mut f, &[0x94, 0x4B, 0x7F], &s), Command::BeatFxPercent(p) if p == 10.0));
    s.beat_fx.percent = 100.0;
    assert!(matches!(tap(&mut f, &[0x94, 0x4B, 0x7F], &s), Command::BeatFxPercent(p) if p == 100.0), "clamped at the high end");
    assert!(engine(f.on_input(&[0x94, 0x4B, 0x00], &s)).is_empty(), "release ignored");
    s.beat_fx.kind = BeatFxKind::Echo;
    assert!(matches!(tap(&mut f, &[0x94, 0x4B, 0x7F], &s), Command::BeatFxBeats(_)), "other kinds still step beats");
}

#[test]
fn beat_fx_level_is_fourteen_bit() {
    let mut f = Flx4::new();
    let s = snap();
    let mut level = |msgs: &[[u8; 3]]| {
        let mut last = None;
        for m in msgs {
            for c in engine(f.on_input(m, &s)) {
                if let Command::BeatFxLevel(v) = c {
                    last = Some(v);
                }
            }
        }
        last.expect("level command")
    };
    assert!((level(&[[0xB4, 0x02, 0x40], [0xB4, 0x22, 0x00]]) - 0.5).abs() < 0.01, "MSB 0x40 is mid-way");
    assert!((level(&[[0xB4, 0x02, 0x7F], [0xB4, 0x22, 0x7F]]) - 1.0).abs() < 1e-6, "full scale");
    assert_eq!(level(&[[0xB4, 0x02, 0x00], [0xB4, 0x22, 0x00]]), 0.0);
}

#[test]
fn fx_on_off_toggles_from_engine_state() {
    let mut f = Flx4::new();
    let mut s = snap();
    assert!(matches!(tap(&mut f, &[0x94, 0x47, 0x7F], &s), Command::BeatFxEnabled(true)));
    s.beat_fx.enabled = true;
    assert!(matches!(tap(&mut f, &[0x94, 0x47, 0x7F], &s), Command::BeatFxEnabled(false)));
    assert!(engine(f.on_input(&[0x94, 0x47, 0x00], &s)).is_empty(), "release ignored");
}

#[test]
fn fx_on_off_from_channel_two_selects_target_b_until_the_switch_has_reported() {
    let mut f = Flx4::new();
    let s = snap();
    let cmds = engine(f.on_input(&[0x95, 0x47, 0x7F], &s));
    assert!(matches!(cmds.as_slice(), [Command::BeatFxTarget(FxTarget::B), Command::BeatFxEnabled(true)]), "{cmds:?}");

    let mut f = Flx4::new();
    f.on_input(&[0x94, 0x10, 0x7F], &s);
    let cmds = engine(f.on_input(&[0x95, 0x47, 0x7F], &s));
    assert!(matches!(cmds.as_slice(), [Command::BeatFxEnabled(true)]), "switch position already known: {cmds:?}");
}

#[test]
fn ch_select_switch_maps_to_target() {
    let target = |f: &mut Flx4, msgs: &[[u8; 3]]| {
        let s = snap();
        let mut last = None;
        for m in msgs {
            for c in engine(f.on_input(m, &s)) {
                if let Command::BeatFxTarget(t) = c {
                    last = Some(t);
                }
            }
        }
        last
    };
    let mut f = Flx4::new();
    let ch1 = [[0x94, 0x10, 0x7F], [0x94, 0x11, 0x00], [0x95, 0x10, 0x00], [0x95, 0x11, 0x00]];
    assert_eq!(target(&mut f, &ch1), Some(FxTarget::A));
    let ch2 = [[0x94, 0x10, 0x00], [0x94, 0x11, 0x00], [0x95, 0x10, 0x00], [0x95, 0x11, 0x7F]];
    assert_eq!(target(&mut f, &ch2), Some(FxTarget::B));
    let both = [[0x94, 0x10, 0x7F], [0x94, 0x11, 0x00], [0x95, 0x10, 0x00], [0x95, 0x11, 0x7F]];
    assert_eq!(target(&mut f, &both), Some(FxTarget::Master));
    assert_eq!(target(&mut f, &ch1), Some(FxTarget::A), "back to channel 1");
}

#[test]
fn ch_select_note_off_form_is_understood() {
    let mut f = Flx4::new();
    let s = snap();
    f.on_input(&[0x94, 0x10, 0x7F], &s);
    f.on_input(&[0x95, 0x11, 0x7F], &s);
    let cmds = engine(f.on_input(&[0x84, 0x10, 0x40], &s));
    assert!(matches!(cmds.as_slice(), [Command::BeatFxTarget(FxTarget::B)]), "{cmds:?}");
}

#[test]
fn fx_on_off_led_echoes_enabled_state() {
    let mut f = Flx4::new();
    let mut s = snap();
    let now = Instant::now();
    let out = f.feedback(&s, now);
    assert!(out.contains(&vec![0x94, 0x47, 0x00]) && out.contains(&vec![0x94, 0x43, 0x00]), "off: steady");
    s.beat_fx.enabled = true;
    let out = f.feedback(&s, now);
    assert!(out.contains(&vec![0x94, 0x47, 0x7F]) && out.contains(&vec![0x94, 0x43, 0x7F]), "on: NOTE ON blinks the button");
    let again = f.feedback(&s, now);
    assert!(again.iter().all(|m| m[1] != 0x47), "unchanged state is not resent");
}

fn all(f: &mut Flx4, msgs: &[[u8; 3]], s: &EngineSnapshot) -> Vec<Command> {
    msgs.iter().flat_map(|m| engine(f.on_input(m, s))).collect()
}

fn fx_states(actions: &[Action]) -> Vec<FxControlState> {
    actions
        .iter()
        .filter_map(|a| match a {
            Action::App(AppAction::FxControl(state)) => Some(*state),
            _ => None,
        })
        .collect()
}

/// LEVEL/DEPTH as captured on the hardware: MSB, LSB, then the same pair mirrored on `0xB5`.
fn level_msgs(msb: u8, lsb: u8) -> [[u8; 3]; 4] {
    [[0xB4, 0x02, msb], [0xB4, 0x22, lsb], [0xB5, 0x02, msb], [0xB5, 0x22, lsb]]
}

fn echo_snap() -> EngineSnapshot {
    let mut s = snap();
    s.beat_fx.kind = BeatFxKind::Echo;
    s.beat_fx.params[0] = 42.0;
    s.beat_fx.level = 0.5;
    s
}

#[test]
fn shift_level_drives_the_focused_parameter_with_soft_pickup() {
    let mut f = Flx4::new();
    let s = echo_snap();
    assert!(all(&mut f, &[[0x90, 0x3F, 0x7F]], &s).is_empty(), "deck SHIFT");
    let far = all(&mut f, &level_msgs(0x60, 0x00), &s);
    assert!(far.is_empty(), "knob far from the stored feedback: picked up later, {far:?}");
    let state = f.fx_control(&s);
    assert_eq!(state.knob_target, FxKnobTarget::Param);
    let pickup = state.pickup.expect("pickup pending");
    assert!((pickup.value - 0.42).abs() < 1e-6 && pickup.knob > 0.7);
    let cmds = all(&mut f, &level_msgs(0x35, 0x40), &s);
    assert!(
        matches!(cmds.as_slice(), [.., Command::BeatFxParam { index: 0, value }] if (*value - 42.0).abs() < 1.0),
        "caught up: feedback follows, B5 mirror ignored: {cmds:?}"
    );
    assert!(cmds.iter().all(|c| matches!(c, Command::BeatFxParam { index: 0, .. })), "never the level while SHIFT is held");
    let cmds = all(&mut f, &level_msgs(0x50, 0x00), &s);
    assert!(matches!(cmds.as_slice(), [.., Command::BeatFxParam { index: 0, value }] if (*value - 63.0).abs() < 1.0), "{cmds:?}");

    assert!(all(&mut f, &[[0x90, 0x3F, 0x00]], &s).is_empty());
    assert!(all(&mut f, &level_msgs(0x50, 0x10), &s).is_empty(), "back on LEVEL, knob 0.63 vs stored 0.5: waits");
    let cmds = all(&mut f, &level_msgs(0x3E, 0x00), &s);
    assert!(matches!(cmds.as_slice(), [.., Command::BeatFxLevel(v)] if (*v - 0.49).abs() < 0.01), "swept through 0.5: engages, {cmds:?}");
}

#[test]
fn either_decks_shift_counts_for_the_fx_section() {
    let mut f = Flx4::new();
    let s = echo_snap();
    f.on_input(&[0x91, 0x3F, 0x7F], &s);
    let cmds = all(&mut f, &level_msgs(0x35, 0x40), &s);
    assert!(matches!(cmds.as_slice(), [Command::BeatFxParam { index: 0, .. }, ..]), "{cmds:?}");
}

#[test]
fn shift_beat_pages_the_parameter_once_per_press_despite_the_twin_messages() {
    let mut f = Flx4::new();
    let mut s = snap();
    s.beat_fx.kind = BeatFxKind::Reverb;
    assert_eq!(f.focus(BeatFxKind::Reverb), Some(0), "Tone first");
    let press = [[0x90, 0x3F, 0x7F], [0x94, 0x6B, 0x7F], [0x95, 0x6B, 0x7F], [0x94, 0x6B, 0x00], [0x95, 0x6B, 0x00]];
    assert!(all(&mut f, &press, &s).is_empty(), "paging sends nothing to the engine");
    assert_eq!(f.focus(BeatFxKind::Reverb), Some(1), "one press, one page");
    all(&mut f, &[[0x94, 0x6B, 0x7F], [0x95, 0x6B, 0x7F], [0x94, 0x6B, 0x00], [0x95, 0x6B, 0x00]], &s);
    assert_eq!(f.focus(BeatFxKind::Reverb), Some(2), "Decay");
    all(&mut f, &[[0x94, 0x6B, 0x7F], [0x95, 0x6B, 0x7F], [0x94, 0x6B, 0x00], [0x95, 0x6B, 0x00]], &s);
    assert_eq!(f.focus(BeatFxKind::Reverb), Some(4), "Voicing is a choice and is skipped");
    all(&mut f, &[[0x94, 0x66, 0x7F], [0x95, 0x66, 0x7F], [0x94, 0x66, 0x00], [0x95, 0x66, 0x00]], &s);
    assert_eq!(f.focus(BeatFxKind::Reverb), Some(2), "SHIFT + BEAT left pages back");
    assert_eq!(f.focus(BeatFxKind::Echo), Some(0), "focus is remembered per kind");
    assert_eq!(f.focus(BeatFxKind::Roll), None, "no continuous parameter");
    assert!(all(&mut f, &[[0x94, 0x6B, 0x7F], [0x95, 0x6B, 0x7F]], &{
        let mut r = snap();
        r.beat_fx.kind = BeatFxKind::Roll;
        r
    })
    .is_empty());
}

#[test]
fn shift_fx_on_off_fires_one_release_fx_from_the_twin_messages() {
    let mut f = Flx4::new();
    let mut s = snap();
    s.beat_fx.enabled = true;
    f.on_input(&[0x94, 0x10, 0x7F], &s);
    f.on_input(&[0x95, 0x11, 0x7F], &s);
    let cmds = all(&mut f, &[[0x90, 0x3F, 0x7F], [0x94, 0x43, 0x7F], [0x95, 0x43, 0x7F], [0x94, 0x43, 0x00], [0x95, 0x43, 0x00]], &s);
    assert!(matches!(cmds.as_slice(), [Command::ReleaseFx]), "{cmds:?}");
    let again = all(&mut f, &[[0x94, 0x43, 0x7F], [0x95, 0x43, 0x7F], [0x94, 0x43, 0x00], [0x95, 0x43, 0x00]], &s);
    assert!(matches!(again.as_slice(), [Command::ReleaseFx]), "a second press fires again: {again:?}");

    let mut f = Flx4::new();
    let cmds = all(&mut f, &[[0x95, 0x43, 0x7F], [0x95, 0x43, 0x00]], &s);
    assert!(matches!(cmds.as_slice(), [Command::BeatFxTarget(FxTarget::B), Command::ReleaseFx]), "CH2 only, switch unseen: {cmds:?}");
}

#[test]
fn shift_beat_twins_do_not_step_the_division_twice() {
    let mut f = Flx4::new();
    let mut s = snap();
    s.beat_fx.beats = 1.0;
    let cmds = all(&mut f, &[[0x94, 0x4B, 0x7F], [0x95, 0x4B, 0x7F], [0x94, 0x4B, 0x00], [0x95, 0x4B, 0x00]], &s);
    assert!(matches!(cmds.as_slice(), [Command::BeatFxBeats(b)] if *b == 2.0), "{cmds:?}");
}

#[test]
fn pad_mode_is_host_driven_and_a_stale_pad_note_cannot_bounce_it() {
    let mut f = Flx4::new();
    let s = snap();
    assert!(f.drain_output().is_empty());
    assert!(engine(f.on_input(&[0x90, 0x1E, 0x7F], &s)).is_empty());
    assert_eq!(f.pad_mode(0), PadMode::PadFx1);
    let leds = f.drain_output();
    assert!(leds.contains(&vec![0x90, 0x1E, 0x7F]), "PAD FX 1 LED goes out with the press, not on the next tick: {leds:02x?}");
    assert!(leds.contains(&vec![0x90, 0x1B, 0x00]), "previous mode LED off");
    f.on_input(&[0x90, 0x1E, 0x00], &s);

    assert!(engine(f.on_input(&[0x97, 0x00, 0x7F], &s)).is_empty(), "hot cue note sent before the LED landed is ignored");
    assert_eq!(f.pad_mode(0), PadMode::PadFx1, "and does not switch the mode back");
    assert!(engine(f.on_input(&[0x97, 0x00, 0x00], &s)).is_empty(), "its release is swallowed too");

    match one(&mut f, &[0x97, 0x10, 0x7F], &s) {
        Command::PadFx { deck: 0, voice: Some(v) } => assert_eq!((v.kind, v.beats), (BeatFxKind::Echo, 0.5)),
        other => panic!("{other:?}"),
    }
    assert!(matches!(one(&mut f, &[0x97, 0x10, 0x00], &s), Command::PadFx { deck: 0, voice: None }));

    f.on_input(&[0x90, 0x3F, 0x7F], &s);
    assert!(engine(f.on_input(&[0x90, 0x6B, 0x7F], &s)).is_empty());
    assert_eq!(f.pad_mode(0), PadMode::PadFx2, "SHIFT + PAD FX 1");
    assert!(f.drain_output().contains(&vec![0x90, 0x6B, 0x7F]));
    assert_eq!(f.pad_mode(1), PadMode::HotCue, "deck 2 keeps its own mode");
}

#[test]
fn hot_cues_still_work_after_returning_from_a_pad_fx_mode() {
    let mut f = Flx4::new();
    let s = snap();
    f.on_input(&[0x91, 0x1E, 0x7F], &s);
    f.on_input(&[0x91, 0x1B, 0x7F], &s);
    assert!(matches!(one(&mut f, &[0x99, 0x03, 0x7F], &s), Command::HotCue { deck: 1, slot: 3, pressed: true }));
    assert!(matches!(one(&mut f, &[0x99, 0x03, 0x00], &s), Command::HotCue { deck: 1, slot: 3, pressed: false }));
}

#[test]
fn pad_fx1_banks_one_shots_and_shift_independent_release() {
    let mut f = Flx4::new();
    let s = snap();
    f.on_input(&[0x91, 0x1E, 0x7F], &s);
    match one(&mut f, &[0x9A, 0x12, 0x7F], &s) {
        Command::PadFx { deck: 1, voice: Some(v) } => assert_eq!(v.kind, BeatFxKind::Trans, "SHIFT bank pad 3"),
        other => panic!("{other:?}"),
    }
    assert!(matches!(one(&mut f, &[0x99, 0x12, 0x00], &s), Command::PadFx { deck: 1, voice: None }), "released after letting go of SHIFT");

    match one(&mut f, &[0x99, 0x15, 0x7F], &s) {
        Command::PadFxRelease { deck: 1, kind: ReleaseFxKind::Backspin, beats } => assert_eq!(beats, 1.0),
        other => panic!("{other:?}"),
    }
    assert!(engine(f.on_input(&[0x99, 0x15, 0x00], &s)).is_empty(), "one-shot ignores the release");

    one(&mut f, &[0x99, 0x10, 0x7F], &s);
    match one(&mut f, &[0x99, 0x13, 0x7F], &s) {
        Command::PadFx { voice: Some(v), .. } => assert_eq!(v.kind, BeatFxKind::Roll, "last pressed wins"),
        other => panic!("{other:?}"),
    }
    match one(&mut f, &[0x99, 0x13, 0x00], &s) {
        Command::PadFx { voice: Some(v), .. } => assert_eq!(v.kind, BeatFxKind::Echo, "falls back to the pad still held"),
        other => panic!("{other:?}"),
    }
    let cmds = engine(f.on_input(&[0x91, 0x20, 0x7F], &s));
    assert!(matches!(cmds.as_slice(), [Command::PadFx { deck: 1, voice: None }]), "mode change lets go: {cmds:?}");
}

#[test]
fn holding_a_pad_redirects_beat_and_level_to_its_voice() {
    let mut f = Flx4::new();
    let mut s = snap();
    s.beat_fx.beats = 1.0;
    f.on_input(&[0x90, 0x1E, 0x7F], &s);
    one(&mut f, &[0x97, 0x10, 0x7F], &s);
    let voice = |c: Command| match c {
        Command::PadFx { deck: 0, voice: Some(v) } => v,
        other => panic!("{other:?}"),
    };
    assert_eq!(voice(tap(&mut f, &[0x94, 0x4B, 0x7F], &s)).beats, 0.75, "1/2 -> 3/4 on the pad, not the unit");
    assert_eq!(voice(tap(&mut f, &[0x94, 0x4A, 0x7F], &s)).beats, 0.5);
    assert_eq!(f.fx_control(&s).knob_target, FxKnobTarget::PadLevel);
    assert!(all(&mut f, &level_msgs(0x10, 0x00), &s).is_empty(), "knob far from the pad's 70%");
    let cmds = all(&mut f, &level_msgs(0x5A, 0x00), &s);
    let Some(Command::PadFx { voice: Some(v), .. }) = cmds.last() else { panic!("{cmds:?}") };
    assert!((v.level - 0.71).abs() < 0.01 && v.beats == 0.5, "level tweak keeps the beat tweak: {v:?}");
    one(&mut f, &[0x97, 0x10, 0x00], &s);
    assert!(matches!(tap(&mut f, &[0x94, 0x4B, 0x7F], &s), Command::BeatFxBeats(b) if b == 2.0), "back on the unit");
}

#[test]
fn pad_fx2_gates_the_beat_fx_and_shift_picks_a_favourite() {
    let mut f = Flx4::new();
    let mut s = snap();
    f.on_input(&[0x90, 0x6B, 0x7F], &s);
    let cmds = engine(f.on_input(&[0x97, 0x52, 0x7F], &s));
    assert!(matches!(cmds.as_slice(), [Command::BeatFxBeats(b), Command::BeatFxEnabled(true)] if *b == 0.5), "{cmds:?}");
    s.beat_fx.enabled = true;
    assert!(matches!(one(&mut f, &[0x97, 0x52, 0x00], &s), Command::BeatFxEnabled(false)), "gate closes on release");
    assert!(matches!(one(&mut f, &[0x97, 0x57, 0x7F], &s), Command::BeatFxBeats(b) if b == 8.0), "already on: division only");
    assert!(engine(f.on_input(&[0x97, 0x57, 0x00], &s)).is_empty());
    s.beat_fx.kind = BeatFxKind::Reverb;
    assert!(matches!(one(&mut f, &[0x97, 0x53, 0x7F], &s), Command::BeatFxPercent(p) if p == 50.0));
    f.on_input(&[0x97, 0x53, 0x00], &s);
    assert!(matches!(one(&mut f, &[0x98, 0x51, 0x7F], &s), Command::BeatFxKind(BeatFxKind::PingPong)));
    f.on_input(&[0x91, 0x6B, 0x7F], &s);
    assert!(matches!(one(&mut f, &[0x9A, 0x57, 0x7F], &s), Command::BeatFxKind(BeatFxKind::FilterSweep)), "deck 2 too");
}

#[test]
fn smart_cfx_toggles_cycles_presets_and_arms_from_centre() {
    let mut f = Flx4::new();
    let mut s = snap();
    let cfx = |f: &mut Flx4, msb: u8, s: &EngineSnapshot| engine(f.on_input(&[0xB6, 0x17, msb], s));
    assert!(matches!(cfx(&mut f, 0x70, &s).as_slice(), [Command::Mixer { channel: 0, param: MixerParam::Filter(_) }]));
    assert!(matches!(tap(&mut f, &[0x96, 0x00, 0x7F], &s), Command::SmartCfx(true)));
    s.smart_cfx.enabled = true;
    assert_eq!(f.fx_control(&s).smart_arming, [true, true], "knob off-centre / unknown");
    assert!(cfx(&mut f, 0x60, &s).is_empty(), "ignored until it passes the centre");
    let cmds = cfx(&mut f, 0x30, &s);
    assert!(matches!(cmds.as_slice(), [Command::Mixer { channel: 0, param: MixerParam::Filter(v) }] if *v < -0.2), "swept through centre: {cmds:?}");
    assert!(matches!(tap(&mut f, &[0x96, 0x08, 0x7F], &s), Command::SmartCfxPreset(SmartCfxPreset::Riser)));
    s.smart_cfx.preset = SmartCfxPreset::CrushChop;
    assert!(matches!(tap(&mut f, &[0x96, 0x08, 0x7F], &s), Command::SmartCfxPreset(SmartCfxPreset::EchoWash)), "wraps");
    assert!(cfx(&mut f, 0x10, &s).is_empty(), "preset change re-arms");
    assert!(!cfx(&mut f, 0x40, &s).is_empty());
}

#[test]
fn fx_control_state_is_reported_when_it_changes() {
    let mut f = Flx4::new();
    let s = echo_snap();
    let states = fx_states(&f.on_input(&[0x90, 0x3F, 0x7F], &s));
    assert_eq!(states.len(), 1);
    assert!(states[0].shift && states[0].knob_target == FxKnobTarget::Param && states[0].focus == Some(0));
    assert!(fx_states(&f.on_input(&[0xB6, 0x40, 0x01], &s)).is_empty(), "unchanged state is not resent");
    let states = fx_states(&f.on_input(&[0x94, 0x63, 0x7F], &s));
    assert_eq!(states[0].hud.map(|h| h.control), Some(FxHudControl::Kind));
    let before = states[0].hud.unwrap().seq;
    f.on_input(&[0x94, 0x63, 0x00], &s);
    let states = fx_states(&f.on_input(&[0x94, 0x4B, 0x7F], &s));
    assert!(states[0].hud.unwrap().seq > before, "every gesture bumps the sequence");
    let held = fx_states(&f.on_input(&[0x90, 0x1E, 0x7F], &s));
    assert_eq!(held[0].pad_modes[0], PadMode::PadFx1);
}

#[test]
fn pad_fx_and_smart_cfx_leds() {
    let mut f = Flx4::new();
    let mut s = snap();
    let now = Instant::now();
    f.on_input(&[0x90, 0x1E, 0x7F], &s);
    f.on_input(&[0x91, 0x6B, 0x7F], &s);
    s.beat_fx.beats = 0.25;
    s.beat_fx.kind = BeatFxKind::PingPong;
    let out = f.feedback(&s, now);
    assert!(out.contains(&vec![0x97, 0x10, 0x7F]) && out.contains(&vec![0x98, 0x17, 0x7F]), "assigned PAD FX 1 pads lit on both layers");
    assert!(out.contains(&vec![0x99, 0x51, 0x7F]) && out.contains(&vec![0x99, 0x50, 0x00]), "PAD FX 2: current division lit");
    assert!(out.contains(&vec![0x9A, 0x51, 0x7F]) && out.contains(&vec![0x9A, 0x50, 0x00]), "PAD FX 2 + SHIFT: current favourite lit");
    assert!(out.contains(&vec![0x96, 0x00, 0x00]), "SMART CFX off");
    assert!(out.contains(&vec![0x95, 0x47, 0x00]) && out.contains(&vec![0x95, 0x43, 0x00]), "FX ON/OFF on both FX channels");
    s.smart_cfx.enabled = true;
    f.on_input(&[0xB6, 0x17, 0x40], &s);
    f.on_input(&[0xB6, 0x18, 0x40], &s);
    let out = f.feedback(&s, now);
    assert!(out.contains(&vec![0x96, 0x00, 0x7F]), "SMART CFX on, knobs armed");
    s.pad_fx[0].voice = Some(PadVoice { kind: BeatFxKind::Echo, beats: 0.5, percent: 50.0, level: 0.7, param0: Some(55.0) });
    one(&mut f, &[0x97, 0x10, 0x7F], &s);
    let blink: Vec<u8> = (0..8).map(|i| now + std::time::Duration::from_millis(i * 125)).filter_map(|t| f.feedback(&s, t).iter().find(|m| m[..2] == [0x97, 0x10]).map(|m| m[2])).collect();
    assert!(blink.contains(&0x00) && blink.contains(&0x7F), "held pad blinks: {blink:02x?}");
}
