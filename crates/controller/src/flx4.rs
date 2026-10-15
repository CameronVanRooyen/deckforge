//! Pioneer DJ DDJ-FLX4 mapping, written from the official MIDI message list
//! (see docs/research/ddj-flx4-midi.md for every byte value and its provenance).

use std::collections::HashMap;
use std::time::{Duration, Instant};

use deckforge_common::{
    beat_fx_knob_params, beat_fx_params, snap_beat_fx_beats_for, step_beat_fx_percent, Band, BeatFxKind, FxTarget, PadFxAction, PadVoice,
    DECK_COUNT, HOT_CUE_SLOTS, PAD_FX1_BANK, PAD_FX1_SHIFT_BANK, PAD_FX2_BEATS, PAD_FX2_FAVOURITES, PAD_FX2_PERCENTS,
};
use deckforge_engine::{Command, EngineSnapshot, JogMode, MixerParam};

use crate::action::{Action, AppAction, FxControlState, FxHud, FxHudControl, FxKnobTarget, FxPickup, HeldPad};
use crate::mapping::{ControllerMapping, MidiOut};

const NOTE_ON: u8 = 0x90;
const CC: u8 = 0xB0;
const GLOBAL_CH: u8 = 6;
const FX_CH_1: u8 = 4;
const FX_CH_2: u8 = 5;
const ON: u8 = 0x7F;

mod note {
    pub const PLAY: u8 = 0x0B;
    pub const PLAY_SHIFT: u8 = 0x0E;
    pub const CUE: u8 = 0x0C;
    pub const CUE_SHIFT: u8 = 0x48;
    pub const SHIFT: u8 = 0x3F;
    pub const SYNC: u8 = 0x58;
    pub const SYNC_SHIFT: u8 = 0x60;
    pub const SYNC_LONG: u8 = 0x5C;
    pub const LOOP_IN: u8 = 0x10;
    pub const LOOP_IN_SHIFT: u8 = 0x4C;
    pub const LOOP_OUT: u8 = 0x11;
    pub const LOOP_OUT_SHIFT: u8 = 0x4E;
    pub const BEAT4: u8 = 0x4D;
    pub const BEAT4_SHIFT: u8 = 0x50;
    pub const CALL_LEFT: u8 = 0x51;
    pub const CALL_LEFT_SHIFT: u8 = 0x3E;
    pub const CALL_RIGHT: u8 = 0x53;
    pub const CALL_RIGHT_SHIFT: u8 = 0x3D;
    pub const PFL: u8 = 0x54;
    pub const PFL_SHIFT: u8 = 0x68;
    pub const JOG_TOUCH: u8 = 0x36;
    pub const JOG_TOUCH_SHIFT: u8 = 0x67;
    pub const FADER_START_PLAY: u8 = 0x66;
    pub const FADER_START_CUE: u8 = 0x52;
    pub const VINYL: u8 = 0x17;
    pub const MODE_HOT_CUE: u8 = 0x1B;
    pub const MODE_PAD_FX1: u8 = 0x1E;
    pub const MODE_BEAT_JUMP: u8 = 0x20;
    pub const MODE_SAMPLER: u8 = 0x22;
    pub const MODE_KEYBOARD: u8 = 0x69;
    pub const MODE_PAD_FX2: u8 = 0x6B;
    pub const MODE_BEAT_LOOP: u8 = 0x6D;
    pub const MODE_KEY_SHIFT: u8 = 0x6F;
    pub const BROWSE_PUSH: u8 = 0x41;
    pub const LOAD_1: u8 = 0x46;
    pub const LOAD_2: u8 = 0x47;
    pub const FX_SELECT: u8 = 0x63;
    pub const FX_SELECT_SHIFT: u8 = 0x64;
    pub const FX_BEAT_LEFT: u8 = 0x4A;
    pub const FX_BEAT_RIGHT: u8 = 0x4B;
    pub const FX_ON: u8 = 0x47;
    pub const FX_ON_SHIFT: u8 = 0x43;
    pub const FX_BEAT_LEFT_SHIFT: u8 = 0x66;
    pub const FX_BEAT_RIGHT_SHIFT: u8 = 0x6B;
    pub const FX_CH_SELECT_1: u8 = 0x10;
    pub const FX_CH_SELECT_2: u8 = 0x11;
    pub const SMART_CFX: u8 = 0x00;
    pub const SMART_CFX_SHIFT: u8 = 0x08;
}

mod cc {
    pub const TEMPO: u8 = 0x00;
    pub const TRIM: u8 = 0x04;
    pub const EQ_HIGH: u8 = 0x07;
    pub const EQ_MID: u8 = 0x0B;
    pub const EQ_LOW: u8 = 0x0F;
    pub const FADER: u8 = 0x13;
    pub const JOG_RING: u8 = 0x21;
    pub const JOG_VINYL: u8 = 0x22;
    pub const JOG_PLATTER: u8 = 0x23;
    pub const JOG_SHIFT: u8 = 0x29;
    pub const VU: u8 = 0x02;
    pub const MASTER_LEVEL: u8 = 0x08;
    pub const PHONES_MIX: u8 = 0x0C;
    pub const PHONES_LEVEL: u8 = 0x0D;
    pub const FX_LEVEL: u8 = 0x02;
    pub const FX_LEVEL_LSB: u8 = 0x22;
    pub const CFX_1: u8 = 0x17;
    pub const CFX_2: u8 = 0x18;
    pub const CROSSFADER: u8 = 0x1F;
    pub const BROWSE: u8 = 0x40;
    pub const BROWSE_SHIFT: u8 = 0x64;
    pub const LSB_OFFSET: u8 = 0x20;
}

/// Unofficial position-query/keepalive observed from rekordbox (docs/research/ddj-flx4-midi.md §6).
pub const KEEPALIVE_SYSEX: [u8; 12] = [0xF0, 0x00, 0x40, 0x05, 0x00, 0x00, 0x04, 0x05, 0x00, 0x50, 0x02, 0xF7];
const KEEPALIVE_INTERVAL: Duration = Duration::from_millis(200);
const PAD_STATUS: [[u8; 2]; DECK_COUNT] = [[0x97, 0x98], [0x99, 0x9A]];
const BEAT_JUMP_PADS: [f64; 8] = [-1.0, 1.0, -2.0, 2.0, -4.0, 4.0, -8.0, 8.0];
const BEAT_LOOP_PADS: [f64; 8] = [0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0, 32.0];
const CALL_JUMP_BEATS: f64 = 4.0;
const TAKEOVER_THRESHOLD: f64 = 0.03;
const CFX_CENTRE: f64 = 0.02;
const PAD_BLINK_MS: u128 = 125;
const SLOW_BLINK_MS: u128 = 250;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PadMode {
    #[default]
    HotCue,
    PadFx1,
    BeatJump,
    Sampler,
    Keyboard,
    PadFx2,
    BeatLoop,
    KeyShift,
}

