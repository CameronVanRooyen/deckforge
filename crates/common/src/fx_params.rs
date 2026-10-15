use crate::{BeatFxKind, ColorFxKind};

pub const BEAT_FX_MAX_PARAMS: usize = 8;
pub const COLOR_FX_MAX_PARAMS: usize = 2;

/// One sub-parameter of a Beat FX kind. Choice parameters (`options` non-empty) hold the option index.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BeatFxParamSpec {
    pub id: &'static str,
    pub label: &'static str,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub step: Option<f32>,
    pub unit: &'static str,
    pub options: &'static [&'static str],
}

impl BeatFxParamSpec {
    const fn amount(id: &'static str, label: &'static str, min: f32, max: f32, default: f32) -> Self {
        Self { id, label, min, max, default, step: None, unit: "%", options: &[] }
    }

    const fn choice(id: &'static str, label: &'static str, options: &'static [&'static str], default: usize, unit: &'static str) -> Self {
        Self { id, label, min: 0.0, max: (options.len() - 1) as f32, default: default as f32, step: Some(1.0), unit, options }
    }

    /// Clamps to the range and snaps choices and stepped values; `None` for non-finite input.
    pub fn sanitize(&self, value: f32) -> Option<f32> {
        if !value.is_finite() {
            return None;
        }
        let v = value.clamp(self.min, self.max);
        Some(match self.step {
            Some(step) if step > 0.0 => (self.min + ((v - self.min) / step).round() * step).clamp(self.min, self.max),
            _ => v,
        })
    }
}

const FEEDBACK: BeatFxParamSpec = BeatFxParamSpec::amount("feedback", "Feedback", 0.0, 100.0, 50.0);
const TONE: BeatFxParamSpec = BeatFxParamSpec::amount("tone", "Tone", 0.0, 100.0, 50.0);

const ECHO_FEEDBACK: BeatFxParamSpec = BeatFxParamSpec::amount("feedback", "Feedback", 0.0, 100.0, 42.0);
const ECHO_FILTER: BeatFxParamSpec = BeatFxParamSpec::amount("filter", "Filter", 0.0, 100.0, 50.0);

const ECHO_PARAMS: [BeatFxParamSpec; 2] = [ECHO_FEEDBACK, ECHO_FILTER];
const PING_PONG_PARAMS: [BeatFxParamSpec; 3] = [
    ECHO_FEEDBACK,
    ECHO_FILTER,
    BeatFxParamSpec::choice("rRatio", "R ratio", &["0.25", "0.5", "0.75", "1", "1.25", "1.5", "1.75", "2"], 2, "x"),
];
const SPIRAL_PARAMS: [BeatFxParamSpec; 2] = [
    BeatFxParamSpec::amount("feedback", "Feedback", 0.0, 150.0, 80.0),
    BeatFxParamSpec::amount("pitch", "Pitch", -100.0, 100.0, 0.0),
];
const REVERB_PARAMS: [BeatFxParamSpec; 5] = [
    TONE,
    BeatFxParamSpec::amount("noise", "Noise", 0.0, 100.0, 50.0),
    BeatFxParamSpec::amount("decay", "Decay", 1.0, 100.0, 50.0),
    BeatFxParamSpec::choice("voicing", "Voicing", &["Classic", "Tension"], 0, ""),
    BeatFxParamSpec::amount("bend", "Bend", 0.0, 100.0, 20.0),
];
const FLANGER_PARAMS: [BeatFxParamSpec; 1] = [FEEDBACK];
const PHASER_PARAMS: [BeatFxParamSpec; 1] = [FEEDBACK];
const SWEEP_PARAMS: [BeatFxParamSpec; 1] = [BeatFxParamSpec::amount("resonance", "Resonance", 0.0, 100.0, 50.0)];
const PULSE_VERB_PARAMS: [BeatFxParamSpec; 5] = [
    BeatFxParamSpec::amount("size", "Size", 1.0, 100.0, 45.0),
    BeatFxParamSpec::amount("chop", "Chop", 0.0, 100.0, 70.0),
    BeatFxParamSpec::amount("noise", "Noise", 0.0, 100.0, 25.0),
    BeatFxParamSpec::choice("accelerate", "Accelerate", &["Off", "On"], 0, ""),
    BeatFxParamSpec::choice("every", "Every", &["1", "2", "4", "8"], 2, "bars"),
];
const DUB_TAPE_PARAMS: [BeatFxParamSpec; 7] = [
    BeatFxParamSpec::amount("feedback", "Feedback", 0.0, 105.0, 45.0),
    TONE,
    BeatFxParamSpec::amount("drive", "Drive", 0.0, 100.0, 25.0),
    BeatFxParamSpec::amount("wobble", "Wobble", 0.0, 100.0, 15.0),
    BeatFxParamSpec::choice("timeMode", "Time change", &["Clean", "Tape", "Reset"], 0, ""),
    BeatFxParamSpec::choice("send", "Send", &["Always", "Throw"], 0, ""),
    BeatFxParamSpec::choice("output", "Output", &["Mix", "Wet only"], 0, ""),
];
const HALO_PARAMS: [BeatFxParamSpec; 3] = [
    BeatFxParamSpec::amount("shimmer", "Shimmer", 0.0, 100.0, 50.0),
    TONE,
    BeatFxParamSpec::amount("fifth", "Fifth", 0.0, 100.0, 0.0),
];
const ASCEND_PARAMS: [BeatFxParamSpec; 3] = [
    BeatFxParamSpec::choice("length", "Length", &["4", "8", "16", "32"], 2, "bars"),
    BeatFxParamSpec::choice("landOn", "Land on", &["Phrase", "Bar"], 0, ""),
    BeatFxParamSpec::choice("release", "Release", &["Downbeat", "Now"], 0, ""),
];
const BACKWASH_PARAMS: [BeatFxParamSpec; 1] = [BeatFxParamSpec::amount("size", "Size", 1.0, 100.0, 60.0)];

