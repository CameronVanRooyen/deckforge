//! Platform-neutral GPU waveform renderer for deckforge. Knows nothing about windows or views.

pub mod bars;
pub mod clock;
pub mod color;
pub mod gpu;
pub mod layout;
pub mod offscreen;
pub mod phrase_nav;
pub mod renderer;
pub mod scene;
pub mod stats;
pub mod style;
pub mod synth;
pub mod text;
pub mod waveform;

pub use clock::{AudioSync, DeckClock, DeckSample};
pub use layout::{CssRect, PixelRect};
pub use renderer::Renderer;
pub use scene::{
    DeckView, FrameInput, PhraseKind, PhraseView, PlacedSurface, SurfaceId, SurfaceKind,
};
pub use stats::{FrameStats, FrameStatsReport};
pub use style::{Style, StyleSpec};
pub use waveform::{Pyramid, WaveformData};
pub use wgpu;