impl PadMode {
    fn from_button(n: u8) -> Option<Self> {
        Some(match n {
            note::MODE_HOT_CUE => PadMode::HotCue,
            note::MODE_PAD_FX1 => PadMode::PadFx1,
            note::MODE_BEAT_JUMP => PadMode::BeatJump,
            note::MODE_SAMPLER => PadMode::Sampler,
            note::MODE_KEYBOARD => PadMode::Keyboard,
            note::MODE_PAD_FX2 => PadMode::PadFx2,
            note::MODE_BEAT_LOOP => PadMode::BeatLoop,
            note::MODE_KEY_SHIFT => PadMode::KeyShift,
            _ => return None,
        })
    }

    fn from_pad_note(n: u8) -> Self {
        match n >> 4 {
            0 => PadMode::HotCue,
            1 => PadMode::PadFx1,
            2 => PadMode::BeatJump,
            3 => PadMode::Sampler,
            4 => PadMode::Keyboard,
            5 => PadMode::PadFx2,
            6 => PadMode::BeatLoop,
            _ => PadMode::KeyShift,
        }
    }

    fn pad_base(self) -> u8 {
        (match self {
            PadMode::HotCue => 0,
            PadMode::PadFx1 => 1,
            PadMode::BeatJump => 2,
            PadMode::Sampler => 3,
            PadMode::Keyboard => 4,
            PadMode::PadFx2 => 5,
            PadMode::BeatLoop => 6,
            PadMode::KeyShift => 7,
        }) << 4
    }

    const BUTTONS: [(u8, PadMode); 8] = [
        (note::MODE_HOT_CUE, PadMode::HotCue),
        (note::MODE_PAD_FX1, PadMode::PadFx1),
        (note::MODE_BEAT_JUMP, PadMode::BeatJump),
        (note::MODE_SAMPLER, PadMode::Sampler),
        (note::MODE_KEYBOARD, PadMode::Keyboard),
        (note::MODE_PAD_FX2, PadMode::PadFx2),
        (note::MODE_BEAT_LOOP, PadMode::BeatLoop),
        (note::MODE_KEY_SHIFT, PadMode::KeyShift),
    ];
}

#[derive(Debug, Clone, Copy, Default)]
struct HiRes {
    msb: u8,
    lsb: u8,
}

impl HiRes {
    fn value(self) -> f64 {
        ((self.msb as u16) << 7 | self.lsb as u16) as f64 / 16_383.0
    }
}

#[derive(Debug, Clone, Copy)]
struct HeldFx {
    shift: bool,
    mode: PadMode,
    voice: Option<PadVoice>,
    order: u64,
}

#[derive(Debug, Default)]
struct DeckState {
    shift: bool,
    jog_touched: bool,
    pad_mode: PadMode,
    /// Set once the host has chosen the mode; the hardware then follows the host-lit mode LED.
    mode_from_host: bool,
    stale_presses: u8,
    held: [Option<HeldFx>; 8],
    beat_jump_scale: f64,
    tempo_engaged: bool,
    last_loaded_token: u64,
}

impl DeckState {
    fn latest_voice(&self) -> Option<(usize, HeldFx)> {
        self.held.iter().enumerate().filter_map(|(i, h)| h.filter(|h| h.voice.is_some()).map(|h| (i, h))).max_by_key(|(_, h)| h.order)
    }

