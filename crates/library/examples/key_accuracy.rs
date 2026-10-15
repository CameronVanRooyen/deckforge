use std::path::{Path, PathBuf};
use std::time::Instant;

use deckforge_analysis::{analyze_beatgrid, analyze_phrases, decode_file, detect_key, Key, Phrase};
use lofty::file::TaggedFileExt;
use lofty::tag::ItemKey;
use walkdir::WalkDir;

fn tag_key(path: &Path) -> Option<String> {
    let file = lofty::read_from_path(path).ok()?;
    let tag = file.primary_tag().or_else(|| file.first_tag())?;
    tag.get_string(ItemKey::InitialKey).map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

#[derive(Default)]
struct Tally {
    exact: usize,
    relative: usize,
    fifth: usize,
    parallel: usize,
    other: usize,
}

fn classify(detected: Key, truth: Key, tally: &mut Tally) -> &'static str {
    let (dc, tc) = (detected.camelot(), truth.camelot());
    let parse = |c: &str| -> (i32, bool) { (c[..c.len() - 1].parse().unwrap_or(0), c.ends_with('A')) };
    let ((dn, dm), (tn, tm)) = (parse(&dc), parse(&tc));
    let dist = (dn - tn).rem_euclid(12).min((tn - dn).rem_euclid(12));
    if dc == tc {
        tally.exact += 1;
        "exact"
    } else if dn == tn {
        tally.relative += 1;
        "relative"
    } else if dist == 1 && dm == tm {
        tally.fifth += 1;
        "fifth"
    } else if detected.tonic == truth.tonic {
        tally.parallel += 1;
        "parallel"
    } else {
        tally.other += 1;
        "other"
    }
}

fn print_phrases(phrases: &[Phrase]) {
    for p in phrases {
        println!("      {:>7.1}s-{:>7.1}s {:>3} bars {:<6} energy {:.2}", p.start_secs, p.end_secs, p.bars, format!("{:?}", p.kind), p.energy);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let show_phrases = args.iter().any(|a| a == "--phrases");
    let mut files: Vec<PathBuf> = Vec::new();
    for root in args.iter().filter(|a| !a.starts_with("--")) {
        files.extend(
            WalkDir::new(root)
                .follow_links(true)
                .into_iter()
                .filter_map(Result::ok)
                .map(|e| e.into_path())
                .filter(|p| p.is_file() && deckforge_analysis::decode::is_supported(p)),
        );
    }
    files.sort();
    let mut tally = Tally::default();
    let mut compared = 0;
    for path in files {
        let Ok(track) = decode_file(&path) else { continue };
        if track.duration_secs() < 60.0 {
            continue;
        }
        let started = Instant::now();
        let estimate = detect_key(&track);
        let key_ms = started.elapsed().as_millis();
        let name = path.file_name().unwrap().to_string_lossy();
        let tag = tag_key(&path);
        let verdict = match (estimate, tag.as_deref().and_then(Key::parse)) {
            (Some(e), Some(t)) => {
                compared += 1;
                classify(e.key, t, &mut tally)
            }
            _ => "-",
        };
        println!(
            "{:<8} detected {:<4} ({:>3}) conf {:.2}  tag {:<8} {key_ms:>4}ms  {name}",
            verdict,
            estimate.map_or("none", |e| e.key.name()),
            estimate.map_or(String::new(), |e| e.key.camelot()),
            estimate.map_or(0.0, |e| e.confidence),
            tag.as_deref().unwrap_or("-"),
        );
        if show_phrases && let Some(beat) = analyze_beatgrid(&track, Default::default()) {
            let t = Instant::now();
            let phrases = analyze_phrases(&track, beat.grid);
            println!("    {:.2} bpm first {:.3}s, phrases {}ms", beat.grid.bpm, beat.grid.first_beat_secs, t.elapsed().as_millis());
            print_phrases(&phrases);
        }
    }
    println!(
        "compared {compared}: exact {} relative {} fifth {} parallel {} other {}",
        tally.exact, tally.relative, tally.fifth, tally.parallel, tally.other
    );
}
