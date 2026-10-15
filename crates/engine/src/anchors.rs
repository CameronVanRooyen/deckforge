use deckforge_common::Beatgrid;

const BEATS_PER_BAR: f64 = 4.0;
const FOUR_BARS: f64 = 4.0;
const EDGE_BEATS: f64 = 0.02;
const EPS: f64 = 1e-9;
const PHRASE_WINDOW: f64 = 0.5;
const MIN_LEAD_BARS: f64 = 1.0;

/// Where an Ascend ramp ends: the track time, how many bars it spans and the bar number it lands on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Landing {
    pub secs: f64,
    pub bars: f64,
    pub bar_number: u32,
}

/// Bar lines of a track: counted from each phrase start (extending back from the first one), or from the grid
/// when there are no phrases. Mirrors the bar lines the waveform draws.
#[derive(Debug, Clone, Copy)]
pub struct BarMap<'a> {
    grid: Beatgrid,
    phrases: &'a [f64],
}

impl<'a> BarMap<'a> {
    pub fn new(grid: Beatgrid, phrases: &'a [f64]) -> Self {
        Self { grid, phrases }
    }

    pub fn bar_secs(&self) -> f64 {
        self.grid.beat_secs() * BEATS_PER_BAR
    }

    fn edge(&self) -> f64 {
        EDGE_BEATS * self.grid.beat_secs()
    }

    fn region(&self, t: f64) -> usize {
        if self.phrases.is_empty() {
            return 0;
        }
        self.phrases.iter().rposition(|p| t >= p - self.edge()).unwrap_or(0)
    }

    fn anchor(&self, region: usize) -> f64 {
        self.phrases.get(region).copied().unwrap_or(self.grid.first_beat_secs)
    }

    fn first_bar(&self, region: usize) -> u32 {
        let mut bars = 0u32;
        for w in self.phrases.windows(2).take(region) {
            bars += (((w[1] - w[0]) / self.bar_secs() - EDGE_BEATS).ceil() as i64).max(1) as u32;
        }
        bars + 1
    }

    fn next_phrase(&self, region: usize) -> Option<f64> {
        self.phrases.get(region + 1).copied()
    }

    /// First bar line after `t`; a phrase start counts as the line that ends the bars before it.
    pub fn next_bar_line(&self, t: f64) -> f64 {
        let region = self.region(t);
        let anchor = self.anchor(region);
        let k = ((t - anchor) / self.bar_secs() + EPS).floor() + 1.0;
        let line = anchor + k * self.bar_secs();
        match self.next_phrase(region) {
            Some(next) if next < line - self.edge() => next,
            _ => line,
        }
    }

    /// 1-based bar number the time falls in, counting continuously across phrases.
    pub fn bar_number(&self, t: f64) -> u32 {
        let region = self.region(t);
        let k = ((t - self.anchor(region)) / self.bar_secs() + EPS).floor() as i64;
        (self.first_bar(region) as i64 + k).max(1) as u32
    }

    fn nearest_line(&self, t: f64, bars_per_line: f64) -> f64 {
        let region = self.region(t);
        let anchor = self.anchor(region);
        let step = self.bar_secs() * bars_per_line;
        anchor + ((t - anchor) / step).round() * step
    }

