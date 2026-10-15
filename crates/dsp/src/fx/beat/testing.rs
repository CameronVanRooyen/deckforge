use std::f32::consts::TAU;

use super::*;

pub(super) const SR: f32 = 48_000.0;
pub(super) const BLOCK: usize = 256;

pub(super) fn unit(kind: BeatFxKind, beats: f32, level: f32) -> BeatFx {
    let mut fx = BeatFx::new(SR);
    fx.raw_core = true;
    fx.set_bpm(120.0);
    fx.set_kind(kind);
    fx.set_beats(beats);
    fx.set_level(level);
    fx
}

pub(super) fn banded(kind: BeatFxKind, beats: f32, level: f32) -> BeatFx {
    let mut fx = unit(kind, beats, level);
    fx.raw_core = false;
    fx
}

pub(super) fn run(fx: &mut BeatFx, l: &[f32], r: &[f32]) -> (Vec<f32>, Vec<f32>) {
    let (mut ol, mut or) = (l.to_vec(), r.to_vec());
    for (cl, cr) in ol.chunks_mut(BLOCK).zip(or.chunks_mut(BLOCK)) {
        fx.process(cl, cr);
    }
    (ol, or)
}

pub(super) fn noise(len: usize, seed: u32, amp: f32) -> Vec<f32> {
    let mut n = crate::fx::util::Noise::new(seed);
    (0..len).map(|_| n.next() * amp).collect()
}

pub(super) fn sine(len: usize, hz: f32, amp: f32) -> Vec<f32> {
    (0..len).map(|i| amp * (TAU * hz * i as f32 / SR).sin()).collect()
}

pub(super) fn rms(v: &[f32]) -> f32 {
    (v.iter().map(|x| x * x).sum::<f32>() / v.len().max(1) as f32).sqrt()
}

pub(super) fn peak(v: &[f32]) -> f32 {
    v.iter().fold(0.0, |m, x| m.max(x.abs()))
}

pub(super) fn db(ratio: f32) -> f32 {
    20.0 * ratio.max(1e-9).log10()
}

/// A drum-like programme: a kick, noise hats and a bass tone on every beat, so the signal has real dynamics.
pub(super) fn music(secs: f32, bpm: f32) -> Vec<f32> {
    let n = (secs * SR) as usize;
    let beat = (60.0 / bpm * SR) as usize;
    let mut hats = crate::fx::util::Noise::new(99);
    (0..n)
        .map(|i| {
            let in_beat = (i % beat) as f32 / SR;
            let kick = 0.7 * (-in_beat * 18.0).exp() * (TAU * (50.0 + 90.0 * (-in_beat * 30.0).exp()) * in_beat).sin();
            let off = ((i + beat / 2) % beat) as f32 / SR;
            let hat = 0.18 * (-off * 60.0).exp() * hats.next();
            let bass = 0.25 * (TAU * 55.0 * i as f32 / SR).sin() * if (i / (beat / 2)) % 2 == 1 { 1.0 } else { 0.2 };
            let pad = 0.1 * (TAU * 330.0 * i as f32 / SR).sin() + 0.08 * (TAU * 495.0 * i as f32 / SR).sin();
            kick + hat + bass + pad
        })
        .collect()
}