    fn latest_held(&self) -> Option<(usize, HeldFx)> {
        self.held.iter().enumerate().filter_map(|(i, h)| h.map(|h| (i, h))).max_by_key(|(_, h)| h.order)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum KnobKey {
    Level,
    Param(BeatFxKind, Option<usize>),
    Pad(usize),
}

pub struct Flx4 {
    decks: [DeckState; DECK_COUNT],
    hires: HashMap<(u8, u8), HiRes>,
    leds: HashMap<(u8, u8), u8>,
    pending: Vec<MidiOut>,
    fx_switch: [bool; 2],
    fx_switch_seen: bool,
    fx_notes_down: [bool; 128],
    focus: [Option<usize>; BeatFxKind::ALL.len()],
    knob: Option<f64>,
    knob_key: KnobKey,
    knob_engaged: bool,
    gate: Option<(usize, usize)>,
    press_order: u64,
    cfx: [Option<f64>; DECK_COUNT],
    cfx_armed: [bool; DECK_COUNT],
    hud: Option<FxHud>,
    reported: Option<FxControlState>,
    last_keepalive: Option<Instant>,
    pub keepalive: bool,
    pub vinyl_mode: bool,
    /// When true, MASTER LEVEL / HEADPHONES MIX / LEVEL drive the software mixer instead of being display-only.
    pub software_monitor_knobs: bool,
}

impl Default for Flx4 {
    fn default() -> Self {
        Self::new()
    }
}

fn shifted(n: u8) -> u8 {
    match n {
        note::PLAY => note::PLAY_SHIFT,
        note::CUE => note::CUE_SHIFT,
        note::SYNC => note::SYNC_SHIFT,
        note::LOOP_IN => note::LOOP_IN_SHIFT,
        note::LOOP_OUT => note::LOOP_OUT_SHIFT,
        note::BEAT4 => note::BEAT4_SHIFT,
        note::PFL => note::PFL_SHIFT,
        other => other,
    }
}

/// Peak level → VU meter value using the segment thresholds in the official list.
pub fn vu_value(peak: f32) -> u8 {
    if peak <= 0.0 {
        return 0;
    }
    let db = 20.0 * peak.log10();
    const POINTS: [(f32, f32); 6] = [(-36.0, 0.0), (-24.0, 38.0), (-12.0, 65.0), (-6.0, 87.0), (-3.0, 101.0), (0.0, 119.0)];
    if db >= 0.0 {
        return 127;
    }
    for w in POINTS.windows(2) {
        let ((d0, v0), (d1, v1)) = (w[0], w[1]);
        if db <= d1 {
            if db <= d0 {
                return v0 as u8;
            }
            return (v0 + (db - d0) / (d1 - d0) * (v1 - v0)).round() as u8;
        }
    }
    127
}

impl Flx4 {
    pub fn new() -> Self {
        let mut flx4 = Self {
            decks: std::array::from_fn(|_| DeckState { beat_jump_scale: 1.0, ..Default::default() }),
            hires: HashMap::new(),
            leds: HashMap::new(),
            pending: Vec::new(),
            fx_switch: [false; 2],
            fx_switch_seen: false,
            fx_notes_down: [false; 128],
            focus: [None; BeatFxKind::ALL.len()],
            knob: None,
            knob_key: KnobKey::Level,
            knob_engaged: true,
            gate: None,
            press_order: 0,
            cfx: [None; DECK_COUNT],
            cfx_armed: [true; DECK_COUNT],
            hud: None,
            reported: None,
            last_keepalive: None,
            keepalive: true,
            vinyl_mode: true,
            software_monitor_knobs: false,
        };
        flx4.reported = Some(flx4.fx_control(&EngineSnapshot::default()));
        flx4
    }

    pub fn pad_mode(&self, deck: usize) -> PadMode {
        self.decks[deck].pad_mode
    }

    fn hires(&mut self, status: u8, cc_num: u8, value: u8) -> Option<(u8, f64)> {
        let (msb_cc, is_lsb) = if (cc::LSB_OFFSET..0x40).contains(&cc_num) {
            (cc_num - cc::LSB_OFFSET, true)
        } else {
            (cc_num, false)
        };
        let entry = self.hires.entry((status, msb_cc)).or_default();
        if is_lsb {
            entry.lsb = value;
        } else {
            entry.msb = value;
            entry.lsb = 0;
        }
        Some((msb_cc, entry.value()))
    }

    fn deck_note(&mut self, deck: usize, n: u8, pressed: bool, snap: &EngineSnapshot) -> Vec<Action> {
        let e = |c: Command| vec![Action::Engine(c)];
        let ds = &snap.decks[deck];
        match (n, pressed) {
            (note::SHIFT, p) => {
                self.decks[deck].shift = p;
                vec![]
            }
            (note::PLAY, true) => e(Command::TogglePlay { deck }),
            (note::CUE, true) => e(Command::CueDown { deck }),
            (note::CUE, false) => e(Command::CueUp { deck }),
            (note::CUE_SHIFT, true) => e(Command::SeekSecs { deck, secs: 0.0 }),
            (note::SYNC, true) => e(Command::Sync { deck, enabled: !ds.sync_enabled }),
            (note::SYNC_SHIFT, true) => {
                let next = ds.tempo_range.next();
                self.decks[deck].tempo_engaged = false;
                e(Command::SetTempoRange { deck, range: next })
            }
            (note::SYNC_LONG, true) => e(Command::SetMasterDeck { deck }),
            (note::LOOP_IN, true) => e(Command::LoopIn { deck }),
            (note::LOOP_OUT, true) => e(Command::LoopOut { deck }),
            (note::LOOP_IN_SHIFT, true) => e(Command::LoopScale { deck, factor: 0.5 }),
            (note::LOOP_OUT_SHIFT, true) => e(Command::LoopScale { deck, factor: 2.0 }),
            (note::BEAT4, true) => {
                if ds.loop_active {
                    e(Command::ReloopExit { deck })
                } else {
                    e(Command::BeatLoop { deck, beats: 4.0 })
                }
            }
            (note::BEAT4_SHIFT, true) => e(Command::ReloopExit { deck }),
            (note::CALL_LEFT, true) if ds.loop_active => e(Command::LoopScale { deck, factor: 0.5 }),
            (note::CALL_RIGHT, true) if ds.loop_active => e(Command::LoopScale { deck, factor: 2.0 }),
            (note::CALL_LEFT, true) => e(Command::BeatJump { deck, beats: -CALL_JUMP_BEATS }),
            (note::CALL_RIGHT, true) => e(Command::BeatJump { deck, beats: CALL_JUMP_BEATS }),
            (note::CALL_LEFT_SHIFT, true) => e(Command::BeatJump { deck, beats: -16.0 }),
            (note::CALL_RIGHT_SHIFT, true) => e(Command::BeatJump { deck, beats: 16.0 }),
            (note::PFL, true) => e(Command::Mixer { channel: deck, param: MixerParam::Pfl(!snap.channels[deck].pfl) }),
            (note::JOG_TOUCH | note::JOG_TOUCH_SHIFT, p) => {
                self.decks[deck].jog_touched = p;
                e(Command::JogTouch { deck, touched: p })
            }
            (note::FADER_START_PLAY, true) => e(Command::Play { deck }),
            (note::FADER_START_CUE, true) => {
                vec![Action::Engine(Command::CueDown { deck }), Action::Engine(Command::CueUp { deck })]
            }
            (n, true) => match PadMode::from_button(n) {
                Some(mode) => self.select_pad_mode(deck, mode, snap),
                None => vec![],
            },
            _ => vec![],
        }
    }

    /// The host owns the pad mode: lighting the mode button is what switches the pads, so the LED goes out at once.
    fn select_pad_mode(&mut self, deck: usize, mode: PadMode, snap: &EngineSnapshot) -> Vec<Action> {
        let actions = self.release_pad_fx(deck, snap);
        let state = &mut self.decks[deck];
        state.pad_mode = mode;
        state.mode_from_host = true;
        let mut out = std::mem::take(&mut self.pending);
        for (button, m) in PadMode::BUTTONS {
            self.set_led(&mut out, NOTE_ON | deck as u8, button, if m == mode { ON } else { 0 });
        }
        self.pending = out;
        self.gesture(FxHudControl::PadMode, Some(deck));
        actions
    }

    fn release_pad_fx(&mut self, deck: usize, snap: &EngineSnapshot) -> Vec<Action> {
        let mut actions = Vec::new();
        let had_voice = self.decks[deck].latest_voice().is_some();
        self.decks[deck].held = [None; 8];
        if had_voice {
            actions.push(Action::Engine(Command::PadFx { deck, voice: None }));
        }
        if self.gate.is_some_and(|(d, _)| d == deck) {
            self.gate = None;
            if snap.beat_fx.enabled {
                actions.push(Action::Engine(Command::BeatFxEnabled(false)));
            }
        }
        actions
    }

    fn pad(&mut self, deck: usize, shift: bool, n: u8, pressed: bool, snap: &EngineSnapshot) -> Vec<Action> {
        let mode = PadMode::from_pad_note(n);
        let pad = (n & 0x0F) as usize;
        if pad >= 8 {
            return vec![];
        }
        let state = &mut self.decks[deck];
        let bit = 1u8 << pad;
        if !state.mode_from_host {
            state.pad_mode = mode;
        } else if pressed && mode != state.pad_mode {
            state.stale_presses |= bit;
            return vec![];
        } else if !pressed && mode != state.pad_mode && state.stale_presses & bit != 0 {
            state.stale_presses &= !bit;
            return vec![];
        }
        let e = |c: Command| vec![Action::Engine(c)];
        match mode {
            PadMode::HotCue if shift => {
                if pressed && pad < HOT_CUE_SLOTS {
                    e(Command::HotCueDelete { deck, slot: pad })
                } else {
                    vec![]
                }
            }
            PadMode::HotCue => e(Command::HotCue { deck, slot: pad, pressed }),
            PadMode::BeatJump if pressed => {
                if shift && pad >= 6 {
                    let s = &mut self.decks[deck].beat_jump_scale;
                    *s = if pad == 6 { (*s * 0.5).max(0.125) } else { (*s * 2.0).min(16.0) };
                    vec![]
                } else {
                    let beats = BEAT_JUMP_PADS[pad] * self.decks[deck].beat_jump_scale;
                    e(Command::BeatJump { deck, beats })
                }
            }
            PadMode::BeatLoop if pressed => e(Command::BeatLoop { deck, beats: BEAT_LOOP_PADS[pad] }),
            PadMode::PadFx1 if pressed => self.pad_fx1_press(deck, pad, shift),
            PadMode::PadFx2 if pressed => self.pad_fx2_press(deck, pad, shift, snap),
            PadMode::PadFx1 | PadMode::PadFx2 => self.pad_fx_release(deck, pad),
            _ => vec![],
        }
    }

    fn next_order(&mut self) -> u64 {
        self.press_order += 1;
        self.press_order
    }

    fn pad_fx1_press(&mut self, deck: usize, pad: usize, shift: bool) -> Vec<Action> {
        let bank = if shift { &PAD_FX1_SHIFT_BANK } else { &PAD_FX1_BANK };
        let order = self.next_order();
        self.gesture(FxHudControl::PadFx, Some(deck));
        match bank[pad].action {
            PadFxAction::Momentary { voice } => {
                self.decks[deck].held[pad] = Some(HeldFx { shift, mode: PadMode::PadFx1, voice: Some(voice), order });
                vec![Action::Engine(Command::PadFx { deck, voice: Some(voice) })]
            }
            PadFxAction::OneShot { release, beats } => {
                for h in self.decks[deck].held.iter_mut().flatten() {
                    h.voice = None;
                }
                self.decks[deck].held[pad] = Some(HeldFx { shift, mode: PadMode::PadFx1, voice: None, order });
                vec![Action::Engine(Command::PadFxRelease { deck, kind: release, beats })]
            }
        }
    }

    fn pad_fx2_press(&mut self, deck: usize, pad: usize, shift: bool, snap: &EngineSnapshot) -> Vec<Action> {
        let order = self.next_order();
        self.decks[deck].held[pad] = Some(HeldFx { shift, mode: PadMode::PadFx2, voice: None, order });
        if shift {
            self.gesture(FxHudControl::Kind, Some(deck));
            return vec![Action::Engine(Command::BeatFxKind(PAD_FX2_FAVOURITES[pad]))];
        }
        self.gesture(FxHudControl::Beats, Some(deck));
        let fx = &snap.beat_fx;
        let mut actions = vec![Action::Engine(if fx.kind.uses_percent() {
            Command::BeatFxPercent(PAD_FX2_PERCENTS[pad])
        } else {
            Command::BeatFxBeats(PAD_FX2_BEATS[pad])
        })];
        if self.gate.is_some() {
            self.gate = Some((deck, pad));
        } else if !fx.enabled {
            self.gate = Some((deck, pad));
            actions.push(Action::Engine(Command::BeatFxEnabled(true)));
        }
        actions
    }

    /// Releases are matched by deck and pad, whatever the SHIFT layer is now.
    fn pad_fx_release(&mut self, deck: usize, pad: usize) -> Vec<Action> {
        let Some(held) = self.decks[deck].held[pad].take() else { return vec![] };
        if self.gate == Some((deck, pad)) {
            self.gate = None;
            return vec![Action::Engine(Command::BeatFxEnabled(false))];
        }
        if held.voice.is_none() {
            return vec![];
        }
        let next = self.decks[deck].latest_voice().and_then(|(_, h)| h.voice);
        vec![Action::Engine(Command::PadFx { deck, voice: next })]
    }

    fn deck_cc(&mut self, deck: usize, status: u8, num: u8, value: u8, snap: &EngineSnapshot) -> Vec<Action> {
        let jog = |mode: JogMode| {
            let ticks = value as i32 - 0x40;
            vec![Action::Engine(Command::Jog { deck, ticks, mode })]
        };
        match num {
            cc::JOG_VINYL if self.decks[deck].jog_touched => jog(JogMode::Scratch),
            cc::JOG_VINYL | cc::JOG_PLATTER | cc::JOG_RING => jog(JogMode::Bend),
            cc::JOG_SHIFT => jog(JogMode::FastSearch),
            _ => {
                let Some((msb, v)) = self.hires(status, num, value) else { return vec![] };
                let mixer = |param| vec![Action::Engine(Command::Mixer { channel: deck, param })];
                match msb {
                    cc::TEMPO => self.tempo(deck, v, snap),
                    cc::FADER => mixer(MixerParam::Fader(v as f32)),
                    cc::TRIM => mixer(MixerParam::Trim(v as f32)),
                    cc::EQ_HIGH => mixer(MixerParam::Eq(Band::High, v as f32)),
                    cc::EQ_MID => mixer(MixerParam::Eq(Band::Mid, v as f32)),
                    cc::EQ_LOW => mixer(MixerParam::Eq(Band::Low, v as f32)),
                    _ => vec![],
                }
            }
        }
    }

    /// Tempo fader with soft takeover: ignored until it meets the engine's current position.
    fn tempo(&mut self, deck: usize, raw: f64, snap: &EngineSnapshot) -> Vec<Action> {
        let position = (raw - 0.5) * 2.0;
        let ds = &snap.decks[deck];
        if ds.sync_enabled {
            self.decks[deck].tempo_engaged = false;
            return vec![];
        }
        let range = ds.tempo_range.fraction();
        let current = ((ds.fader_tempo_ratio - 1.0) / range).clamp(-1.0, 1.0);
        let state = &mut self.decks[deck];
        if !state.tempo_engaged {
            if (position - current).abs() <= TAKEOVER_THRESHOLD || ds.fader_tempo_ratio == 1.0 && !ds.loaded {
                state.tempo_engaged = true;
            } else {
                return vec![];
            }
        }
        vec![Action::Engine(Command::SetPitchFader { deck, position })]
    }

    fn global_cc(&mut self, status: u8, num: u8, value: u8) -> Vec<Action> {
        match num {
            cc::BROWSE | cc::BROWSE_SHIFT => {
                let delta = if value < 0x40 { value as i32 } else { value as i32 - 0x80 };
                let delta = if num == cc::BROWSE_SHIFT { delta * 10 } else { delta };
                return vec![Action::App(AppAction::BrowseScroll { delta })];
            }
            _ => {}
        }
        let Some((msb, v)) = self.hires(status, num, value) else { return vec![] };
        let v = v as f32;
        let engine = |c| vec![Action::Engine(c)];
        let monitor = |name: &'static str, cmd: Command, software: bool| {
            if software {
                vec![Action::Engine(cmd), Action::App(AppAction::HardwareKnob { name, value: v })]
            } else {
                vec![Action::App(AppAction::HardwareKnob { name, value: v })]
            }
        };
        let sw = self.software_monitor_knobs;
        match msb {
            cc::CROSSFADER => engine(Command::Crossfader(v)),
            cc::CFX_1 => self.cfx_knob(0, v as f64),
            cc::CFX_2 => self.cfx_knob(1, v as f64),
            cc::MASTER_LEVEL => monitor("masterLevel", Command::MasterLevel(v), sw),
            cc::PHONES_MIX => monitor("headphoneMix", Command::HeadphoneMix(v), sw),
            cc::PHONES_LEVEL => monitor("headphoneLevel", Command::HeadphoneLevel(v), sw),
            _ => vec![],
        }
    }

    fn global_note(&mut self, n: u8, pressed: bool, snap: &EngineSnapshot) -> Vec<Action> {
        if !pressed {
            return vec![];
        }
        match n {
            note::SMART_CFX => self.smart_cfx(false, snap),
            note::SMART_CFX_SHIFT => self.smart_cfx(true, snap),
            note::BROWSE_PUSH => vec![Action::App(AppAction::BrowseToggleFocus)],
            note::LOAD_1 => vec![Action::App(AppAction::LoadSelected { deck: 0 })],
            note::LOAD_2 => vec![Action::App(AppAction::LoadSelected { deck: 1 })],
            _ => vec![],
        }
    }

    /// SHIFTed FX buttons arrive on both `0x94` and `0x95` (verified); a note already down is the twin and is dropped.
    fn fx_note(&mut self, channel: u8, n: u8, pressed: bool, snap: &EngineSnapshot) -> Vec<Action> {
        match n {
            note::FX_CH_SELECT_1 if channel == FX_CH_1 => return self.fx_switch_changed(0, pressed),
            note::FX_CH_SELECT_2 if channel == FX_CH_2 => return self.fx_switch_changed(1, pressed),
            note::FX_CH_SELECT_1 | note::FX_CH_SELECT_2 => return vec![],
            _ => {}
        }
        let down = &mut self.fx_notes_down[(n & 0x7F) as usize];
        if !pressed {
            *down = false;
            return vec![];
        }
        if std::mem::replace(down, true) {
            return vec![];
        }
        let e = |c: Command| vec![Action::Engine(c)];
        let fx = &snap.beat_fx;
        match n {
            note::FX_SELECT => {
                self.gesture(FxHudControl::Kind, None);
                e(Command::BeatFxKind(fx.kind.next()))
            }
            note::FX_SELECT_SHIFT => {
                self.gesture(FxHudControl::Kind, None);
                e(Command::BeatFxKind(fx.kind.prev()))
            }
            note::FX_BEAT_LEFT | note::FX_BEAT_RIGHT => self.fx_beat(n == note::FX_BEAT_RIGHT, snap),
            note::FX_BEAT_LEFT_SHIFT | note::FX_BEAT_RIGHT_SHIFT => {
                self.page_focus(fx.kind, n == note::FX_BEAT_RIGHT_SHIFT);
                self.gesture(FxHudControl::Page, None);
                vec![]
            }
            note::FX_ON | note::FX_ON_SHIFT => {
                let mut actions = Vec::new();
                if !self.fx_switch_seen && channel == FX_CH_2 {
                    actions.push(Action::Engine(Command::BeatFxTarget(FxTarget::B)));
                }
                if n == note::FX_ON {
                    self.gesture(FxHudControl::OnOff, None);
                    actions.push(Action::Engine(Command::BeatFxEnabled(!fx.enabled)));
                } else {
                    self.gesture(FxHudControl::Release, None);
                    self.gate = None;
                    actions.push(Action::Engine(Command::ReleaseFx));
                }
                actions
            }
            _ => vec![],
        }
    }

    fn fx_beat(&mut self, up: bool, snap: &EngineSnapshot) -> Vec<Action> {
        if let Some((deck, pad, held)) = self.latest_pad_voice() {
            self.gesture(FxHudControl::Beats, Some(deck));
            let Some(mut voice) = held.voice else { return vec![] };
            if voice.kind.uses_percent() {
                voice.percent = step_beat_fx_percent(voice.percent, up);
            } else {
                let options = voice.kind.beat_options();
                let at = snap_beat_fx_beats_for(voice.kind, voice.beats).and_then(|b| options.iter().position(|o| *o == b)).unwrap_or(0);
                voice.beats = options[if up { (at + 1).min(options.len() - 1) } else { at.saturating_sub(1) }];
            }
            if let Some(h) = self.decks[deck].held[pad].as_mut() {
                h.voice = Some(voice);
            }
            return vec![Action::Engine(Command::PadFx { deck, voice: Some(voice) })];
        }
        self.gesture(FxHudControl::Beats, None);
        let e = |c: Command| vec![Action::Engine(c)];
        let fx = &snap.beat_fx;
        if fx.kind.uses_percent() {
            return e(Command::BeatFxPercent(step_beat_fx_percent(fx.percent, up)));
        }
        if let Some((index, spec)) = fx.kind.primary_param().and_then(|i| beat_fx_params(fx.kind).get(i).map(|spec| (i, spec))) {
            let count = spec.options.len().max(1);
            let at = fx.params[index].round().max(0.0) as usize;
            let next = if up { (at + 1).min(count - 1) } else { at.saturating_sub(1) };
            return e(Command::BeatFxParam { index, value: next as f32 });
        }
        let options = fx.kind.beat_options();
        let beats = snap_beat_fx_beats_for(fx.kind, fx.beats).unwrap_or(1.0);
        let at = options.iter().position(|b| *b == beats).unwrap_or(0);
        e(Command::BeatFxBeats(options[if up { (at + 1).min(options.len() - 1) } else { at.saturating_sub(1) }]))
    }

    fn latest_pad_voice(&self) -> Option<(usize, usize, HeldFx)> {
        (0..DECK_COUNT).filter_map(|d| self.decks[d].latest_voice().map(|(pad, h)| (d, pad, h))).max_by_key(|(_, _, h)| h.order)
    }

    /// The parameter SHIFT + LEVEL/DEPTH drives for a kind: the last paged-to one, else its first continuous parameter.
    pub fn focus(&self, kind: BeatFxKind) -> Option<usize> {
        self.focus[kind.slot()].or_else(|| beat_fx_knob_params(kind).next())
    }

    fn page_focus(&mut self, kind: BeatFxKind, forward: bool) {
        let pages: Vec<usize> = beat_fx_knob_params(kind).collect();
        let Some(current) = self.focus(kind) else { return };
        let at = pages.iter().position(|p| *p == current).unwrap_or(0);
        let next = if forward { (at + 1) % pages.len() } else { (at + pages.len() - 1) % pages.len() };
        self.focus[kind.slot()] = Some(pages[next]);
    }

    fn shift_held(&self) -> bool {
        self.decks.iter().any(|d| d.shift)
    }

    fn current_knob_key(&self, snap: &EngineSnapshot) -> KnobKey {
        if let Some((deck, _, _)) = self.latest_pad_voice() {
            KnobKey::Pad(deck)
        } else if self.shift_held() {
            KnobKey::Param(snap.beat_fx.kind, self.focus(snap.beat_fx.kind))
        } else {
            KnobKey::Level
        }
    }

    fn knob_stored(&self, key: KnobKey, snap: &EngineSnapshot) -> Option<f64> {
        match key {
            KnobKey::Level => Some(snap.beat_fx.level as f64),
            KnobKey::Param(kind, index) => {
                let index = index?;
                let spec = beat_fx_params(kind).get(index)?;
                Some(((snap.beat_fx.params[index] - spec.min) / (spec.max - spec.min)).clamp(0.0, 1.0) as f64)
            }
            KnobKey::Pad(deck) => self.decks[deck].latest_voice().and_then(|(_, h)| h.voice).map(|v| v.level as f64),
        }
    }

    /// LEVEL/DEPTH with soft pickup: after its target changes the knob does nothing until it meets the stored value.
    fn fx_level(&mut self, raw: f64, snap: &EngineSnapshot) -> Vec<Action> {
        let key = self.current_knob_key(snap);
        let previous = self.knob.replace(raw);
        if key != self.knob_key {
            self.knob_key = key;
            self.knob_engaged = false;
        }
        self.gesture(
            match key {
                KnobKey::Level => FxHudControl::Level,
                KnobKey::Param(..) => FxHudControl::Param,
                KnobKey::Pad(_) => FxHudControl::PadFx,
            },
            match key {
                KnobKey::Pad(deck) => Some(deck),
                _ => None,
            },
        );
        let Some(stored) = self.knob_stored(key, snap) else { return vec![] };
        if !self.knob_engaged {
            let crossed = previous.is_some_and(|p| (p - stored) * (raw - stored) <= 0.0);
            if (raw - stored).abs() <= TAKEOVER_THRESHOLD || crossed {
                self.knob_engaged = true;
            } else {
                return vec![];
            }
        }
        let e = |c: Command| vec![Action::Engine(c)];
        match key {
            KnobKey::Level => e(Command::BeatFxLevel(raw as f32)),
            KnobKey::Param(kind, Some(index)) => {
                let spec = beat_fx_params(kind)[index];
                e(Command::BeatFxParam { index, value: spec.min + raw as f32 * (spec.max - spec.min) })
            }
            KnobKey::Param(_, None) => vec![],
            KnobKey::Pad(deck) => {
                let Some((pad, held)) = self.decks[deck].latest_voice() else { return vec![] };
                let Some(mut voice) = held.voice else { return vec![] };
                voice.level = raw as f32;
                if let Some(h) = self.decks[deck].held[pad].as_mut() {
                    h.voice = Some(voice);
                }
                e(Command::PadFx { deck, voice: Some(voice) })
            }
        }
    }

    fn gesture(&mut self, control: FxHudControl, deck: Option<usize>) {
        let seq = self.hud.map_or(1, |h| h.seq + 1);
        self.hud = Some(FxHud { seq, control, deck });
    }

    pub fn fx_control(&self, snap: &EngineSnapshot) -> FxControlState {
        let key = self.current_knob_key(snap);
        let engaged = key == self.knob_key && self.knob_engaged;
        let stored = self.knob_stored(key, snap);
        let pickup = match (engaged, self.knob, stored) {
            (false, Some(knob), Some(value)) if (knob - value).abs() > TAKEOVER_THRESHOLD => Some(FxPickup {
                target: match key {
                    KnobKey::Level => FxKnobTarget::Level,
                    KnobKey::Param(..) => FxKnobTarget::Param,
                    KnobKey::Pad(_) => FxKnobTarget::PadLevel,
                },
                deck: match key {
                    KnobKey::Pad(d) => Some(d),
                    _ => None,
                },
                index: match key {
                    KnobKey::Param(_, i) => i,
                    _ => None,
                },
                value: value as f32,
                knob: knob as f32,
            }),
            _ => None,
        };
        FxControlState {
            shift: self.shift_held(),
            knob_target: match key {
                KnobKey::Level => FxKnobTarget::Level,
                KnobKey::Param(..) => FxKnobTarget::Param,
                KnobKey::Pad(_) => FxKnobTarget::PadLevel,
            },
            focus: self.focus(snap.beat_fx.kind),
            pickup,
            pad_modes: [self.decks[0].pad_mode, self.decks[1].pad_mode],
            held: std::array::from_fn(|d| self.decks[d].latest_held().map(|(pad, h)| HeldPad { pad: pad as u8, shift: h.shift, mode: h.mode })),
            smart_arming: [!self.cfx_armed[0], !self.cfx_armed[1]],
            hud: self.hud,
        }
    }

    fn smart_cfx(&mut self, preset: bool, snap: &EngineSnapshot) -> Vec<Action> {
        self.cfx_armed = std::array::from_fn(|ch| self.cfx[ch].is_some_and(|v| (v - 0.5).abs() * 2.0 <= CFX_CENTRE));
        if preset {
            self.gesture(FxHudControl::SmartPreset, None);
            vec![Action::Engine(Command::SmartCfxPreset(snap.smart_cfx.preset.next()))]
        } else {
            self.gesture(FxHudControl::SmartCfx, None);
            vec![Action::Engine(Command::SmartCfx(!snap.smart_cfx.enabled))]
        }
    }

    /// CFX knob; after a Smart CFX switch it is ignored until it passes the centre so the sound does not lurch.
    fn cfx_knob(&mut self, ch: usize, v: f64) -> Vec<Action> {
        let b = (v - 0.5) * 2.0;
        let previous = self.cfx[ch].replace(v).map(|p| (p - 0.5) * 2.0);
        if !self.cfx_armed[ch] {
            let crossed = previous.is_some_and(|p| p * b <= 0.0);
            if b.abs() <= CFX_CENTRE || crossed {
                self.cfx_armed[ch] = true;
            } else {
                return vec![];
            }
        }
        vec![Action::Engine(Command::Mixer { channel: ch, param: MixerParam::Filter(b as f32) })]
    }

    fn fx_switch_changed(&mut self, side: usize, on: bool) -> Vec<Action> {
        self.fx_switch[side] = on;
        self.fx_switch_seen = true;
        let target = match self.fx_switch {
            [true, true] => FxTarget::Master,
            [true, false] => FxTarget::A,
            [false, true] => FxTarget::B,
            [false, false] => return vec![],
        };
        vec![Action::Engine(Command::BeatFxTarget(target))]
    }

    fn set_led(&mut self, out: &mut Vec<MidiOut>, status: u8, data1: u8, value: u8) {
        if self.leds.get(&(status, data1)) != Some(&value) {
            self.leds.insert((status, data1), value);
            out.push(vec![status, data1, value]);
        }
    }

    fn deck_led(&mut self, out: &mut Vec<MidiOut>, deck: usize, n: u8, on: bool) {
        let status = NOTE_ON | deck as u8;
        let v = if on { ON } else { 0 };
        self.set_led(out, status, n, v);
        let s = shifted(n);
        if s != n {
            self.set_led(out, status, s, v);
        }
    }
}

impl ControllerMapping for Flx4 {
    fn name(&self) -> &'static str {
        "Pioneer DJ DDJ-FLX4"
    }

