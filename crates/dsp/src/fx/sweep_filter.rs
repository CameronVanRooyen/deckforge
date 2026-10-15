use super::filters::{HighPass, LowPass};
use crate::smooth::Smoothed;

const STEPS: usize = 124;
const BLEND_START: f32 = 4.0;
const BLEND_SPAN: f32 = 34.0;
const BLEND_SMOOTH_MS: f32 = 5.0;

const HPF_HZ: [f32; STEPS] = [
    61.0, 67.0, 73.0, 79.0, 85.0, 92.0, 98.0, 104.0, 111.0, 116.0, 122.0, 128.0, 135.0, 141.0, 147.0, 154.0, 160.0, 167.0, 174.0, 182.0, 189.0, 196.0, 205.0, 213.0,
    222.0, 230.0, 240.0, 249.0, 259.0, 267.0, 278.0, 292.0, 305.0, 319.0, 332.0, 346.0, 360.0, 373.0, 387.0, 400.0, 418.0, 436.0, 453.0, 471.0, 488.0, 506.0, 523.0,
    541.0, 559.0, 576.0, 606.0, 636.0, 666.0, 696.0, 726.0, 756.0, 786.0, 816.0, 845.0, 875.0, 915.0, 955.0, 994.0, 1034.0, 1073.0, 1113.0, 1152.0, 1192.0, 1232.0,
    1271.0, 1331.0, 1391.0, 1451.0, 1511.0, 1570.0, 1630.0, 1690.0, 1750.0, 1810.0, 1869.0, 1950.0, 2031.0, 2112.0, 2193.0, 2273.0, 2354.0, 2435.0, 2516.0, 2597.0,
    2677.0, 2792.0, 2907.0, 3021.0, 3136.0, 3250.0, 3365.0, 3480.0, 3594.0, 3709.0, 3823.0, 4001.0, 4178.0, 4355.0, 4532.0, 4709.0, 4886.0, 5064.0, 5241.0, 5418.0,
    5595.0, 5800.0, 6006.0, 6211.0, 6417.0, 6622.0, 6827.0, 7033.0, 7238.0, 7444.0, 7749.0, 8050.0, 8350.0, 8650.0, 8951.0,
];

const LPF_HZ: [f32; STEPS] = [
    12000.0, 11535.1, 11088.2, 10658.6, 10245.7, 9848.7, 9467.2, 9100.4, 8747.8, 8408.9, 8083.1, 7770.0, 7468.9, 7179.6, 6901.4, 6634.1, 6377.0, 6130.0, 5992.5, 5754.2,
    5494.8, 5353.8, 5131.0, 4936.1, 4768.8, 4598.7, 4425.5, 4279.1, 4119.1, 3945.4, 3777.6, 3685.5, 3538.9, 3407.6, 3281.4, 3160.1, 2983.4, 2881.4, 2773.6, 2670.0,
    2570.4, 2474.7, 2332.7, 2214.3, 2109.3, 2027.6, 1949.0, 1873.5, 1800.9, 1731.1, 1664.1, 1599.6, 1517.6, 1448.1, 1400.8, 1335.8, 1292.8, 1232.0, 1193.1, 1136.1,
    1080.9, 1037.5, 990.7, 955.6, 917.0, 870.0, 817.1, 785.5, 755.0, 725.8, 697.7, 670.6, 634.7, 609.7, 575.7, 552.6, 530.4, 509.1, 488.6, 468.9, 449.9, 427.7, 405.2,
    387.4, 370.8, 356.4, 337.6, 321.3, 304.3, 292.5, 281.2, 270.3, 259.8, 249.7, 240.1, 230.8, 221.8, 213.2, 205.0, 197.0, 189.4, 179.1, 169.2, 161.7, 155.4, 149.4,
    143.6, 138.1, 132.7, 127.6, 122.6, 117.9, 113.3, 108.9, 104.7, 100.6, 96.7, 94.7, 94.5, 94.2, 93.9, 93.6, 93.3, 93.0,
];

fn table_hz(table: &[f32; STEPS], amount: f32) -> f32 {
    let step = (amount.clamp(0.0, 1.0) * 255.0).round() as usize >> 1;
    table[step.min(STEPS - 1)]
}

fn blend_for(amount: f32) -> f32 {
    ((amount.clamp(0.0, 1.0) * 255.0 - BLEND_START) / BLEND_SPAN).clamp(0.0, 1.0)
}

/// Centre-detent filter knob for the wet signal: below centre a low-pass closes, above centre a high-pass opens, at centre it is clean.
#[derive(Debug, Clone)]
pub struct SweepFilter {
    sample_rate: f32,
    hp: [HighPass<1>; 2],
    lp: [LowPass<1>; 2],
    hp_blend: Smoothed,
    lp_blend: Smoothed,
    hp_hz: f32,
    lp_hz: f32,
}

