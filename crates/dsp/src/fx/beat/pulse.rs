use super::*;
use crate::fx::noise_bed::NoiseBed;

const IN_HP_HZ: f32 = 280.0;
const OUT_HP_HZ: f32 = 300.0;
const WET_GAIN: f32 = 1.2;
const NOISE_PEAK: f32 = 2.0;
const NOISE_LO_HZ: f32 = 2_200.0;
const NOISE_HI_HZ: f32 = 9_500.0;
const NOISE_DUCK_DB: f32 = 6.0;
const DUCK_DB: f32 = 3.0;
const ATTACK_SECS: f32 = 0.002;
const GAIN_SMOOTH_MS: f32 = 0.8;
const MIN_BEATS: f32 = 0.125;
const MAX_STEPS: u32 = 6;
const EVERY_BARS: [u32; 4] = [1, 2, 4, 8];

#[derive(Debug, Clone)]
pub(super) struct PulseState {
    noise: NoiseBed,
    duck: Ducker,
    gain: Smoothed,
    bars: u32,
}

impl PulseState {
    pub(super) fn new(sample_rate: f32) -> Self {
        Self {
            noise: NoiseBed::new(sample_rate, NOISE_DUCK_DB, DUCK_RELEASE_MS),
            duck: Ducker::new(sample_rate, DUCK_DB, DUCK_ATTACK_MS, DUCK_RELEASE_MS),
            gain: Smoothed::new(1.0, sample_rate, GAIN_SMOOTH_MS),
            bars: 0,
        }
    }

    pub(super) fn noise_gains_settle(&mut self, gains: [f32; 3]) {
        self.noise.settle_band_gains(gains);
    }

    pub(super) fn reset(&mut self) {
        self.noise.reset();
        self.duck.reset();
        self.gain.reset(1.0);
        self.bars = 0;
    }
}

/// Wet gain over one tremolo cycle: a short fade into a duck at the cycle start, then a swell back to unity.
pub(super) fn pulse_gain(phase: f32, depth: f32, attack: f32) -> f32 {
    let onset = if phase < attack {
        let t = phase / attack;
        t * t * (3.0 - 2.0 * t)
    } else {
        1.0
    };
    let recover = 1.0 - phase;
    1.0 - depth * recover * recover * onset
}

impl BeatFx {
    /// Tremolo division in beats after `steps` doublings of the rate.
    pub(super) fn pulse_division(&self, steps: u32) -> f32 {
        (self.beats / (1u32 << steps.min(MAX_STEPS)) as f32).max(MIN_BEATS)
    }

    pub(super) fn pulse_steps(&self) -> u32 {
        if self.param(3) < 0.5 {
            return 0;
        }
        let every = EVERY_BARS[self.param(4).round().clamp(0.0, 3.0) as usize];
        self.pulse.bars / every
    }

