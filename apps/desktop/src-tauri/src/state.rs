use std::sync::Arc;

use deckforge_common::DECK_COUNT;
use deckforge_controller::ControllerService;
use deckforge_engine::{AudioManager, EngineHandle};
use deckforge_library::{Library, StoredCues};
use parking_lot::Mutex;

use crate::analysis::AnalysisQueue;

#[derive(Default, Clone)]
pub struct LoadedTrack {
    pub track_id: i64,
    pub persisted: StoredCues,
}

pub struct AppState {
    pub engine: Arc<EngineHandle>,
    pub audio: AudioManager,
    pub library: Arc<Library>,
    pub controller: ControllerService,
    pub analysis: AnalysisQueue,
    pub loaded: Arc<Mutex<[Option<LoadedTrack>; DECK_COUNT]>>,
}
