/// Peak meter with hold-free exponential decay, updated once per block.
#[derive(Debug, Clone, Copy, Default)]
pub struct PeakMeter {
    level: f32,
    block_peak: f32,
}

impl PeakMeter {
    #[inline]
    pub fn feed(&mut self, x: f32) {
        let a = x.abs();
        if a > self.block_peak {
            self.block_peak = a;
        }
    }

    pub fn end_block(&mut self, frames: usize, sample_rate: f32) {
        let decay = (-(frames as f32) / (0.3 * sample_rate)).exp();
        self.level = (self.level * decay).max(self.block_peak);
        self.block_peak = 0.0;
    }

    pub fn level(&self) -> f32 {
        self.level
    }
}
