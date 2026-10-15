/// Depth at which the effect reaches full level; beyond it the dry signal fades out.
pub const WET_FULL_AT: f32 = 0.75;

/// Beat FX mix law: the wet rises to full by three quarters travel while the dry holds, then the dry falls away so full depth is wet only.
#[inline]
pub fn beat_mix(depth: f32) -> (f32, f32) {
    let x = depth.clamp(0.0, 1.0);
    let wet = (x / WET_FULL_AT).min(1.0);
    let dry = if x <= WET_FULL_AT { 1.0 } else { (1.0 - x) / (1.0 - WET_FULL_AT) };
    (dry, wet)
}

/// Dry gain once the effect's send and swap gates are applied; fully open gates give `law_dry`, closed gates give unity.
#[inline]
pub fn gated_dry(law_dry: f32, gates: f32) -> f32 {
    1.0 - (1.0 - law_dry) * gates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dry_holds_to_three_quarters_then_falls_to_nothing_while_wet_tops_out() {
        assert_eq!(beat_mix(0.0), (1.0, 0.0));
        assert_eq!(beat_mix(0.75), (1.0, 1.0));
        let (dry, wet) = beat_mix(0.875);
        assert!((dry - 0.5).abs() < 1e-6 && wet == 1.0);
        assert_eq!(beat_mix(1.0), (0.0, 1.0));
        assert!((beat_mix(0.375).1 - 0.5).abs() < 1e-6);
        assert_eq!(beat_mix(7.0), (0.0, 1.0));
    }

    #[test]
    fn closed_gates_leave_the_dry_alone() {
        assert_eq!(gated_dry(0.0, 0.0), 1.0);
        assert_eq!(gated_dry(0.0, 1.0), 0.0);
        assert!((gated_dry(0.2, 0.5) - 0.6).abs() < 1e-6);
    }
}
