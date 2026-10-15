use crate::svf::Svf;

const INDEX_MAX: f32 = 255.0;
const INDEX_STEP_SECS: f32 = 0.000_333_333;
const WEIGHT_RAMP_SECS: f32 = 0.01;
const LPF_OPEN_INDEX: f32 = 126.0;
const HPF_OPEN_INDEX: f32 = 130.0;
const BLEND_STEPS: f32 = 16.0;
const SWEEP_STEPS: f32 = 126.0;
const LPF_MIN_HZ: f32 = 100.0;
const LPF_SPAN_HZ: f32 = 25_000.0;
const HPF_MIN_HZ: f32 = 20.0;
const HPF_SPAN_HZ: f32 = 8_200.0;
const PARAM_DAMPING: f32 = 0.5;
const HPF_SOFT_BELOW_HZ: f32 = 249.9;
const HPF_SOFT_SCALE: f32 = 35.652;
const HPF_SOFT_MAX_DAMPING: f32 = 2.0;
const HPF_SOFT_FLOOR: f32 = 0.03;
const CHUNK: usize = 16;

/// One position of the Color FX filter: analog corners, damping (1/Q) and the dry / low-pass / high-pass weights.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FilterSetting {
    pub lpf_hz: f32,
    pub hpf_hz: f32,
    pub lpf_damping: f32,
    pub hpf_damping: f32,
    pub weights: [f32; 3],
}

/// Knob -1..1 to rekordbox's 0..255 Color FX index; the centre (127.5) sits in the 126..130 dry zone.
pub fn filter_index(value: f32) -> f32 {
    (value.clamp(-1.0, 1.0) + 1.0) * 0.5 * INDEX_MAX
}

/// rekordbox's DJM-900NXS2 FILTER law at a fractional index.
pub fn filter_setting(index: f32) -> FilterSetting {
    let k = index.clamp(0.0, INDEX_MAX);
    let lpf_hz = LPF_MIN_HZ + LPF_SPAN_HZ * (k.min(LPF_OPEN_INDEX) / SWEEP_STEPS).powi(3);
    let rise = ((k - HPF_OPEN_INDEX).max(0.0) / SWEEP_STEPS).powi(3);
    let hpf_hz = HPF_MIN_HZ + HPF_SPAN_HZ * rise;
    let soft = if k > HPF_OPEN_INDEX && hpf_hz < HPF_SOFT_BELOW_HZ {
        (4.0 * (1.0 - HPF_SOFT_SCALE * rise).max(0.0).powi(3) + HPF_SOFT_FLOOR).min(HPF_SOFT_MAX_DAMPING)
    } else {
        0.0
    };
    let lp = ((LPF_OPEN_INDEX - k) / BLEND_STEPS).clamp(0.0, 1.0);
    let hp = ((k - HPF_OPEN_INDEX) / BLEND_STEPS).clamp(0.0, 1.0);
    FilterSetting { lpf_hz, hpf_hz, lpf_damping: PARAM_DAMPING, hpf_damping: PARAM_DAMPING.max(soft), weights: [1.0 - lp - hp, lp, hp] }
}

/// Bipolar Color FX filter after rekordbox's DJM-900NXS2 model: parallel 12 dB/oct low- and high-pass with a cubic
/// cutoff sweep, crossfaded against the dry signal over the first 16 index steps either side of the centre.
#[derive(Debug, Clone)]
pub struct DjFilter {
    sample_rate: f32,
    lp: [Svf; 2],
    hp: [Svf; 2],
    index: f32,
    target: f32,
    weights: [f32; 3],
    index_slew: f32,
    weight_step: f32,
}

impl DjFilter {
    pub fn new(sample_rate: f32) -> Self {
        let centre = filter_index(0.0);
        Self {
            sample_rate,
            lp: [Svf::default(); 2],
            hp: [Svf::default(); 2],
            index: centre,
            target: centre,
            weights: [1.0, 0.0, 0.0],
            index_slew: CHUNK as f32 / (INDEX_STEP_SECS * sample_rate),
            weight_step: 1.0 / (WEIGHT_RAMP_SECS * sample_rate),
        }
    }

