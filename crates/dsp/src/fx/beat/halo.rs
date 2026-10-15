use super::*;

const OCTAVE: f32 = 2.0;
const FIFTH: f32 = 1.498_307_1;
const WINDOW_SECS: f32 = 0.04;
const SHIMMER_GAIN: f32 = 0.5;
const SHIMMER_HP_HZ: f32 = 280.0;
const IN_HP_HZ: f32 = 260.0;
const OUT_HP_HZ: f32 = 220.0;
const TONE_LP_MIN_HZ: f32 = 3_500.0;
const TONE_LP_SPAN: f32 = 4.0;
const WET_GAIN: f32 = 0.8;
const FIFTH_LEVEL: f32 = 0.7;
const DUCK_DB: f32 = 3.5;
const HOLD_MS: f32 = 40.0;
const FROZEN_RT60: f32 = 4_000.0;
const SHIMMER_SMOOTH_MS: f32 = 60.0;

#[derive(Debug, Clone)]
pub(super) struct HaloState {
    octave: [PitchShifter; 2],
    fifth: [PitchShifter; 2],
    shimmer_hp: [HighPass<1>; 2],
    shimmer_lp: [LowPass<1>; 2],
    out_lp: [LowPass<1>; 2],
    prev: [f32; 2],
    freeze: Smoothed,
    shimmer: Smoothed,
    duck: Ducker,
    pub(super) freeze_held: bool,
}

impl HaloState {
    pub(super) fn new(sample_rate: f32) -> Self {
        let window = (WINDOW_SECS * sample_rate) as usize;
        Self {
            octave: std::array::from_fn(|_| PitchShifter::new(window)),
            fifth: std::array::from_fn(|_| PitchShifter::new(window)),
            shimmer_hp: [HighPass::default(); 2],
            shimmer_lp: [LowPass::default(); 2],
            out_lp: [LowPass::default(); 2],
            prev: [0.0; 2],
            freeze: Smoothed::new(0.0, sample_rate, HOLD_MS),
            shimmer: Smoothed::new(0.0, sample_rate, SHIMMER_SMOOTH_MS),
            duck: Ducker::new(sample_rate, DUCK_DB, DUCK_ATTACK_MS, DUCK_RELEASE_MS),
            freeze_held: false,
        }
    }

    pub(super) fn reset(&mut self) {
        self.octave.iter_mut().for_each(PitchShifter::reset);
        self.fifth.iter_mut().for_each(PitchShifter::reset);
        self.shimmer_hp.iter_mut().for_each(HighPass::reset);
        self.shimmer_lp.iter_mut().for_each(LowPass::reset);
        self.out_lp.iter_mut().for_each(LowPass::reset);
        self.prev = [0.0; 2];
        self.freeze.reset(0.0);
        self.shimmer.reset(self.shimmer.target());
        self.duck.reset();
        self.freeze_held = false;
    }

    pub(super) fn holding(&self) -> bool {
        self.freeze_held
    }

    pub(super) fn release_holds(&mut self) {
        self.freeze_held = false;
    }
}

