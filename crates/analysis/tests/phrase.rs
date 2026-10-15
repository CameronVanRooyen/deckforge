mod common;

use common::*;
use deckforge_analysis::{analyze_phrase_structure, analyze_phrases, analyze_phrases_with_downbeat, Phrase, PhraseKind};
use deckforge_common::{Beatgrid, TrackAudio};

const BPM: f64 = 128.0;
const SR: u32 = 22_050;

#[derive(Clone, Copy)]
struct Section {
    beats: usize,
    kick: bool,
    bass: bool,
    hats: bool,
    pad: bool,
    sweep: bool,
}

const fn bars(n: usize) -> usize {
    n * 4
}

const INTRO: Section = Section { beats: bars(16), kick: false, bass: false, hats: true, pad: false, sweep: false };
const DROP: Section = Section { beats: bars(32), kick: true, bass: true, hats: true, pad: false, sweep: false };
const BREAK: Section = Section { beats: bars(16), kick: false, bass: false, hats: false, pad: true, sweep: false };
const BUILD: Section = Section { beats: bars(8), kick: false, bass: false, hats: false, pad: false, sweep: true };
const OUTRO: Section = Section { beats: bars(16), kick: true, bass: false, hats: true, pad: false, sweep: false };
const FILL: Section = Section { beats: bars(2), ..BUILD };
const PICKUP: Section = Section { beats: 2, ..BUILD };

fn render(sections: &[Section], lead_in_beats: usize) -> TrackAudio {
    let beat = 60.0 / BPM;
    let total_beats: usize = sections.iter().map(|s| s.beats).sum();
    let secs = (total_beats + lead_in_beats) as f64 * beat;
    let mut buf = vec![0f32; (secs * f64::from(SR)) as usize];
    let mut noise = Noise::new(99);
    let mut beat_index = lead_in_beats;
    for s in sections {
        for k in 0..s.beats {
            let tb = (beat_index + k) as f64 * beat;
            if s.kick {
                add_kick(&mut buf, SR, tb, 0.7);
            }
            if s.hats {
                add_noise_burst(&mut buf, SR, tb + beat * 0.5, 0.18, 0.025, true, &mut noise);
                if k % 2 == 1 {
                    add_noise_burst(&mut buf, SR, tb, 0.12, 0.05, false, &mut noise);
                }
            }
            if s.bass {
                add_tone(&mut buf, SR, 55.0, tb + beat * 0.5, tb + beat * 0.95, 0.35, 2);
            }
            if s.pad && k % 4 == 0 {
                let end = tb + beat * (s.beats - k).min(4) as f64;
                for f in [220.0, 261.63, 329.63, 440.0] {
                    add_tone(&mut buf, SR, f, tb, end, 0.04, 3);
                }
            }
            if s.sweep {
                let start = (tb * f64::from(SR)) as usize;
                let n = (beat * f64::from(SR)) as usize;
                let mut prev = 0.0f32;
                for i in 0..n {
                    let progress = (k as f32 + i as f32 / n as f32) / s.beats as f32;
                    let x = noise.next();
                    if let Some(slot) = buf.get_mut(start + i) {
                        *slot += (x - prev) * 0.5 * progress * progress;
                    }
                    prev = x;
                }
            }
        }
        beat_index += s.beats;
    }
    stereo(SR, buf)
}

fn full_structure() -> Vec<Section> {
    vec![INTRO, DROP, BREAK, BUILD, DROP, OUTRO]
}

