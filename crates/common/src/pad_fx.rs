use serde::{Deserialize, Serialize};

use crate::BeatFxKind;

/// One-shot transition effects: the first three are rekordbox's Release FX types; `RollOut` is a pad-only extra.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReleaseFxKind {
    #[default]
    EchoOut,
    Backspin,
    VinylBrake,
    RollOut,
}

impl ReleaseFxKind {
    /// Types offered for SHIFT + FX ON/OFF, as in rekordbox's Release FX selector.
    pub const SELECTABLE: [ReleaseFxKind; 3] = [ReleaseFxKind::EchoOut, ReleaseFxKind::Backspin, ReleaseFxKind::VinylBrake];
}

/// Release lengths in beats.
pub const RELEASE_FX_BEATS: [f64; 5] = [0.5, 1.0, 2.0, 4.0, 8.0];
pub const RELEASE_FX_DEFAULT_BEATS: f64 = 2.0;

pub fn snap_release_fx_beats(beats: f64) -> Option<f64> {
    if !beats.is_finite() || beats <= 0.0 {
        return None;
    }
    RELEASE_FX_BEATS.iter().copied().min_by(|a, b| (a / beats).ln().abs().total_cmp(&(b / beats).ln().abs()))
}

/// A held Pad FX effect: a Beat FX kind with its own snapshot of settings, running on the pad's deck.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PadVoice {
    pub kind: BeatFxKind,
    pub beats: f64,
    pub percent: f64,
    pub level: f32,
    /// First sub-parameter of the kind (feedback for the echo family); `None` keeps the kind's default.
    pub param0: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PadFxAction {
    Momentary { voice: PadVoice },
    OneShot { release: ReleaseFxKind, beats: f64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PadFxSlot {
    pub label: &'static str,
    pub action: PadFxAction,
}

const fn hold(label: &'static str, kind: BeatFxKind, beats: f64, percent: f64, level: f32, param0: Option<f32>) -> PadFxSlot {
    PadFxSlot { label, action: PadFxAction::Momentary { voice: PadVoice { kind, beats, percent, level, param0 } } }
}

const fn shot(label: &'static str, release: ReleaseFxKind, beats: f64) -> PadFxSlot {
    PadFxSlot { label, action: PadFxAction::OneShot { release, beats } }
}

/// PAD FX 1, pads 1-8: four held effects on the top row, four one-shot transitions on the bottom row.
pub const PAD_FX1_BANK: [PadFxSlot; 8] = [
    hold("Echo", BeatFxKind::Echo, 0.5, 50.0, 0.7, Some(55.0)),
    hold("Ping Pong", BeatFxKind::PingPong, 0.25, 50.0, 0.7, Some(50.0)),
    hold("Reverb", BeatFxKind::Reverb, 1.0, 60.0, 0.6, None),
    hold("Roll", BeatFxKind::Roll, 0.5, 50.0, 1.0, None),
    shot("Echo Out", ReleaseFxKind::EchoOut, 2.0),
    shot("Backspin", ReleaseFxKind::Backspin, 1.0),
    shot("Vinyl Brake", ReleaseFxKind::VinylBrake, 1.0),
    shot("Roll Out", ReleaseFxKind::RollOut, 2.0),
];

/// PAD FX 1 with SHIFT held: the remaining Beat FX kinds, all held.
pub const PAD_FX1_SHIFT_BANK: [PadFxSlot; 8] = [
    hold("Flanger", BeatFxKind::Flanger, 4.0, 50.0, 0.8, None),
    hold("Phaser", BeatFxKind::Phaser, 4.0, 50.0, 0.8, None),
    hold("Trans", BeatFxKind::Trans, 0.25, 50.0, 1.0, None),
    hold("Filter Sweep", BeatFxKind::FilterSweep, 4.0, 50.0, 0.8, None),
    hold("Down Echo", BeatFxKind::DownEcho, 1.0, 50.0, 0.7, None),
    hold("Up Echo", BeatFxKind::UpEcho, 1.0, 50.0, 0.7, None),
    hold("Spiral", BeatFxKind::Spiral, 1.0, 50.0, 0.7, None),
    hold("Delay", BeatFxKind::Delay, 0.75, 50.0, 0.7, None),
];

/// PAD FX 2 pads set the Beat FX unit's division (or, for percent kinds, its amount) and gate it while held.
pub const PAD_FX2_BEATS: [f64; 8] = [0.125, 0.25, 0.5, 0.75, 1.0, 2.0, 4.0, 8.0];
pub const PAD_FX2_PERCENTS: [f64; 8] = [10.0, 20.0, 35.0, 50.0, 65.0, 80.0, 90.0, 100.0];

/// PAD FX 2 with SHIFT held: favourite Beat FX kinds, one press selects.
pub const PAD_FX2_FAVOURITES: [BeatFxKind; 8] = [
    BeatFxKind::Echo,
    BeatFxKind::PingPong,
    BeatFxKind::Reverb,
    BeatFxKind::Roll,
    BeatFxKind::Flanger,
    BeatFxKind::Phaser,
    BeatFxKind::Trans,
    BeatFxKind::FilterSweep,
];

/// One-knob Smart CFX macros driven by each channel's CFX knob.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SmartCfxPreset {
    #[default]
    EchoWash,
    Riser,
    Space,
    CrushChop,
}

impl SmartCfxPreset {
    pub const ALL: [SmartCfxPreset; 4] = [SmartCfxPreset::EchoWash, SmartCfxPreset::Riser, SmartCfxPreset::Space, SmartCfxPreset::CrushChop];

    pub fn next(self) -> Self {
        let i = Self::ALL.iter().position(|p| *p == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_beat_fx_kind_with_a_simple_hold_is_reachable_from_pad_fx1() {
        let kinds: Vec<BeatFxKind> = PAD_FX1_BANK
            .iter()
            .chain(PAD_FX1_SHIFT_BANK.iter())
            .filter_map(|s| match s.action {
                PadFxAction::Momentary { voice } => Some(voice.kind),
                PadFxAction::OneShot { .. } => None,
            })
            .collect();
        for kind in BeatFxKind::ALL.iter().take(12) {
            assert!(kinds.contains(kind), "{kind:?}");
        }
    }

    #[test]
    fn release_beats_snap_and_presets_cycle() {
        assert_eq!(snap_release_fx_beats(3.0), Some(4.0));
        assert_eq!(snap_release_fx_beats(0.1), Some(0.5));
        assert_eq!(snap_release_fx_beats(f64::NAN), None);
        assert_eq!(SmartCfxPreset::CrushChop.next(), SmartCfxPreset::EchoWash);
    }
}
