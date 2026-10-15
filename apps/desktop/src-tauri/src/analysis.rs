use std::sync::Arc;
use std::thread;

use crossbeam_channel::{unbounded, Receiver, Sender};
use deckforge_analysis::{
    analyze_beatgrid, analyze_phrases_with_downbeat, compute_waveform, decode_file, detect_key, measure_loudness, BpmRange, Phrase,
};
use deckforge_common::DECK_COUNT;
use deckforge_engine::{Command, EngineHandle};
use deckforge_library::{AnalysisState, Library, MusicalAnalysis};
use parking_lot::Mutex;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::commands::{err, phrase_starts_command, CmdResult};
use crate::state::{AppState, LoadedTrack};

const WORKERS: usize = 2;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Progress {
    track_id: i64,
    state: AnalysisState,
    queued: usize,
}

/// Background waveform, beatgrid, key and phrase analysis; urgent jobs (tracks just loaded) jump the queue.
pub struct AnalysisQueue {
    normal: Sender<i64>,
    urgent: Sender<i64>,
    receivers: (Receiver<i64>, Receiver<i64>),
}

impl AnalysisQueue {
    pub fn new() -> Self {
        let (normal, nrx) = unbounded();
        let (urgent, urx) = unbounded();
        Self { normal, urgent, receivers: (nrx, urx) }
    }

    pub fn enqueue(&self, id: i64) {
        let _ = self.normal.send(id);
    }

    pub fn enqueue_urgent(&self, id: i64) {
        let _ = self.urgent.send(id);
    }

    pub fn spawn_workers(
        &self,
        app: AppHandle,
        library: Arc<Library>,
        engine: Arc<EngineHandle>,
        loaded: Arc<Mutex<[Option<LoadedTrack>; DECK_COUNT]>>,
    ) {
        let in_flight = Arc::new(Mutex::new(std::collections::HashSet::<i64>::new()));
        for i in 0..WORKERS {
            let (nrx, urx) = self.receivers.clone();
            let (app, library, engine, loaded, in_flight) =
                (app.clone(), library.clone(), engine.clone(), loaded.clone(), in_flight.clone());
            thread::Builder::new()
                .name(format!("deckforge-analysis-{i}"))
                .spawn(move || loop {
                    let id = match urx.try_recv() {
                        Ok(id) => id,
                        Err(_) => crossbeam_channel::select! {
                            recv(urx) -> m => match m { Ok(id) => id, Err(_) => return },
                            recv(nrx) -> m => match m { Ok(id) => id, Err(_) => return },
                        },
                    };
                    if !in_flight.lock().insert(id) {
                        continue;
                    }
                    if library.track(id).map(|t| t.analysis == AnalysisState::Done).unwrap_or(true)
                        && library.analysis(id).ok().flatten().is_some_and(|a| a.is_current())
                    {
                        in_flight.lock().remove(&id);
                        continue;
                    }
                    analyze_one(&app, &library, &engine, &loaded, id, nrx.len() + urx.len());
                    in_flight.lock().remove(&id);
                })
                .expect("spawn analysis worker");
        }
    }
}

fn emit(app: &AppHandle, track_id: i64, state: AnalysisState, queued: usize) {
    let _ = app.emit("analysis://progress", Progress { track_id, state, queued });
}

fn analyze_one(
    app: &AppHandle,
    library: &Library,
    engine: &EngineHandle,
    loaded: &Mutex<[Option<LoadedTrack>; DECK_COUNT]>,
    id: i64,
    queued: usize,
) {
    let _ = library.set_analysis_state(id, AnalysisState::Running, None);
    emit(app, id, AnalysisState::Running, queued);
    let result = library
        .track_path(id)
        .map_err(|e| e.to_string())
        .and_then(|p| decode_file(&p).map_err(|e| e.to_string()));
    match result {
        Ok(track) => {
            let beat = analyze_beatgrid(&track, BpmRange::default());
            let waveform = compute_waveform(&track).to_bytes();
            let grid = beat.map(|b| b.grid);
            let confidence = beat.map_or(0.0, |b| b.confidence);
            let locked = library.locked_beatgrid(id).ok().flatten();
            let known_downbeat = locked.and_then(|_| library.downbeat_beat(id).ok().flatten());
            let structure = locked.or(grid).and_then(|g| analyze_phrases_with_downbeat(&track, g, known_downbeat));
            let musical = MusicalAnalysis {
                key: detect_key(&track),
                downbeat_beat: structure.as_ref().map(|s| s.downbeat_beat),
                loudness: measure_loudness(&track),
                phrases: structure.map(|s| s.phrases).unwrap_or_default(),
            };
            if let Err(e) = library.save_full_analysis(id, grid, confidence, &waveform, track.duration_secs(), &musical) {
                log::error!("saving analysis for {id}: {e}");
            }
            let stored = library.beatgrid(id).ok().flatten();
            for (deck, slot) in loaded.lock().iter().enumerate() {
                if slot.as_ref().is_some_and(|l| l.track_id == id) {
                    if stored.is_some() {
                        engine.send(Command::SetBeatgrid { deck, grid: stored });
                    }
                    engine.send(phrase_starts_command(library, deck, id));
                }
            }
            emit(app, id, AnalysisState::Done, queued);
            let _ = app.emit("library://changed", ());
        }
        Err(e) => {
            log::warn!("analysis failed for {id}: {e}");
            let _ = library.set_analysis_state(id, AnalysisState::Failed, Some(&e));
            emit(app, id, AnalysisState::Failed, queued);
        }
    }
}

#[tauri::command]
pub fn track_phrases(state: State<'_, AppState>, track_id: i64) -> CmdResult<Vec<Phrase>> {
    state.library.phrases(track_id).map_err(err)
}