    fn matches_port(&self, port_name: &str) -> bool {
        port_name.to_ascii_uppercase().contains("FLX4")
    }

    fn on_connect(&mut self) -> Vec<MidiOut> {
        self.leds.clear();
        self.hires.clear();
        self.fx_switch = [false; 2];
        self.fx_switch_seen = false;
        self.fx_notes_down = [false; 128];
        self.pending.clear();
        self.knob = None;
        self.knob_key = KnobKey::Level;
        self.knob_engaged = true;
        self.gate = None;
        self.cfx = [None; DECK_COUNT];
        self.cfx_armed = [true; DECK_COUNT];
        for d in &mut self.decks {
            d.tempo_engaged = false;
            d.last_loaded_token = 0;
            d.mode_from_host = false;
            d.stale_presses = 0;
            d.held = [None; 8];
            d.shift = false;
        }
        self.last_keepalive = None;
        let vinyl = if self.vinyl_mode { ON } else { 0 };
        let mut out = vec![vec![NOTE_ON, note::VINYL, vinyl], vec![NOTE_ON | 1, note::VINYL, vinyl]];
        if self.keepalive {
            out.push(KEEPALIVE_SYSEX.to_vec());
        }
        out
    }

    fn on_disconnect(&mut self) -> Vec<MidiOut> {
        let mut out: Vec<MidiOut> = self
            .leds
            .iter()
            .filter(|&(_, &v)| v != 0)
            .map(|(&(s, d), _)| vec![s, d, 0])
            .collect();
        out.sort();
        self.leds.clear();
        out
    }

