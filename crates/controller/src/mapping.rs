use std::time::Instant;

use deckforge_engine::EngineSnapshot;

use crate::action::Action;

pub type MidiOut = Vec<u8>;

/// A device-specific translation between raw MIDI and engine/app actions, plus LED feedback.
pub trait ControllerMapping: Send {
    fn name(&self) -> &'static str;
    fn matches_port(&self, port_name: &str) -> bool;
    /// Messages to send right after the ports open (LED reset, mode setup, position query).
    fn on_connect(&mut self) -> Vec<MidiOut>;
    /// Messages to send before closing (LEDs off).
    fn on_disconnect(&mut self) -> Vec<MidiOut>;
    fn on_input(&mut self, message: &[u8], snapshot: &EngineSnapshot) -> Vec<Action>;
    /// Messages that must go out right after an input instead of waiting for the next feedback tick.
    fn drain_output(&mut self) -> Vec<MidiOut> {
        Vec::new()
    }
    /// Called ~30×/s; returns only LED messages whose value changed since last sent.
    fn feedback(&mut self, snapshot: &EngineSnapshot, now: Instant) -> Vec<MidiOut>;
    /// Periodic housekeeping (keepalive); called with feedback.
    fn tick(&mut self, now: Instant) -> Vec<MidiOut>;
}
