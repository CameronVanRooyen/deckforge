//! Native GPU waveform layer behind a transparent webview. One thin host module per platform.

mod loader;
mod render;
mod shared;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

use std::cell::RefCell;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use deckforge_engine::EngineSnapshot;
use deckforge_gfx::gpu::{new_instance, Gpu};
use deckforge_gfx::{AudioSync, CssRect, FrameStatsReport, StyleSpec, SurfaceId};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, WebviewWindow, WindowEvent};

use crate::state::AppState;
use render::RenderThread;
use shared::{LoaderMsg, NativeInfo, Shared};

const FAST_POLL: Duration = Duration::from_millis(3);
const IDLE_POLL: Duration = Duration::from_millis(16);

#[derive(Debug)]
#[cfg_attr(target_os = "macos", allow(dead_code))]
pub enum HostError {
    Unsupported(&'static str),
    Failed(String),
}

impl std::fmt::Display for HostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HostError::Unsupported(why) => write!(f, "native waveforms unsupported: {why}"),
            HostError::Failed(why) => write!(f, "native waveforms failed: {why}"),
        }
    }
}

/// Raw handles of the child surface a host created behind the webview.
pub struct RawTarget {
    pub window: RawWindowHandle,
    pub display: RawDisplayHandle,
}

/// Embeds a GPU-presentable surface behind a transparent webview. All methods run on the main thread.
pub trait NativeHost {
    fn attach(&mut self, window: &WebviewWindow) -> Result<RawTarget, HostError>;
    fn resize(&mut self, width_px: u32, height_px: u32);
    fn detach(&mut self);
}

#[cfg(target_os = "macos")]
fn platform_host() -> Box<dyn NativeHost> {
    Box::<macos::MacHost>::default()
}

#[cfg(target_os = "windows")]
fn platform_host() -> Box<dyn NativeHost> {
    Box::<windows::WindowsHost>::default()
}

#[cfg(target_os = "linux")]
fn platform_host() -> Box<dyn NativeHost> {
    Box::<linux::LinuxHost>::default()
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn platform_host() -> Box<dyn NativeHost> {
    struct Unsupported;
    impl NativeHost for Unsupported {
        fn attach(&mut self, _: &WebviewWindow) -> Result<RawTarget, HostError> {
            Err(HostError::Unsupported("no native host for this platform"))
        }
        fn resize(&mut self, _: u32, _: u32) {}
        fn detach(&mut self) {}
    }
    Box::new(Unsupported)
}

thread_local! {
    static HOST: RefCell<Option<Box<dyn NativeHost>>> = const { RefCell::new(None) };
}

pub struct NativeHandle {
    shared: Arc<Shared>,
}

struct PumpSync {
    audio: AudioSync,
    last_frames: u64,
}

impl NativeHandle {
    fn shared(&self) -> &Arc<Shared> {
        &self.shared
    }
}

/// Publishes the newest engine snapshot to the renderer; returns how long the state pump should sleep.
pub fn publish(app: &AppHandle, snapshot: &EngineSnapshot, sync: &mut PublishState) -> Duration {
    let Some(handle) = app.try_state::<NativeHandle>() else { return IDLE_POLL };
    let shared = handle.shared();
    if snapshot.frames_processed != sync.0.last_frames || sync.0.last_frames == 0 {
        sync.0.last_frames = snapshot.frames_processed;
        let at = sync.0.audio.observe(shared.now_secs(), snapshot.frames_processed, snapshot.sample_rate);
        shared.publish(at, *snapshot);
    }
    if shared.wants_fast_poll() { FAST_POLL } else { IDLE_POLL }
}

pub struct PublishState(PumpSync);

impl Default for PublishState {
    fn default() -> Self {
        Self(PumpSync { audio: AudioSync::default(), last_frames: 0 })
    }
}

pub fn native_requested() -> bool {
    std::env::var("DECKFORGE_NATIVE_WAVEFORMS").map(|v| v != "0").unwrap_or(true)
}

fn init(app: &tauri::App) -> Result<NativeInfo, HostError> {
    let window = app.get_webview_window("main").ok_or(HostError::Failed("no main window".into()))?;
    let mut host = platform_host();
    let target = host.attach(&window)?;
    let instance = new_instance();
    let surface = unsafe {
        instance
            .create_surface_unsafe(deckforge_gfx::wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: Some(target.display),
                raw_window_handle: target.window,
            })
            .map_err(|e| HostError::Failed(e.to_string()))?
    };
    let gpu = Gpu::new(Some(&surface), instance).map_err(|e| HostError::Failed(e.to_string()))?;
    let adapter = gpu.adapter.get_info();
    let info = NativeInfo { adapter: adapter.name.clone(), backend: format!("{:?}", adapter.backend), host: std::env::consts::OS.into(), gpu_timing: false };

    let (tx, rx) = crossbeam_channel::unbounded();
    let shared = Arc::new(Shared::new(tx));
    *shared.info.lock() = info.clone();
    let size = window.inner_size().map_err(|e| HostError::Failed(e.to_string()))?;
    let scale = window.scale_factor().unwrap_or(1.0);
    shared.update(|i| {
        i.window_px = (size.width, size.height);
        i.scale = scale;
    });

    let library = app.state::<AppState>().library.clone();
    let loader_shared = shared.clone();
    thread::Builder::new()
        .name("deckforge-native-loader".into())
        .spawn(move || loader::run(library, loader_shared, rx))
        .map_err(|e| HostError::Failed(e.to_string()))?;
    let render_shared = shared.clone();
    thread::Builder::new()
        .name("deckforge-native-render".into())
        .spawn(move || RenderThread { shared: render_shared, gpu, surface }.run())
        .map_err(|e| HostError::Failed(e.to_string()))?;

    let events_shared = shared.clone();
    window.on_window_event(move |event| match event {
        WindowEvent::Resized(size) => {
            events_shared.update(|i| i.window_px = (size.width, size.height));
            HOST.with(|h| {
                if let Some(host) = h.borrow_mut().as_mut() {
                    host.resize(size.width, size.height);
                }
            });
        }
        WindowEvent::ScaleFactorChanged { scale_factor, new_inner_size, .. } => {
            events_shared.update(|i| {
                i.scale = *scale_factor;
                i.window_px = (new_inner_size.width, new_inner_size.height);
            });
        }
        WindowEvent::Destroyed => {
            events_shared.shutdown();
            HOST.with(|h| {
                if let Some(mut host) = h.borrow_mut().take() {
                    host.detach();
                }
            });
        }
        _ => {}
    });

    HOST.with(|h| *h.borrow_mut() = Some(host));
    app.manage(NativeHandle { shared });
    Ok(info)
}

