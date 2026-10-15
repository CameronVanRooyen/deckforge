//! Phrase-relative navigation: bar levels, countdowns and two-deck alignment. Mirrors `apps/desktop/src/lib/waveform/phraseNav.ts`.

use crate::bars::{GridLine, LineKind};
use crate::scene::{GridView, PhraseView};

const BEATS_PER_BAR: f64 = 4.0;
const SNAP_BARS: f64 = 0.05;
const EDGE_SECS: f64 = 1e-6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BarLevel {
    Bar,
    Four,
    Eight,
    Sixteen,
    Phrase,
}

/// A bar line placed inside its phrase: `bar_in_phrase` is 1-based, `phrase_bars` is 0 outside any phrase.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BarMark {
    pub level: BarLevel,
    pub bar_in_phrase: i64,
    pub phrase_bars: i64,
    pub phrase: Option<usize>,
}

pub fn bar_secs(grid: GridView) -> f64 {
    60.0 / grid.bpm * BEATS_PER_BAR
}

/// Length in bars, rounded when within a twentieth of a bar of a whole number.
pub fn phrase_bars(p: &PhraseView, grid: GridView) -> (i64, bool) {
    let bars = (p.end_secs - p.start_secs) / bar_secs(grid);
    let whole = (bars - bars.round()).abs() < SNAP_BARS;
    (if whole { bars.round() } else { bars.ceil() } as i64, whole)
}

pub fn phrase_at(phrases: &[PhraseView], secs: f64) -> Option<usize> {
    phrases
        .iter()
        .rposition(|p| secs + EDGE_SECS >= p.start_secs && secs < p.end_secs - EDGE_SECS)
}

pub fn next_phrase(phrases: &[PhraseView], secs: f64) -> Option<usize> {
    phrases.iter().position(|p| p.start_secs > secs + EDGE_SECS)
}

/// Classifies a non-beat grid line by its position inside the containing phrase.
pub fn classify(line: &GridLine, phrases: &[PhraseView], grid: GridView) -> BarMark {
    let bar = bar_secs(grid);
    if line.kind == LineKind::Phrase
        && let Some(i) = line.phrase
        && let Some(p) = phrases.get(i)
    {
        return BarMark {
            level: BarLevel::Phrase,
            bar_in_phrase: 1,
            phrase_bars: phrase_bars(p, grid).0,
            phrase: Some(i),
        };
    }
    let (k, len, phrase, next_start) = match phrase_at(phrases, line.secs) {
        Some(i) => {
            let p = &phrases[i];
            let k = ((line.secs - p.start_secs) / bar + SNAP_BARS).floor() as i64;
            (k, phrase_bars(p, grid).0, Some(i), p.end_secs)
        }
        None => (
            line.bar - 1,
            0,
            None,
            next_phrase(phrases, line.secs).map_or(f64::INFINITY, |n| phrases[n].start_secs),
        ),
    };
    let crowded = next_start - line.secs < bar * (1.0 - SNAP_BARS);
    let level = if k <= 0 || crowded {
        BarLevel::Bar
    } else if k % 16 == 0 && (len == 0 || len > 16) {
        BarLevel::Sixteen
    } else if k % 8 == 0 && (len == 0 || len > 8) {
        BarLevel::Eight
    } else if k % 4 == 0 {
        BarLevel::Four
    } else {
        BarLevel::Bar
    };
    BarMark {
        level,
        bar_in_phrase: k + 1,
        phrase_bars: len,
        phrase,
    }
}

/// Where the playhead sits relative to the phrase it is in and the phrase that comes next.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Countdown {
    pub current: Option<usize>,
    pub next: usize,
    /// Beats from the position to the next phrase start.
    pub beats: f64,
    /// Whole beats still to play before the change, shown as `bars.beats`.
    pub remaining_beats: i64,
    /// 0..1 through the current phrase, 0 before the first phrase.
    pub progress: f64,
}