impl SweepFilter {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            hp: [HighPass::default(); 2],
            lp: [LowPass::default(); 2],
            hp_blend: Smoothed::new(0.0, sample_rate, BLEND_SMOOTH_MS),
            lp_blend: Smoothed::new(0.0, sample_rate, BLEND_SMOOTH_MS),
            hp_hz: 0.0,
            lp_hz: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.hp.iter_mut().for_each(HighPass::reset);
        self.lp.iter_mut().for_each(LowPass::reset);
        self.hp_blend.reset(0.0);
        self.lp_blend.reset(0.0);
    }

    /// Sets the knob (0..1, centre 0.5); cutoffs come from the sweep tables.
    pub fn set(&mut self, knob: f32) {
        let v = knob.clamp(0.0, 1.0);
        let (lp_amount, hp_amount) = if v < 0.5 { (1.0 - 2.0 * v, 0.0) } else { (0.0, 2.0 * v - 1.0) };
        self.lp_blend.set(blend_for(lp_amount));
        self.hp_blend.set(blend_for(hp_amount));
        let (lp_hz, hp_hz) = (table_hz(&LPF_HZ, lp_amount), table_hz(&HPF_HZ, hp_amount));
        if blend_active(lp_amount) && lp_hz != self.lp_hz {
            self.lp_hz = lp_hz;
            self.lp.iter_mut().for_each(|f| f.set(lp_hz, self.sample_rate));
        }
        if blend_active(hp_amount) && hp_hz != self.hp_hz {
            self.hp_hz = hp_hz;
            self.hp.iter_mut().for_each(|f| f.set(hp_hz, self.sample_rate));
        }
    }

    /// Filters one frame; high-pass first, then low-pass, each blended with the unfiltered signal.
    #[inline]
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        let (hb, lb) = (self.hp_blend.next(), self.lp_blend.next());
        if hb == 0.0 && lb == 0.0 && self.hp_blend.is_settled() && self.lp_blend.is_settled() {
            return (l, r);
        }
        let run = |ch: usize, x: f32, s: &mut Self| {
            let high = x + (s.hp[ch].process(x) - x) * hb;
            high + (s.lp[ch].process(high) - high) * lb
        };
        (run(0, l, self), run(1, r, self))
    }
}

fn blend_active(amount: f32) -> bool {
    blend_for(amount) > 0.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::testkit::{db, tone_amplitude};
    use std::f32::consts::TAU;

    const SR: f32 = 48_000.0;

    fn gain_db(knob: f32, hz: f32) -> f32 {
        let mut f = SweepFilter::new(SR);
        f.set(knob);
        let x: Vec<f32> = (0..48_000).map(|i| 0.5 * (TAU * hz * i as f32 / SR).sin()).collect();
        let y: Vec<f32> = x.iter().map(|v| f.process(*v, *v).0).collect();
        db(tone_amplitude(&y[24_000..], hz) / tone_amplitude(&x[24_000..], hz))
    }

    #[test]
    fn the_centre_is_clean_and_both_sides_filter() {
        for hz in [100.0, 1_000.0, 8_000.0] {
            assert!(gain_db(0.5, hz).abs() < 1e-3, "{hz} Hz at centre");
        }
        assert!(gain_db(0.0, 2_000.0) < -20.0, "far left closes the low-pass to about 93 Hz");
        assert!(gain_db(0.0, 60.0).abs() < 6.0);
        assert!(gain_db(1.0, 200.0) < -20.0, "far right opens the high-pass to about 9 kHz");
        assert!(gain_db(1.0, 15_000.0).abs() < 1.5);
        assert!(gain_db(0.25, 8_000.0) < gain_db(0.4, 8_000.0), "lower knob, darker");
        assert!(gain_db(0.75, 500.0) < gain_db(0.6, 500.0), "higher knob, thinner");
    }

    #[test]
    fn the_filter_fades_in_over_the_first_steps_and_cutoffs_follow_the_tables() {
        assert_eq!(blend_for(0.0), 0.0);
        assert!(blend_for(10.0 / 255.0) > 0.0 && blend_for(10.0 / 255.0) < 1.0);
        assert_eq!(blend_for(40.0 / 255.0), 1.0);
        assert_eq!((table_hz(&LPF_HZ, 0.0), table_hz(&LPF_HZ, 1.0)), (12_000.0, 93.0));
        assert_eq!((table_hz(&HPF_HZ, 0.0), table_hz(&HPF_HZ, 1.0)), (61.0, 8_951.0));
        assert!(LPF_HZ.windows(2).all(|w| w[1] <= w[0]) && HPF_HZ.windows(2).all(|w| w[1] >= w[0]));
    }
}
