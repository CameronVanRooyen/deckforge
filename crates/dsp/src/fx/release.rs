use std::f32::consts::PI;

use deckforge_common::ReleaseFxKind;

use super::ring::StereoRing;
use super::util::OnePole;
use crate::limiter::soft_limit;

const HISTORY_SECS: f32 = 10.0;
const ECHO_SECS: f32 = 4.0;
const MAX_LENGTH_SECS: f64 = 6.0;
const DRY_CUT_MS: f32 = 10.0;
const ENTRY_FADE_MS: f32 = 5.0;
const RETURN_MS: f32 = 30.0;
const ECHO_REPEATS: f64 = 8.0;
const ECHO_TAIL_DB: f64 = -36.0;
const ECHO_HP_HZ: f32 = 200.0;
const ECHO_LP_HZ: f32 = 4_500.0;
const ECHO_MIN_BEATS: f64 = 0.125;
const ECHO_MAX_BEATS: f64 = 1.0;
const SPIN_UP_SHARE: f64 = 0.1;
const SPIN_PEAK_REVERSE: f64 = 3.0;
const SPIN_FADE_FROM: f64 = 0.7;
const BRAKE_FADE_FROM: f64 = 0.8;
const ROLL_BEATS: f64 = 0.25;
const ROLL_SEAM: f64 = 64.0;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Idle,
    Running,
    Returning,
}

/// Release FX on a live signal: Echo Out, Backspin, Vinyl Brake and Roll Out, after which the signal returns to normal.
/// Keeps a rolling history so a transition can start at any moment; `process` never allocates.
pub struct ReleaseFx {
    sample_rate: f32,
    history: StereoRing,
    echo: StereoRing,
    echo_hp: [OnePole; 2],
    echo_lp: [OnePole; 2],
    kind: ReleaseFxKind,
    phase: Phase,
    total: f64,
    elapsed: f64,
    delay: f64,
    rate: f64,
    echo_delay: f32,
    echo_feedback: f32,
    roll_len: f64,
    fade: f32,
}

fn ms_step(sample_rate: f32, ms: f32) -> f32 {
    1.0 / (ms * 0.001 * sample_rate).max(1.0)
}

