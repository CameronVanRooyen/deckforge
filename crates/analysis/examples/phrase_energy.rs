use std::path::Path;

use deckforge_analysis::{analyze_beatgrid, decode_file};

const SPAN: usize = 4;
const SEARCH: i64 = 8;
const EDGE_MIN: f64 = 12.0;

fn db(x: f64) -> f64 {
    10.0 * x.max(1e-12).log10()
}

fn one_pole(fc: f64, sr: f64) -> f32 {
    (-std::f64::consts::TAU * fc / sr).exp() as f32
}

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("audio path");
    let boundaries: Vec<f64> = args.filter_map(|a| a.parse().ok()).collect();
    let track = decode_file(Path::new(&path)).expect("decode");
    let grid = analyze_beatgrid(&track, Default::default()).expect("grid").grid;
    let sr = f64::from(track.sample_rate);
    let beat = grid.beat_secs();
    let offset = grid.first_beat_secs - (grid.first_beat_secs / beat).floor() * beat;
    let beats = ((track.duration_secs() - offset) / beat).floor() as usize;
    let a = one_pole(150.0, sr);
    let (mut l1, mut l2) = (0.0f32, 0.0f32);
    let (mut total, mut low) = (vec![0.0; beats], vec![0.0; beats]);
    let mut cursor = 0usize;
    for b in 0..beats {
        let end = (((offset + (b + 1) as f64 * beat) * sr) as usize).min(track.frames());
        let start = ((offset + b as f64 * beat) * sr) as usize;
        let (mut t, mut lo, mut n) = (0.0f64, 0.0f64, 0.0f64);
        for i in cursor.min(start)..end {
            let (l, r) = track.frame(i);
            let x = 0.5 * (l + r);
            l1 = x + a * (l1 - x);
            l2 = l1 + a * (l2 - l1);
            if i >= start {
                t += f64::from(x * x);
                lo += f64::from(l2 * l2);
                n += 1.0;
            }
        }
        cursor = end;
        total[b] = db(t / n.max(1.0));
        low[b] = db(lo / n.max(1.0));
    }
    let mean = |v: &[f64], r: std::ops::Range<usize>| v[r.clone()].iter().sum::<f64>() / r.len() as f64;
    let delta = |v: &[f64], q: usize| mean(v, q..q + SPAN) - mean(v, q - SPAN..q);
    let edge = |q: usize| delta(&total, q).abs() + delta(&low, q).abs();
    if boundaries.is_empty() {
        let step: usize = std::env::var("STEP").ok().and_then(|s| s.parse().ok()).unwrap_or(SPAN);
        for b in (0..beats).step_by(step) {
            let end = (b + step).min(beats);
            println!("beat {b:5} {:7.1}s  tot {:6.1}  low {:6.1}", offset + b as f64 * beat, mean(&total, b..end), mean(&low, b..end));
        }
    }
    for &t in &boundaries {
        let q = ((t - offset) / beat).round() as i64;
        if q < SEARCH + SPAN as i64 || q + SEARCH + SPAN as i64 >= beats as i64 {
            println!("{t:8.2}s beat {q:5}  (edge of track)");
            continue;
        }
        let best = (q - SEARCH..=q + SEARCH).max_by(|&x, &y| edge(x as usize).total_cmp(&edge(y as usize))).unwrap();
        let qq = q as usize;
        println!(
            "{t:8.2}s beat {q:5}  dTot {:+6.1} dB dLow {:+6.1} dB  edge {:5.1}  best edge {:5.1} at {:+} beats  ratio {:.2}",
            delta(&total, qq),
            delta(&low, qq),
            edge(qq),
            edge(best as usize),
            best - q,
            edge(qq) / edge(best as usize).max(1e-9)
        );
    }
    let nearest = |q: i64| boundaries.iter().map(|&t| ((t - offset) / beat).round() as i64 - q).min_by_key(|d| d.abs());
    let mut found = [0usize; 4];
    let mut total_edges = 0usize;
    let lo = (SEARCH + SPAN as i64) as usize;
    for q in lo..beats.saturating_sub(lo) {
        let e = edge(q);
        let is_peak = e >= EDGE_MIN && (q - 8..=q + 8).all(|k| edge(k) <= e);
        if !is_peak {
            continue;
        }
        total_edges += 1;
        let d = nearest(q as i64).map_or(i64::MAX, i64::abs);
        println!("TRUE edge beat {q:5} {:7.1}s strength {e:5.1} nearest boundary {d} beats away", offset + q as f64 * beat);
        for (slot, limit) in found.iter_mut().zip([0, 1, 2, 8]) {
            *slot += usize::from(d <= limit);
        }
    }
    println!("EDGES total {total_edges} exact {} within1 {} within2 {} within8 {}", found[0], found[1], found[2], found[3]);
}
