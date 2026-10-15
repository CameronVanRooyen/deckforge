use deckforge_common::TrackAudio;

/// Catmull-Rom interpolated stereo frame at a fractional position; silence outside the track.
#[inline]
pub fn read_cubic(track: &TrackAudio, position: f64) -> (f32, f32) {
    let frames = track.frames() as isize;
    let base = position.floor();
    let t = (position - base) as f32;
    let i = base as isize;
    if i < -2 || i > frames + 1 {
        return (0.0, 0.0);
    }
    let s = &track.samples;
    let at = |idx: isize, ch: usize| -> f32 {
        if idx < 0 || idx >= frames { 0.0 } else { s[idx as usize * 2 + ch] }
    };
    let mut out = [0.0f32; 2];
    for (ch, o) in out.iter_mut().enumerate() {
        let p0 = at(i - 1, ch);
        let p1 = at(i, ch);
        let p2 = at(i + 1, ch);
        let p3 = at(i + 2, ch);
        let a = -0.5 * p0 + 1.5 * p1 - 1.5 * p2 + 0.5 * p3;
        let b = p0 - 2.5 * p1 + 2.0 * p2 - 0.5 * p3;
        let c = -0.5 * p0 + 0.5 * p2;
        *o = ((a * t + b) * t + c) * t + p1;
    }
    (out[0], out[1])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_on_integer_positions_and_silent_outside() {
        let t = TrackAudio::new(44_100, vec![0.0, 0.0, 1.0, -1.0, 0.5, -0.5, 0.25, -0.25]);
        assert_eq!(read_cubic(&t, 1.0), (1.0, -1.0));
        assert_eq!(read_cubic(&t, 2.0), (0.5, -0.5));
        assert_eq!(read_cubic(&t, -10.0), (0.0, 0.0));
        assert_eq!(read_cubic(&t, 100.0), (0.0, 0.0));
    }
}
