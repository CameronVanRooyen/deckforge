mod analysis;
mod commands;
mod native_view;
pub mod spotify;
mod state;

use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use deckforge_common::DECK_COUNT;
use deckforge_controller::{AppAction, ControllerService, Ddj400};
use deckforge_engine::{
    create_engine, list_output_devices, ActiveOutput, AudioManager, Command, MixerParam, OutputDeviceInfo, OutputRequest, Routing, RoutingMode,
    SecondaryOutputRequest,
};
use deckforge_library::Library;
use parking_lot::Mutex;
use tauri::{AppHandle, Emitter, Manager, RunEvent};

use crate::analysis::AnalysisQueue;
use crate::state::AppState;

const STATE_INTERVAL: Duration = Duration::from_millis(16);
const ENGINE_RATE: u32 = 48_000;
const CONTROLLER_GRACE: Duration = Duration::from_secs(3);

fn data_dir() -> std::path::PathBuf {
    std::env::var_os("DECKFORGE_DATA_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| dirs::data_dir().unwrap_or_else(std::env::temp_dir).join("deckforge"))
}

fn is_ddj400_audio(name: &str) -> bool {
    name.to_ascii_uppercase().contains("DDJ-400")
}

fn ddj400_audio_device(devices: &[OutputDeviceInfo]) -> Option<&OutputDeviceInfo> {
    devices.iter().find(|d| is_ddj400_audio(&d.name) && d.max_channels >= 4)
}

fn ddj400_routing() -> Routing {
    Routing { mode: RoutingMode::MasterAndPhones, master: [0, 1], phones: [2, 3] }
}

fn ddj400_request(saved: Option<OutputRequest>, device: &OutputDeviceInfo) -> OutputRequest {
    let mut request = saved.filter(|r| r.device_id.as_deref() == Some(device.id.as_str())).unwrap_or_default();
    request.device_id = Some(device.id.clone());
    request.routing = Some(ddj400_routing());
    request
}

fn fallback_request(saved: Option<OutputRequest>, devices: &[OutputDeviceInfo]) -> OutputRequest {
    saved
        .filter(|request| {
            request.device_id.as_deref().is_none_or(|id| devices.iter().any(|d| d.id == id && !is_ddj400_audio(&d.name)))
        })
        .unwrap_or_default()
}

fn initial_output_for_devices(saved: Option<OutputRequest>, devices: &[OutputDeviceInfo]) -> (OutputRequest, bool) {
    if let Some(device) = ddj400_audio_device(devices) {
        return (ddj400_request(saved, device), true);
    }
    (fallback_request(saved, devices), false)
}

fn initial_output(library: &Library) -> (OutputRequest, bool) {
    initial_output_for_devices(library.setting::<OutputRequest>("audio.output"), &list_output_devices())
}

fn use_hardware_phones(saved: Option<bool>) -> bool {
    saved.unwrap_or(true)
}

fn output_change_request(
    saved: Option<OutputRequest>,
    devices: &[OutputDeviceInfo],
    was_present: &mut bool,
    active: Option<&ActiveOutput>,
) -> Option<OutputRequest> {
    let ddj = ddj400_audio_device(devices);
    let arrived = ddj.is_some() && !*was_present;
    let removed = ddj.is_none() && *was_present;
    *was_present = ddj.is_some();
    match (ddj, arrived, removed) {
        (Some(device), true, _)
            if !active.is_some_and(|a| is_ddj400_audio(&a.device_name) && a.routing == ddj400_routing()) =>
        {
            Some(ddj400_request(saved, device))
        }
        (None, _, true) => Some(fallback_request(saved, devices)),
        _ => None,
    }
}

/// Selects the DDJ-400's four-channel output while connected and restores the saved/system output on removal.
fn follow_ddj400(state: &AppState, was_present: &mut bool) {
    let devices = list_output_devices();
    let saved = state.library.setting::<OutputRequest>("audio.output");
    let active = state.audio.status().active;
    let Some(request) = output_change_request(saved, &devices, was_present, active.as_ref()) else { return };
    if ddj400_audio_device(&devices).is_some() {
        log::info!("DDJ-400 audio detected; switching to Master 1/2 and Phones 3/4");
        if state.library.setting::<bool>("audio.hardwarePhones").is_none() {
            state.engine.send(Command::HardwarePhones(true));
        }
    } else {
        log::info!("DDJ-400 audio removed; restoring the saved or system output");
    }
    if let Err(e) = state.audio.open(request) {
        log::warn!("switching output failed: {e}");
    }
}

#[cfg(test)]
mod ddj400_audio_tests {
    use super::*;

    fn device(id: &str, name: &str, max_channels: u16, is_default: bool) -> OutputDeviceInfo {
        OutputDeviceInfo { id: id.into(), name: name.into(), max_channels, sample_rates: vec![44_100], is_default }
    }

    fn ddj() -> OutputDeviceInfo {
        device("ddj-id", "DDJ-400 Audio Out", 4, false)
    }

    fn active(device_id: &str, name: &str, channels: u16, routing: Routing) -> ActiveOutput {
        ActiveOutput {
            device_id: device_id.into(),
            device_name: name.into(),
            sample_rate: 44_100,
            channels,
            requested_buffer: Some(256),
            routing,
            sample_format: "f32".into(),
        }
    }

    fn usb() -> OutputDeviceInfo {
        device("usb-id", "USB Studio Output", 2, false)
    }

    #[test]
    fn selects_four_channel_ddj_and_preserves_same_device_audio_preferences() {
        let saved = OutputRequest {
            device_id: Some("ddj-id".into()),
            sample_rate: Some(44_100),
            buffer_frames: Some(128),
            routing: Some(Routing { mode: RoutingMode::MasterOnly, master: [0, 0], phones: [0, 0] }),
        };
        let (request, selected) = initial_output_for_devices(Some(saved), &[ddj()]);
        assert!(selected);
        assert_eq!(request.device_id.as_deref(), Some("ddj-id"));
        assert_eq!(request.sample_rate, Some(44_100));
        assert_eq!(request.buffer_frames, Some(128));
        assert_eq!(request.routing, Some(ddj400_routing()));
    }

    #[test]
    fn routes_master_and_phones_to_the_two_ddj_stereo_pairs() {
        let request = ddj400_request(None, &ddj());
        assert_eq!(request.device_id.as_deref(), Some("ddj-id"));
        assert_eq!(request.routing, Some(ddj400_routing()));
    }

    #[test]
    fn uses_saved_hardware_headphone_preference_and_defaults_on_for_ddj() {
        assert!(use_hardware_phones(None));
        assert!(use_hardware_phones(Some(true)));
        assert!(!use_hardware_phones(Some(false)));
    }

    #[test]
    fn ignores_a_ddj_named_output_without_four_channels() {
        let devices = [device("ddj-id", "DDJ-400 Audio Out", 2, true), usb()];
        let (request, selected) = initial_output_for_devices(None, &devices);
        assert!(!selected);
        assert_eq!(request.device_id, None, "system output remains selected");
    }

    #[test]
    fn restores_saved_non_ddj_output_and_uses_system_fallback_when_needed() {
        let saved = OutputRequest { device_id: Some("usb-id".into()), sample_rate: Some(48_000), buffer_frames: Some(256), routing: None };
        let restored = fallback_request(Some(saved.clone()), &[usb()]);
        assert_eq!(restored.device_id.as_deref(), Some("usb-id"));
        assert_eq!(restored.sample_rate, Some(48_000));
        assert_eq!(restored.buffer_frames, Some(256));

        let ddj_saved = OutputRequest { device_id: Some("ddj-id".into()), ..OutputRequest::default() };
        assert!(fallback_request(Some(ddj_saved), &[usb()]).device_id.is_none());
        assert!(fallback_request(None, &[usb()]).device_id.is_none());
    }

    #[test]
    fn hotplug_selects_ddj_then_restores_and_reselects_it() {
        let saved = OutputRequest { device_id: Some("usb-id".into()), sample_rate: Some(48_000), ..OutputRequest::default() };
        let mut present = false;
        let usb_active = active("usb-id", "USB Studio Output", 2, Routing::for_channel_count(2));
        assert!(output_change_request(Some(saved.clone()), &[usb()], &mut present, Some(&usb_active)).is_none());

        let arrived = output_change_request(Some(saved.clone()), &[usb(), ddj()], &mut present, Some(&usb_active)).unwrap();
        assert_eq!(arrived.device_id.as_deref(), Some("ddj-id"));
        assert_eq!(arrived.routing, Some(ddj400_routing()));

        let ddj_active = active("ddj-id", "DDJ-400 Audio Out", 4, ddj400_routing());
        let removed = output_change_request(Some(saved), &[usb()], &mut present, Some(&ddj_active)).unwrap();
        assert_eq!(removed.device_id.as_deref(), Some("usb-id"));

        let reconnected = output_change_request(None, &[ddj()], &mut present, Some(&usb_active)).unwrap();
        assert_eq!(reconnected.device_id.as_deref(), Some("ddj-id"));
    }

    #[test]
    fn hotplug_corrects_a_ddj_already_open_with_the_wrong_routing() {
        let mut present = false;
        let wrong = active("ddj-id", "DDJ-400 Audio Out", 4, Routing { mode: RoutingMode::MasterOnly, master: [0, 1], phones: [0, 0] });
        let corrected = output_change_request(None, &[ddj()], &mut present, Some(&wrong)).unwrap();
        assert_eq!(corrected.routing, Some(ddj400_routing()));

        present = false;
        let correct = active("ddj-id", "DDJ-400 Audio Out", 4, ddj400_routing());
        assert!(output_change_request(None, &[ddj()], &mut present, Some(&correct)).is_none(), "already correct route is not reopened");
    }
}

fn restore_secondary(library: &Library, audio: &AudioManager) {
    let Some(saved) = library.setting::<SecondaryOutputRequest>(commands::SECONDARY_SETTING) else { return };
    if !list_output_devices().iter().any(|d| d.id == saved.device_id) {
        log::info!("saved additional output is not connected; leaving it off");
        return;
    }
    if let Err(e) = audio.set_secondary(Some(saved)) {
        log::warn!("additional output unavailable at startup: {e}");
    }
}

fn spawn_state_pump(app: AppHandle) {
    thread::Builder::new()
        .name("deckforge-state-pump".into())
        .spawn(move || {
            let started = Instant::now();
            let mut last_status = Instant::now() - Duration::from_secs(5);
            let mut faders_initialised = false;
            let mut ddj400_present = ddj400_audio_device(&list_output_devices()).is_some();
            let mut native = native_view::PublishState::default();
            let mut next_ui = Instant::now();
            loop {
                let state = app.state::<AppState>();
                let snapshot = state.engine.snapshot();
                let poll = native_view::publish(&app, &snapshot, &mut native);
                if Instant::now() >= next_ui {
                    next_ui = (next_ui + STATE_INTERVAL).max(Instant::now());
                    let _ = app.emit("engine://state", snapshot);
                    state.engine.collect_garbage();
                    commands::persist_cues(&state);
                }
                if last_status.elapsed() >= Duration::from_secs(1) {
                    last_status = Instant::now();
                    let controller = state.controller.status();
                    if !faders_initialised && started.elapsed() >= CONTROLLER_GRACE {
                        faders_initialised = true;
                        if !controller.connected {
                            for channel in 0..DECK_COUNT {
                                state.engine.send(Command::Mixer { channel, param: MixerParam::Fader(1.0) });
                            }
                        }
                    }
                    follow_ddj400(&state, &mut ddj400_present);
                    let _ = app.emit("controller://status", controller);
                    let _ = app.emit("audio://status", state.audio.status());
                }
                thread::sleep(poll);
            }
        })
        .expect("spawn state pump");
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let dir = data_dir();
            log::info!("data directory: {}", dir.display());
            let library = Arc::new(Library::open(&dir.join("library.sqlite"))?);
            let (handle, rt) = create_engine(ENGINE_RATE);
            let engine = Arc::new(handle);
            let saved_hardware_phones = library.setting::<bool>("audio.hardwarePhones");
            engine.send(Command::HardwarePhones(saved_hardware_phones.unwrap_or(false)));
            if let Some(curve) = library.setting("mixer.crossfaderCurve") {
                engine.send(Command::CrossfaderCurve(curve));
            }
            if let Some(curve) = library.setting("mixer.eqCurve") {
                engine.send(Command::EqCurve(curve));
            }
            let audio = AudioManager::spawn(rt);
            let (request, targets_ddj400) = initial_output(&library);
            if targets_ddj400 {
                engine.send(Command::HardwarePhones(use_hardware_phones(saved_hardware_phones)));
            }
            if let Err(e) = audio.open(request) {
                log::warn!("audio output unavailable at startup: {e}");
            }
            restore_secondary(&library, &audio);

            Box::leak(Box::new(deckforge_controller::watch_device_changes()));
            let handle_for_actions = app.handle().clone();
            let controller = ControllerService::spawn(
                Box::new(Ddj400::new()),
                engine.clone(),
                Box::new(move |action: AppAction| {
                    let _ = handle_for_actions.emit("controller://app-action", action);
                }),
            );
            let loaded = Arc::new(Mutex::new([None, None]));
            let analysis = AnalysisQueue::new();
            analysis.spawn_workers(app.handle().clone(), library.clone(), engine.clone(), loaded.clone());
            for id in library.pending_analysis().unwrap_or_default() {
                analysis.enqueue(id);
            }
            app.manage(spotify::SpotifyState::new(&library)?);
            app.manage(AppState { engine, audio, library, controller, analysis, loaded });
            spawn_state_pump(app.handle().clone());
            native_view::setup(app);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::engine_command,
            commands::engine_snapshot,
            commands::load_track,
            commands::eject,
            commands::library_tracks,
            commands::library_track,
            commands::library_import,
            commands::playlists_list,
            commands::playlist_create,
            commands::playlist_rename,
            commands::playlist_delete,
            commands::playlist_add_tracks,
            commands::playlist_remove_track,
            commands::track_analyze,
            commands::track_set_beatgrid,
            commands::track_waveform,
            commands::track_artwork,
            analysis::track_phrases,
            commands::audio_devices,
            commands::audio_open,
            commands::audio_status,
            commands::audio_set_routing,
            commands::audio_set_secondary,
            commands::audio_set_secondary_level,
            commands::controller_status,
            commands::midi_log,
            commands::midi_clear_log,
            commands::midi_inject,
            commands::ui_report,
            native_view::native_waveforms_available,
            native_view::native_set_rects,
            native_view::native_set_style,
            native_view::native_set_zoom,
            native_view::native_drag_preview,
            native_view::native_set_latency,
            native_view::native_reload_track,
            native_view::native_frame_stats,
            commands::selftest_requested,
            spotify::spotify_status,
            spotify::spotify_set_client_id,
            spotify::spotify_login,
            spotify::spotify_logout,
            spotify::spotify_playlists,
            spotify::spotify_playlist_tracks,
            spotify::spotify_import_playlist,
        ])
        .build(tauri::generate_context!())
        .expect("error while building deckforge")
        .run(|app, event| {
            if let RunEvent::ExitRequested { .. } = event
                && let Some(state) = app.try_state::<AppState>()
            {
                commands::persist_cues(&state);
                state.controller.shutdown();
                state.audio.close();
            }
        });
}
