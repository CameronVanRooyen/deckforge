/// One-pole parameter smoother that converges exponentially toward its target.
#[derive(Debug, Clone, Copy)]
pub struct Smoothed {
    current: f32,
    target: f32,
    coeff: f32,
}

impl Smoothed {
    pub fn new(value: f32, sample_rate: f32, time_ms: f32) -> Self {
        let mut s = Self { current: value, target: value, coeff: 0.0 };
        s.set_time(sample_rate, time_ms);
        s
    }

    pub fn set_time(&mut self, sample_rate: f32, time_ms: f32) {
        let samples = (time_ms * 0.001 * sample_rate).max(1.0);
        self.coeff = (-1.0 / samples).exp();
    }

    pub fn set(&mut self, target: f32) {
        self.target = target;
    }

    pub fn reset(&mut self, value: f32) {
        self.current = value;
        self.target = value;
    }

    pub fn target(&self) -> f32 {
        self.target
    }

    pub fn value(&self) -> f32 {
        self.current
    }

    pub fn is_settled(&self) -> bool {
        (self.current - self.target).abs() < 1e-6
    }

    #[inline]
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> f32 {
        self.current = self.target + (self.current - self.target) * self.coeff;
        if self.is_settled() {
            self.current = self.target;
        }
        self.current
    }
}
