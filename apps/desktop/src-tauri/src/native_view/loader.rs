use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, RecvTimeoutError};
use deckforge_analysis::PhraseKind;
use deckforge_gfx::{PhraseKind as GfxKind, PhraseView, Pyramid, WaveformData};
use deckforge_library::Library;

use super::shared::{LoaderMsg, Shared, Upload};

const RETRY_MIN: Duration = Duration::from_millis(400);
const RETRY_MAX: Duration = Duration::from_secs(2);
const PHRASE_RETRIES: u32 = 6;

#[derive(Default)]
struct DeckLoad {
    token: Option<u64>,
    waveform_loaded: bool,
    phrase_attempts: u32,
    phrases_loaded: bool,
    next_try: Option<Instant>,
    backoff: Duration,
}

fn gfx_kind(kind: PhraseKind) -> GfxKind {
    match kind {
        PhraseKind::Intro => GfxKind::Intro,
        PhraseKind::Build => GfxKind::Build,
        PhraseKind::Drop => GfxKind::Drop,
        PhraseKind::Break => GfxKind::Break,
        PhraseKind::Outro => GfxKind::Outro,
    }
}

fn attempt(deck: usize, load: &mut DeckLoad, library: &Library, shared: &Shared) {
    let Some(token) = load.token else { return };
    let id = token as i64;
    if !load.waveform_loaded {
        let bytes = library.analysis(id).ok().flatten().map(|a| a.waveform).unwrap_or_default();
        if let Some(pyramid) = WaveformData::from_bytes(&bytes).map(|wf| Pyramid::build(&wf)) {
            shared.push_upload(Upload::Pyramid { deck, token, pyramid: Box::new(pyramid) });
            load.waveform_loaded = true;
        }
    }
    if load.waveform_loaded && !load.phrases_loaded {
        load.phrase_attempts += 1;
        let phrases: Vec<PhraseView> = library
            .phrases(id)
            .unwrap_or_default()
            .into_iter()
            .map(|p| PhraseView { start_secs: p.start_secs, end_secs: p.end_secs, kind: gfx_kind(p.kind), energy: p.energy, fill_start_secs: p.fill_start_secs })
            .collect();
        if !phrases.is_empty() || load.phrase_attempts >= PHRASE_RETRIES {
            load.phrases_loaded = true;
            if !phrases.is_empty() {
                shared.push_upload(Upload::Phrases { deck, token, phrases });
            }
        }
    }
    if load.waveform_loaded && load.phrases_loaded {
        load.next_try = None;
    } else {
        load.backoff = (load.backoff * 2).clamp(RETRY_MIN, RETRY_MAX);
        load.next_try = Some(Instant::now() + load.backoff);
    }
}

/// Fetches waveform and phrase data off the render thread and retries while analysis is still running.
pub fn run(library: Arc<Library>, shared: Arc<Shared>, rx: Receiver<LoaderMsg>) {
    let mut decks = [DeckLoad::default(), DeckLoad::default()];
    loop {
        let wait = decks.iter().filter_map(|d| d.next_try).min().map(|t| t.saturating_duration_since(Instant::now()));
        let msg = match wait {
            Some(w) => rx.recv_timeout(w),
            None => rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
        };
        match msg {
            Ok(LoaderMsg::Want { deck, token }) => {
                decks[deck] = DeckLoad { token, backoff: RETRY_MIN / 2, ..Default::default() };
                if token.is_none() {
                    shared.push_upload(Upload::Clear { deck });
                } else {
                    attempt(deck, &mut decks[deck], &library, &shared);
                }
            }
            Ok(LoaderMsg::Reload { track_id }) => {
                for (deck, load) in decks.iter_mut().enumerate() {
                    if load.token == Some(track_id) {
                        *load = DeckLoad { token: Some(track_id), backoff: RETRY_MIN / 2, ..Default::default() };
                        attempt(deck, load, &library, &shared);
                    }
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                let now = Instant::now();
                for (deck, load) in decks.iter_mut().enumerate() {
                    if load.next_try.is_some_and(|t| t <= now) {
                        attempt(deck, load, &library, &shared);
                    }
                }
            }
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}
