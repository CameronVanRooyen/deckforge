//! Bar lines on one global bar phase (grid beat 0 is a downbeat); 4-bar groups count from each phrase.
//! Mirrors `apps/desktop/src/lib/waveform/bars.ts` (shared fixtures keep them equal).

use crate::scene::{GridView, PhraseView};

const BEATS_PER_BAR: f64 = 4.0;
const FOUR_BAR_BEATS: f64 = 16.0;
const EDGE_BEATS: f64 = 0.02;
const TICK_MERGE_BEATS: f64 = 0.1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    Phrase,
    Four,
    Bar,
    Beat,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridLine {
    pub secs: f64,
    pub kind: LineKind,
    pub bar: i64,
    pub phrase: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BarPosition {
    pub bar: i64,
    pub beat: i64,
    pub fraction: f64,
    pub to_four_bar: i64,
    pub beats_since_anchor: f64,
}

fn valid_grid(grid: GridView) -> bool {
    grid.bpm.is_finite() && grid.bpm > 0.0 && grid.first_beat_secs.is_finite()
}

/// Global bar lattice: bar `k` starts at `origin + k * bar`, and bar 0 is the first one at or after 0 s.
struct Lattice {
    origin: f64,
    beat: f64,
    bar: f64,
}

impl Lattice {
    fn new(grid: GridView) -> Self {
        let beat = 60.0 / grid.bpm;
        let bar = beat * BEATS_PER_BAR;
        let edge = EDGE_BEATS * beat;
        let origin = grid.first_beat_secs - ((grid.first_beat_secs + edge) / bar).floor() * bar;
        Self { origin, beat, bar }
    }

    fn secs(&self, k: i64) -> f64 {
        self.origin + k as f64 * self.bar
    }

    /// Index of the first bar line at or after `secs` (within the edge tolerance).
    fn first_at_or_after(&self, secs: f64) -> i64 {
        ((secs - self.origin) / self.bar - EDGE_BEATS / BEATS_PER_BAR).ceil() as i64
    }
}

/// A run of bars grouped from one anchor bar line (a phrase start, or the first phrase for the bars before it).
struct Region {
    anchor: i64,
    next: Option<i64>,
    phrase: Option<usize>,
}

fn regions_of(phrases: &[PhraseView], lattice: &Lattice) -> Vec<Region> {
    let mut anchors: Vec<(i64, usize)> = Vec::with_capacity(phrases.len());
    for (i, p) in phrases.iter().enumerate() {
        let k = lattice.first_at_or_after(p.start_secs);
        match anchors.last_mut() {
            Some(last) if last.0 >= k => *last = (last.0, i),
            _ => anchors.push((k, i)),
        }
    }
    let Some(&(first, _)) = anchors.first() else {
        return vec![Region { anchor: 0, next: None, phrase: None }];
    };
    let mut regions = vec![Region { anchor: first, next: Some(first), phrase: None }];
    for (n, &(k, i)) in anchors.iter().enumerate() {
        regions.push(Region { anchor: k, next: anchors.get(n + 1).map(|a| a.0), phrase: Some(i) });
    }
    regions
}

/// The region a bar index belongs to: the last phrase anchored at or before it.
fn region_of_bar(regions: &[Region], k: i64) -> &Region {
    regions.iter().skip(1).rev().find(|r| k >= r.anchor).unwrap_or(&regions[0])
}

fn bar_line_kind(region: &Region, k: i64) -> LineKind {
    if region.phrase.is_some() && k == region.anchor {
        LineKind::Phrase
    } else if (k - region.anchor).rem_euclid(BEATS_PER_BAR as i64) == 0 {
        LineKind::Four
    } else {
        LineKind::Bar
    }
}

/// Bar lines and beat ticks inside `[from_secs, to_secs]`, sorted by time. Bar numbers are global (bar 1 starts at
/// the first downbeat of the track); 4-bar lines count from each phrase start.
pub fn grid_lines(phrases: &[PhraseView], grid: GridView, from_secs: f64, to_secs: f64) -> Vec<GridLine> {
    if !valid_grid(grid) || from_secs.is_nan() || to_secs.is_nan() || to_secs < from_secs {
        return Vec::new();
    }
    let lattice = Lattice::new(grid);
    let beat = lattice.beat;
    let regions = regions_of(phrases, &lattice);
    let mut lines = Vec::new();
    let k0 = ((from_secs - lattice.origin) / lattice.bar - 1e-9).ceil() as i64;
    let k1 = ((to_secs - lattice.origin) / lattice.bar + 1e-9).floor() as i64;
    for k in k0..=k1 {
        let region = region_of_bar(&regions, k);
        let kind = bar_line_kind(region, k);
        lines.push(GridLine {
            secs: lattice.secs(k),
            kind,
            bar: k + 1,
            phrase: if kind == LineKind::Phrase { region.phrase } else { None },
        });
    }
    let n0 = ((from_secs - grid.first_beat_secs) / beat - 1e-9).ceil() as i64;
    let n1 = ((to_secs - grid.first_beat_secs) / beat + 1e-9).floor() as i64;
    for n in n0..=n1 {
        let secs = grid.first_beat_secs + n as f64 * beat;
        let offset = (secs - lattice.origin) / beat;
        let nearest = offset.round();
        let on_bar_line = (offset - nearest).abs() <= TICK_MERGE_BEATS && nearest.rem_euclid(BEATS_PER_BAR) == 0.0;
        if !on_bar_line {
            lines.push(GridLine { secs, kind: LineKind::Beat, bar: 0, phrase: None });
        }
    }
    lines.sort_by(|a, b| a.secs.total_cmp(&b.secs));
    lines
}

/// Global bar and beat for a position, plus the distance to the next 4-bar line counted from the surrounding
/// phrase (or to the next phrase, if that comes first).
pub fn bar_position(phrases: &[PhraseView], grid: GridView, secs: f64) -> Option<BarPosition> {
    if !valid_grid(grid) {
        return None;
    }
    let lattice = Lattice::new(grid);
    let beat = lattice.beat;
    let regions = regions_of(phrases, &lattice);
    let bt = (secs - lattice.origin) / beat;
    let whole = (bt + 1e-9).floor();
    let region = region_of_bar(&regions, (whole / BEATS_PER_BAR).floor() as i64);
    let since = (secs - lattice.secs(region.anchor)) / beat;
    let to_multiple = (FOUR_BAR_BEATS - since.rem_euclid(FOUR_BAR_BEATS) - 1e-6).ceil();
    let to_region_end = region.next.map_or(f64::INFINITY, |n| ((lattice.secs(n) - secs) / beat - 1e-6).ceil());
    Some(BarPosition {
        bar: (whole / BEATS_PER_BAR).floor() as i64 + 1,
        beat: whole.rem_euclid(BEATS_PER_BAR) as i64 + 1,
        fraction: bt - whole,
        to_four_bar: to_multiple.min(to_region_end).max(0.0) as i64,
        beats_since_anchor: since,
    })
}

#[cfg(test)]
pub(crate) mod fixtures {
    use super::*;
    use crate::scene::PhraseKind;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct RawLine {
        secs: f64,
        kind: String,
        bar: i64,
        phrase: i64,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct RawPosition {
        secs: f64,
        bar: i64,
        beat: i64,
        to_four_bar: i64,
        beats_since_anchor: f64,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct RawPhrase {
        start_secs: f64,
        end_secs: f64,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct RawCase {
        name: String,
        bpm: f64,
        first_beat_secs: f64,
        phrases: Vec<RawPhrase>,
        from_secs: f64,
        to_secs: f64,
        lines: Vec<RawLine>,
        positions: Vec<RawPosition>,
    }

    #[derive(Deserialize)]
    struct RawFixtures {
        cases: Vec<RawCase>,
    }

    pub struct Case {
        pub name: String,
        pub grid: GridView,
        pub phrases: Vec<PhraseView>,
        pub from_secs: f64,
        pub to_secs: f64,
        pub lines: Vec<GridLine>,
        pub positions: Vec<(f64, BarPosition)>,
    }

    fn kind_of(name: &str) -> LineKind {
        match name {
            "phrase" => LineKind::Phrase,
            "four" => LineKind::Four,
            "bar" => LineKind::Bar,
            _ => LineKind::Beat,
        }
    }

    pub fn load() -> Vec<Case> {
        let raw: RawFixtures =
            serde_json::from_str(include_str!("../fixtures/bar_lines.json")).unwrap();
        raw.cases
            .into_iter()
            .map(|c| Case {
                name: c.name,
                grid: GridView {
                    bpm: c.bpm,
                    first_beat_secs: c.first_beat_secs,
                },
                phrases: c
                    .phrases
                    .iter()
                    .map(|p| PhraseView {
                        start_secs: p.start_secs,
                        end_secs: p.end_secs,
                        kind: PhraseKind::Drop,
                        energy: 1.0,
                        fill_start_secs: None,
                    })
                    .collect(),
                from_secs: c.from_secs,
                to_secs: c.to_secs,
                lines: c
                    .lines
                    .iter()
                    .map(|l| GridLine {
                        secs: l.secs,
                        kind: kind_of(&l.kind),
                        bar: l.bar,
                        phrase: usize::try_from(l.phrase).ok(),
                    })
                    .collect(),
                positions: c
                    .positions
                    .iter()
                    .map(|p| {
                        (
                            p.secs,
                            BarPosition {
                                bar: p.bar,
                                beat: p.beat,
                                fraction: 0.0,
                                to_four_bar: p.to_four_bar,
                                beats_since_anchor: p.beats_since_anchor,
                            },
                        )
                    })
                    .collect(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::PhraseKind;

    const BEAT: f64 = 60.0 / 128.0;
    const BAR: f64 = BEAT * 4.0;
    const GRID: GridView = GridView {
        bpm: 128.0,
        first_beat_secs: 0.0,
    };

    fn phrase(start: f64, end: f64) -> PhraseView {
        PhraseView {
            start_secs: start,
            end_secs: end,
            kind: PhraseKind::Drop,
            energy: 1.0,
            fill_start_secs: None,
        }
    }

    fn odd() -> Vec<PhraseView> {
        vec![
            phrase(0.0, 16.0 * BAR),
            phrase(16.0 * BAR, 18.0 * BAR),
            phrase(18.0 * BAR, 50.0 * BAR),
            phrase(50.0 * BAR, 66.0 * BAR + 2.0 * BEAT),
            phrase(66.0 * BAR + 2.0 * BEAT, 98.0 * BAR + 2.0 * BEAT),
        ]
    }

    fn bars_only(lines: Vec<GridLine>) -> Vec<GridLine> {
        lines
            .into_iter()
            .filter(|l| l.kind != LineKind::Beat)
            .collect()
    }

    #[test]
    fn phrase_start_and_sixteen_beat_multiples_are_the_strong_lines() {
        let drop = 18.0 * BAR;
        let lines = bars_only(grid_lines(&odd(), GRID, drop - 1e-6, drop + 17.0 * BAR));
        assert_eq!(lines[0].kind, LineKind::Phrase);
        assert_eq!(lines[0].phrase, Some(2));
        let fours: Vec<f64> = lines
            .iter()
            .filter(|l| l.kind == LineKind::Four)
            .map(|l| ((l.secs - drop) / BEAT).round())
            .collect();
        assert_eq!(fours, [16.0, 32.0, 48.0, 64.0]);
    }

    #[test]
    fn global_lattice_lines_become_plain_bars_inside_a_phrase() {
        let lattice = 20.0 * BAR;
        let line = grid_lines(&odd(), GRID, lattice - 1e-6, lattice + 1e-6)
            .into_iter()
            .find(|l| l.kind != LineKind::Beat)
            .unwrap();
        assert_eq!(line.kind, LineKind::Bar);
    }

    #[test]
    fn bar_numbers_are_global() {
        let lines = bars_only(grid_lines(&odd(), GRID, 14.0 * BAR, 22.0 * BAR));
        let numbers: Vec<i64> = lines.iter().map(|l| l.bar).collect();
        assert_eq!(numbers, [15, 16, 17, 18, 19, 20, 21, 22, 23]);
    }

    #[test]
    fn an_off_bar_phrase_start_does_not_shift_the_bar_phase() {
        let start = 66.0 * BAR + 2.0 * BEAT;
        let lines = bars_only(grid_lines(&odd(), GRID, start - 1e-6, start + 8.0 * BAR));
        for (i, l) in lines.iter().enumerate() {
            assert!((l.secs - (67.0 + i as f64) * BAR).abs() < 1e-9);
        }
        assert_eq!((lines[0].kind, lines[0].bar, lines[0].phrase), (LineKind::Phrase, 68, Some(4)));
    }

    #[test]
    fn a_pickup_intro_keeps_bar_one_on_the_first_downbeat() {
        let grid = GridView { bpm: 128.0, first_beat_secs: 3.0 * BEAT };
        let intro = [phrase(0.0, 3.0 * BEAT + 8.0 * BAR)];
        let lines = bars_only(grid_lines(&intro, grid, 0.0, 2.0 * BAR));
        assert_eq!((lines[0].kind, lines[0].bar), (LineKind::Phrase, 1));
        assert!((lines[0].secs - 3.0 * BEAT).abs() < 1e-9);
        let p = bar_position(&intro, grid, BEAT + 0.01).unwrap();
        assert_eq!((p.bar, p.beat), (0, 3));
    }

    #[test]
    fn position_counts_from_the_phrase_start_and_stops_at_the_next_one() {
        let p = bar_position(&odd(), GRID, 18.0 * BAR + 5.5 * BEAT).unwrap();
        assert_eq!((p.bar, p.beat, p.to_four_bar), (20, 2, 11));
        let q = bar_position(&odd(), GRID, 16.0 * BAR + 1.5 * BEAT).unwrap();
        assert_eq!((q.bar, q.beat, q.to_four_bar), (17, 2, 7));
    }

    #[test]
    fn matches_the_shared_fixtures_used_by_the_typescript_helper() {
        let cases = fixtures::load();
        assert!(!cases.is_empty());
        for c in cases {
            let got = grid_lines(&c.phrases, c.grid, c.from_secs, c.to_secs);
            assert_eq!(got.len(), c.lines.len(), "{}: line count", c.name);
            for (i, (g, e)) in got.iter().zip(&c.lines).enumerate() {
                assert!((g.secs - e.secs).abs() < 1e-9, "{} #{i}", c.name);
                assert_eq!(
                    (g.kind, g.bar, g.phrase),
                    (e.kind, e.bar, e.phrase),
                    "{} #{i}",
                    c.name
                );
            }
            for (secs, e) in &c.positions {
                let g = bar_position(&c.phrases, c.grid, *secs).unwrap();
                assert_eq!(
                    (g.bar, g.beat, g.to_four_bar),
                    (e.bar, e.beat, e.to_four_bar),
                    "{} @{secs}",
                    c.name
                );
                assert!((g.beats_since_anchor - e.beats_since_anchor).abs() < 1e-9);
            }
        }
    }
}