    fn on_input(&mut self, message: &[u8], snapshot: &EngineSnapshot) -> Vec<Action> {
        let mut actions = self.input(message, snapshot);
        let state = self.fx_control(snapshot);
        if self.reported.replace(state) != Some(state) {
            actions.push(Action::App(AppAction::FxControl(state)));
        }
        actions
    }

    fn drain_output(&mut self) -> Vec<MidiOut> {
        std::mem::take(&mut self.pending)
    }

    fn feedback(&mut self, snap: &EngineSnapshot, now: Instant) -> Vec<MidiOut> {
        let mut out = std::mem::take(&mut self.pending);
        self.led_feedback(&mut out, snap, now);
        out
    }

    fn tick(&mut self, now: Instant) -> Vec<MidiOut> {
        if !self.keepalive {
            return vec![];
        }
        match self.last_keepalive {
            Some(t) if now.duration_since(t) < KEEPALIVE_INTERVAL => vec![],
            _ => {
                self.last_keepalive = Some(now);
                vec![KEEPALIVE_SYSEX.to_vec()]
            }
        }
    }
}

impl Flx4 {
    fn input(&mut self, message: &[u8], snapshot: &EngineSnapshot) -> Vec<Action> {
        let [status, data1, data2] = match message {
            [a, b, c] => [*a, *b, *c],
            _ => return vec![],
        };
        let kind = status & 0xF0;
        let channel = status & 0x0F;
        let note_kind = kind == NOTE_ON || kind == 0x80;
        let pressed = kind == NOTE_ON && data2 > 0;
        match (note_kind, channel) {
            (true, 0 | 1) => self.deck_note(channel as usize, data1, pressed, snapshot),
            (true, 7..=10) => {
                let deck = if channel <= 8 { 0 } else { 1 };
                let shift = channel == 8 || channel == 10;
                self.pad(deck, shift, data1, pressed, snapshot)
            }
            (true, GLOBAL_CH) => self.global_note(data1, pressed, snapshot),
            (true, FX_CH_1 | FX_CH_2) => self.fx_note(channel, data1, pressed, snapshot),
            (false, FX_CH_1) if kind == CC && matches!(data1, cc::FX_LEVEL | cc::FX_LEVEL_LSB) => {
                let Some((_, v)) = self.hires(status, data1, data2) else { return vec![] };
                self.fx_level(v, snapshot)
            }
            (false, 0 | 1) if kind == CC => self.deck_cc(channel as usize, status, data1, data2, snapshot),
            (false, GLOBAL_CH) if kind == CC => self.global_cc(status, data1, data2),
            _ => vec![],
        }
    }