impl BeatFx {
    pub(super) fn run_halo(&mut self, l: &mut [f32], r: &mut [f32]) {
        let sr = self.sample_rate;
        let size = self.percent;
        let t = percent_position(size);
        let shimmer = self.param(0) * 0.01;
        let tone = self.param(1) * 0.01;
        let fifth = self.param(2) * 0.01 * FIFTH_LEVEL;
        if !self.enabled {
            self.halo.release_holds();
        }
        let frozen = self.halo.freeze_held;
        let rt = if frozen { FROZEN_RT60 } else { reverb_rt60_for_percent(size) };
        if self.rt60_applied == 0.0 {
            self.rt60.reset(rt);
        }
        self.rt60.set(rt);
        self.reverb.set_size(reverb_size_for_percent(size));
        self.reverb.set_output_floor(0.3);
        self.reverb.set_predelay_ms(14.0 + 16.0 * t);
        self.reverb.set_hf_ratio(REVERB_HF_RATIO + 0.3 * tone);
        self.reverb.set_modulation_ms(reverb_modulation_ms(t));
        self.halo.freeze.set(if frozen { 1.0 } else { 0.0 });
        self.halo.shimmer.set(SHIMMER_GAIN * shimmer * shimmer);
        let tone_lp = TONE_LP_MIN_HZ * TONE_LP_SPAN.powf(tone);
        for ch in 0..2 {
            self.reverb_in_hp[ch].set(IN_HP_HZ, sr);
            self.reverb_out_hp[ch].set(OUT_HP_HZ, sr);
            self.halo.shimmer_hp[ch].set(SHIMMER_HP_HZ, sr);
            self.halo.shimmer_lp[ch].set(tone_lp, sr);
            self.halo.out_lp[ch].set(tone_lp * 1.6, sr);
        }
        for i in 0..l.len() {
            if i % REVERB_CHUNK == 0 {
                let rt = self.rt60.next();
                if (rt - self.rt60_applied).abs() > 1e-4 {
                    self.reverb.set_rt60(rt);
                    self.rt60_applied = rt;
                }
            }
            let send = self.send.next();
            let gate = self.swap.next();
            let lvl = self.level.next();
            let freeze = self.halo.freeze.next();
            let amount = self.halo.shimmer.next() * (1.0 - freeze);
            let (xl, xr) = (l[i], r[i]);
            let (dry_law, wet_law) = beat_mix(lvl);
            let duck = 1.0 + (self.halo.duck.process(xl, xr) - 1.0) * (1.0 - freeze) * dry_law;
            let inputs = [xl, xr];
            let mut fed = [0.0f32; 2];
            for ch in 0..2 {
                let seed = self.halo.shimmer_hp[ch].process(self.halo.prev[ch]);
                let lifted = self.halo.octave[ch].process(seed, OCTAVE) + fifth * self.halo.fifth[ch].process(seed, FIFTH);
                let lifted = self.halo.shimmer_lp[ch].process(lifted);
                fed[ch] = self.reverb_in_hp[ch].process(inputs[ch] * send * (1.0 - freeze)) + amount * lifted.tanh();
            }
            let (wl, wr) = self.reverb.process(fed[0], fed[1]);
            let wets = [wl, wr];
            let wet = wet_law * WET_GAIN * duck * gate;
            let dry = gated_dry(dry_law, send * gate);
            let mut out = [0.0f32; 2];
            for ch in 0..2 {
                let cleaned = self.reverb_out_hp[ch].process(soft_limit(wets[ch]));
                self.halo.prev[ch] = cleaned;
                out[ch] = self.halo.out_lp[ch].process(cleaned) * wet;
            }
            l[i] = xl * dry + soft_limit(out[0]);
            r[i] = xr * dry + soft_limit(out[1]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::*;
    use crate::fx::testkit::tone_amplitude;

    fn halo(level: f32) -> BeatFx {
        let mut fx = unit(BeatFxKind::Halo, 1.0, level);
        fx.set_percent(60.0);
        fx.set_enabled(true);
        fx
    }

    fn burst_then_silence(hz: f32, burst_secs: f32, total_secs: f32) -> Vec<f32> {
        let burst = (burst_secs * SR) as usize;
        sine(burst, hz, 0.4).into_iter().chain(vec![0.0; (total_secs * SR) as usize - burst]).collect()
    }

    fn tail_partial(shimmer: f32, fifth: f32, hz: f32) -> f32 {
        let mut fx = halo(1.0);
        fx.set_param(0, shimmer);
        fx.set_param(2, fifth);
        let x = burst_then_silence(440.0, 1.0, 5.0);
        let (out, _) = run(&mut fx, &x, &x);
        tone_amplitude(&out[48_000 * 2..48_000 * 4], hz)
    }

    #[test]
    fn shimmer_puts_partials_an_octave_up_in_the_tail() {
        let (off, on) = (tail_partial(0.0, 0.0, 880.0), tail_partial(100.0, 0.0, 880.0));
        let (off2, on2) = (tail_partial(0.0, 0.0, 1_760.0), tail_partial(100.0, 0.0, 1_760.0));
        eprintln!("halo 880 Hz partial: {off:.5} -> {on:.5} ({:+.1} dB); 1760 Hz: {:+.1} dB", db(on / off), db(on2 / off2));
        assert!(on > 8.0 * off, "an octave above the 440 Hz source");
        assert!(on2 > 2.0 * off2, "and another octave on the next pass");
    }

    #[test]
    fn the_fifth_voice_adds_a_partial_seven_semitones_up() {
        let hz = 440.0 * 1.498_307;
        let (without, with) = (tail_partial(60.0, 0.0, hz), tail_partial(60.0, 100.0, hz));
        eprintln!("fifth partial {without:.5} -> {with:.5} ({:+.1} dB)", db(with / without));
        assert!(with > 4.0 * without);
    }

    #[test]
    fn freeze_holds_an_infinite_stable_tail() {
        let mut fx = halo(1.0);
        let x = burst_then_silence(440.0, 1.5, 40.0);
        let mut free = halo(1.0);
        let (unfrozen, _) = run(&mut free, &x, &x);
        let (frozen, _) = {
            let (mut l, mut r) = (x.clone(), x.clone());
            for (b, (cl, cr)) in l.chunks_mut(BLOCK).zip(r.chunks_mut(BLOCK)).enumerate() {
                if b * BLOCK == 48_000 * 2 {
                    fx.trigger(FxTrigger::FreezeOn);
                }
                fx.process(cl, cr);
            }
            (l, r)
        };
        assert!(fx.held());
        let early = rms(&frozen[48_000 * 6..48_000 * 12]);
        let late = rms(&frozen[48_000 * 30..48_000 * 40]);
        eprintln!("halo freeze: {early:.4} at 6-12 s, {late:.4} at 30-40 s; unfrozen {:.6} at 30-40 s", rms(&unfrozen[48_000 * 30..48_000 * 40]));
        assert!(frozen.iter().all(|v| v.is_finite()) && peak(&frozen) < 2.0);
        assert!(db(late / early).abs() < 3.0, "stable: {:+.2} dB", db(late / early));
        assert!(late > 0.01, "audible");
        assert!(rms(&unfrozen[48_000 * 30..48_000 * 40]) < 0.1 * late, "the unfrozen reverb is long gone");
    }

    #[test]
    fn freeze_closes_the_input_and_the_shimmer_loop() {
        let mut fx = halo(0.75);
        fx.set_param(0, 100.0);
        let mut x = noise(48_000 * 12, 8, 0.4);
        x[..48_000].fill(0.0);
        let mut l = x.clone();
        let mut r = x.clone();
        for (b, (cl, cr)) in l.chunks_mut(BLOCK).zip(r.chunks_mut(BLOCK)).enumerate() {
            if b == 0 {
                fx.trigger(FxTrigger::FreezeOn);
            }
            fx.process(cl, cr);
        }
        let wet: Vec<f32> = l.iter().zip(&x).map(|(o, d)| o - d).collect();
        assert!(rms(&wet[48_000 * 6..]) < 1e-4, "frozen silence stays silent");
    }

    #[test]
    fn stays_finite_and_bounded_for_thirty_seconds_at_maximum() {
        for (name, x) in [
            ("noise", noise(48_000 * 30, 99, 1.0)),
            ("square", (0..48_000 * 30).map(|i| if (i / 55) % 2 == 0 { 1.0 } else { -1.0 }).collect()),
            ("sine", sine(48_000 * 30, 330.0, 1.0)),
        ] {
            let mut fx = halo(1.0);
            fx.set_percent(100.0);
            fx.set_param(0, 100.0);
            fx.set_param(2, 100.0);
            fx.set_param(1, 100.0);
            let (out, _) = run(&mut fx, &x, &x);
            assert!(out.iter().all(|v| v.is_finite()), "{name}");
            assert!(peak(&out) < 4.0, "{name}: peak {}", peak(&out));
        }
    }

    #[test]
    fn stays_within_two_db_of_the_dry_programme_at_default_settings() {
        for level in [0.5f32, 1.0] {
            let mut fx = banded(BeatFxKind::Halo, 1.0, level);
            fx.set_percent(60.0);
            fx.set_enabled(true);
            let x = music(16.0, 120.0);
            let (out, _) = run(&mut fx, &x, &x);
            let gain = db(rms(&out[48_000 * 4..]) / rms(&x[48_000 * 4..]));
            eprintln!("halo level {level}: {gain:+.2} dB");
            assert!(gain < 2.0 && gain > -3.0, "level {level}: {gain:+.2} dB");
        }
    }

    #[test]
    fn the_tail_rings_after_disable_then_the_unit_goes_idle() {
        let mut fx = halo(0.8);
        let x = noise(48_000, 12, 0.3);
        let _ = run(&mut fx, &x, &x);
        fx.set_enabled(false);
        let silence = vec![0.0; 48_000 * 20];
        let (out, _) = run(&mut fx, &silence, &silence);
        assert!(rms(&out[2_000..20_000]) > 0.003);
        assert!(fx.is_idle());
    }
}
