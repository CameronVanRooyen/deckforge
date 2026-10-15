mod common;

use common::*;
use deckforge_analysis::{detect_key, Key};

const MAJOR_PROGRESSION: [(usize, &str); 4] = [(0, "maj"), (5, "maj"), (7, "maj"), (0, "maj")];
const MINOR_PROGRESSION: [(usize, &str); 4] = [(0, "min"), (5, "min"), (7, "maj"), (0, "min")];

fn chord_semitones(quality: &str) -> [i32; 3] {
    if quality == "maj" { [0, 4, 7] } else { [0, 3, 7] }
}

fn song(tonic: u8, minor: bool, cents: f64, with_drums: bool) -> deckforge_common::TrackAudio {
    let sr = 22_050;
    let secs = 48.0;
    let mut buf = vec![0f32; (secs * f64::from(sr)) as usize];
    let mut noise = Noise::new(7);
    let progression = if minor { MINOR_PROGRESSION } else { MAJOR_PROGRESSION };
    let detune = 2f64.powf(cents / 1200.0);
    let chord_secs = 2.4;
    let hz = |midi: i32| 440.0 * 2f64.powf(f64::from(midi - 69) / 12.0) * detune;
    let mut t = 0.0;
    let mut i = 0;
    while t < secs {
        let (degree, quality) = progression[i % progression.len()];
        let root = i32::from(tonic) + degree as i32;
        for semis in chord_semitones(quality) {
            add_tone(&mut buf, sr, hz(48 + root + semis), t, t + chord_secs, 0.12, 6);
        }
        add_tone(&mut buf, sr, hz(36 + root), t, t + chord_secs, 0.2, 3);
        t += chord_secs;
        i += 1;
    }
    if with_drums {
        let mut beat = 0.0;
        while beat < secs {
            add_kick(&mut buf, sr, beat, 0.6);
            add_noise_burst(&mut buf, sr, beat + 0.234, 0.2, 0.03, true, &mut noise);
            beat += 0.46875;
        }
    }
    for s in buf.iter_mut() {
        *s += noise.next() * 0.02;
    }
    stereo(sr, buf)
}

fn assert_key(tonic: u8, minor: bool, camelot: &str, name: &str, cents: f64) {
    let est = detect_key(&song(tonic, minor, cents, true)).expect("key");
    assert_eq!(est.key, Key::new(tonic, minor), "{name} at {cents} cents: got {} ({})", est.key.name(), est.key.camelot());
    assert_eq!(est.key.camelot(), camelot);
    assert_eq!(est.key.name(), name);
}

#[test]
fn detects_a_minor() {
    assert_key(9, true, "8A", "Am", 0.0);
}

#[test]
fn detects_c_major() {
    assert_key(0, false, "8B", "C", 0.0);
}

#[test]
fn detects_f_sharp_minor() {
    assert_key(6, true, "11A", "F#m", 0.0);
}

#[test]
fn detects_e_flat_major() {
    assert_key(3, false, "5B", "Eb", 0.0);
}

#[test]
fn tolerates_detuning() {
    assert_key(9, true, "8A", "Am", 20.0);
    assert_key(9, true, "8A", "Am", -20.0);
    assert_key(3, false, "5B", "Eb", 20.0);
    assert_key(6, true, "11A", "F#m", -22.0);
}

#[test]
fn detects_without_drums() {
    let est = detect_key(&song(2, true, 0.0, false)).expect("key");
    assert_eq!(est.key, Key::new(2, true));
}

#[test]
fn silence_has_no_key() {
    assert!(detect_key(&stereo(22_050, vec![0.0; 22_050 * 30])).is_none());
}

#[test]
fn camelot_and_names_cover_the_wheel() {
    let majors = ["8B", "3B", "10B", "5B", "12B", "7B", "2B", "9B", "4B", "11B", "6B", "1B"];
    let minors = ["5A", "12A", "7A", "2A", "9A", "4A", "11A", "6A", "1A", "8A", "3A", "10A"];
    for t in 0..12u8 {
        assert_eq!(Key::new(t, false).camelot(), majors[usize::from(t)]);
        assert_eq!(Key::new(t, true).camelot(), minors[usize::from(t)]);
    }
}

#[test]
fn parses_tag_notations() {
    let table: &[(&str, Option<(u8, bool)>)] = &[
        ("Am", Some((9, true))),
        ("A minor", Some((9, true))),
        ("a min", Some((9, true))),
        ("Amin", Some((9, true))),
        ("A", Some((9, false))),
        ("A major", Some((9, false))),
        ("8A", Some((9, true))),
        ("08A", Some((9, true))),
        ("8a", Some((9, true))),
        ("8B", Some((0, false))),
        ("11B", Some((9, false))),
        ("Abm", Some((8, true))),
        ("G#m", Some((8, true))),
        ("G♯m", Some((8, true))),
        ("D♭", Some((1, false))),
        ("C#", Some((1, false))),
        ("Db", Some((1, false))),
        ("Bb", Some((10, false))),
        ("Bbm", Some((10, true))),
        ("F#m", Some((6, true))),
        ("Gb", Some((6, false))),
        ("Ebm", Some((3, true))),
        ("D#m", Some((3, true))),
        ("1m", Some((9, true))),
        ("1d", Some((0, false))),
        ("6d", Some((11, false))),
        ("12m", Some((2, true))),
        ("Open Key 1m", Some((9, true))),
        ("  Cm  ", Some((0, true))),
        ("13A", None),
        ("0A", None),
        ("", None),
        ("H", None),
        ("none", None),
        ("Am7", None),
    ];
    for (tag, expected) in table {
        let parsed = Key::parse(tag).map(|k| (k.tonic, k.minor));
        assert_eq!(parsed, *expected, "tag {tag:?}");
    }
}
