use serde::{Deserialize, Serialize};

/// Rectangle in CSS pixels relative to the webview's top-left corner.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct CssRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Integer rectangle in device pixels, clipped to the render target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PixelRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl PixelRect {
    pub fn is_empty(&self) -> bool {
        self.w == 0 || self.h == 0
    }
}

impl CssRect {
    /// Converts to device pixels with edge rounding (not size rounding), clipped to `target`.
    pub fn to_pixels(&self, scale: f64, target: (u32, u32)) -> PixelRect {
        if !(self.x.is_finite() && self.y.is_finite() && self.w.is_finite() && self.h.is_finite())
            || scale <= 0.0
        {
            return PixelRect::default();
        }
        let edge = |v: f64, max: u32| (v * scale).round().clamp(0.0, max as f64) as u32;
        let x0 = edge(self.x, target.0);
        let y0 = edge(self.y, target.1);
        let x1 = edge(self.x + self.w, target.0);
        let y1 = edge(self.y + self.h, target.1);
        PixelRect {
            x: x0,
            y: y0,
            w: x1.saturating_sub(x0),
            h: y1.saturating_sub(y0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_edges_and_scales() {
        let r = CssRect {
            x: 10.25,
            y: 4.0,
            w: 100.5,
            h: 50.0,
        }
        .to_pixels(2.0, (4000, 2000));
        assert_eq!(
            r,
            PixelRect {
                x: 21,
                y: 8,
                w: 201,
                h: 100
            }
        );
    }

    #[test]
    fn adjacent_rects_share_an_edge_without_gap_or_overlap() {
        let a = CssRect {
            x: 0.0,
            y: 0.0,
            w: 33.3,
            h: 10.0,
        }
        .to_pixels(1.5, (1000, 1000));
        let b = CssRect {
            x: 33.3,
            y: 0.0,
            w: 10.0,
            h: 10.0,
        }
        .to_pixels(1.5, (1000, 1000));
        assert_eq!(a.x + a.w, b.x);
    }

    #[test]
    fn clips_to_target_and_rejects_garbage() {
        let r = CssRect {
            x: -10.0,
            y: 0.0,
            w: 5000.0,
            h: 20.0,
        }
        .to_pixels(1.0, (800, 600));
        assert_eq!((r.x, r.w), (0, 800));
        assert!(
            CssRect {
                x: f64::NAN,
                y: 0.0,
                w: 1.0,
                h: 1.0
            }
            .to_pixels(1.0, (10, 10))
            .is_empty()
        );
        assert!(
            CssRect {
                x: 0.0,
                y: 0.0,
                w: 10.0,
                h: 10.0
            }
            .to_pixels(0.0, (10, 10))
            .is_empty()
        );
    }
}
