#![allow(dead_code)]

use std::f64::consts::TAU;

use deckforge_common::TrackAudio;

pub struct Noise(u64);

impl Noise {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        ((self.0 >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0) as f32
    }
}

pub fn stereo(sample_rate: u32, mono: Vec<f32>) -> TrackAudio {
    TrackAudio::new(sample_rate, mono.into_iter().flat_map(|s| [s, s]).collect())
}

pub fn add_kick(buf: &mut [f32], sr: u32, start_secs: f64, amp: f32) {
    let start = (start_secs * f64::from(sr)) as usize;
    let mut phase = 0.0f64;
    for k in 0..(0.25 * f64::from(sr)) as usize {
        let Some(slot) = buf.get_mut(start + k) else { break };
        let t = k as f64 / f64::from(sr);
        phase += TAU * (45.0 + 110.0 * (-t * 35.0).exp()) / f64::from(sr);
        *slot += (phase.sin() * (-t * 14.0).exp()) as f32 * amp;
    }
}

pub fn add_noise_burst(buf: &mut [f32], sr: u32, start_secs: f64, amp: f32, decay: f64, highpass: bool, noise: &mut Noise) {
    let start = (start_secs * f64::from(sr)) as usize;
    let mut prev = 0.0f32;
    for k in 0..(decay * 6.0 * f64::from(sr)) as usize {
        let Some(slot) = buf.get_mut(start + k) else { break };
        let t = k as f64 / f64::from(sr);
        let n = noise.next();
        let v = if highpass { n - prev } else { n };
        prev = n;
        *slot += v * (-t / decay).exp() as f32 * amp;
    }
}

/// Adds a tone with decaying harmonics over [start, end) secs.
pub fn add_tone(buf: &mut [f32], sr: u32, freq: f64, start_secs: f64, end_secs: f64, amp: f32, harmonics: usize) {
    let a = (start_secs * f64::from(sr)) as usize;
    let b = ((end_secs * f64::from(sr)) as usize).min(buf.len());
    let fade = (0.02 * f64::from(sr)) as usize;
    for (i, slot) in buf.iter_mut().enumerate().take(b).skip(a) {
        let t = i as f64 / f64::from(sr);
        let mut v = 0.0f64;
        for h in 1..=harmonics {
            let f = freq * h as f64;
            if f > f64::from(sr) * 0.45 {
                break;
            }
            v += (TAU * f * t).sin() / h as f64;
        }
        let env = ((i - a).min(b - i - 1) as f64 / fade as f64).min(1.0);
        *slot += (v * env) as f32 * amp;
    }
}
