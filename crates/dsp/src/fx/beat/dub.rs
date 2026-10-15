use std::f32::consts::TAU;

use super::*;

pub(super) const RELEASE_FEEDBACK: f32 = 0.8;
const WET_NORM: f32 = 0.8;
const FEEDBACK_SMOOTH_MS: f32 = 25.0;
const GATE_MS: f32 = 6.0;
const HOLD_MS: f32 = 30.0;
const DRY_MS: f32 = 8.0;
const WOW_HZ: f32 = 0.6;
const FLUTTER_HZ: f32 = 6.5;
const WOW_CENTS: f32 = 10.0;
const FLUTTER_CENTS: f32 = 3.0;
const TAPE_SLEW: f32 = 0.35;
const RESET_JUMP: f32 = 0.015;
const LP_MIN_HZ: f32 = 1_000.0;
const LP_SPAN: f32 = 12.0;
const HP_MAX_HZ: f32 = 270.0;
const HP_MIN_HZ: f32 = 90.0;
const DRIVE_SPAN: f32 = 3.0;
const RIGHT_OFFSET_MS: f32 = 0.5;
const FREEZE_LP_HZ: f32 = 21_000.0;
const FREEZE_HP_HZ: f32 = 20.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TimeMode {
    Clean,
    Tape,
    Reset,
}