#[test]
fn labels_and_aligns_a_full_arrangement() {
    let sections = full_structure();
    let track = render(&sections, 0);
    let grid = Beatgrid { bpm: BPM, first_beat_secs: 0.0 };
    let phrases = analyze_phrases(&track, grid);
    let bar = 60.0 / BPM * 4.0;
    let mut kinds: Vec<PhraseKind> = phrases.iter().map(|p| p.kind).collect();
    kinds.dedup();
    assert_eq!(
        kinds,
        [PhraseKind::Intro, PhraseKind::Drop, PhraseKind::Break, PhraseKind::Build, PhraseKind::Drop, PhraseKind::Outro],
        "layout: {phrases:?}"
    );
    let mut section_bar = 0usize;
    for s in &sections {
        let starts_here = phrases.iter().any(|p| (p.start_secs / bar - section_bar as f64).abs() < 1e-6);
        assert!(starts_here, "no phrase starts at section bar {section_bar}; layout: {phrases:?}");
        section_bar += s.beats / 4;
    }
    for p in &phrases[..phrases.len() - 1] {
        assert!(((p.end_secs - p.start_secs) / bar - f64::from(p.bars)).abs() < 1e-6, "{p:?} is not a whole number of bars");
    }
    assert!((phrases.last().unwrap().end_secs - track.duration_secs()).abs() < 1e-6);
    for pair in phrases.windows(2) {
        assert!((pair[0].end_secs - pair[1].start_secs).abs() < 1e-9, "phrases must be contiguous");
    }
    assert!(phrases.iter().all(|p| (0.0..=1.0).contains(&p.energy)));
    let drop = phrases.iter().find(|p| p.kind == PhraseKind::Drop).unwrap();
    let intro = &phrases[0];
    let brk = phrases.iter().find(|p| p.kind == PhraseKind::Break).unwrap();
    assert!((drop.energy - 1.0).abs() < 0.2, "drop energy {}", drop.energy);
    assert!(intro.energy < drop.energy && brk.energy < drop.energy);
}

#[test]
fn estimates_downbeat_when_grid_starts_on_beat_three() {
    let track = render(&full_structure(), 0);
    let period = 60.0 / BPM;
    let grid = Beatgrid { bpm: BPM, first_beat_secs: 2.0 * period };
    let analysis = analyze_phrase_structure(&track, grid).unwrap();
    assert_eq!(analysis.downbeat_beat.rem_euclid(4), 2, "downbeat beat {}", analysis.downbeat_beat);
    let first = analysis.phrases[0].start_secs;
    let bar = period * 4.0;
    let off = (first / bar).round() * bar - first;
    assert!(off.abs() < 0.05, "first phrase should sit on a true bar line, off by {off}");
    assert_eq!(analysis.phrases[0].kind, PhraseKind::Intro);
}

#[test]
fn estimates_downbeat_for_each_grid_phase() {
    let period = 60.0 / BPM;
    let track = render(&full_structure(), 0);
    for beat_offset in 0..4i64 {
        let grid = Beatgrid { bpm: BPM, first_beat_secs: beat_offset as f64 * period };
        let a = analyze_phrase_structure(&track, grid).unwrap();
        assert_eq!(a.downbeat_beat.rem_euclid(4), (4 - beat_offset) % 4, "offset {beat_offset}: {}", a.downbeat_beat);
    }
}

#[test]
fn handles_lead_in_before_first_bar() {
    let track = render(&full_structure(), 2);
    let period = 60.0 / BPM;
    let grid = Beatgrid { bpm: BPM, first_beat_secs: 0.0 };
    let analysis = analyze_phrase_structure(&track, grid).unwrap();
    assert_eq!(analysis.downbeat_beat.rem_euclid(4), 2);
    assert!((analysis.phrases[0].start_secs - 2.0 * period).abs() < 0.05);
}

#[test]
fn no_valid_grid_gives_no_phrases() {
    let track = render(&[INTRO], 0);
    assert!(analyze_phrases(&track, Beatgrid { bpm: 0.0, first_beat_secs: 0.0 }).is_empty());
    assert!(analyze_phrases(&stereo(SR, vec![0.0; 1000]), Beatgrid { bpm: 128.0, first_beat_secs: 0.0 }).is_empty());
}

#[test]
fn silence_is_a_single_phrase_or_none() {
    let track = stereo(SR, vec![0.0; SR as usize * 60]);
    let phrases = analyze_phrases(&track, Beatgrid { bpm: 128.0, first_beat_secs: 0.0 });
    assert!(phrases.len() <= 1);
}

fn odd_structure() -> Vec<Section> {
    vec![INTRO, FILL, DROP, BREAK, PICKUP, DROP, OUTRO]
}

fn beat_of(secs: f64) -> f64 {
    secs * BPM / 60.0
}