    /// FX ON/OFF: per the official list's footnote, NOTE ON blinks and NOTE OFF lights steadily (needs a hardware look).
    fn led_feedback(&mut self, out: &mut Vec<MidiOut>, snap: &EngineSnapshot, now: Instant) {
        let fx_on = if snap.beat_fx.enabled { ON } else { 0 };
        for status in [NOTE_ON | FX_CH_1, NOTE_ON | FX_CH_2] {
            self.set_led(out, status, note::FX_ON, fx_on);
            self.set_led(out, status, note::FX_ON_SHIFT, fx_on);
        }
        let ms = instant_ms(now);
        let blink = (ms / 500).is_multiple_of(2);
        let fast = (ms / PAD_BLINK_MS).is_multiple_of(2);
        let arming = self.cfx_armed.iter().any(|a| !a);
        let smart = snap.smart_cfx.enabled && (!arming || (ms / SLOW_BLINK_MS).is_multiple_of(2));
        self.set_led(out, NOTE_ON | GLOBAL_CH, note::SMART_CFX, if smart { ON } else { 0 });
        for (deck, pad_status) in PAD_STATUS.iter().enumerate() {
            let d = snap.decks[deck];
            let play_on = d.playing && !d.previewing || (d.loaded && !d.playing && blink);
            let at_cue = (d.position_secs - d.cue_secs).abs() < 0.02;
            let cue_on = d.previewing || (d.loaded && !d.playing && (at_cue || blink));
            self.deck_led(out, deck, note::PLAY, play_on);
            self.deck_led(out, deck, note::CUE, cue_on);
            self.deck_led(out, deck, note::SYNC, d.sync_enabled);
            self.deck_led(out, deck, note::LOOP_IN, d.loop_active);
            self.deck_led(out, deck, note::LOOP_OUT, d.loop_active);
            self.deck_led(out, deck, note::BEAT4, d.loop_active);
            self.deck_led(out, deck, note::PFL, snap.channels[deck].pfl);

            let mode = self.decks[deck].pad_mode;
            for (button, m) in PadMode::BUTTONS {
                self.set_led(out, NOTE_ON | deck as u8, button, if m == mode { ON } else { 0 });
            }
            let base = mode.pad_base();
            let beat_secs = if d.grid_bpm > 0.0 { 60.0 / d.grid_bpm } else { 0.5 };
            let loop_beats = match (d.loop_in_secs, d.loop_out_secs) {
                (Some(a), Some(b)) if d.loop_active => Some((b - a) / beat_secs),
                _ => None,
            };
            for pad in 0..8u8 {
                let lit = |shift: bool| match mode {
                    PadMode::HotCue => d.hot_cues[pad as usize].is_some(),
                    PadMode::BeatJump => d.loaded,
                    PadMode::BeatLoop => loop_beats.is_some_and(|b| (b - BEAT_LOOP_PADS[pad as usize]).abs() < 1e-3),
                    PadMode::PadFx1 => !self.pad_fx1_running(deck, pad as usize, shift, snap) || fast,
                    PadMode::PadFx2 => self.pad_fx2_led(deck, pad as usize, shift, snap, fast),
                    _ => false,
                };
                let (plain, shifted) = (lit(false), lit(true));
                self.set_led(out, pad_status[0], base + pad, if plain { ON } else { 0 });
                self.set_led(out, pad_status[1], base + pad, if shifted { ON } else { 0 });
            }
            let vu = vu_value(snap.channels[deck].meter_l.max(snap.channels[deck].meter_r));
            self.set_led(out, CC | deck as u8, cc::VU, vu);

            if d.loaded && d.track_token != self.decks[deck].last_loaded_token {
                self.decks[deck].last_loaded_token = d.track_token;
                out.push(vec![0x9F, deck as u8, ON]);
            }
        }
    }