    /// Places the end of an `length_bars` ramp that starts at `now`: on the phrase start nearest to the nominal end
    /// (else the nearest 4-bar line) or, for bar landings, on the nearest bar line. Never closer than a bar.
    pub fn ascend_landing(&self, now: f64, length_bars: f64, on_phrase: bool) -> Landing {
        let bar = self.bar_secs();
        let nominal = now + length_bars * bar;
        let earliest = now + MIN_LEAD_BARS.max(length_bars / 4.0) * bar;
        let phrase = on_phrase
            .then(|| {
                self.phrases
                    .iter()
                    .copied()
                    .filter(|p| *p >= earliest - EPS && (p - nominal).abs() <= PHRASE_WINDOW * length_bars * bar)
                    .min_by(|a, b| (a - nominal).abs().total_cmp(&(b - nominal).abs()))
            })
            .flatten();
        let secs = phrase.unwrap_or_else(|| {
            let step = if on_phrase { FOUR_BARS } else { 1.0 };
            let mut line = self.nearest_line(nominal, step);
            while line < earliest - EPS {
                line += step * bar;
            }
            line
        });
        Landing { secs, bars: (secs - now) / bar, bar_number: self.bar_number(secs) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GRID: Beatgrid = Beatgrid { bpm: 120.0, first_beat_secs: 0.25 };

    #[test]
    fn without_phrases_bars_count_from_the_grid() {
        let map = BarMap::new(GRID, &[]);
        assert_eq!(map.bar_secs(), 2.0);
        assert!((map.next_bar_line(0.3) - 2.25).abs() < 1e-9);
        assert!((map.next_bar_line(2.25) - 4.25).abs() < 1e-9, "on a line, the next one");
        assert_eq!(map.bar_number(0.3), 1);
        assert_eq!(map.bar_number(4.3), 3);
    }

    #[test]
    fn bars_restart_on_each_phrase_start_and_the_phrase_start_is_the_next_line() {
        let phrases = [1.0, 17.0, 40.0];
        let map = BarMap::new(GRID, &phrases);
        assert!((map.next_bar_line(14.5) - 15.0).abs() < 1e-9);
        assert!((map.next_bar_line(16.2) - 17.0).abs() < 1e-9, "a phrase start cuts the bar short");
        assert!((map.next_bar_line(17.0) - 19.0).abs() < 1e-9);
        assert_eq!(map.bar_number(1.2), 1);
        assert_eq!(map.bar_number(17.2), 9);
        assert_eq!(map.bar_number(0.5), 1, "before the first phrase the bars extend backwards but never count below 1");
    }

    #[test]
    fn ascend_lands_on_the_phrase_start_nearest_the_nominal_end() {
        let phrases = [0.25, 32.25, 64.25, 96.25];
        let map = BarMap::new(GRID, &phrases);
        let from = 20.25;
        let landing = map.ascend_landing(from, 16.0, true);
        assert!((landing.secs - 64.25).abs() < 1e-9, "nominal end 52.25 s: the 64.25 s phrase is 12 s away, the 32.25 s one 20 s");
        assert!((landing.bars - (64.25 - from) / 2.0).abs() < 1e-9);
        assert_eq!(landing.bar_number, 33);
    }

    #[test]
    fn ascend_falls_back_to_four_bar_lines_without_phrases_and_to_bar_lines_when_asked() {
        let map = BarMap::new(GRID, &[]);
        let landing = map.ascend_landing(3.0, 16.0, true);
        let lines = (landing.secs - 0.25) / 8.0;
        assert!((lines - lines.round()).abs() < 1e-9, "a 4-bar line: {}", landing.secs);
        assert!(landing.secs >= 3.0 + 4.0 * 2.0 && (landing.secs - (3.0 + 32.0)).abs() <= 4.01);
        let landing = map.ascend_landing(3.0, 16.0, false);
        let bars = (landing.secs - 0.25) / 2.0;
        assert!((bars - bars.round()).abs() < 1e-9, "a bar line");
        assert!((landing.secs - 35.0).abs() <= 1.01);
    }

    #[test]
    fn a_far_phrase_start_is_ignored_in_favour_of_the_four_bar_line() {
        let map = BarMap::new(GRID, &[0.25, 200.25]);
        let landing = map.ascend_landing(10.0, 8.0, true);
        let lines = (landing.secs - 0.25) / 8.0;
        assert!((lines - lines.round()).abs() < 1e-9);
        assert!(landing.secs < 60.0);
    }

    #[test]
    fn the_ramp_never_ends_sooner_than_a_bar_or_a_quarter_of_its_length() {
        let map = BarMap::new(GRID, &[0.25, 12.25]);
        let landing = map.ascend_landing(11.5, 4.0, true);
        assert!(landing.secs >= 11.5 + 2.0, "{}", landing.secs);
    }
}
