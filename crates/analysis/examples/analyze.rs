use std::path::Path;
use std::time::Instant;

fn main() {
    for arg in std::env::args().skip(1) {
        let t = Instant::now();
        let track = match deckforge_analysis::decode_file(Path::new(&arg)) {
            Ok(t) => t,
            Err(e) => {
                println!("{arg}: {e}");
                continue;
            }
        };
        let decode_ms = t.elapsed().as_millis();
        let t = Instant::now();
        let a = deckforge_analysis::analyze_beatgrid(&track, Default::default());
        let wf = deckforge_analysis::compute_waveform(&track);
        let key = deckforge_analysis::detect_key(&track);
        let loudness = deckforge_analysis::measure_loudness(&track);
        let name = Path::new(&arg).file_name().unwrap().to_string_lossy();
        match a {
            Some(a) => {
                println!(
                    "{:>7.2} bpm  first {:.3}s  conf {:.2}  key {}  {}Hz {:.0}s  decode {}ms analyze {}ms wf {}pts  {}",
                    a.grid.bpm, a.grid.first_beat_secs, a.confidence,
                    key.map_or("-".to_string(), |k| format!("{} ({}, {:.2})", k.key.camelot(), k.key.name(), k.confidence)),
                    track.sample_rate, track.duration_secs(), decode_ms, t.elapsed().as_millis(), wf.len(), name
                );
                if let Some(l) = loudness {
                    println!("      loudness {:.1} LUFS, sample peak {:.3}", l.integrated_lufs, l.sample_peak);
                }
                let Some(structure) = deckforge_analysis::analyze_phrase_structure(&track, a.grid) else { continue };
                println!(
                    "      first downbeat: grid beat {} at {:.3}s (confidence {:.2})",
                    structure.downbeat_beat,
                    a.grid.secs_of_beat(structure.downbeat_beat as f64),
                    structure.downbeat_confidence
                );
                for p in structure.phrases {
                    let fill = p.fill_start_secs.map_or(String::new(), |f| format!("  fill from {f:.1}s"));
                    println!("      {:>7.1}s-{:>7.1}s {:>3} bars {:<6} energy {:.2}{fill}", p.start_secs, p.end_secs, p.bars, p.kind.as_str(), p.energy);
                }
            }
            None => println!("no grid: {name}"),
        }
    }
}
