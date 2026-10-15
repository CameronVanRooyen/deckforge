use serde::{Deserialize, Serialize};

use crate::color::{Rgba, parse_css_color};

/// Resolved colours and options for every surface; mirrors the CSS waveform theme.
#[derive(Debug, Clone, PartialEq)]
pub struct Style {
    pub bg: Rgba,
    pub deck: [Rgba; 2],
    pub low: [Rgba; 3],
    pub mid: [Rgba; 3],
    pub high: [Rgba; 3],
    pub ghost: Rgba,
    pub played_tint: Rgba,
    pub grid_beat: Rgba,
    pub grid_bar: Rgba,
    pub grid_four: Rgba,
    pub grid_shadow: Rgba,
    pub grid_marker: Rgba,
    pub aligned: Rgba,
    pub loop_color: Rgba,
    pub cue: Rgba,
    pub playhead: Rgba,
    pub flag_text: Rgba,
    pub bar_number: Rgba,
    pub warn: Rgba,
    pub hover: Rgba,
    /// Intro, build, drop, break, outro.
    pub phrases: [Rgba; 5],
    pub sub_pixel: bool,
    pub overview_phrase_band: bool,
}

fn hex(s: &str) -> Rgba {
    parse_css_color(s).expect("built-in colour is valid")
}

impl Default for Style {
    fn default() -> Self {
        Self {
            bg: hex("#09090b"),
            deck: [hex("#38bdf8"), hex("#fb7185")],
            low: [hex("#7cb3ff"), hex("#3b82f6"), hex("#1e3a8a")],
            mid: [hex("#fbbf24"), hex("#d97706"), hex("#78350f")],
            high: [hex("#fafafa"), hex("#d4d4d8"), hex("#a1a1aa")],
            ghost: hex("rgba(161,161,170,0.13)"),
            played_tint: hex("rgba(9,9,11,0.46)"),
            grid_beat: hex("rgba(250,250,250,0.3)"),
            grid_bar: hex("rgba(250,250,250,0.5)"),
            grid_four: hex("rgba(250,250,250,0.78)"),
            grid_shadow: hex("rgba(9,9,11,0.6)"),
            grid_marker: hex("rgba(250,250,250,0.72)"),
            aligned: hex("#4ade80"),
            loop_color: hex("#a3e635"),
            cue: hex("#f59e0b"),
            playhead: hex("#fafafa"),
            flag_text: hex("#09090b"),
            bar_number: hex("rgba(212,212,216,0.6)"),
            warn: hex("#ef4444"),
            hover: hex("rgba(250,250,250,0.3)"),
            phrases: [
                hex("#2dd4bf"),
                hex("#facc15"),
                hex("#f87171"),
                hex("#818cf8"),
                hex("#94a3b8"),
            ],
            sub_pixel: true,
            overview_phrase_band: true,
        }
    }
}

/// Partial style update sent over IPC. Colours are CSS strings; unknown or invalid values are reported, not applied.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct StyleSpec {
    pub bg: Option<String>,
    pub deck_a: Option<String>,
    pub deck_b: Option<String>,
    pub low: Option<[String; 3]>,
    pub mid: Option<[String; 3]>,
    pub high: Option<[String; 3]>,
    pub ghost: Option<String>,
    pub played_tint: Option<String>,
    pub grid_beat: Option<String>,
    pub grid_bar: Option<String>,
    pub grid_four: Option<String>,
    pub grid_shadow: Option<String>,
    pub grid_marker: Option<String>,
    pub aligned: Option<String>,
    pub loop_color: Option<String>,
    pub cue: Option<String>,
    pub playhead: Option<String>,
    pub flag_text: Option<String>,
    pub bar_number: Option<String>,
    pub warn: Option<String>,
    pub hover: Option<String>,
    pub phrases: Option<[String; 5]>,
    pub sub_pixel: Option<bool>,
    pub overview_phrase_band: Option<bool>,
}

impl Style {
    /// Applies the spec and returns a message per field that failed to parse.
    pub fn apply(&mut self, spec: &StyleSpec) -> Vec<String> {
        let mut errors = Vec::new();
        let mut one = |name: &str, value: &Option<String>, target: &mut Rgba| {
            if let Some(v) = value {
                match parse_css_color(v) {
                    Some(c) => *target = c,
                    None => errors.push(format!("{name}: invalid colour '{v}'")),
                }
            }
        };
        one("bg", &spec.bg, &mut self.bg);
        one("deckA", &spec.deck_a, &mut self.deck[0]);
        one("deckB", &spec.deck_b, &mut self.deck[1]);
        one("ghost", &spec.ghost, &mut self.ghost);
        one("playedTint", &spec.played_tint, &mut self.played_tint);
        one("gridBeat", &spec.grid_beat, &mut self.grid_beat);
        one("gridBar", &spec.grid_bar, &mut self.grid_bar);
        one("gridFour", &spec.grid_four, &mut self.grid_four);
        one("gridShadow", &spec.grid_shadow, &mut self.grid_shadow);
        one("gridMarker", &spec.grid_marker, &mut self.grid_marker);
        one("aligned", &spec.aligned, &mut self.aligned);
        one("loopColor", &spec.loop_color, &mut self.loop_color);
        one("cue", &spec.cue, &mut self.cue);
        one("playhead", &spec.playhead, &mut self.playhead);
        one("flagText", &spec.flag_text, &mut self.flag_text);
        one("barNumber", &spec.bar_number, &mut self.bar_number);
        one("warn", &spec.warn, &mut self.warn);
        one("hover", &spec.hover, &mut self.hover);
        let mut group = |name: &str, values: &Option<Vec<String>>, targets: &mut [Rgba]| {
            if let Some(vs) = values {
                let parsed: Vec<Option<Rgba>> = vs.iter().map(|v| parse_css_color(v)).collect();
                if parsed.iter().all(Option::is_some) {
                    for (t, p) in targets.iter_mut().zip(parsed) {
                        *t = p.expect("checked");
                    }
                } else {
                    errors.push(format!("{name}: invalid colour in list"));
                }
            }
        };
        group("low", &spec.low.as_ref().map(|a| a.to_vec()), &mut self.low);
        group("mid", &spec.mid.as_ref().map(|a| a.to_vec()), &mut self.mid);
        group(
            "high",
            &spec.high.as_ref().map(|a| a.to_vec()),
            &mut self.high,
        );
        group(
            "phrases",
            &spec.phrases.as_ref().map(|a| a.to_vec()),
            &mut self.phrases,
        );
        if let Some(v) = spec.overview_phrase_band {
            self.overview_phrase_band = v;
        }
        if let Some(v) = spec.sub_pixel {
            self.sub_pixel = v;
        }
        errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_spec_only_changes_given_fields() {
        let mut style = Style::default();
        let before = style.clone();
        let spec: StyleSpec =
            serde_json::from_str(r##"{"deckA":"#ff0000","subPixel":false}"##).unwrap();
        assert!(style.apply(&spec).is_empty());
        assert_eq!(style.deck[0], [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(style.deck[1], before.deck[1]);
        assert!(!style.sub_pixel);
        assert_eq!(style.low, before.low);
    }

    #[test]
    fn invalid_colours_are_reported_and_skipped() {
        let mut style = Style::default();
        let before = style.clone();
        let spec = StyleSpec {
            bg: Some("nope".into()),
            low: Some(["#fff".into(), "bad".into(), "#000".into()]),
            ..Default::default()
        };
        let errors = style.apply(&spec);
        assert_eq!(errors.len(), 2);
        assert_eq!(style, before);
    }
}
