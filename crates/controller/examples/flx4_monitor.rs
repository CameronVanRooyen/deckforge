//! Prints every MIDI message from a connected DDJ-FLX4, optionally appending to a log file.
//! `--host-leds` lights the selected pad-mode button and that mode's pads, as a host would.

use std::fs::OpenOptions;
use std::io::Write;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use deckforge_controller::flx4::KEEPALIVE_SYSEX;
use deckforge_controller::service::describe;
use midir::{MidiInput, MidiOutput};

fn is_flx4(name: &str) -> bool {
    name.to_ascii_uppercase().contains("FLX4")
}

const MODE_BUTTONS: [u8; 8] = [0x1B, 0x1E, 0x20, 0x22, 0x69, 0x6B, 0x6D, 0x6F];
const MODE_PAD_BASES: [u8; 8] = [0x00, 0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70];
const PAD_STATUS: [[u8; 2]; 2] = [[0x97, 0x98], [0x99, 0x9A]];

fn mode_leds(deck: u8, selected: usize) -> Vec<[u8; 3]> {
    let mut out: Vec<[u8; 3]> = MODE_BUTTONS
        .iter()
        .enumerate()
        .map(|(i, &n)| [0x90 | deck, n, if i == selected { 0x7F } else { 0x00 }])
        .collect();
    for status in PAD_STATUS[deck as usize] {
        for (i, base) in MODE_PAD_BASES.iter().enumerate() {
            for pad in 0..8 {
                out.push([status, base + pad, if i == selected { 0x7F } else { 0x00 }]);
            }
        }
    }
    out
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let host_leds = args.iter().any(|a| a == "--host-leds");
    let log_path = args.into_iter().find(|a| !a.starts_with("--"));
    let input = MidiInput::new("deckforge-monitor").expect("MIDI unavailable");
    let port = input
        .ports()
        .into_iter()
        .find(|p| input.port_name(p).is_ok_and(|n| is_flx4(&n)))
        .expect("no DDJ-FLX4 MIDI input found");
    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    let _conn = input
        .connect(
            &port,
            "deckforge-monitor-in",
            move |_, bytes, _| {
                let _ = tx.send(bytes.to_vec());
            },
            (),
        )
        .expect("could not open the FLX4 input");

    let output = MidiOutput::new("deckforge-monitor").expect("MIDI unavailable");
    let mut out = output
        .ports()
        .into_iter()
        .find(|p| output.port_name(p).is_ok_and(|n| is_flx4(&n)))
        .and_then(|p| output.connect(&p, "deckforge-monitor-out").ok());

    let mut log = log_path.map(|p| OpenOptions::new().create(true).append(true).open(p).expect("cannot open log file"));
    if let (true, Some(o)) = (host_leds, out.as_mut()) {
        for deck in 0..2 {
            for msg in mode_leds(deck, 0) {
                let _ = o.send(&msg);
            }
        }
    }
    let suffix = if host_leds { " (driving mode LEDs)" } else { "" };
    println!("Listening to DDJ-FLX4{suffix}. Press controls; Ctrl+C to stop.");
    let start = Instant::now();
    let mut last_keepalive = Instant::now() - Duration::from_secs(1);
    loop {
        if last_keepalive.elapsed() >= Duration::from_millis(200) {
            if let Some(o) = out.as_mut() {
                let _ = o.send(&KEEPALIVE_SYSEX);
            }
            last_keepalive = Instant::now();
        }
        if let Ok(bytes) = rx.recv_timeout(Duration::from_millis(50)) {
            let hex: Vec<String> = bytes.iter().map(|b| format!("{b:02X}")).collect();
            let mode_press = host_leds && bytes.len() == 3 && bytes[2] > 0 && (bytes[0] == 0x90 || bytes[0] == 0x91);
            let selected = MODE_BUTTONS.iter().position(|&n| bytes.get(1) == Some(&n));
            if let (true, Some(selected), Some(o)) = (mode_press, selected, out.as_mut()) {
                for msg in mode_leds(bytes[0] & 0x0F, selected) {
                    let _ = o.send(&msg);
                }
            }
            let line = format!("{:>9.3}s  {:<10} {}", start.elapsed().as_secs_f64(), hex.join(" "), describe(&bytes));
            println!("{line}");
            if let Some(f) = log.as_mut() {
                let _ = writeln!(f, "{line}");
            }
        }
    }
}
