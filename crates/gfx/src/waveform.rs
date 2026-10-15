pub const TEX_WIDTH: usize = 4096;
pub const MAX_LEVELS: usize = 24;

/// Decoded peak envelopes: four bands of 0..255 per display point.
#[derive(Debug, Clone, PartialEq)]
pub struct WaveformData {
    pub points_per_sec: f64,
    pub peak: Vec<u8>,
    pub low: Vec<u8>,
    pub mid: Vec<u8>,
    pub high: Vec<u8>,
}

impl WaveformData {
    /// Parses `[u32 n][f64 pps][peak][low][mid][high]`, little-endian.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let n = u32::from_le_bytes(bytes.get(0..4)?.try_into().ok()?) as usize;
        let pps = f64::from_le_bytes(bytes.get(4..12)?.try_into().ok()?);
        if n == 0 || !pps.is_finite() || pps <= 0.0 {
            return None;
        }
        let body = bytes.get(12..12usize.checked_add(n.checked_mul(4)?)?)?;
        Some(Self {
            points_per_sec: pps,
            peak: body[..n].to_vec(),
            low: body[n..2 * n].to_vec(),
            mid: body[2 * n..3 * n].to_vec(),
            high: body[3 * n..].to_vec(),
        })
    }

    pub fn len(&self) -> usize {
        self.peak.len()
    }

    pub fn is_empty(&self) -> bool {
        self.peak.is_empty()
    }

    pub fn duration_secs(&self) -> f64 {
        self.len() as f64 / self.points_per_sec
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LevelInfo {
    pub row_offset: u32,
    pub len: u32,
}

/// Max-reduction mip chain packed into one RGBA8 texture: texels are (low, mid, high, peak).
#[derive(Debug, Clone)]
pub struct Pyramid {
    pub width: u32,
    pub rows: u32,
    pub texels: Vec<[u8; 4]>,
    pub levels: Vec<LevelInfo>,
    pub points_per_sec: f64,
    pub len: usize,
}

impl Pyramid {
    pub fn build(wf: &WaveformData) -> Self {
        let mut level: Vec<[u8; 4]> = (0..wf.len())
            .map(|i| [wf.low[i], wf.mid[i], wf.high[i], wf.peak[i]])
            .collect();
        let mut levels = Vec::new();
        let mut texels: Vec<[u8; 4]> = Vec::new();
        let mut rows = 0u32;
        loop {
            let len = level.len();
            let level_rows = len.div_ceil(TEX_WIDTH) as u32;
            levels.push(LevelInfo {
                row_offset: rows,
                len: len as u32,
            });
            texels.extend_from_slice(&level);
            texels.resize((rows + level_rows) as usize * TEX_WIDTH, [0; 4]);
            rows += level_rows;
            if len <= 1 || levels.len() == MAX_LEVELS {
                break;
            }
            level = level
                .chunks(2)
                .map(|c| {
                    let mut m = c[0];
                    for t in c.iter().skip(1) {
                        for k in 0..4 {
                            m[k] = m[k].max(t[k]);
                        }
                    }
                    m
                })
                .collect();
        }
        Self {
            width: TEX_WIDTH as u32,
            rows,
            texels,
            levels,
            points_per_sec: wf.points_per_sec,
            len: wf.len(),
        }
    }

    pub fn texel(&self, level: usize, index: usize) -> [u8; 4] {
        let info = self.levels[level];
        if index >= info.len as usize {
            return [0; 4];
        }
        self.texels[(info.row_offset as usize + index / TEX_WIDTH) * TEX_WIDTH + index % TEX_WIDTH]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(n: usize) -> WaveformData {
        WaveformData {
            points_per_sec: 150.0,
            peak: (0..n).map(|i| (i % 251) as u8).collect(),
            low: (0..n).map(|i| (i * 3 % 256) as u8).collect(),
            mid: vec![7; n],
            high: (0..n).map(|i| (255 - i % 256) as u8).collect(),
        }
    }

    fn pack(wf: &WaveformData) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(wf.len() as u32).to_le_bytes());
        out.extend_from_slice(&wf.points_per_sec.to_le_bytes());
        for b in [&wf.peak, &wf.low, &wf.mid, &wf.high] {
            out.extend_from_slice(b);
        }
        out
    }

    #[test]
    fn parses_the_stored_layout() {
        let wf = sample(1000);
        assert_eq!(WaveformData::from_bytes(&pack(&wf)).unwrap(), wf);
    }

    #[test]
    fn rejects_truncated_empty_and_bogus_headers() {
        let bytes = pack(&sample(100));
        assert!(WaveformData::from_bytes(&bytes[..bytes.len() - 1]).is_none());
        assert!(WaveformData::from_bytes(&[]).is_none());
        let mut zero = bytes.clone();
        zero[0..4].copy_from_slice(&0u32.to_le_bytes());
        assert!(WaveformData::from_bytes(&zero).is_none());
        let mut huge = bytes;
        huge[0..4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(WaveformData::from_bytes(&huge).is_none());
    }

    #[test]
    fn pyramid_levels_hold_the_max_of_their_children() {
        let wf = sample(10_000);
        let p = Pyramid::build(&wf);
        assert_eq!(p.levels[0].len, 10_000);
        assert_eq!(p.levels[1].len, 5_000);
        assert_eq!(p.levels.last().unwrap().len, 1);
        for k in [0usize, 1, 999, 4999] {
            let a = p.texel(0, 2 * k);
            let b = p.texel(0, 2 * k + 1);
            let m = p.texel(1, k);
            for c in 0..4 {
                assert_eq!(m[c], a[c].max(b[c]));
            }
        }
        let top = p.texel(p.levels.len() - 1, 0);
        assert_eq!(top[3], *wf.peak.iter().max().unwrap());
        assert!(p.rows as usize * TEX_WIDTH == p.texels.len());
    }
}
