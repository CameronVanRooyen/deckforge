use std::collections::VecDeque;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crossbeam_channel::{bounded, select, tick, Receiver, Sender};
use deckforge_engine::EngineHandle;
use midir::{MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};
use parking_lot::Mutex;
use serde::Serialize;

use crate::action::{Action, AppAction};
use crate::mapping::{ControllerMapping, MidiOut};

const LOG_CAPACITY: usize = 400;
const SCAN_INTERVAL: Duration = Duration::from_secs(2);
const FEEDBACK_INTERVAL: Duration = Duration::from_millis(33);
const CLIENT_NAME: &str = "deckforge";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MidiDirection {
    In,
    Out,
    Injected,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MidiLogEntry {
    pub time_ms: u64,
    pub direction: MidiDirection,
    pub bytes: Vec<u8>,
    pub summary: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControllerStatus {
    pub connected: bool,
    pub mapping: String,
    pub input_port: Option<String>,
    pub output_port: Option<String>,
    pub available_inputs: Vec<String>,
    pub available_outputs: Vec<String>,
    pub messages_in: u64,
    pub messages_out: u64,
    pub last_error: Option<String>,
}

enum Event {
    Midi(Vec<u8>, MidiDirection),
    Shutdown(Sender<()>),
}

struct Shared {
    status: Mutex<ControllerStatus>,
    log: Mutex<VecDeque<MidiLogEntry>>,
}

/// Runs controller discovery, input mapping, LED feedback and diagnostics on its own thread.
pub struct ControllerService {
    tx: Sender<Event>,
    shared: Arc<Shared>,
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

pub fn describe(bytes: &[u8]) -> String {
    match bytes {
        [0xF0, ..] => format!("SysEx {} bytes", bytes.len()),
        [s, d1, d2] => {
            let ch = (s & 0x0F) + 1;
            match s & 0xF0 {
                0x90 => format!("Note On  ch{ch} {d1:#04x} vel {d2:#04x}"),
                0x80 => format!("Note Off ch{ch} {d1:#04x} vel {d2:#04x}"),
                0xB0 => format!("CC       ch{ch} {d1:#04x} = {d2:#04x}"),
                _ => format!("{bytes:02x?}"),
            }
        }
        _ => format!("{bytes:02x?}"),
    }
}

struct Connection {
    _input: MidiInputConnection<()>,
    output: Option<MidiOutputConnection>,
    input_name: String,
    output_name: Option<String>,
}

impl ControllerService {
    pub fn spawn(
        mapping: Box<dyn ControllerMapping>,
        engine: Arc<EngineHandle>,
        app: Box<dyn Fn(AppAction) + Send>,
    ) -> Self {
        let (tx, rx) = bounded::<Event>(4096);
        let shared = Arc::new(Shared {
            status: Mutex::new(ControllerStatus { mapping: mapping.name().to_string(), ..Default::default() }),
            log: Mutex::new(VecDeque::with_capacity(LOG_CAPACITY)),
        });
        let worker_shared = shared.clone();
        let worker_tx = tx.clone();
        thread::Builder::new()
            .name("deckforge-controller".into())
            .spawn(move || run(mapping, engine, app, rx, worker_tx, worker_shared))
            .expect("spawn controller thread");
        Self { tx, shared }
    }

    pub fn status(&self) -> ControllerStatus {
        self.shared.status.lock().clone()
    }

    pub fn log(&self) -> Vec<MidiLogEntry> {
        self.shared.log.lock().iter().cloned().collect()
    }

    pub fn clear_log(&self) {
        self.shared.log.lock().clear();
    }

    /// Turns controller LEDs off; call before the application exits.
    pub fn shutdown(&self) {
        let (done_tx, done_rx) = bounded(1);
        if self.tx.send(Event::Shutdown(done_tx)).is_ok() {
            let _ = done_rx.recv_timeout(Duration::from_millis(500));
        }
    }

    /// Feeds a message through the mapping as if the controller had sent it (simulation and tests).
    pub fn inject(&self, bytes: Vec<u8>) -> bool {
        self.tx.try_send(Event::Midi(bytes, MidiDirection::Injected)).is_ok()
    }
}

fn record(shared: &Shared, direction: MidiDirection, bytes: &[u8]) {
    let mut log = shared.log.lock();
    if log.len() >= LOG_CAPACITY {
        log.pop_front();
    }
    log.push_back(MidiLogEntry { time_ms: now_ms(), direction, bytes: bytes.to_vec(), summary: describe(bytes) });
}

fn send_all(conn: &mut Option<Connection>, shared: &Shared, messages: Vec<MidiOut>) {
    let Some(c) = conn else { return };
    let Some(out) = &mut c.output else { return };
    let mut sent = 0;
    for m in messages {
        if out.send(&m).is_ok() {
            sent += 1;
            if m.first() != Some(&0xF0) && m.get(1) != Some(&0x02) {
                record(shared, MidiDirection::Out, &m);
            }
        }
    }
    shared.status.lock().messages_out += sent;
}

/// Keeps a MIDI client alive on the calling thread so the OS keeps the port list current for hot-plug.
/// On macOS this must be called from a thread with a running run loop (the app's main thread).
pub fn watch_device_changes() -> Option<MidiInput> {
    MidiInput::new(CLIENT_NAME).ok()
}

type PortScan = (Vec<String>, Vec<String>, Option<(String, Option<String>)>);

fn scan(mapping: &dyn ControllerMapping) -> PortScan {
    let ins: Vec<String> = MidiInput::new(CLIENT_NAME)
        .map(|m| m.ports().iter().filter_map(|p| m.port_name(p).ok()).collect())
        .unwrap_or_default();
    let outs: Vec<String> = MidiOutput::new(CLIENT_NAME)
        .map(|m| m.ports().iter().filter_map(|p| m.port_name(p).ok()).collect())
        .unwrap_or_default();
    let input = ins.iter().find(|n| mapping.matches_port(n)).cloned();
    let output = outs.iter().find(|n| mapping.matches_port(n)).cloned();
    (ins, outs, input.map(|i| (i, output)))
}

fn connect(input_name: &str, output_name: Option<&str>, tx: Sender<Event>) -> Result<Connection, String> {
    let midi_in = MidiInput::new(CLIENT_NAME).map_err(|e| e.to_string())?;
    let port = midi_in
        .ports()
        .into_iter()
        .find(|p| midi_in.port_name(p).ok().as_deref() == Some(input_name))
        .ok_or("input port vanished")?;
    let input = midi_in
        .connect(
            &port,
            "deckforge-in",
            move |_, bytes, _| {
                let _ = tx.try_send(Event::Midi(bytes.to_vec(), MidiDirection::In));
            },
            (),
        )
        .map_err(|e| e.to_string())?;
    let output = output_name.and_then(|name| {
        let midi_out = MidiOutput::new(CLIENT_NAME).ok()?;
        let port = midi_out.ports().into_iter().find(|p| midi_out.port_name(p).ok().as_deref() == Some(name))?;
        midi_out.connect(&port, "deckforge-out").ok()
    });
    Ok(Connection {
        _input: input,
        output_name: output.as_ref().and(output_name.map(str::to_string)),
        output,
        input_name: input_name.to_string(),
    })
}

fn run(
    mut mapping: Box<dyn ControllerMapping>,
    engine: Arc<EngineHandle>,
    app: Box<dyn Fn(AppAction) + Send>,
    rx: Receiver<Event>,
    tx: Sender<Event>,
    shared: Arc<Shared>,
) {
    let feedback = tick(FEEDBACK_INTERVAL);
    let scanner = tick(SCAN_INTERVAL);
    let mut conn: Option<Connection> = None;

    loop {
        select! {
            recv(rx) -> ev => {
                let (bytes, direction) = match ev {
                    Ok(Event::Midi(bytes, direction)) => (bytes, direction),
                    Ok(Event::Shutdown(done)) => {
                        let off = mapping.on_disconnect();
                        send_all(&mut conn, &shared, off);
                        let _ = done.send(());
                        continue;
                    }
                    Err(_) => return,
                };
                record(&shared, direction, &bytes);
                shared.status.lock().messages_in += 1;
                let snapshot = engine.snapshot();
                for action in mapping.on_input(&bytes, &snapshot) {
                    match action {
                        Action::Engine(cmd) => {
                            engine.send(cmd);
                        }
                        Action::App(a) => app(a),
                    }
                }
                let urgent = mapping.drain_output();
                send_all(&mut conn, &shared, urgent);
            }
            recv(feedback) -> _ => {
                let now = Instant::now();
                let snapshot = engine.snapshot();
                let mut out = mapping.feedback(&snapshot, now);
                out.extend(mapping.tick(now));
                send_all(&mut conn, &shared, out);
            }
            recv(scanner) -> _ => {
                let (ins, outs, found) = scan(mapping.as_ref());
                let still_present = conn.as_ref().is_some_and(|c| ins.contains(&c.input_name));
                if conn.is_some() && !still_present {
                    log::warn!("controller disconnected");
                    conn = None;
                }
                if conn.is_none()
                    && let Some((input, output)) = found
                {
                    match connect(&input, output.as_deref(), tx.clone()) {
                        Ok(c) => {
                            log::info!("controller connected: {input}");
                            conn = Some(c);
                            let init = mapping.on_connect();
                            send_all(&mut conn, &shared, init);
                            shared.status.lock().last_error = None;
                        }
                        Err(e) => shared.status.lock().last_error = Some(e),
                    }
                }
                let mut st = shared.status.lock();
                st.available_inputs = ins;
                st.available_outputs = outs;
                st.connected = conn.is_some();
                st.input_port = conn.as_ref().map(|c| c.input_name.clone());
                st.output_port = conn.as_ref().and_then(|c| c.output_name.clone());
            }
        }
    }
}
