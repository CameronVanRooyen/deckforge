//! Read-only CoreMIDI port and message monitor for a connected DDJ-400.

use std::sync::mpsc;
use std::time::Duration;

use deckforge_controller::service::describe;
use midir::{MidiInput, MidiOutput};

fn main() {
    let input = MidiInput::new("deckforge-ddj400-monitor").expect("MIDI unavailable");
    let inputs = input.ports();
    let mut selected = None;
    println!("MIDI inputs:");
    for port in &inputs {
        let name = input.port_name(port).unwrap_or_else(|_| "<unknown>".into());
        println!("  {name}");
        if name.to_ascii_uppercase().contains("DDJ-400") {
            selected = Some((port.clone(), name));
        }
    }

    let output = MidiOutput::new("deckforge-ddj400-monitor").expect("MIDI unavailable");
    println!("MIDI outputs:");
    for port in output.ports() {
        println!("  {}", output.port_name(&port).unwrap_or_else(|_| "<unknown>".into()));
    }

    let Some((port, name)) = selected else {
        eprintln!("No DDJ-400 MIDI input found.");
        std::process::exit(1);
    };
    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    let _connection = input
        .connect(&port, "deckforge-ddj400-monitor-in", move |_, bytes, _| {
            let _ = tx.send(bytes.to_vec());
        }, ())
        .expect("could not open DDJ-400 MIDI input");
    println!("Listening on {name}; press Ctrl-C to stop. This monitor sends no MIDI output.");

    loop {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(bytes) => {
                let hex = bytes.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ");
                println!("{hex:<18} {}", describe(&bytes));
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}
