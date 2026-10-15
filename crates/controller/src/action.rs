use deckforge_common::DECK_COUNT;
use deckforge_engine::Command;
use serde::Serialize;

use crate::flx4::PadMode;

/// What the LEVEL/DEPTH knob drives right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FxKnobTarget {
    #[default]
    Level,
    Param,
    PadLevel,
}

/// LEVEL/DEPTH is waiting to catch up with the stored value of its new target.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FxPickup {
    pub target: FxKnobTarget,
    pub deck: Option<usize>,
    pub index: Option<usize>,
    pub value: f32,
    pub knob: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FxHudControl {
    Level,
    Param,
    Page,
    Beats,
    Kind,
    OnOff,
    Release,
    PadFx,
    PadMode,
    SmartCfx,
    SmartPreset,
}

/// The latest FX hardware gesture; `seq` changes on every gesture so the UI can pop its overlay.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FxHud {
    pub seq: u64,
    pub control: FxHudControl,
    pub deck: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeldPad {
    pub pad: u8,
    pub shift: bool,
    pub mode: PadMode,
}

/// Controller-side FX state the hardware cannot show: SHIFT layer, focused parameter, pickup, pad modes and arming.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FxControlState {
    pub shift: bool,
    pub knob_target: FxKnobTarget,
    pub focus: Option<usize>,
    pub pickup: Option<FxPickup>,
    pub pad_modes: [PadMode; DECK_COUNT],
    pub held: [Option<HeldPad>; DECK_COUNT],
    pub smart_arming: [bool; DECK_COUNT],
    pub hud: Option<FxHud>,
}

/// Non-engine actions a controller can request from the application (library browser, UI).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AppAction {
    BrowseScroll { delta: i32 },
    BrowseToggleFocus,
    LoadSelected { deck: usize },
    /// Hardware knobs that act in the controller's own analog stage; surfaced for display only.
    HardwareKnob { name: &'static str, value: f32 },
    FxControl(FxControlState),
}

#[derive(Debug, Clone)]
pub enum Action {
    Engine(Command),
    App(AppAction),
}
