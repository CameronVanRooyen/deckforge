/// Stereo circular buffer with fractional-delay reads. Reads beyond the written history return silence,
/// so a reset needs no memset.
#[derive(Debug, Clone)]
pub struct StereoRing {
    l: Vec<f32>,
    r: Vec<f32>,
    mask: usize,
    pos: usize,
    valid: usize,
}

impl StereoRing {
    pub fn new(max_samples: usize) -> Self {
        let cap = (max_samples + 8).next_power_of_two();
        Self { l: vec![0.0; cap], r: vec![0.0; cap], mask: cap - 1, pos: 0, valid: 0 }
    }

    pub fn capacity(&self) -> usize {
        self.mask + 1
    }

    pub fn reset(&mut self) {
        self.valid = 0;
    }

    pub fn position(&self) -> usize {
        self.pos
    }

    #[inline]
    pub fn write(&mut self, l: f32, r: f32) {
        self.l[self.pos] = l;
        self.r[self.pos] = r;
        self.pos = (self.pos + 1) & self.mask;
        self.valid = (self.valid + 1).min(self.mask + 1);
    }

    #[inline]
    fn tap(buf: &[f32], mask: usize, pos: usize, valid: usize, k: usize) -> f32 {
        if k <= valid { buf[pos.wrapping_sub(k) & mask] } else { 0.0 }
    }

    #[inline]
    fn read_channel(&self, buf: &[f32], delay: f32) -> f32 {
        let d = delay.clamp(1.0, (self.mask - 2) as f32);
        let k = d as usize;
        let f = d - k as f32;
        let a = Self::tap(buf, self.mask, self.pos, self.valid, k);
        let b = Self::tap(buf, self.mask, self.pos, self.valid, k + 1);
        a + (b - a) * f
    }

    #[inline]
    pub fn read_l(&self, delay: f32) -> f32 {
        self.read_channel(&self.l, delay)
    }

    #[inline]
    pub fn read_r(&self, delay: f32) -> f32 {
        self.read_channel(&self.r, delay)
    }

    #[inline]
    pub fn read(&self, delay: f32) -> (f32, f32) {
        (self.read_l(delay), self.read_r(delay))
    }

    /// One channel of the frame written `k` frames ago (1 = newest); silence beyond the history.
    #[inline]
    pub fn ago(&self, right: bool, k: usize) -> f32 {
        let buf = if right { &self.r } else { &self.l };
        Self::tap(buf, self.mask, self.pos, self.valid, k)
    }

    #[inline]
    pub fn peek(&self, absolute_index: usize) -> (f32, f32) {
        let i = absolute_index & self.mask;
        (self.l[i], self.r[i])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn impulse_reads_back_at_exact_delay() {
        let mut ring = StereoRing::new(1000);
        let delay = 100;
        let mut heard = Vec::new();
        for n in 0..300 {
            heard.push(ring.read(delay as f32).0);
            ring.write(if n == 0 { 1.0 } else { 0.0 }, 0.0);
        }
        assert_eq!(heard.iter().position(|v| *v == 1.0), Some(delay));
    }

    #[test]
    fn fractional_delay_interpolates_and_unwritten_history_is_silent() {
        let mut ring = StereoRing::new(64);
        assert_eq!(ring.read(10.0), (0.0, 0.0));
        ring.write(1.0, 1.0);
        ring.write(0.0, 0.0);
        assert!((ring.read_l(2.5) - 0.5).abs() < 1e-6);
    }
}
