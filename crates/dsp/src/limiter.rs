pub const LIMIT_KNEE: f32 = 0.891;

/// Stateless soft limiter: transparent below the knee, asymptotically approaches 1.0 above it.
#[inline]
pub fn soft_limit(x: f32) -> f32 {
    let a = x.abs();
    if a <= LIMIT_KNEE {
        x
    } else {
        let range = 1.0 - LIMIT_KNEE;
        let y = LIMIT_KNEE + range * ((a - LIMIT_KNEE) / range).tanh();
        y.copysign(x)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn never_exceeds_full_scale() {
        for i in 0..2000 {
            let x = i as f32 / 100.0 - 10.0;
            assert!(soft_limit(x).abs() <= 1.0);
        }
        assert_eq!(soft_limit(0.5), 0.5);
    }
}