impl TimeMode {
    fn from_param(v: f32) -> Self {
        match v.round() as i32 {
            1 => Self::Tape,
            2 => Self::Reset,
            _ => Self::Clean,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct DubState {
    feedback: Smoothed,
    throw: Smoothed,
    freeze: Smoothed,
    dry: Smoothed,
    pub(super) throw_held: bool,
    pub(super) freeze_held: bool,
    wow: f32,
    flutter: f32,
    delay_cur: f32,
    reset_gate: SwapGate,
    last_target: f32,
    pending_target: f32,
}

impl DubState {
    pub(super) fn new(sample_rate: f32) -> Self {
        Self {
            feedback: Smoothed::new(0.45, sample_rate, FEEDBACK_SMOOTH_MS),
            throw: Smoothed::new(1.0, sample_rate, GATE_MS),
            freeze: Smoothed::new(0.0, sample_rate, HOLD_MS),
            dry: Smoothed::new(1.0, sample_rate, DRY_MS),
            throw_held: false,
            freeze_held: false,
            wow: 0.0,
            flutter: 0.0,
            delay_cur: 0.0,
            reset_gate: SwapGate::new(sample_rate),
            last_target: 0.0,
            pending_target: 0.0,
        }
    }

    pub(super) fn reset(&mut self) {
        self.feedback.reset(self.feedback.target());
        self.throw.reset(1.0);
        self.freeze.reset(0.0);
        self.dry.reset(1.0);
        self.throw_held = false;
        self.freeze_held = false;
        self.reset_gate.reset_open();
        self.last_target = 0.0;
    }

    pub(super) fn holding(&self) -> bool {
        self.throw_held || self.freeze_held
    }

    pub(super) fn release_holds(&mut self) {
        self.throw_held = false;
        self.freeze_held = false;
    }
}

fn depth_samples(cents: f32, hz: f32, sample_rate: f32) -> f32 {
    ((cents / 1200.0).exp2() - 1.0) * sample_rate / (TAU * hz)
}

impl BeatFx {
    pub(super) fn run_dub_tape(&mut self, l: &mut [f32], r: &mut [f32]) {
        let sr = self.sample_rate;
        let level_fb = (self.param(0) * 0.01).min(1.05);
        let tone = self.param(1) * 0.01;
        let drive = 1.0 + DRIVE_SPAN * self.param(2) * 0.01;
        let wobble = self.param(3) * 0.01;
        let mode = TimeMode::from_param(self.param(4));
        let throw_mode = self.param(5) >= 0.5;
        let wet_only = self.param(6) >= 0.5;
        if !self.enabled {
            self.dub.release_holds();
        }
        let freezing = self.dub.freeze_held;
        let feedback_target = if freezing { 1.0 } else if self.enabled { level_fb } else { level_fb.min(RELEASE_FEEDBACK) };
        let throw_open = if !throw_mode || self.dub.throw_held { 1.0 } else { 0.0 };
        let dry_target = if wet_only { 0.0 } else { 1.0 };
        self.dub.feedback.set(feedback_target);
        self.dub.freeze.set(if freezing { 1.0 } else { 0.0 });
        self.dub.throw.set(throw_open);
        self.dub.dry.set(dry_target);
        let (lp_hz, hp_hz) = if freezing {
            (FREEZE_LP_HZ, FREEZE_HP_HZ)
        } else {
            (LP_MIN_HZ * LP_SPAN.powf(tone), HP_MAX_HZ * (HP_MIN_HZ / HP_MAX_HZ).powf(tone))
        };
        for ch in 0..2 {
            self.loop_lp[ch].set(lp_hz, sr);
            self.loop_hp[ch].set(hp_hz, sr);
        }
        let target = self.delay_target().max(2.0);
        if mode != TimeMode::Tape {
            self.dub.delay_cur = self.taps.delay();
        }
        if mode != TimeMode::Reset {
            self.dub.last_target = target;
        }
        if self.delay_snap {
            self.dub.delay_cur = target;
            self.dub.last_target = target;
            self.taps.snap(target);
            self.delay_snap = false;
            self.dub.feedback.reset(feedback_target);
            self.dub.throw.reset(throw_open);
            self.dub.dry.reset(dry_target);
        }
        let grid_inc = 4.0 / (self.beat_secs() * sr);
        let right_offset = RIGHT_OFFSET_MS * sr / 1000.0;
        let wow_depth = depth_samples(WOW_CENTS * wobble, WOW_HZ, sr);
        let flutter_depth = depth_samples(FLUTTER_CENTS * wobble, FLUTTER_HZ, sr);
        for i in 0..l.len() {
            self.grid16 += grid_inc;
            let boundary = self.grid16 >= 1.0;
            if boundary {
                self.grid16 -= 1.0;
            }
            let lvl = self.level.next();
            let send = self.send.next();
            let gate = self.swap.next();
            let freeze = self.dub.freeze.next();
            let throw = self.dub.throw.next();
            let fb = self.dub.feedback.next();
            let dry = self.dub.dry.next();
            self.dub.wow = (self.dub.wow + WOW_HZ / sr).fract();
            self.dub.flutter = (self.dub.flutter + FLUTTER_HZ / sr).fract();
            let wobble_at = |offset: f32, d: &DubState| wow_depth * (TAU * (d.wow + offset)).sin() + flutter_depth * (TAU * (d.flutter + offset)).sin();
            let (mod_l, mod_r) = (wobble_at(0.0, &self.dub) * (1.0 - freeze), wobble_at(0.25, &self.dub) * (1.0 - freeze));
            let dip = match mode {
                TimeMode::Reset => self.dub_reset_step(target),
                _ => 1.0,
            };
            let (yl, yr) = match mode {
                TimeMode::Clean => {
                    self.taps.follow(target, boundary, &self.ring);
                    self.taps.read_modulated(&self.ring, right_offset, mod_l, mod_r)
                }
                TimeMode::Tape => {
                    self.dub.delay_cur += (target - self.dub.delay_cur).clamp(-TAPE_SLEW, TAPE_SLEW);
                    (self.ring.read_l(self.dub.delay_cur + mod_l), self.ring.read_r(self.dub.delay_cur + right_offset + mod_r))
                }
                TimeMode::Reset => self.taps.read_modulated(&self.ring, right_offset, mod_l, mod_r),
            };
            let (xl, xr) = (l[i], r[i]);
            let (fl, fr) = (self.dub_stage(0, yl, drive, freeze), self.dub_stage(1, yr, drive, freeze));
            let feed = send * throw * (1.0 - freeze);
            self.ring.write(soft_limit(xl * feed + fb * fl), soft_limit(xr * feed + fb * fr));
            let (dry_law, wet_law) = beat_mix(lvl);
            let wet = wet_law * WET_NORM * gate * dip;
            let dry_gain = dry * gated_dry(dry_law, send * gate);
            l[i] = xl * dry_gain + yl * wet;
            r[i] = xr * dry_gain + yr * wet;
        }
    }

    #[inline]
    fn dub_stage(&mut self, ch: usize, y: f32, drive: f32, freeze: f32) -> f32 {
        let filtered = self.loop_hp[ch].process(self.loop_lp[ch].process(y));
        let saturated = (drive * filtered).tanh() / drive;
        saturated + (filtered - saturated) * freeze
    }

    /// Reset mode: a big time change dips the wet path, clears the loop and restarts it at the new time.
    #[inline]
    fn dub_reset_step(&mut self, target: f32) -> f32 {
        if !self.dub.reset_gate.is_swapping() && (target - self.dub.last_target).abs() > RESET_JUMP * self.dub.last_target.max(1.0) {
            self.dub.pending_target = target;
            self.dub.reset_gate.begin();
        }
        let dip = self.dub.reset_gate.next();
        if self.dub.reset_gate.ready() {
            self.ring.reset();
            self.loop_lp.iter_mut().for_each(LowPass::reset);
            self.loop_hp.iter_mut().for_each(HighPass::reset);
            self.taps.snap(self.dub.pending_target);
            self.dub.last_target = self.dub.pending_target;
            self.dub.reset_gate.finish();
        }
        dip
    }
}

#[cfg(test)]
mod tests {
    use std::f32::consts::TAU;

    use super::super::testing::*;
    use super::*;
    use crate::fx::testkit::{crossing_hz, cents_deviation, max_step, semitones};

    fn dub(beats: f32, level: f32) -> BeatFx {
        let mut fx = unit(BeatFxKind::DubTape, beats, level);
        fx.set_enabled(true);
        fx
    }

    fn block_run(fx: &mut BeatFx, x: &[f32], mut each: impl FnMut(&mut BeatFx, usize)) -> Vec<f32> {
        let (mut l, mut r) = (x.to_vec(), x.to_vec());
        for (b, (cl, cr)) in l.chunks_mut(BLOCK).zip(r.chunks_mut(BLOCK)).enumerate() {
            each(fx, b);
            fx.process(cl, cr);
        }
        l
    }

    #[test]
    fn clean_mode_keeps_a_sine_within_five_cents_across_time_changes() {
        let divisions = [0.25f32, 0.5, 0.75, 1.0, 0.25];
        let seg = 2 * 48_000;
        let x = sine(seg * divisions.len(), 1_000.0, 0.3);
        let mut fx = banded(BeatFxKind::DubTape, divisions[0], 0.9);
        fx.set_bpm(127.3);
        fx.set_param(3, 0.0);
        fx.set_enabled(true);
        let out = block_run(&mut fx, &x, |fx, b| {
            let start = b * BLOCK;
            if start.is_multiple_of(seg) && start > 0 {
                fx.set_beats(divisions[start / seg]);
            }
        });
        let settled = &out[seg / 2..];
        let worst = cents_deviation(settled, 1_000.0, 480).iter().fold(0.0f32, |m, c| m.max(c.abs()));
        let step = max_step(settled);
        let sine_step = peak(settled) * TAU * 1_000.0 / SR;
        eprintln!("dub tape clean: worst {worst:.2} cents, step {step:.4} vs {sine_step:.4}");
        assert!(worst < 5.0, "{worst} cents");
        assert!(step < 1.25 * sine_step);
    }

    #[test]
    fn tape_mode_bends_the_repeats_when_the_time_changes() {
        let mut fx = dub(0.5, 0.75);
        fx.set_param(4, 1.0);
        fx.set_param(3, 0.0);
        let x = sine(48_000 * 8, 220.0, 0.3);
        let out = block_run(&mut fx, &x, |fx, b| {
            if b == 48_000 * 4 / BLOCK {
                fx.set_beats(1.0);
            }
        });
        let wet: Vec<f32> = out.iter().zip(&x).map(|(o, d)| o - d).collect();
        let at = 48_000 * 4;
        let before = crossing_hz(&wet[at - 12_000..at - 2_000]);
        let during = crossing_hz(&wet[at + 4_000..at + 12_000]);
        eprintln!("tape mode: {before:.1} Hz -> {during:.1} Hz");
        assert!(semitones(before, during).abs() > 0.5, "the glide bends the loop");
    }

    #[test]
    fn reset_mode_clears_the_old_repeats_on_a_time_change() {
        let measure = |mode: f32| {
            let mut fx = dub(0.5, 1.0);
            fx.set_param(4, mode);
            fx.set_param(0, 90.0);
            let burst: Vec<f32> = noise(24_000, 3, 0.4).into_iter().chain(vec![0.0; 48_000 * 6]).collect();
            let out = block_run(&mut fx, &burst, |fx, b| {
                if b == 48_000 * 2 / BLOCK {
                    fx.set_beats(2.0);
                }
            });
            rms(&out[48_000 * 3..48_000 * 4])
        };
        let (reset, clean) = (measure(2.0), measure(0.0));
        eprintln!("after the change: reset {reset:.5}, clean {clean:.5}");
        assert!(reset < 0.05 * clean, "reset starts from an empty loop, clean keeps the repeats");
    }

    #[test]
    fn self_oscillation_at_one_oh_five_feedback_is_bounded_for_sixty_seconds() {
        for (name, drive) in [("default drive", 25.0f32), ("no drive", 0.0)] {
            let mut fx = dub(0.75, 1.0);
            fx.set_param(0, 105.0);
            fx.set_param(2, drive);
            let burst: Vec<f32> = noise(48_000 * 3, 21, 0.5).into_iter().chain(vec![0.0; 48_000 * 57]).collect();
            let out = block_run(&mut fx, &burst, |_, _| {});
            assert!(out.iter().all(|v| v.is_finite()), "{name}");
            let worst = peak(&out);
            let late = rms(&out[48_000 * 50..]);
            let mid = rms(&out[48_000 * 30..48_000 * 40]);
            eprintln!("{name}: peak {worst:.3}, rms {mid:.3} at 30-40 s, {late:.3} at 50-60 s");
            assert!(worst < 1.5, "{name}: peak {worst}");
            assert!(late > 0.01, "{name}: the loop keeps ringing");
            assert!((db(late / mid)).abs() < 3.0, "{name}: steady, not growing");
        }
    }

    #[test]
    fn throw_only_sends_while_held() {
        let mut fx = dub(0.5, 1.0);
        fx.set_param(5, 1.0);
        fx.set_param(6, 1.0);
        let x = noise(48_000 * 10, 6, 0.4);
        let out = block_run(&mut fx, &x, |fx, b| {
            let t = b * BLOCK;
            if t == 48_000 * 4 {
                fx.trigger(FxTrigger::ThrowOn);
            }
            if t == 48_000 * 4 + 12_288 {
                fx.trigger(FxTrigger::ThrowOff);
            }
        });
        let hold_end = 48_000 * 4 + 12_288;
        let gated_out = rms(&out[..48_000 * 4]);
        let during = rms(&out[48_000 * 4 + 6_000..hold_end]);
        let ringing = rms(&out[hold_end + 6_000..hold_end + 30_000]);
        let after = rms(&out[48_000 * 8..]);
        eprintln!("throw: {gated_out:.5} before, {during:.4} while held, {ringing:.4} ringing, {after:.5} later");
        assert!(gated_out < 1e-4, "nothing passes before the throw");
        assert!(during > 0.005, "thrown audio circulates");
        assert!(ringing > 0.0005, "the repeats keep going after release");
        assert!(after < ringing * 0.1, "and decay: {after}");
        let mut quiet = dub(0.5, 1.0);
        quiet.set_param(5, 1.0);
        quiet.set_param(6, 1.0);
        let out = block_run(&mut quiet, &x, |_, _| {});
        assert!(rms(&out) < 1e-4, "never held, never sends");
    }

    #[test]
    fn freeze_holds_the_loop_without_growth() {
        let mut fx = dub(0.25, 1.0);
        let mut z = 0.0f32;
        let x: Vec<f32> = noise(48_000 * 40, 33, 0.8).into_iter().map(|v| { z += 0.3 * (v - z); z }).collect();
        let out = block_run(&mut fx, &x, |fx, b| {
            if b * BLOCK == 48_000 * 2 {
                fx.trigger(FxTrigger::FreezeOn);
            }
        });
        assert!(fx.held());
        let early = rms(&out[48_000 * 6..48_000 * 12]);
        let late = rms(&out[48_000 * 30..48_000 * 40]);
        eprintln!("freeze: {early:.4} at 6-12 s, {late:.4} at 30-40 s");
        assert!(out.iter().all(|v| v.is_finite()) && peak(&out) < 2.0);
        assert!(db(late / early).abs() < 2.0, "holds level: {:+.2} dB", db(late / early));
    }

    #[test]
    fn freeze_closes_the_input() {
        let mut fx = dub(0.25, 1.0);
        fx.set_param(6, 1.0);
        let mut x = noise(48_000 * 4, 61, 0.4);
        x[..48_000].fill(0.0);
        let out = block_run(&mut fx, &x, |fx, b| {
            if b * BLOCK == 0 {
                fx.trigger(FxTrigger::FreezeOn);
            }
        });
        assert!(rms(&out[48_000 * 2..]) < 1e-4, "a frozen empty loop stays empty");
    }

    #[test]
    fn wet_only_removes_the_dry_signal() {
        let mut fx = dub(1.0, 1.0);
        fx.set_param(6, 1.0);
        let x = sine(48_000 * 2, 440.0, 0.5);
        let out = block_run(&mut fx, &x, |_, _| {});
        assert!(peak(&out[2_000..20_000]) < 1e-3, "before the first repeat only silence");
        assert!(rms(&out[30_000..60_000]) > 0.05, "then the repeats");
    }

    #[test]
    fn loudness_stays_within_two_db_of_dry_at_default_settings() {
        for level in [0.5f32, 1.0] {
            let mut fx = dub(0.75, level);
            let x = music(16.0, 120.0);
            let out = block_run(&mut fx, &x, |_, _| {});
            let gain = db(rms(&out[48_000 * 4..]) / rms(&x[48_000 * 4..]));
            eprintln!("dub tape level {level}: {gain:+.2} dB");
            assert!(gain < 2.0 && gain > -3.0, "level {level}: {gain:+.2} dB");
        }
    }

    #[test]
    fn stays_finite_and_bounded_for_thirty_seconds_with_every_control_ridden() {
        for (name, x) in [
            ("noise", noise(48_000 * 30, 99, 1.0)),
            ("square", (0..48_000 * 30).map(|i| if (i / 55) % 2 == 0 { 1.0 } else { -1.0 }).collect()),
            ("sine", sine(48_000 * 30, 330.0, 1.0)),
        ] {
            let mut fx = dub(0.5, 1.0);
            fx.set_param(0, 105.0);
            fx.set_param(3, 100.0);
            fx.set_param(2, 100.0);
            let out = block_run(&mut fx, &x, |fx, b| {
                if b % 400 == 0 {
                    fx.set_beats([0.5, 2.0, 0.125, 16.0, 1.0][(b / 400) % 5]);
                    fx.set_param(4, (b / 400 % 3) as f32);
                }
                if b % 700 == 0 {
                    fx.trigger(if (b / 700) % 2 == 0 { FxTrigger::FreezeOn } else { FxTrigger::FreezeOff });
                }
            });
            assert!(out.iter().all(|v| v.is_finite()), "{name}");
            assert!(peak(&out) < 3.0, "{name}: peak {}", peak(&out));
        }
    }

    #[test]
    fn disabling_lets_a_runaway_loop_ring_out_and_go_idle() {
        let mut fx = dub(0.5, 1.0);
        fx.set_param(0, 105.0);
        let x = noise(48_000 * 4, 4, 0.5);
        let _ = block_run(&mut fx, &x, |_, _| {});
        fx.set_enabled(false);
        let silence = vec![0.0; 48_000 * 70];
        let out = block_run(&mut fx, &silence, |_, _| {});
        assert!(rms(&out[..24_000]) > 0.01);
        assert!(rms(&out[48_000 * 60..]) < 1e-3, "the tail dies");
        assert!(fx.is_idle());
    }
}