fn assert_on_downbeats(phrases: &[Phrase], downbeat_beat: f64) {
    for p in phrases {
        let off = (beat_of(p.start_secs) - downbeat_beat).rem_euclid(4.0);
        assert!(off < 1e-6 || 4.0 - off < 1e-6, "{:?} at beat {} is off the bar phase; layout: {phrases:?}", p.kind, beat_of(p.start_secs));
    }
}

#[test]
fn drops_start_on_the_downbeat_after_a_fill() {
    let track = render(&odd_structure(), 0);
    let phrases = analyze_phrases(&track, Beatgrid { bpm: BPM, first_beat_secs: 0.0 });
    let first_drop = phrases.iter().find(|p| p.kind == PhraseKind::Drop && beat_of(p.start_secs) > 64.0).unwrap();
    assert!((beat_of(first_drop.start_secs) - 72.0).abs() < 1e-6, "layout: {phrases:?}");
    let second = phrases.iter().find(|p| beat_of(p.start_secs) > 200.0 && p.kind == PhraseKind::Drop).unwrap();
    assert!((beat_of(second.start_secs) - 266.0).abs() <= 2.0, "layout: {phrases:?}");
}

#[test]
fn a_mid_track_pickup_keeps_one_global_bar_phase() {
    let track = render(&odd_structure(), 0);
    let analysis = analyze_phrase_structure(&track, Beatgrid { bpm: BPM, first_beat_secs: 0.0 }).unwrap();
    assert_eq!(analysis.downbeat_beat, 0);
    assert_on_downbeats(&analysis.phrases, 0.0);
    let last = analysis.phrases.last().unwrap();
    assert_eq!(last.kind, PhraseKind::Outro, "layout: {:?}", analysis.phrases);
    assert!((beat_of(last.start_secs) - 396.0).abs() <= 2.0, "outro at beat {}", beat_of(last.start_secs));
}

#[test]
fn a_supplied_downbeat_fixes_the_bar_phase() {
    let track = render(&full_structure(), 0);
    let grid = Beatgrid { bpm: BPM, first_beat_secs: 0.0 };
    let analysis = analyze_phrases_with_downbeat(&track, grid, Some(1)).unwrap();
    assert_eq!(analysis.downbeat_beat, 1);
    assert_on_downbeats(&analysis.phrases, 1.0);
}

#[test]
fn phrase_starts_stay_on_grid_beats() {
    let track = render(&odd_structure(), 0);
    let grid = Beatgrid { bpm: BPM, first_beat_secs: 0.0 };
    for p in analyze_phrases(&track, grid) {
        let beat = beat_of(p.start_secs);
        assert!((beat - beat.round()).abs() < 1e-6, "{:?} starts between beats at {beat}", p.kind);
    }
}

#[test]
fn drops_stay_on_their_downbeat_for_any_lead_in() {
    for lead_in in [1usize, 2, 3] {
        let track = render(&odd_structure(), lead_in);
        let grid = Beatgrid { bpm: BPM, first_beat_secs: 0.0 };
        let analysis = analyze_phrase_structure(&track, grid).unwrap();
        assert_eq!(analysis.downbeat_beat, lead_in as i64, "lead-in {lead_in}");
        assert_on_downbeats(&analysis.phrases, lead_in as f64);
        let drop = analysis.phrases.iter().find(|p| p.kind == PhraseKind::Drop && beat_of(p.start_secs) > 64.0).unwrap();
        assert!((beat_of(drop.start_secs) - (72 + lead_in) as f64).abs() < 1e-6, "lead-in {lead_in}: {:?}", analysis.phrases);
    }
}

#[test]
fn a_two_bar_fill_keeps_the_following_drop_whole() {
    let track = render(&[INTRO, FILL, DROP, OUTRO], 0);
    let phrases = analyze_phrases(&track, Beatgrid { bpm: BPM, first_beat_secs: 0.0 });
    let drop = phrases.iter().find(|p| (beat_of(p.start_secs) - 72.0).abs() < 1e-6).expect("phrase at the drop");
    assert_eq!((drop.kind, drop.bars), (PhraseKind::Drop, 32), "{phrases:?}");
}
