pub mod bpm;
pub mod decode;
pub mod downbeat;
pub mod filter;
pub mod key;
pub mod loudness;
pub mod phrase;
pub mod rhythm;
pub mod waveform;

pub use bpm::{analyze_beatgrid, BeatAnalysis, BpmRange};
pub use decode::{decode_file, DecodeError};
pub use key::{detect_key, Key, KeyEstimate};
pub use loudness::{measure_loudness, Loudness};
pub use phrase::{analyze_phrase_structure, analyze_phrases, analyze_phrases_with_downbeat, Phrase, PhraseAnalysis, PhraseKind};
pub use waveform::{compute_waveform, Waveform, WAVEFORM_POINTS_PER_SEC};
