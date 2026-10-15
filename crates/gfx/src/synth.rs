//! Deterministic synthetic waveforms and decks for tests, benches and screenshots.

use crate::scene::{DeckView, GridView, HotCueView, LoopView, PhraseKind, PhraseView};
use crate::waveform::WaveformData;

fn noise(i: u64) -> f64 {
    let mut x = i.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5_4A32_D192_ED03;
    x ^= x >> 33;
    x = x.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    x ^= x >> 33;
    (x >> 11) as f64 / (1u64 << 53) as f64
}

/// A dance-track-like envelope: kick on every beat, sustained mids in the drop, hats on offbeats, a breakdown.
pub fn synth_track(duration_secs: f64, bpm: f64) -> WaveformData {
    let pps = 150.0;
    let n = (duration_secs * pps) as usize;
    let beat = 60.0 / bpm;
    let section = |t: f64| -> (f64, f64, f64) {
        let bar = (t / (beat * 4.0)) as usize;
        match (bar / 8) % 6 {
            0 => (0.55, 0.25, 0.15),
            1 => (0.85, 0.5, 0.45),
            2 => (1.0, 0.85, 0.8),
            3 => (0.2, 0.55, 0.3),
            4 => (1.0, 0.9, 0.85),
            _ => (0.6, 0.35, 0.25),
        }
    };
    let (mut peak, mut low, mut mid, mut high) =
        (vec![0u8; n], vec![0u8; n], vec![0u8; n], vec![0u8; n]);
    for i in 0..n {
        let t = i as f64 / pps;
        let phase = (t / beat).fract();
        let (kick_gain, mid_gain, hat_gain) = section(t);
        let kick = (-phase * 7.0).exp() * kick_gain;
        let offbeat = ((t / beat + 0.5).fract() * 2.0).min(1.0);
        let hat = (-(1.0 - offbeat) * 9.0).exp() * hat_gain * (0.6 + 0.4 * noise(i as u64));
        let bar_pos = (t / (beat * 4.0)).fract();
        let pad = mid_gain
            * (0.55 + 0.35 * (bar_pos * std::f64::consts::TAU).sin().abs())
            * (0.85 + 0.15 * noise(i as u64 + 7));
        let height = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        let l = (kick * 0.95 + 0.08 * pad).min(1.0);
        let m = (pad * 0.7 + kick * 0.15).min(1.0);
        let h = (hat * 0.55 + 0.05 * pad).min(1.0);
        low[i] = height(l);
        mid[i] = height(m);
        high[i] = height(h);
        peak[i] = height((l + m * 0.6 + h * 0.4).min(1.0));
    }
    WaveformData {
        points_per_sec: pps,
        peak,
        low,
        mid,
        high,
    }
}

pub fn synth_deck(pos_secs: f64, duration_secs: f64, bpm: f64, token: u64) -> DeckView {
    let mut deck = DeckView {
        loaded: true,
        token,
        pos_secs,
        duration_secs,
        playing: true,
        cue_secs: 1.2,
        ..Default::default()
    };
    deck.grid = Some(GridView {
        bpm,
        first_beat_secs: 0.18,
    });
    let beat = 60.0 / bpm;
    let bar = beat * 4.0;
    deck.hot_cues[0] = Some(HotCueView {
        secs: 0.18,
        rgb: 0xFF3B5C,
    });
    deck.hot_cues[1] = Some(HotCueView {
        secs: bar * 16.0,
        rgb: 0xFF8A00,
    });
    deck.hot_cues[2] = Some(HotCueView {
        secs: bar * 24.0,
        rgb: 0xFFD21F,
    });
    deck.hot_cues[4] = Some(HotCueView {
        secs: bar * 40.0,
        rgb: 0x1FC8FF,
    });
    deck.loop_region = Some(LoopView {
        in_secs: pos_secs + beat * 4.0,
        out_secs: pos_secs + beat * 8.0,
        active: true,
    });
    deck
}

pub fn synth_phrases(duration_secs: f64, bpm: f64) -> Vec<PhraseView> {
    let bar = 60.0 / bpm * 4.0;
    let plan = [
        (PhraseKind::Intro, 8, 0.4),
        (PhraseKind::Build, 8, 0.65),
        (PhraseKind::Drop, 16, 1.0),
        (PhraseKind::Break, 8, 0.35),
        (PhraseKind::Build, 8, 0.7),
        (PhraseKind::Drop, 16, 0.95),
        (PhraseKind::Outro, 8, 0.45),
    ];
    let mut t = 0.18;
    let mut out = Vec::new();
    for (kind, bars, energy) in plan {
        let end = (t + bars as f64 * bar).min(duration_secs);
        out.push(PhraseView {
            start_secs: t,
            end_secs: end,
            kind,
            energy,
            fill_start_secs: (kind == PhraseKind::Build).then_some(end - bar * 0.5),
        });
        t = end;
        if t >= duration_secs {
            break;
        }
    }
    out
}
