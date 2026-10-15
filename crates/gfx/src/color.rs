pub type Rgba = [f32; 4];

fn hex_pair(s: &str) -> Option<f32> {
    u8::from_str_radix(s, 16).ok().map(|v| v as f32 / 255.0)
}

fn function_args<'a>(s: &'a str, names: &[&str]) -> Option<Vec<&'a str>> {
    let lower = s.to_ascii_lowercase();
    let name = names.iter().find(|n| lower.starts_with(**n))?;
    let rest = s[name.len()..]
        .trim()
        .strip_prefix('(')?
        .strip_suffix(')')?;
    Some(
        rest.split([',', '/', ' '])
            .filter(|p| !p.is_empty())
            .collect(),
    )
}

fn channel(p: &str) -> Option<f32> {
    match p.strip_suffix('%') {
        Some(pct) => pct.parse::<f32>().ok().map(|v| v / 100.0),
        None => p.parse::<f32>().ok().map(|v| v / 255.0),
    }
}

fn alpha(p: &str) -> Option<f32> {
    match p.strip_suffix('%') {
        Some(pct) => pct.parse::<f32>().ok().map(|v| v / 100.0),
        None => p.parse::<f32>().ok(),
    }
}

/// Parses `#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb(r,g,b)` and `rgba(r,g,b,a)` into straight-alpha RGBA.
pub fn parse_css_color(input: &str) -> Option<Rgba> {
    let s = input.trim();
    if let Some(hex) = s.strip_prefix('#') {
        if !hex.is_ascii() {
            return None;
        }
        return match hex.len() {
            3 => {
                let c: Vec<f32> = hex
                    .chars()
                    .map(|c| hex_pair(&format!("{c}{c}")))
                    .collect::<Option<_>>()?;
                Some([c[0], c[1], c[2], 1.0])
            }
            6 => Some([
                hex_pair(&hex[0..2])?,
                hex_pair(&hex[2..4])?,
                hex_pair(&hex[4..6])?,
                1.0,
            ]),
            8 => Some([
                hex_pair(&hex[0..2])?,
                hex_pair(&hex[2..4])?,
                hex_pair(&hex[4..6])?,
                hex_pair(&hex[6..8])?,
            ]),
            _ => None,
        };
    }
    let args = function_args(s, &["rgba", "rgb"])?;
    if args.len() < 3 || args.len() > 4 {
        return None;
    }
    let a = args.get(3).map_or(Some(1.0), |p| alpha(p))?;
    Some([
        channel(args[0])?,
        channel(args[1])?,
        channel(args[2])?,
        a.clamp(0.0, 1.0),
    ])
}

pub fn rgb_u32(rgb: u32, a: f32) -> Rgba {
    [
        ((rgb >> 16) & 0xff) as f32 / 255.0,
        ((rgb >> 8) & 0xff) as f32 / 255.0,
        (rgb & 0xff) as f32 / 255.0,
        a,
    ]
}

pub fn with_alpha(c: Rgba, a: f32) -> Rgba {
    [c[0], c[1], c[2], c[3] * a]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Rgba, b: Rgba) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() < 0.005)
    }

    #[test]
    fn parses_hex_forms() {
        assert!(close(
            parse_css_color("#38bdf8").unwrap(),
            [56.0 / 255.0, 189.0 / 255.0, 248.0 / 255.0, 1.0]
        ));
        assert!(close(
            parse_css_color("#fff").unwrap(),
            [1.0, 1.0, 1.0, 1.0]
        ));
        assert!(close(
            parse_css_color("#00000080").unwrap(),
            [0.0, 0.0, 0.0, 0.5]
        ));
    }

    #[test]
    fn parses_rgb_functions() {
        assert!(close(
            parse_css_color("rgba(9,9,11,0.46)").unwrap(),
            [9.0 / 255.0, 9.0 / 255.0, 11.0 / 255.0, 0.46]
        ));
        assert!(close(
            parse_css_color("rgb(255 0 0 / 50%)").unwrap(),
            [1.0, 0.0, 0.0, 0.5]
        ));
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_css_color("blue").is_none());
        assert!(parse_css_color("#12").is_none());
        assert!(parse_css_color("rgb(1,2)").is_none());
        assert!(parse_css_color("#éééééé").is_none());
    }
}