/// Attaches the native layer on the main thread; failures leave the canvas fallback in charge.
pub fn setup(app: &tauri::App) {
    if !native_requested() {
        log::info!("native waveforms disabled by DECKFORGE_NATIVE_WAVEFORMS=0");
        return;
    }
    match init(app) {
        Ok(info) => log::info!("native waveforms active: {} ({}) on {}", info.adapter, info.backend, info.host),
        Err(e) => log::warn!("{e}; using canvas waveforms"),
    }
}

fn handle(app: &AppHandle) -> Option<Arc<Shared>> {
    app.try_state::<NativeHandle>().map(|h| h.shared.clone())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RectMsg {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[tauri::command]
pub fn native_waveforms_available(app: AppHandle) -> bool {
    handle(&app).is_some()
}

#[tauri::command]
pub fn native_set_rects(app: AppHandle, rects: Vec<RectMsg>) {
    let Some(shared) = handle(&app) else { return };
    let parsed: Vec<(SurfaceId, CssRect)> =
        rects.iter().filter_map(|r| SurfaceId::parse(&r.id).map(|id| (id, CssRect { x: r.x, y: r.y, w: r.w, h: r.h }))).collect();
    shared.update(|i| {
        let next: std::collections::HashMap<_, _> = parsed.into_iter().collect();
        if i.rects != next {
            i.rects = next;
        }
    });
}

#[tauri::command]
pub fn native_set_style(app: AppHandle, spec: StyleSpec) -> Vec<String> {
    let Some(shared) = handle(&app) else { return Vec::new() };
    let mut errors = Vec::new();
    shared.update(|i| {
        errors = i.style.apply(&spec);
        i.style_rev += 1;
    });
    errors
}

#[tauri::command]
pub fn native_set_zoom(app: AppHandle, pps: f64, deck: Option<usize>) {
    let Some(shared) = handle(&app) else { return };
    if !pps.is_finite() || pps <= 0.0 {
        return;
    }
    shared.update(|i| match deck {
        Some(d) if d < 2 => i.zoom[d] = pps,
        _ => i.zoom = [pps, pps],
    });
}

#[tauri::command]
pub fn native_drag_preview(app: AppHandle, deck: usize, secs: Option<f64>) {
    let Some(shared) = handle(&app) else { return };
    if deck > 1 {
        return;
    }
    shared.update(|i| {
        i.drag[deck] = secs.filter(|s| s.is_finite());
        i.drag_rev[deck] += 1;
    });
}

#[tauri::command]
pub fn native_set_latency(app: AppHandle, ms: f64) {
    if let Some(shared) = handle(&app) {
        shared.update(|i| i.latency_secs = (ms / 1000.0).clamp(0.0, 0.5));
    }
}

#[tauri::command]
pub fn native_reload_track(app: AppHandle, track_id: u64) {
    if let Some(shared) = handle(&app) {
        shared.request_loader(LoaderMsg::Reload { track_id });
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeStats {
    pub active: bool,
    pub info: NativeInfo,
    pub frames: FrameStatsReport,
    pub gpu_ms: Option<f64>,
}

#[tauri::command]
pub fn native_frame_stats(app: AppHandle) -> NativeStats {
    match handle(&app) {
        Some(s) => NativeStats { active: true, info: s.info.lock().clone(), frames: s.stats.lock().report(), gpu_ms: *s.gpu_ms.lock() },
        None => NativeStats { active: false, info: NativeInfo::default(), frames: FrameStatsReport::default(), gpu_ms: None },
    }
}