/// Sub-parameters of a kind, beyond level, beats and percent. Mirrors `BEAT_FX_PARAMS` in the UI's types.ts.
pub fn beat_fx_params(kind: BeatFxKind) -> &'static [BeatFxParamSpec] {
    match kind {
        BeatFxKind::PingPong => &PING_PONG_PARAMS,
        BeatFxKind::Spiral => &SPIRAL_PARAMS,
        BeatFxKind::Delay | BeatFxKind::Echo | BeatFxKind::UpEcho | BeatFxKind::DownEcho => &ECHO_PARAMS,
        BeatFxKind::Reverb => &REVERB_PARAMS,
        BeatFxKind::Flanger => &FLANGER_PARAMS,
        BeatFxKind::Phaser => &PHASER_PARAMS,
        BeatFxKind::Roll | BeatFxKind::Trans => &[],
        BeatFxKind::FilterSweep => &SWEEP_PARAMS,
        BeatFxKind::PulseVerb => &PULSE_VERB_PARAMS,
        BeatFxKind::DubTape => &DUB_TAPE_PARAMS,
        BeatFxKind::Halo => &HALO_PARAMS,
        BeatFxKind::Ascend => &ASCEND_PARAMS,
        BeatFxKind::Backwash => &BACKWASH_PARAMS,
    }
}

/// Indices of the continuous sub-parameters SHIFT + LEVEL/DEPTH can drive, in paging order.
pub fn beat_fx_knob_params(kind: BeatFxKind) -> impl Iterator<Item = usize> {
    beat_fx_params(kind).iter().enumerate().filter(|(_, s)| s.options.is_empty()).map(|(i, _)| i)
}

const SPACE_PARAMS: [BeatFxParamSpec; 1] = [BeatFxParamSpec::amount("shimmer", "Shimmer", 0.0, 100.0, 0.0)];
const NOISE_PARAMS: [BeatFxParamSpec; 1] = [BeatFxParamSpec::amount("level", "Level", 0.0, 100.0, 50.0)];

/// Sub-parameters of a Color FX kind. Mirrors `COLOR_FX_PARAMS` in the UI's types.ts.
pub fn color_fx_params(kind: ColorFxKind) -> &'static [BeatFxParamSpec] {
    match kind {
        ColorFxKind::Space => &SPACE_PARAMS,
        ColorFxKind::Noise => &NOISE_PARAMS,
        ColorFxKind::Filter | ColorFxKind::DubEcho | ColorFxKind::Crush => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_fits_the_parameter_array_and_defaults_are_in_range() {
        for kind in BeatFxKind::ALL {
            let specs = beat_fx_params(kind);
            assert!(specs.len() <= BEAT_FX_MAX_PARAMS, "{kind:?}");
            for s in specs {
                assert!(s.min <= s.default && s.default <= s.max, "{kind:?} {}", s.id);
                assert_eq!(s.sanitize(s.default), Some(s.default));
                if !s.options.is_empty() {
                    assert_eq!(s.max as usize, s.options.len() - 1);
                }
            }
        }
    }

    #[test]
    fn color_fx_kinds_fit_their_parameter_array() {
        for kind in [ColorFxKind::Filter, ColorFxKind::Space, ColorFxKind::DubEcho, ColorFxKind::Crush, ColorFxKind::Noise] {
            let specs = color_fx_params(kind);
            assert!(specs.len() <= COLOR_FX_MAX_PARAMS, "{kind:?}");
            assert!(specs.iter().all(|s| s.min <= s.default && s.default <= s.max));
        }
    }

    #[test]
    fn sanitize_clamps_snaps_choices_and_rejects_garbage() {
        let accelerate = beat_fx_params(BeatFxKind::PulseVerb)[3];
        assert_eq!(accelerate.sanitize(0.6), Some(1.0));
        assert_eq!(accelerate.sanitize(9.0), Some(1.0));
        assert_eq!(accelerate.sanitize(f32::NAN), None);
        let feedback = beat_fx_params(BeatFxKind::DubTape)[0];
        assert_eq!(feedback.sanitize(200.0), Some(105.0));
        assert_eq!(feedback.sanitize(-5.0), Some(0.0));
    }
}