    fn pad_fx1_running(&self, deck: usize, pad: usize, shift: bool, snap: &EngineSnapshot) -> bool {
        let bank = if shift { &PAD_FX1_SHIFT_BANK } else { &PAD_FX1_BANK };
        let held = self.decks[deck].held[pad].is_some_and(|h| h.shift == shift && h.mode == PadMode::PadFx1);
        let running = match bank[pad].action {
            PadFxAction::OneShot { release, .. } => snap.pad_fx[deck].release == Some(release),
            PadFxAction::Momentary { voice } => held && snap.pad_fx[deck].voice.is_some_and(|v| v.kind == voice.kind),
        };
        held || running
    }

    fn pad_fx2_led(&self, deck: usize, pad: usize, shift: bool, snap: &EngineSnapshot, fast: bool) -> bool {
        let fx = &snap.beat_fx;
        if shift {
            return PAD_FX2_FAVOURITES[pad] == fx.kind;
        }
        if self.gate == Some((deck, pad)) {
            return fast;
        }
        if fx.kind.uses_percent() {
            let nearest = PAD_FX2_PERCENTS.iter().enumerate().min_by(|a, b| (a.1 - fx.percent).abs().total_cmp(&(b.1 - fx.percent).abs()));
            nearest.is_some_and(|(i, _)| i == pad)
        } else {
            (PAD_FX2_BEATS[pad] - fx.beats).abs() < 1e-9
        }
    }
}

fn instant_ms(now: Instant) -> u128 {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    now.duration_since(*START.get_or_init(Instant::now)).as_millis()
}
