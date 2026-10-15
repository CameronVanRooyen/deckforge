use std::f64::consts::PI;

use deckforge_common::TrackAudio;
use serde::{Deserialize, Serialize};

const BLOCK_SECS: f64 = 0.4;
const HOP_SECS: f64 = 0.1;
const ABSOLUTE_GATE_LUFS: f64 = -70.0;
const RELATIVE_GATE_LU: f64 = -10.0;
const LOUDNESS_OFFSET: f64 = -0.691;

/// ITU-R BS.1770 / EBU R128 integrated loudness and the absolute sample peak (full scale 1.0).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Loudness {
    pub integrated_lufs: f64,
    pub sample_peak: f64,
}

#[derive(Clone, Copy)]
struct Section {
    b: [f64; 3],
    a: [f64; 2],
    s: [f64; 2],
}

impl Section {
    #[inline]
    fn process(&mut self, x: f64) -> f64 {
        let y = self.b[0] * x + self.s[0];
        self.s[0] = self.b[1] * x - self.a[0] * y + self.s[1];
        self.s[1] = self.b[2] * x - self.a[1] * y;
        y
    }
}

/// The two K-weighting stages (high-frequency shelf, then the revised low-frequency B high-pass) at `sr`.
fn k_weighting(sr: f64) -> [Section; 2] {
    let (f0, gain_db, q) = (1_681.974_450_955_533, 3.999_843_853_973_347, 0.707_175_236_955_419_6);
    let k = (PI * f0 / sr).tan();
    let vh = 10f64.powf(gain_db / 20.0);
    let vb = vh.powf(0.499_666_774_154_541_6);
    let a0 = 1.0 + k / q + k * k;
    let shelf = Section {
        b: [(vh + vb * k / q + k * k) / a0, 2.0 * (k * k - vh) / a0, (vh - vb * k / q + k * k) / a0],
        a: [2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0],
        s: [0.0; 2],
    };
    let (f0, q) = (38.135_470_876_024_44, 0.500_327_037_323_877_3);
    let k = (PI * f0 / sr).tan();
    let a0 = 1.0 + k / q + k * k;
    let highpass = Section {
        b: [1.0, -2.0, 1.0],
        a: [2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0],
        s: [0.0; 2],
    };
    [shelf, highpass]
}

pub fn measure_loudness(track: &TrackAudio) -> Option<Loudness> {
    let sr = f64::from(track.sample_rate);
    let hop = (HOP_SECS * sr).round() as usize;
    let hops_per_block = (BLOCK_SECS / HOP_SECS).round() as usize;
    if hop == 0 || track.frames() < hop * hops_per_block {
        return None;
    }
    let mut filters = [k_weighting(sr), k_weighting(sr)];
    let mut hop_energy = Vec::with_capacity(track.frames() / hop + 1);
    let mut acc = 0.0f64;
    let mut peak = 0.0f64;
    for i in 0..track.frames() {
        let (l, r) = track.frame(i);
        for (x, f) in [f64::from(l), f64::from(r)].into_iter().zip(filters.iter_mut()) {
            peak = peak.max(x.abs());
            let y = f.iter_mut().fold(x, |v, s| s.process(v));
            acc += y * y;
        }
        if (i + 1) % hop == 0 {
            hop_energy.push(acc);
            acc = 0.0;
        }
    }
    let block_len = (hop * hops_per_block) as f64;
    let blocks: Vec<f64> =
        hop_energy.windows(hops_per_block).map(|w| w.iter().sum::<f64>() / block_len).collect();
    let lufs = |z: f64| LOUDNESS_OFFSET + 10.0 * z.max(1e-20).log10();
    let mean_of = |gate: f64| {
        let kept: Vec<f64> = blocks.iter().copied().filter(|z| lufs(*z) > gate).collect();
        (!kept.is_empty()).then(|| kept.iter().sum::<f64>() / kept.len() as f64)
    };
    let integrated = mean_of(ABSOLUTE_GATE_LUFS)
        .and_then(|z| mean_of(lufs(z) + RELATIVE_GATE_LU))
        .map_or(f64::NEG_INFINITY, lufs);
    Some(Loudness { integrated_lufs: integrated, sample_peak: peak })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(freq: f64, amp: f64, secs: f64, sr: u32) -> TrackAudio {
        let n = (secs * f64::from(sr)) as usize;
        TrackAudio::new(
            sr,
            (0..n).flat_map(|i| {
                let v = (amp * (2.0 * PI * freq * i as f64 / f64::from(sr)).sin()) as f32;
                [v, v]
            })
            .collect(),
        )
    }

    #[test]
    fn a_full_scale_1k_sine_in_both_channels_reads_zero_lufs() {
        for sr in [44_100, 48_000] {
            let l = measure_loudness(&sine(1_000.0, 1.0, 5.0, sr)).unwrap();
            assert!(l.integrated_lufs.abs() < 0.05, "{sr}: {}", l.integrated_lufs);
            assert!((l.sample_peak - 1.0).abs() < 1e-3);
        }
    }

    #[test]
    fn quiet_passages_are_gated_out() {
        let sr = 48_000;
        let mut loud = sine(1_000.0, 0.5, 10.0, sr);
        loud.samples.extend(sine(1_000.0, 0.005, 10.0, sr).samples);
        let alone = measure_loudness(&sine(1_000.0, 0.5, 10.0, sr)).unwrap().integrated_lufs;
        let with_quiet = measure_loudness(&loud).unwrap().integrated_lufs;
        assert!((with_quiet - alone).abs() < 0.1, "{alone} vs {with_quiet}");
        assert_eq!(measure_loudness(&sine(1_000.0, 0.0, 2.0, sr)).unwrap().integrated_lufs, f64::NEG_INFINITY);
    }
}
