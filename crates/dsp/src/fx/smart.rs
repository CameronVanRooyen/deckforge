use deckforge_common::{ColorFxKind, SmartCfxPreset};

use super::color::ColorFx;
use crate::smooth::Smoothed;

const DEAD_ZONE: f32 = 0.03;
const CHOP_SMOOTH_MS: f32 = 2.0;
const CHOP_RATES: [f64; 3] = [0.25, 0.125, 0.0625];
const CHOP_ONSET: f32 = 0.3;
const CHOP_DUTY: f64 = 0.5;

#[derive(Debug, Clone, Copy)]
struct Recipe {
    blocks: [ColorFxKind; 2],
    filter: [f32; 2],
    chop: [bool; 2],
}

fn recipe(preset: SmartCfxPreset) -> Recipe {
    use ColorFxKind as K;
    match preset {
        SmartCfxPreset::EchoWash => Recipe { blocks: [K::DubEcho, K::DubEcho], filter: [0.75, 0.75], chop: [false, false] },
        SmartCfxPreset::Riser => Recipe { blocks: [K::Space, K::Noise], filter: [0.75, 0.6], chop: [false, true] },
        SmartCfxPreset::Space => Recipe { blocks: [K::Space, K::Space], filter: [0.45, 0.45], chop: [false, false] },
        SmartCfxPreset::CrushChop => Recipe { blocks: [K::Filter, K::Crush], filter: [0.7, 0.0], chop: [true, false] },
    }
}

fn amount_of(value: f32) -> f32 {
    ((value.abs() - DEAD_ZONE) / (1.0 - DEAD_ZONE)).clamp(0.0, 1.0)
}

/// One-knob Smart CFX macro for a channel: a left-side and a right-side Color FX block, a filter amount and a beat chop.
pub struct SmartCfx {
    sample_rate: f32,
    preset: SmartCfxPreset,
    blocks: [ColorFx; 2],
    value: f32,
    bpm: f64,
    beat: f64,
    chop: Smoothed,
}

impl SmartCfx {
    pub fn new(sample_rate: f32, seed: u32) -> Self {
        let mut fx = Self {
            sample_rate,
            preset: SmartCfxPreset::default(),
            blocks: [ColorFx::new(sample_rate, seed.wrapping_add(11)), ColorFx::new(sample_rate, seed.wrapping_add(23))],
            value: 0.0,
            bpm: 120.0,
            beat: 0.0,
            chop: Smoothed::new(1.0, sample_rate, CHOP_SMOOTH_MS),
        };
        fx.set_preset(SmartCfxPreset::default());
        fx
    }

    pub fn preset(&self) -> SmartCfxPreset {
        self.preset
    }

    pub fn set_preset(&mut self, preset: SmartCfxPreset) {
        self.preset = preset;
        let r = recipe(preset);
        for (block, kind) in self.blocks.iter_mut().zip(r.blocks) {
            block.set_kind(kind);
        }
    }

    /// Bipolar knob, -1 (left) .. 1 (right); centre leaves the signal untouched apart from ringing tails.
    pub fn set_value(&mut self, value: f32) {
        self.value = value.clamp(-1.0, 1.0);
        self.blocks[0].set_value(self.value.min(0.0));
        self.blocks[1].set_value(self.value.max(0.0));
    }

    pub fn value(&self) -> f32 {
        self.value
    }

    /// Value for the channel's bipolar filter while the macro is in charge of it.
    pub fn filter_value(&self) -> f32 {
        let side = usize::from(self.value > 0.0);
        self.value * recipe(self.preset).filter[side]
    }

    pub fn set_bpm(&mut self, bpm: f64) {
        self.bpm = bpm.clamp(30.0, 400.0);
        for b in &mut self.blocks {
            b.set_bpm(self.bpm as f32);
        }
    }

    /// Beat position of the channel's deck at the start of the next block, so the chop lands on the grid.
    pub fn set_beat_position(&mut self, beat: Option<f64>) {
        if let Some(b) = beat {
            self.beat = b;
        }
    }

    fn chop_depth(&self) -> (f32, f64) {
        let side = usize::from(self.value > 0.0);
        if !recipe(self.preset).chop[side] {
            return (0.0, CHOP_RATES[0]);
        }
        let a = amount_of(self.value);
        let depth = ((a - CHOP_ONSET * 0.5) / (1.0 - CHOP_ONSET * 0.5)).clamp(0.0, 1.0);
        let step = (((a - CHOP_ONSET) / (1.0 - CHOP_ONSET)).clamp(0.0, 0.999) * CHOP_RATES.len() as f32) as usize;
        (depth, CHOP_RATES[step])
    }

    pub fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        let n = l.len().min(r.len());
        for b in &mut self.blocks {
            b.process(&mut l[..n], &mut r[..n]);
        }
        let (depth, rate) = self.chop_depth();
        let per_sample = self.bpm / 60.0 / self.sample_rate as f64;
        if depth <= 0.0 && self.chop.is_settled() && self.chop.value() >= 1.0 {
            self.beat += per_sample * n as f64;
            return;
        }
        for i in 0..n {
            let open = (self.beat / rate).rem_euclid(1.0) < CHOP_DUTY;
            self.chop.set(if open { 1.0 } else { 1.0 - depth });
            let g = self.chop.next();
            l[i] *= g;
            r[i] *= g;
            self.beat += per_sample;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::testkit::{white, SR};

    fn rms(v: &[f32]) -> f32 {
        (v.iter().map(|x| x * x).sum::<f32>() / v.len().max(1) as f32).sqrt()
    }

    fn run(fx: &mut SmartCfx, x: &[f32]) -> Vec<f32> {
        let mut l = x.to_vec();
        let mut r = x.to_vec();
        for (cl, cr) in l.chunks_mut(256).zip(r.chunks_mut(256)) {
            fx.process(cl, cr);
        }
        l
    }

    #[test]
    fn centred_knob_leaves_the_signal_alone_for_every_preset() {
        for preset in SmartCfxPreset::ALL {
            let mut fx = SmartCfx::new(SR, 1);
            fx.set_preset(preset);
            let x = white(9_600, 2, 0.3);
            assert_eq!(run(&mut fx, &x), x, "{preset:?}");
            assert_eq!(fx.filter_value(), 0.0);
        }
    }

    #[test]
    fn filter_amount_follows_the_side_recipe() {
        let mut fx = SmartCfx::new(SR, 1);
        fx.set_preset(SmartCfxPreset::CrushChop);
        fx.set_value(-1.0);
        assert!((fx.filter_value() + 0.7).abs() < 1e-6, "left closes a low-pass");
        fx.set_value(1.0);
        assert_eq!(fx.filter_value(), 0.0, "right is crush only");
    }

    #[test]
    fn crush_and_chop_left_gates_on_the_beat() {
        let mut fx = SmartCfx::new(SR, 1);
        fx.set_preset(SmartCfxPreset::CrushChop);
        fx.set_bpm(120.0);
        fx.set_beat_position(Some(0.0));
        fx.set_value(-1.0);
        let x = vec![0.5f32; SR as usize / 2];
        let y = run(&mut fx, &x);
        let open = rms(&y[100..700]);
        let closed = rms(&y[1_000..1_450]);
        assert!(open > 0.45 && closed < 0.05, "open {open}, closed {closed}");
    }
}
