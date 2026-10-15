use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use crossbeam_channel::Sender;
use deckforge_engine::{DeckSnapshot, EngineSnapshot};
use deckforge_gfx::{CssRect, FrameStats, PhraseView, Pyramid, Style, SurfaceId};
use parking_lot::{Condvar, Mutex};

pub enum Upload {
    Pyramid { deck: usize, token: u64, pyramid: Box<Pyramid> },
    Phrases { deck: usize, token: u64, phrases: Vec<PhraseView> },
    Clear { deck: usize },
}

pub enum LoaderMsg {
    Want { deck: usize, token: Option<u64> },
    Reload { track_id: u64 },
}

#[derive(Clone, Copy)]
pub struct TimedSnapshot {
    pub at: f64,
    pub snapshot: EngineSnapshot,
}

pub struct Inputs {
    pub dirty: bool,
    pub quit: bool,
    pub rects: HashMap<SurfaceId, CssRect>,
    pub style: Style,
    pub style_rev: u64,
    pub zoom: [f64; 2],
    pub beatmatch_view: bool,
    pub drag: [Option<f64>; 2],
    pub drag_rev: [u64; 2],
    pub latency_secs: f64,
    pub window_px: (u32, u32),
    pub scale: f64,
    pub snapshot: Option<TimedSnapshot>,
    pub snapshot_rev: u64,
    pub uploads: Vec<Upload>,
}

pub struct Work {
    pub rects: Vec<(SurfaceId, CssRect)>,
    pub style: Style,
    pub style_rev: u64,
    pub zoom: [f64; 2],
    pub beatmatch_view: bool,
    pub drag: [Option<f64>; 2],
    pub drag_rev: [u64; 2],
    pub latency_secs: f64,
    pub window_px: (u32, u32),
    pub scale: f64,
    pub snapshot: Option<TimedSnapshot>,
    pub snapshot_rev: u64,
    pub uploads: Vec<Upload>,
}

#[derive(Default, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeInfo {
    pub adapter: String,
    pub backend: String,
    pub host: String,
    pub gpu_timing: bool,
}

/// State shared between IPC commands, the state pump, the loader and the render thread.
pub struct Shared {
    pub epoch: Instant,
    state: Mutex<Inputs>,
    wake: Condvar,
    pub stats: Mutex<FrameStats>,
    pub gpu_ms: Mutex<Option<f64>>,
    pub info: Mutex<NativeInfo>,
    pub continuous: AtomicBool,
    loader: Sender<LoaderMsg>,
}

fn deck_visually_equal(a: &DeckSnapshot, b: &DeckSnapshot) -> bool {
    a.loaded == b.loaded
        && a.track_token == b.track_token
        && a.playing == b.playing
        && a.previewing == b.previewing
        && a.tempo_ratio == b.tempo_ratio
        && a.has_grid == b.has_grid
        && a.grid_bpm == b.grid_bpm
        && a.first_beat_secs == b.first_beat_secs
        && a.cue_secs == b.cue_secs
        && a.duration_secs == b.duration_secs
        && a.loop_in_secs == b.loop_in_secs
        && a.loop_out_secs == b.loop_out_secs
        && a.loop_active == b.loop_active
        && a.hot_cues == b.hot_cues
        && (a.playing || a.previewing || a.position_secs == b.position_secs)
}

impl Shared {
    pub fn new(loader: Sender<LoaderMsg>) -> Self {
        Self {
            epoch: Instant::now(),
            state: Mutex::new(Inputs {
                dirty: true,
                quit: false,
                rects: HashMap::new(),
                style: Style::default(),
                style_rev: 1,
                zoom: [90.0, 90.0],
                beatmatch_view: false,
                drag: [None, None],
                drag_rev: [0, 0],
                latency_secs: 0.0,
                window_px: (0, 0),
                scale: 1.0,
                snapshot: None,
                snapshot_rev: 0,
                uploads: Vec::new(),
            }),
            wake: Condvar::new(),
            stats: Mutex::new(FrameStats::default()),
            gpu_ms: Mutex::new(None),
            info: Mutex::new(NativeInfo::default()),
            continuous: AtomicBool::new(false),
            loader,
        }
    }

    pub fn now_secs(&self) -> f64 {
        self.epoch.elapsed().as_secs_f64()
    }

    pub fn update(&self, f: impl FnOnce(&mut Inputs)) {
        let mut g = self.state.lock();
        f(&mut g);
        g.dirty = true;
        self.wake.notify_one();
    }

    pub fn shutdown(&self) {
        self.update(|i| i.quit = true);
    }

    /// Stores the newest engine snapshot; wakes the renderer only when something visible changed.
    pub fn publish(&self, at: f64, snapshot: EngineSnapshot) {
        let mut g = self.state.lock();
        let changed = match &g.snapshot {
            None => true,
            Some(prev) => !prev.snapshot.decks.iter().zip(&snapshot.decks).all(|(a, b)| deck_visually_equal(a, b)),
        };
        if changed {
            for (deck, d) in snapshot.decks.iter().enumerate() {
                let token = d.loaded.then_some(d.track_token);
                let prev = g.snapshot.as_ref().map(|p| p.snapshot.decks[deck]);
                if prev.is_none_or(|p| p.loaded != d.loaded || p.track_token != d.track_token) {
                    let _ = self.loader.send(LoaderMsg::Want { deck, token });
                }
            }
        }
        g.snapshot = Some(TimedSnapshot { at, snapshot });
        g.snapshot_rev += 1;
        if changed {
            g.dirty = true;
            self.wake.notify_one();
        }
    }

    pub fn push_upload(&self, upload: Upload) {
        self.update(|i| i.uploads.push(upload));
    }

    /// Blocks until there is something to draw (or `continuous`), then takes a copy of the inputs.
    pub fn next_work(&self, continuous: bool) -> Option<Work> {
        let mut g = self.state.lock();
        while !g.dirty && !continuous && !g.quit {
            self.wake.wait(&mut g);
        }
        if g.quit {
            return None;
        }
        g.dirty = false;
        let uploads = std::mem::take(&mut g.uploads);
        Some(Work {
            rects: g.rects.iter().map(|(k, v)| (*k, *v)).collect(),
            style: g.style.clone(),
            style_rev: g.style_rev,
            zoom: g.zoom,
            beatmatch_view: g.beatmatch_view,
            drag: g.drag,
            drag_rev: g.drag_rev,
            latency_secs: g.latency_secs,
            window_px: g.window_px,
            scale: g.scale,
            snapshot: g.snapshot,
            snapshot_rev: g.snapshot_rev,
            uploads,
        })
    }

    pub fn set_continuous(&self, on: bool) {
        self.continuous.store(on, Ordering::Relaxed);
    }

    pub fn wants_fast_poll(&self) -> bool {
        self.continuous.load(Ordering::Relaxed)
    }

    pub fn request_loader(&self, msg: LoaderMsg) {
        let _ = self.loader.send(msg);
    }
}