impl Countdown {
    pub fn bars_beats(&self) -> (i64, i64) {
        (self.remaining_beats / 4, self.remaining_beats % 4)
    }

    pub fn final_bar(&self) -> bool {
        self.remaining_beats <= 4
    }
}

pub fn countdown(phrases: &[PhraseView], grid: GridView, secs: f64) -> Option<Countdown> {
    if grid.bpm.is_nan() || grid.bpm <= 0.0 {
        return None;
    }
    let next = next_phrase(phrases, secs)?;
    let beat = 60.0 / grid.bpm;
    let beats = (phrases[next].start_secs - secs) / beat;
    let current = phrase_at(phrases, secs);
    let progress = current.map_or(0.0, |i| {
        let p = &phrases[i];
        ((secs - p.start_secs) / (p.end_secs - p.start_secs).max(1e-9)).clamp(0.0, 1.0)
    });
    Some(Countdown {
        current,
        next,
        beats,
        remaining_beats: (beats - 1e-6).ceil().max(0.0) as i64,
        progress,
    })
}

/// Wall-clock seconds until a deck reaches its next phrase start at its current playback rate.
pub fn wall_secs_to_next(phrases: &[PhraseView], pos: f64, rate: f64) -> Option<(usize, f64)> {
    let next = next_phrase(phrases, pos)?;
    Some((next, (phrases[next].start_secs - pos) / rate.max(0.05)))
}

