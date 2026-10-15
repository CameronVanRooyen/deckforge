use deckforge_common::TrackAudio;

use crate::interp::read_cubic;

const GRAIN: usize = 2048;
const HOP: usize = GRAIN / 4;
const SEARCH: usize = 384;
const CORR_LEN: usize = 512;
const CORR_STEP: usize = 4;
const SEARCH_STEP: usize = 2;
/// Hann at 75% overlap sums to 2.0; this restores unity gain.
const OLA_GAIN: f32 = 0.5;

/// WSOLA time-stretcher: changes tempo while preserving pitch (key lock).
/// Positions are in source frames; `step` is source frames per output frame at native pitch.
pub struct Stretcher {
    window: Vec<f32>,
    ola_l: Vec<f32>,
    ola_r: Vec<f32>,
    ready: usize,
    read_pos: f64,
    prev_grain: f64,
    discontinuity: bool,
    grain_l: Vec<f32>,
    grain_r: Vec<f32>,
    natural: Vec<f32>,
    candidate: Vec<f32>,
}

impl Default for Stretcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Stretcher {
    pub fn new() -> Self {
        let window = (0..GRAIN)
            .map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / GRAIN as f32).cos())
            .collect();
        Self {
            window,
            ola_l: vec![0.0; GRAIN],
            ola_r: vec![0.0; GRAIN],
            ready: 0,
            read_pos: 0.0,
            prev_grain: 0.0,
            discontinuity: true,
            grain_l: vec![0.0; GRAIN],
            grain_r: vec![0.0; GRAIN],
            natural: vec![0.0; CORR_LEN / CORR_STEP],
            candidate: vec![0.0; (CORR_LEN + 2 * SEARCH) / CORR_STEP + 1],
        }
    }

    /// Repositions the read head; the overlap-add tail crossfades into the new material.
    pub fn jump(&mut self, position: f64) {
        self.read_pos = position;
        self.discontinuity = true;
    }

    pub fn position(&self) -> f64 {
        self.read_pos
    }

    pub fn latency_frames() -> usize {
        GRAIN / 2
    }

    fn mono_at(track: &TrackAudio, pos: f64) -> f32 {
        let (l, r) = read_cubic(track, pos);
        l + r
    }

    fn best_offset(&mut self, track: &TrackAudio, nominal: f64, step: f64) -> f64 {
        if self.discontinuity {
            return 0.0;
        }
        let natural_start = self.prev_grain + HOP as f64 * step;
        for (k, v) in self.natural.iter_mut().enumerate() {
            *v = Self::mono_at(track, natural_start + (k * CORR_STEP) as f64 * step);
        }
        let base = nominal - SEARCH as f64 * step;
        for (k, v) in self.candidate.iter_mut().enumerate() {
            *v = Self::mono_at(track, base + (k * CORR_STEP) as f64 * step);
        }
        let n = self.natural.len();
        let mut best = (SEARCH as isize, f32::MIN);
        let mut d = 0usize;
        while d <= 2 * SEARCH {
            let off = d / CORR_STEP;
            if off + n > self.candidate.len() {
                break;
            }
            let mut dot = 0.0;
            let mut energy = 1e-9;
            for k in 0..n {
                let c = self.candidate[off + k];
                dot += c * self.natural[k];
                energy += c * c;
            }
            let score = dot / energy.sqrt();
            if score > best.1 {
                best = (d as isize, score);
            }
            d += SEARCH_STEP * CORR_STEP;
        }
        (best.0 - SEARCH as isize) as f64 * step
    }

    fn synthesize_grain(&mut self, track: &TrackAudio, step: f64, tempo: f64) {
        let nominal = self.read_pos;
        let start = nominal + self.best_offset(track, nominal, step);
        self.discontinuity = false;
        self.prev_grain = start;
        for k in 0..GRAIN {
            let (l, r) = read_cubic(track, start + k as f64 * step);
            self.grain_l[k] = l * self.window[k] * OLA_GAIN;
            self.grain_r[k] = r * self.window[k] * OLA_GAIN;
        }
        for k in 0..GRAIN {
            self.ola_l[k] += self.grain_l[k];
            self.ola_r[k] += self.grain_r[k];
        }
        self.ready = HOP;
        self.read_pos += HOP as f64 * step * tempo;
    }

    /// Fills the output slices; returns the source position consumed per output frame on average.
    pub fn render(&mut self, track: &TrackAudio, step: f64, tempo: f64, out_l: &mut [f32], out_r: &mut [f32]) {
        let mut written = 0;
        while written < out_l.len() {
            if self.ready == 0 {
                self.ola_l.copy_within(HOP.., 0);
                self.ola_r.copy_within(HOP.., 0);
                let len = self.ola_l.len();
                self.ola_l[len - HOP..].fill(0.0);
                self.ola_r[len - HOP..].fill(0.0);
                self.synthesize_grain(track, step, tempo);
            }
            let offset = HOP - self.ready;
            let n = self.ready.min(out_l.len() - written);
            out_l[written..written + n].copy_from_slice(&self.ola_l[offset..offset + n]);
            out_r[written..written + n].copy_from_slice(&self.ola_r[offset..offset + n]);
            self.ready -= n;
            written += n;
        }
    }

    /// Source position currently audible, accounting for the grains still queued.
    pub fn audible_position(&self, step: f64, tempo: f64) -> f64 {
        self.read_pos - (self.ready as f64 + GRAIN as f64 * 0.5) * step * tempo
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zero_crossings(x: &[f32]) -> usize {
        x.windows(2).filter(|w| w[0] <= 0.0 && w[1] > 0.0).count()
    }

    #[test]
    fn preserves_pitch_while_changing_tempo() {
        let sr = 44_100.0;
        let freq = 440.0;
        let samples: Vec<f32> = (0..sr as usize * 4)
            .flat_map(|i| {
                let s = (std::f64::consts::TAU * freq * i as f64 / sr).sin() as f32 * 0.5;
                [s, s]
            })
            .collect();
        let track = TrackAudio::new(sr as u32, samples);
        for tempo in [0.8, 1.0, 1.25] {
            let mut st = Stretcher::new();
            st.jump(0.0);
            let n = sr as usize;
            let mut l = vec![0.0; n];
            let mut r = vec![0.0; n];
            st.render(&track, 1.0, tempo, &mut l, &mut r);
            let tail = &l[n / 4..];
            let measured = zero_crossings(tail) as f64 / (tail.len() as f64 / sr);
            assert!((measured - freq).abs() < 4.0, "tempo {tempo}: {measured} Hz");
            let expected_pos = n as f64 * tempo;
            assert!((st.position() - expected_pos).abs() < (GRAIN * 2) as f64, "tempo {tempo}");
            let rms = (tail.iter().map(|v| v * v).sum::<f32>() / tail.len() as f32).sqrt();
            assert!((rms - 0.5 / std::f32::consts::SQRT_2).abs() < 0.05, "tempo {tempo}: rms {rms}");
        }
    }
}