fn smoothstep(edge0: f64, edge1: f64, x: f64) -> f64 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl ReleaseFx {
    pub fn new(sample_rate: f32) -> Self {
        let mut fx = Self {
            sample_rate,
            history: StereoRing::new((HISTORY_SECS * sample_rate) as usize),
            echo: StereoRing::new((ECHO_SECS * sample_rate) as usize),
            echo_hp: [OnePole::default(); 2],
            echo_lp: [OnePole::default(); 2],
            kind: ReleaseFxKind::EchoOut,
            phase: Phase::Idle,
            total: 1.0,
            elapsed: 0.0,
            delay: 1.0,
            rate: 1.0,
            echo_delay: 1.0,
            echo_feedback: 0.0,
            roll_len: 1.0,
            fade: 0.0,
        };
        for ch in 0..2 {
            fx.echo_hp[ch].set_cutoff(ECHO_HP_HZ, sample_rate);
            fx.echo_lp[ch].set_cutoff(ECHO_LP_HZ, sample_rate);
        }
        fx
    }

    pub fn is_running(&self) -> bool {
        self.phase == Phase::Running
    }

    pub fn running(&self) -> Option<ReleaseFxKind> {
        self.is_running().then_some(self.kind)
    }

    /// Fraction of the transition played, while it runs.
    pub fn progress(&self) -> Option<f32> {
        self.is_running().then(|| (self.elapsed / self.total).clamp(0.0, 1.0) as f32)
    }

    /// Starts a transition of `beats` at `bpm`; restarting while one runs begins again from the live signal.
    pub fn trigger(&mut self, kind: ReleaseFxKind, beats: f64, bpm: f64) {
        let sr = self.sample_rate as f64;
        let beat = 60.0 / bpm.clamp(30.0, 400.0);
        let beats = if beats.is_finite() && beats > 0.0 { beats } else { 1.0 };
        let restart_fade = if self.phase == Phase::Running { self.fade } else { 0.0 };
        self.kind = kind;
        self.total = ((beats * beat).min(MAX_LENGTH_SECS) * sr).max(1.0);
        self.elapsed = 0.0;
        self.delay = 1.0;
        self.rate = 1.0;
        let echo_beats = (beats / ECHO_REPEATS).clamp(ECHO_MIN_BEATS, ECHO_MAX_BEATS);
        self.echo_delay = ((echo_beats * beat * sr) as f32).clamp(2.0, (self.echo.capacity() - 4) as f32);
        let repeats = self.total / self.echo_delay as f64;
        self.echo_feedback = 10f64.powf(ECHO_TAIL_DB / 20.0 / repeats.max(1.0)).min(0.9) as f32;
        self.roll_len = (ROLL_BEATS * beat * sr).max(ROLL_SEAM * 2.0);
        self.fade = restart_fade;
        self.phase = Phase::Running;
    }

    /// Ends a running transition early and lets the signal return.
    pub fn cancel(&mut self) {
        if self.phase == Phase::Running {
            self.phase = Phase::Returning;
            self.fade = 0.0;
        }
    }

    pub fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        let n = l.len().min(r.len());
        for i in 0..n {
            let (xl, xr) = (l[i], r[i]);
            self.history.write(xl, xr);
            let (yl, yr) = match self.phase {
                Phase::Idle => {
                    self.echo.write(xl, xr);
                    (xl, xr)
                }
                Phase::Running => self.run(xl, xr),
                Phase::Returning => {
                    self.echo.write(xl, xr);
                    self.fade = (self.fade + ms_step(self.sample_rate, RETURN_MS)).min(1.0);
                    if self.fade >= 1.0 {
                        self.phase = Phase::Idle;
                    }
                    (xl * self.fade, xr * self.fade)
                }
            };
            l[i] = yl;
            r[i] = yr;
        }
    }

    fn run(&mut self, xl: f32, xr: f32) -> (f32, f32) {
        let u = self.elapsed / self.total;
        let out = match self.kind {
            ReleaseFxKind::EchoOut => self.echo_out(xl, xr, u),
            ReleaseFxKind::Backspin => self.backspin(xl, xr, u),
            ReleaseFxKind::VinylBrake => self.brake(xl, xr, u),
            ReleaseFxKind::RollOut => self.roll_out(xl, xr, u),
        };
        if self.kind != ReleaseFxKind::EchoOut {
            self.echo.write(xl, xr);
        }
        self.elapsed += 1.0;
        if self.elapsed >= self.total {
            self.phase = Phase::Returning;
            self.fade = 0.0;
        }
        out
    }

    fn echo_out(&mut self, xl: f32, xr: f32, u: f64) -> (f32, f32) {
        self.fade = (self.fade + ms_step(self.sample_rate, DRY_CUT_MS)).min(1.0);
        let (el, er) = self.echo.read(self.echo_delay);
        let g = self.echo_feedback;
        let fl = self.echo_lp[0].lp(self.echo_hp[0].hp(el));
        let fr = self.echo_lp[1].lp(self.echo_hp[1].hp(er));
        self.echo.write(soft_limit(fl * g), soft_limit(fr * g));
        let tail = (1.0 - smoothstep(0.85, 1.0, u)) as f32;
        let dry = 1.0 - self.fade;
        (xl * dry + el * tail, xr * dry + er * tail)
    }

    fn read_history(&self) -> (f32, f32) {
        let max = (self.history.capacity() - 4) as f64;
        if self.delay >= max { (0.0, 0.0) } else { self.history.read(self.delay as f32) }
    }

    fn entry(&mut self, xl: f32, xr: f32, wl: f32, wr: f32) -> (f32, f32) {
        self.fade = (self.fade + ms_step(self.sample_rate, ENTRY_FADE_MS)).min(1.0);
        let f = self.fade;
        (xl + (wl - xl) * f, xr + (wr - xr) * f)
    }

    fn backspin(&mut self, xl: f32, xr: f32, u: f64) -> (f32, f32) {
        self.rate = if u < SPIN_UP_SHARE {
            let t = u / SPIN_UP_SHARE;
            1.0 - (1.0 + SPIN_PEAK_REVERSE) * 0.5 * (1.0 - (PI as f64 * t).cos())
        } else {
            let t = (u - SPIN_UP_SHARE) / (1.0 - SPIN_UP_SHARE);
            -SPIN_PEAK_REVERSE * (1.0 - t) * (1.0 - t)
        };
        let (wl, wr) = self.read_history();
        self.delay += 1.0 - self.rate;
        let env = (1.0 - smoothstep(SPIN_FADE_FROM, 1.0, u)) as f32;
        self.entry(xl, xr, wl * env, wr * env)
    }

    fn brake(&mut self, xl: f32, xr: f32, u: f64) -> (f32, f32) {
        self.rate = 1.0 - u;
        let (wl, wr) = self.read_history();
        self.delay += 1.0 - self.rate;
        let env = (1.0 - smoothstep(BRAKE_FADE_FROM, 1.0, u)) as f32;
        self.entry(xl, xr, wl * env, wr * env)
    }

    fn roll_out(&mut self, xl: f32, xr: f32, u: f64) -> (f32, f32) {
        let k = self.elapsed;
        let at = k % self.roll_len;
        self.delay = k + self.roll_len - at + 1.0;
        let seam = (at / ROLL_SEAM).min((self.roll_len - at) / ROLL_SEAM).min(1.0) as f32;
        let (wl, wr) = self.read_history();
        let env = (1.0 - u) as f32 * seam;
        self.entry(xl, xr, wl * env, wr * env)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alloc_guard::allocations_in;
    use crate::fx::testkit::{white, SR};

    const BPM: f64 = 120.0;

    fn run(fx: &mut ReleaseFx, input: &[f32]) -> Vec<f32> {
        let mut l = input.to_vec();
        let mut r = input.to_vec();
        for (cl, cr) in l.chunks_mut(256).zip(r.chunks_mut(256)) {
            fx.process(cl, cr);
        }
        l
    }

    fn sine(len: usize, hz: f32, from: usize) -> Vec<f32> {
        (0..len).map(|i| 0.5 * (2.0 * PI * hz * (i + from) as f32 / SR).sin()).collect()
    }

    fn rms(v: &[f32]) -> f32 {
        (v.iter().map(|x| x * x).sum::<f32>() / v.len().max(1) as f32).sqrt()
    }

    fn crossings(v: &[f32]) -> usize {
        v.windows(2).filter(|w| w[0] <= 0.0 && w[1] > 0.0).count()
    }

    #[test]
    fn idle_passes_the_signal_and_every_kind_returns_to_it_afterwards() {
        for kind in [ReleaseFxKind::EchoOut, ReleaseFxKind::Backspin, ReleaseFxKind::VinylBrake, ReleaseFxKind::RollOut] {
            let mut fx = ReleaseFx::new(SR);
            let x = white(SR as usize, 3, 0.3);
            assert_eq!(run(&mut fx, &x), x, "{kind:?} idle");
            fx.trigger(kind, 1.0, BPM);
            assert!(fx.is_running());
            let _ = run(&mut fx, &white(SR as usize / 2 + 2_000, 4, 0.3));
            assert!(!fx.is_running(), "{kind:?} finished");
            let x = white(4_000, 5, 0.3);
            let y = run(&mut fx, &x);
            assert_eq!(&y[2_000..], &x[2_000..], "{kind:?} back to normal");
        }
    }

    #[test]
    fn echo_out_cuts_the_live_signal_and_rings_the_last_beat() {
        let mut fx = ReleaseFx::new(SR);
        let _ = run(&mut fx, &white(SR as usize, 7, 0.3));
        fx.trigger(ReleaseFxKind::EchoOut, 2.0, BPM);
        let after = sine(SR as usize / 2, 1_000.0, 0);
        let y = run(&mut fx, &after);
        let window = &y[2_400..12_000];
        assert!(rms(window) > 0.05, "echo tail is heard");
        let leak = crate::fx::testkit::tone_amplitude(window, 1_000.0);
        assert!(leak < 0.01, "new input does not reach the output: {leak}");
        assert!(rms(&y[y.len() - 2_000..]) < rms(&y[..4_000]) * 0.2, "tail decays by the end");
    }

    #[test]
    fn vinyl_brake_slows_the_pitch_down_to_a_stop() {
        let mut fx = ReleaseFx::new(SR);
        let len = SR as usize * 2;
        let x = sine(len, 1_000.0, 0);
        let _ = run(&mut fx, &x[..SR as usize]);
        fx.trigger(ReleaseFxKind::VinylBrake, 2.0, BPM);
        let y = run(&mut fx, &x[SR as usize..]);
        let q = SR as usize / 8;
        let early = crossings(&y[..q]);
        let late = crossings(&y[6 * q..7 * q]);
        assert!(early as f32 > 90.0, "starts near full speed: {early}");
        assert!((late as f32) < early as f32 * 0.5, "slows down: {early} -> {late}");
    }

    #[test]
    fn backspin_plays_backwards_and_roll_out_fades() {
        let mut fx = ReleaseFx::new(SR);
        let x = white(SR as usize * 2, 9, 0.3);
        let _ = run(&mut fx, &x[..SR as usize]);
        fx.trigger(ReleaseFxKind::Backspin, 2.0, BPM);
        let _ = run(&mut fx, &x[SR as usize..SR as usize + 12_000]);
        assert!(fx.rate < -1.0, "spinning backwards at {}", fx.rate);

        let mut fx = ReleaseFx::new(SR);
        let _ = run(&mut fx, &x[..SR as usize]);
        fx.trigger(ReleaseFxKind::RollOut, 2.0, BPM);
        let y = run(&mut fx, &x[SR as usize..SR as usize + SR as usize]);
        assert!(rms(&y[..8_000]) > rms(&y[36_000..44_000]) * 3.0, "roll fades out");
    }

    #[test]
    fn processing_does_not_allocate() {
        let mut fx = ReleaseFx::new(SR);
        fx.trigger(ReleaseFxKind::Backspin, 4.0, BPM);
        let mut l = vec![0.1f32; 512];
        let mut r = vec![0.1f32; 512];
        let (_, allocations) = allocations_in(|| fx.process(&mut l, &mut r));
        assert_eq!(allocations, 0);
    }
}