#[cfg(test)]
pub(crate) mod fixtures {
    use super::*;
    use crate::scene::PhraseKind;
    use serde::Deserialize;

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct RawPhrase {
        pub start_secs: f64,
        pub end_secs: f64,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Mark {
        pub secs: f64,
        pub bar: i64,
        pub kind: String,
        pub level: String,
        pub bar_in_phrase: i64,
        pub phrase_bars: i64,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Count {
        pub secs: f64,
        pub next: usize,
        pub remaining_beats: i64,
        pub progress: f64,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Case {
        pub name: String,
        pub bpm: f64,
        pub first_beat_secs: f64,
        pub phrases: Vec<RawPhrase>,
        pub marks: Vec<Mark>,
        pub countdowns: Vec<Count>,
    }

    #[derive(Deserialize)]
    struct Raw {
        cases: Vec<Case>,
    }

    pub fn load() -> Vec<Case> {
        serde_json::from_str::<Raw>(include_str!("../fixtures/phrase_nav.json"))
            .unwrap()
            .cases
    }

    pub fn views(c: &Case) -> (GridView, Vec<PhraseView>) {
        let grid = GridView {
            bpm: c.bpm,
            first_beat_secs: c.first_beat_secs,
        };
        let phrases = c
            .phrases
            .iter()
            .map(|p| PhraseView {
                start_secs: p.start_secs,
                end_secs: p.end_secs,
                kind: PhraseKind::Drop,
                energy: 1.0,
                fill_start_secs: None,
            })
            .collect();
        (grid, phrases)
    }

    pub fn level_name(l: BarLevel) -> &'static str {
        match l {
            BarLevel::Bar => "bar",
            BarLevel::Four => "four",
            BarLevel::Eight => "eight",
            BarLevel::Sixteen => "sixteen",
            BarLevel::Phrase => "phrase",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::PhraseKind;

    const GRID: GridView = GridView {
        bpm: 120.0,
        first_beat_secs: 0.0,
    };
    const BAR: f64 = 2.0;

    fn arrangement() -> Vec<PhraseView> {
        [(0.0, 8.0), (8.0, 40.0), (40.0, 46.0), (46.0, 62.0)]
            .iter()
            .map(|(a, b)| PhraseView {
                start_secs: a * BAR,
                end_secs: b * BAR,
                kind: PhraseKind::Drop,
                energy: 1.0,
                fill_start_secs: None,
            })
            .collect()
    }

    fn line(secs: f64, kind: LineKind, phrase: Option<usize>) -> GridLine {
        GridLine {
            secs,
            kind,
            bar: 0,
            phrase,
        }
    }

    #[test]
    fn levels_count_from_the_phrase_start_inside_long_phrases() {
        let ps = arrangement();
        let at = |bar: f64| classify(&line(bar * BAR, LineKind::Bar, None), &ps, GRID);
        assert_eq!(at(12.0).level, BarLevel::Four);
        assert_eq!(at(16.0).level, BarLevel::Eight);
        assert_eq!(at(24.0).level, BarLevel::Sixteen);
        assert_eq!(at(13.0).level, BarLevel::Bar);
        assert_eq!((at(24.0).bar_in_phrase, at(24.0).phrase_bars), (17, 32));
        let start = classify(&line(8.0 * BAR, LineKind::Phrase, Some(1)), &ps, GRID);
        assert_eq!((start.level, start.phrase_bars), (BarLevel::Phrase, 32));
    }

    #[test]
    fn an_eight_bar_phrase_has_no_eight_bar_level_and_short_phrases_stay_plain() {
        let ps = arrangement();
        assert_eq!(
            classify(&line(4.0 * BAR, LineKind::Four, None), &ps, GRID).level,
            BarLevel::Four
        );
        let six = classify(&line(44.0 * BAR, LineKind::Four, None), &ps, GRID);
        assert_eq!(
            (six.level, six.bar_in_phrase, six.phrase_bars),
            (BarLevel::Four, 5, 6)
        );
    }

    #[test]
    fn countdown_reads_bars_and_beats_to_the_next_phrase() {
        let ps = arrangement();
        let c = countdown(&ps, GRID, 38.0 * BAR - 0.25).unwrap();
        assert_eq!(c.next, 2);
        assert_eq!(c.bars_beats(), (2, 1));
        assert!(!c.final_bar());
        let last = countdown(&ps, GRID, 39.5 * BAR).unwrap();
        assert_eq!((last.bars_beats(), last.final_bar()), ((0, 2), true));
        assert!((last.progress - 31.5 / 32.0).abs() < 1e-9);
        assert!(countdown(&ps, GRID, 50.0 * BAR).is_none());
    }

    #[test]
    fn wall_time_scales_with_the_playback_rate() {
        let ps = arrangement();
        let (i, t) = wall_secs_to_next(&ps, 36.0 * BAR, 0.5).unwrap();
        assert_eq!(i, 2);
        assert!((t - 16.0).abs() < 1e-9);
    }

    #[test]
    fn matches_the_shared_fixtures_used_by_the_typescript_helper() {
        for c in fixtures::load() {
            let (grid, phrases) = fixtures::views(&c);
            for m in &c.marks {
                let kind = match m.kind.as_str() {
                    "phrase" => LineKind::Phrase,
                    "four" => LineKind::Four,
                    _ => LineKind::Bar,
                };
                let phrase = (kind == LineKind::Phrase)
                    .then(|| phrase_at(&phrases, m.secs))
                    .flatten();
                let got = classify(
                    &GridLine {
                        secs: m.secs,
                        kind,
                        bar: m.bar,
                        phrase,
                    },
                    &phrases,
                    grid,
                );
                assert_eq!(
                    (
                        fixtures::level_name(got.level),
                        got.bar_in_phrase,
                        got.phrase_bars
                    ),
                    (m.level.as_str(), m.bar_in_phrase, m.phrase_bars),
                    "{} @{}",
                    c.name,
                    m.secs
                );
            }
            for e in &c.countdowns {
                let got = countdown(&phrases, grid, e.secs).unwrap();
                assert_eq!(
                    (got.next, got.remaining_beats),
                    (e.next, e.remaining_beats),
                    "{} @{}",
                    c.name,
                    e.secs
                );
                assert!((got.progress - e.progress).abs() < 1e-9);
            }
        }
    }
}
