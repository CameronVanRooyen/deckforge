use deckforge_common::{Beatgrid, TrackAudio};

use crate::filter::{mono, Biquad, Chain};

pub const SIXTEENTHS_PER_BEAT: usize = 4;
pub const BANDS: usize = 7;

/// Index of each band in [`Sixteenths::power`].
pub mod band {
    pub const TOTAL: usize = 0;
    pub const SUB: usize = 1;
    pub const SNARE: usize = 2;
    pub const HAT: usize = 3;
    pub const LOW: usize = 4;
    pub const MID: usize = 5;
    pub const HIGH: usize = 6;
}

/// Mean-square band power per sixteenth note of a constant grid, starting at its earliest beat at or after 0 s.
#[derive(Debug, Clone)]
pub struct Sixteenths {
    /// Grid beat index of the first analysed beat (0 or negative when the grid anchor lies later in the track).
    pub first_beat_index: i64,
    pub start_secs: f64,
    pub beat_secs: f64,
    pub power: Vec<[f32; BANDS]>,
}

impl Sixteenths {
    pub fn beats(&self) -> usize {
        self.power.len() / SIXTEENTHS_PER_BEAT
    }

    pub fn beat_secs_at(&self, beat: usize) -> f64 {
        self.start_secs + beat as f64 * self.beat_secs
    }

    /// Mean power of one band over sixteenths `[from, to)`.
    pub fn mean(&self, band: usize, from: usize, to: usize) -> f64 {
        let to = to.min(self.power.len());
        if to <= from {
            return 0.0;
        }
        self.power[from..to].iter().map(|p| f64::from(p[band])).sum::<f64>() / (to - from) as f64
    }
}

fn filter_bank(sr: f64) -> [Chain; BANDS] {
    [
        Chain::new([]),
        Chain::lowpass(100.0, sr, 1),
        Chain::bandpass(1_000.0, 5_000.0, sr, 1),
        Chain::new([Biquad::highpass(5_000.0, sr), Biquad::highpass(5_000.0, sr), Biquad::lowpass(7_000.0, sr)]),
        Chain::lowpass(150.0, sr, 2),
        Chain::bandpass(150.0, 2_000.0, sr, 2),
        Chain::highpass(2_000.0, sr, 2),
    ]
}

pub fn sixteenths(track: &TrackAudio, grid: Beatgrid) -> Sixteenths {
    let sr = f64::from(track.sample_rate);
    let beat_secs = grid.beat_secs();
    let beats_before = (grid.first_beat_secs / beat_secs + 1e-6).floor();
    let start_secs = (grid.first_beat_secs - beats_before * beat_secs).max(0.0);
    let slice = beat_secs / SIXTEENTHS_PER_BEAT as f64;
    let count = ((track.duration_secs() - start_secs) / slice).floor().max(0.0) as usize;
    let count = count - count % SIXTEENTHS_PER_BEAT;
    let mut bank = filter_bank(sr);
    let first_sample = ((start_secs * sr).round() as usize).min(track.frames());
    for i in 0..first_sample {
        let x = mono(track, i);
        bank.iter_mut().for_each(|f| {
            f.process(x);
        });
    }
    let mut power = Vec::with_capacity(count);
    let mut cursor = first_sample;
    for s in 0..count {
        let end = (((start_secs + (s + 1) as f64 * slice) * sr).round() as usize).min(track.frames());
        let mut acc = [0.0f64; BANDS];
        for i in cursor..end {
            let x = mono(track, i);
            for (a, f) in acc.iter_mut().zip(bank.iter_mut()) {
                let y = f.process(x);
                *a += y * y;
            }
        }
        let n = (end - cursor).max(1) as f64;
        power.push(acc.map(|a| (a / n) as f32));
        cursor = end;
    }
    Sixteenths { first_beat_index: -(beats_before as i64), start_secs, beat_secs, power }
}

pub fn db(power: f64) -> f64 {
    10.0 * (power + 1e-10).log10()
}

pub fn median(values: &mut [f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(f64::total_cmp);
    let n = values.len();
    if n % 2 == 1 { values[n / 2] } else { 0.5 * (values[n / 2 - 1] + values[n / 2]) }
}

pub fn percentile(values: &[f64], p: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    sorted[((sorted.len() - 1) as f64 * p).round() as usize]
}