    pub(super) fn run_pulse_verb(&mut self, l: &mut [f32], r: &mut [f32]) {
        let size = self.param(0);
        let chop = self.param(1) * 0.01;
        let noise_amount = self.param(2) * 0.01;
        let t = percent_position(size);
        let rt = reverb_rt60_for_percent(size);
        if self.rt60_applied == 0.0 {
            self.rt60.reset(rt);
        }
        self.rt60.set(rt);
        self.reverb.set_size(reverb_size_for_percent(size));
        self.reverb.set_output_floor(0.3);
        self.reverb.set_predelay_ms(12.0 + 20.0 * t);
        self.reverb.set_hf_ratio(REVERB_HF_RATIO);
        self.reverb.set_modulation_ms(reverb_modulation_ms(t));
        self.pulse.noise.set_band(NOISE_LO_HZ, NOISE_HI_HZ);
        self.pulse.noise.set_band_gains(self.band_gain.map(|g| g.target()));
        for ch in 0..2 {
            self.reverb_in_hp[ch].set(IN_HP_HZ, self.sample_rate);
            self.reverb_out_hp[ch].set(OUT_HP_HZ, self.sample_rate);
        }
        let per_bar = self.bar_clock.per_bar;
        let beat_frames = per_bar / 4.0;
        let mut to_next = self.bar_clock.to_next;
        let beat_secs = self.beat_secs();
        let mut steps = self.pulse_steps();
        let mut division = self.pulse_division(steps);
        for i in 0..l.len() {
            if i % REVERB_CHUNK == 0 {
                let rt = self.rt60.next();
                if (rt - self.rt60_applied).abs() > 1e-4 {
                    self.reverb.set_rt60(rt);
                    self.rt60_applied = rt;
                }
            }
            to_next -= 1.0;
            if to_next <= BOUNDARY_SLACK {
                to_next += per_bar;
                self.pulse.bars += 1;
                let next = self.pulse_steps();
                if next != steps {
                    steps = next;
                    division = self.pulse_division(steps);
                }
            }
            let send = self.send.next();
            let gate = self.swap.next();
            let lvl = self.level.next();
            let (xl, xr) = (l[i], r[i]);
            self.pulse.noise.track(xl, xr);
            let (dry_law, wet_law) = beat_mix(lvl);
            let duck = 1.0 + (self.pulse.duck.process(xl, xr) - 1.0) * dry_law;
            let hiss = noise_amount * self.pulse.noise.reference() * NOISE_PEAK * lvl * send * gate * self.pulse.noise.duck_gain();
            let bed = if hiss > 0.0 { self.pulse.noise.next() } else { [0.0; 2] };
            let input = [self.reverb_in_hp[0].process(xl * send) + bed[0] * hiss, self.reverb_in_hp[1].process(xr * send) + bed[1] * hiss];
            let (wl, wr) = self.reverb.process(input[0], input[1]);
            let (wl, wr) = (self.reverb_out_hp[0].process(wl), self.reverb_out_hp[1].process(wr));
            let position = 4.0 - to_next / beat_frames;
            let phase = (position / division as f64).rem_euclid(1.0) as f32;
            let attack = ATTACK_SECS / (division * beat_secs);
            self.pulse.gain.set(pulse_gain(phase, chop, attack));
            let wet = wet_law * WET_GAIN * duck * self.pulse.gain.next() * gate;
            let dry = gated_dry(dry_law, send * gate);
            l[i] = xl * dry + soft_limit(wl * wet);
            r[i] = xr * dry + soft_limit(wr * wet);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::*;
    use crate::fx::testkit::band_psd;
    use crate::fx::util::OnePole;

    fn enabled(beats: f32, level: f32) -> BeatFx {
        let mut fx = unit(BeatFxKind::PulseVerb, beats, level);
        fx.set_enabled(true);
        fx.align_phase(0.0);
        fx
    }

    fn wet_of(out: &[f32], x: &[f32]) -> Vec<f32> {
        out.iter().zip(x).map(|(o, d)| o - d).collect()
    }

    #[test]
    fn the_wet_signal_is_gated_by_the_beat() {
        let mut fx = enabled(1.0, 0.75);
        fx.set_param(1, 100.0);
        fx.set_param(2, 0.0);
        let x = noise(48_000 * 12, 5, 0.3);
        let (out, _) = run(&mut fx, &x, &x);
        let wet = wet_of(&out, &x);
        let beat = 24_000;
        let at = |from: f32, to: f32| {
            let mut acc = 0.0;
            for b in 6..20 {
                acc += rms(&wet[b * beat + (from * beat as f32) as usize..b * beat + (to * beat as f32) as usize]);
            }
            acc / 14.0
        };
        let on_the_beat = at(0.02, 0.1);
        let between = at(0.5, 0.9);
        eprintln!("pulse verb: {:.4} on the beat, {:.4} between ({:.1} dB)", on_the_beat, between, db(on_the_beat / between));
        assert!(between > 4.0 * on_the_beat, "ducks on the beat and swells between");
    }

    fn pulses_in_bar(wet: &[f32], bar: usize, bar_frames: usize) -> usize {
        let seg = &wet[bar * bar_frames..(bar + 1) * bar_frames];
        let mut lp = OnePole::default();
        lp.set_cutoff(120.0, SR);
        let env: Vec<f32> = seg.iter().map(|v| lp.lp(v.abs())).collect();
        let top = env.iter().cloned().fold(0.0, f32::max);
        let (mut count, mut below) = (0, false);
        for v in &env {
            if !below && *v < 0.35 * top {
                below = true;
                count += 1;
            } else if below && *v > 0.6 * top {
                below = false;
            }
        }
        count
    }

    #[test]
    fn accelerate_doubles_the_pulse_rate_every_n_bars() {
        let mut fx = enabled(1.0, 0.75);
        fx.set_param(1, 100.0);
        fx.set_param(2, 0.0);
        fx.set_param(3, 1.0);
        fx.set_param(4, 0.0);
        let bar = 96_000;
        let x = noise(bar * 5, 8, 0.3);
        let (out, _) = run(&mut fx, &x, &x);
        let wet = wet_of(&out, &x);
        let counts: Vec<usize> = (1..4).map(|b| pulses_in_bar(&wet, b, bar)).collect();
        eprintln!("pulses per bar after the first: {counts:?}");
        assert!((7..=9).contains(&counts[0]), "{counts:?}");
        assert!((14..=17).contains(&counts[1]), "{counts:?}");
        assert!(counts[2] >= 24, "{counts:?}");
    }

    #[test]
    fn accelerate_holds_the_rate_for_the_chosen_number_of_bars() {
        let mut fx = enabled(1.0, 0.75);
        fx.set_param(3, 1.0);
        fx.set_param(4, 2.0);
        let x = noise(96_000 * 9, 9, 0.2);
        let mut seen = Vec::new();
        for (cl, cr) in x.chunks(BLOCK).zip(x.chunks(BLOCK)) {
            let (mut l, mut r) = (cl.to_vec(), cr.to_vec());
            fx.process(&mut l, &mut r);
            seen.push(fx.pulse_steps());
        }
        let per_bar = 96_000 / BLOCK;
        assert_eq!(seen[per_bar / 2], 0);
        assert_eq!(seen[3 * per_bar + per_bar / 2], 0, "three bars in, still the first division with 4 bars per step");
        assert_eq!(seen[4 * per_bar + per_bar / 2], 1);
        assert_eq!(seen[8 * per_bar + per_bar / 2], 2);
    }

    #[test]
    fn the_wet_is_high_passed_so_the_lows_stay_clean() {
        let mut fx = enabled(1.0, 0.75);
        fx.set_param(2, 0.0);
        let x = noise(48_000 * 8, 3, 0.3);
        let (out, _) = run(&mut fx, &x, &x);
        let wet = wet_of(&out[48_000 * 2..], &x[48_000 * 2..]);
        let lows = band_psd(&wet, 40.0, 140.0);
        let mids = band_psd(&wet, 1_000.0, 3_000.0);
        eprintln!("wet psd lows {:.1} dB under mids", db((mids / lows).sqrt()));
        assert!(mids > 100.0 * lows, "lows are at least 20 dB under the mids");
    }

    #[test]
    fn noise_bed_adds_a_hiss_that_follows_the_amount() {
        let tail = |amount: f32| {
            let mut fx = enabled(1.0, 0.75);
            fx.set_param(2, amount);
            fx.set_param(0, 20.0);
            let x: Vec<f32> = noise(48_000 * 2, 4, 0.2).into_iter().chain(vec![0.0; 48_000 * 2]).collect();
            let (out, _) = run(&mut fx, &x, &x);
            rms(&out[48_000 * 3..48_000 * 4])
        };
        let (none, some, lots) = (tail(0.0), tail(40.0), tail(100.0));
        eprintln!("noise bed in silence: {none:.5} / {some:.5} / {lots:.5}");
        assert!(some > none * 1.5 && lots > some * 1.3);
        assert!(lots < 0.2, "a soft bed, not a roar");
    }

    #[test]
    fn stays_within_two_db_of_the_dry_programme_at_default_settings() {
        for level in [0.5f32, 1.0] {
            let mut fx = banded(BeatFxKind::PulseVerb, 1.0, level);
            fx.set_enabled(true);
            let x = music(16.0, 120.0);
            let (out, _) = run(&mut fx, &x, &x);
            let gain = db(rms(&out[48_000 * 4..]) / rms(&x[48_000 * 4..]));
            eprintln!("pulse verb level {level}: {gain:+.2} dB against the dry programme");
            assert!(gain < 2.0 && gain > -3.0, "level {level}: {gain:+.2} dB");
        }
    }

    #[test]
    fn stays_finite_and_bounded_for_thirty_seconds_of_worst_case_input() {
        for (name, x) in [
            ("noise", noise(48_000 * 30, 99, 1.0)),
            ("square", (0..48_000 * 30).map(|i| if (i / 55) % 2 == 0 { 1.0 } else { -1.0 }).collect()),
            ("sine", sine(48_000 * 30, 330.0, 1.0)),
        ] {
            let mut fx = enabled(0.125, 1.0);
            fx.set_param(1, 100.0);
            fx.set_param(2, 100.0);
            fx.set_param(3, 1.0);
            fx.set_param(4, 0.0);
            fx.set_param(0, 100.0);
            let (out, _) = run(&mut fx, &x, &x);
            assert!(out.iter().all(|v| v.is_finite()), "{name}");
            assert!(peak(&out) < 4.0, "{name}: peak {}", peak(&out));
        }
    }

    #[test]
    fn tail_rings_after_disable_then_the_unit_goes_idle() {
        let mut fx = enabled(1.0, 0.75);
        let x = noise(48_000, 12, 0.3);
        let _ = run(&mut fx, &x, &x);
        fx.set_enabled(false);
        let silence = vec![0.0; 48_000 * 9];
        let (out, _) = run(&mut fx, &silence, &silence);
        assert!(rms(&out[2_000..20_000]) > 0.003, "audible tail");
        assert!(rms(&out[48_000 * 7..]) < rms(&out[2_000..20_000]) * 0.3);
        assert!(fx.is_idle());
    }
}