    pub fn set(&mut self, value: f32) {
        self.target = filter_index(value);
    }

    pub fn is_bypassed(&self) -> bool {
        let dry = [1.0, 0.0, 0.0];
        self.weights == dry && filter_setting(self.index).weights == dry && filter_setting(self.target).weights == dry
    }

    /// Processes a block in place; the index slews one step per 0.33 ms and the weights ramp over 10 ms, so sweeps never zipper.
    pub fn process_block(&mut self, left: &mut [f32], right: &mut [f32]) {
        if self.is_bypassed() {
            self.index = self.target;
            self.lp.iter_mut().chain(&mut self.hp).for_each(Svf::reset);
            return;
        }
        let n = left.len();
        let mut i = 0;
        while i < n {
            let end = (i + CHUNK).min(n);
            self.index += (self.target - self.index).clamp(-self.index_slew, self.index_slew);
            let s = filter_setting(self.index);
            let run_lp = s.weights[1] > 0.0 || self.weights[1] > 0.0;
            let run_hp = s.weights[2] > 0.0 || self.weights[2] > 0.0;
            for f in &mut self.lp {
                if run_lp { f.set_analog(s.lpf_hz, s.lpf_damping, self.sample_rate) } else { f.reset() }
            }
            for f in &mut self.hp {
                if run_hp { f.set_analog(s.hpf_hz, s.hpf_damping, self.sample_rate) } else { f.reset() }
            }
            for j in i..end {
                for (w, t) in self.weights.iter_mut().zip(s.weights) {
                    *w += (t - *w).clamp(-self.weight_step, self.weight_step);
                }
                let [dry, wl, wh] = self.weights;
                for (ch, buf) in [&mut *left, &mut *right].into_iter().enumerate() {
                    let x = buf[j];
                    let lo = if run_lp { self.lp[ch].process(x).low } else { 0.0 };
                    let hi = if run_hp { self.hp[ch].process(x).high } else { 0.0 };
                    buf[j] = dry * x + wl * lo + wh * hi;
                }
            }
            i = end;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::{PI, SQRT_2, TAU};

    const SR: f32 = 48_000.0;
    const HPF_TABLE: [(f32, f32); 5] = [(131.0, 20.004), (146.0, 36.790), (159.0, 119.98), (191.0, 950.45), (255.0, 8_026.3)];
    const LPF_TABLE: [(f32, f32); 5] = [(0.0, 100.0), (32.0, 509.52), (64.0, 3_376.2), (96.0, 11_157.1), (125.0, 24_509.5)];

    fn level_after(value: f32, freq: f32) -> f32 {
        let mut f = DjFilter::new(SR);
        f.set(value);
        let n = 9_600;
        let mut l = vec![0.0; n];
        for _ in 0..6 {
            l = (0..n).map(|i| (TAU * freq * i as f32 / SR).sin()).collect();
            let mut r = l.clone();
            f.process_block(&mut l, &mut r);
        }
        let tail = &l[n / 2..];
        (tail.iter().map(|x| x * x).sum::<f32>() / tail.len() as f32).sqrt() * SQRT_2
    }

    fn db(g: f32) -> f32 {
        20.0 * g.log10()
    }

    fn hpf_travel(fraction: f32) -> f32 {
        let centre = filter_index(0.0);
        (centre + fraction * (INDEX_MAX - centre)) / INDEX_MAX * 2.0 - 1.0
    }

    #[test]
    fn cutoffs_follow_the_decoded_cubic_law() {
        for (k, hz) in HPF_TABLE {
            let s = filter_setting(k);
            assert!((s.hpf_hz - hz).abs() < hz * 0.001, "HPF at {k}: {} vs {hz}", s.hpf_hz);
        }
        for (k, hz) in LPF_TABLE {
            let s = filter_setting(k);
            assert!((s.lpf_hz - hz).abs() < hz * 0.001, "LPF at {k}: {} vs {hz}", s.lpf_hz);
        }
        let between = filter_setting(159.5).hpf_hz;
        assert!(between > filter_setting(159.0).hpf_hz && between < filter_setting(160.0).hpf_hz);
    }

    #[test]
    fn knob_maps_to_the_index_and_blends_in_over_sixteen_steps() {
        assert!(filter_index(-1.0).abs() < 1e-6 && (filter_index(1.0) - 255.0).abs() < 1e-4);
        assert_eq!(filter_setting(filter_index(0.0)).weights, [1.0, 0.0, 0.0]);
        assert_eq!(filter_setting(118.0).weights, [0.5, 0.5, 0.0]);
        assert_eq!(filter_setting(110.0).weights, [0.0, 1.0, 0.0]);
        assert_eq!(filter_setting(138.0).weights, [0.5, 0.0, 0.5]);
        assert_eq!(filter_setting(146.0).weights, [0.0, 0.0, 1.0]);
    }

    #[test]
    fn hpf_starts_soft_and_settles_on_the_resonant_q() {
        assert!((filter_setting(146.0).hpf_damping - 2.0).abs() < 1e-3);
        assert!((filter_setting(159.375).hpf_damping - 0.689).abs() < 0.01);
        assert!((filter_setting(191.0).hpf_damping - 0.5).abs() < 1e-6);
        assert!((filter_setting(40.0).lpf_damping - 0.5).abs() < 1e-6);
    }

    #[test]
    fn analog_corner_lands_where_the_bilinear_map_puts_it() {
        let s = filter_setting(255.0);
        let digital = SR / PI * (PI * s.hpf_hz / SR).atan();
        assert!((db(level_after(1.0, digital)) - db(2.0)).abs() < 0.3, "Q = 2 peaks +6 dB at the corner");
    }

    #[test]
    fn quarter_hpf_travel_keeps_the_low_end_around_100_hz() {
        let v = hpf_travel(0.25);
        assert!((filter_setting(filter_index(v)).hpf_hz - 123.9).abs() < 0.5);
        assert!(db(level_after(v, 100.0)).abs() < 2.0, "100 Hz at 25 %: {} dB", db(level_after(v, 100.0)));
        assert!(db(level_after(v, 200.0)).abs() < 3.0);
        let ten = db(level_after(hpf_travel(0.1), 50.0));
        assert!(ten > -3.0, "10 % barely touches 50 Hz: {ten} dB");
    }

    #[test]
    fn center_is_transparent_and_extremes_cut() {
        assert!((level_after(0.0, 1_000.0) - 1.0).abs() < 1e-3);
        assert!(level_after(-1.0, 5_000.0) < 0.01);
        assert!(level_after(-1.0, 30.0) > 0.9);
        assert!(level_after(1.0, 100.0) < 0.01);
        assert!(level_after(1.0, 15_000.0) > 0.9);
    }

    #[test]
    fn low_pass_half_travel_still_passes_the_mids() {
        let at = db(level_after(-0.5, 2_000.0));
        assert!(at > -1.5, "LPF at half travel: {at} dB at 2 kHz");
    }

    #[test]
    fn sweeping_does_not_click() {
        let mut f = DjFilter::new(SR);
        let n = 48_000;
        let mut l: Vec<f32> = (0..n).map(|i| 0.5 * (TAU * 220.0 * i as f32 / SR).sin()).collect();
        let mut r = l.clone();
        for (b, (cl, cr)) in l.chunks_mut(256).zip(r.chunks_mut(256)).enumerate() {
            f.set(((b as f32 * 0.05).sin() * 1.2).clamp(-1.0, 1.0));
            f.process_block(cl, cr);
        }
        let jump = l.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0.0f32, f32::max);
        assert!(jump < 0.2, "largest step {jump}");
    }
}
