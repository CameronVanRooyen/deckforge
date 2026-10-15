use std::fs::File;
use std::path::Path;

use deckforge_common::TrackAudio;
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("cannot open {path}: {source}")]
    Open { path: String, source: std::io::Error },
    #[error("unsupported or unreadable audio: {0}")]
    Unsupported(String),
    #[error("file contains no decodable audio")]
    Empty,
}

pub const SUPPORTED_EXTENSIONS: &[&str] =
    &["wav", "wave", "mp3", "flac", "aif", "aiff", "aifc", "m4a", "mp4", "aac", "ogg", "alac", "caf"];

pub fn is_supported(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| SUPPORTED_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

/// Decodes an entire file into interleaved stereo f32 at its native sample rate.
pub fn decode_file(path: &Path) -> Result<TrackAudio, DecodeError> {
    let file = File::open(path)
        .map_err(|source| DecodeError::Open { path: path.display().to_string(), source })?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let unsupported = |e: SymError| DecodeError::Unsupported(e.to_string());
    let mut format = symphonia::default::get_probe()
        .probe(&hint, mss, FormatOptions::default(), MetadataOptions::default())
        .map_err(unsupported)?;
    let track = format
        .default_track(TrackType::Audio)
        .ok_or_else(|| DecodeError::Unsupported("no audio track".into()))?;
    let track_id = track.id;
    let params = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .ok_or_else(|| DecodeError::Unsupported("missing audio codec parameters".into()))?;
    let mut sample_rate = params.sample_rate.unwrap_or(0);
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(params, &AudioDecoderOptions::default())
        .map_err(unsupported)?;
    let capacity = track.num_frames.map(|n| n as usize * 2).unwrap_or(0);

    let mut out: Vec<f32> = Vec::with_capacity(capacity);
    let mut scratch: Vec<f32> = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(Some(p)) => p,
            Ok(None) => break,
            Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(SymError::ResetRequired) => break,
            Err(e) => return Err(unsupported(e)),
        };
        if packet.track_id != track_id {
            continue;
        }
        let buf = match decoder.decode(&packet) {
            Ok(b) => b,
            Err(SymError::DecodeError(_)) => continue,
            Err(e) => return Err(unsupported(e)),
        };
        let channels = buf.num_planes();
        if channels == 0 || buf.frames() == 0 {
            continue;
        }
        if sample_rate == 0 {
            sample_rate = buf.spec().rate();
        }
        scratch.resize(buf.samples_interleaved(), 0.0);
        buf.copy_to_slice_interleaved(&mut scratch);
        if channels == 1 {
            out.extend(scratch.iter().flat_map(|&s| [s, s]));
        } else {
            out.extend(scratch.chunks_exact(channels).flat_map(|f| [f[0], f[1]]));
        }
    }
    if out.is_empty() || sample_rate == 0 {
        return Err(DecodeError::Empty);
    }
    Ok(TrackAudio::new(sample_rate, out))
}
