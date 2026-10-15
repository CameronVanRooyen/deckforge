use std::collections::VecDeque;

use serde::Serialize;

const WINDOW: usize = 600;

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameStatsReport {
    pub frames: u64,
    pub window: usize,
    pub p50_ms: f64,
    pub p99_ms: f64,
    pub max_ms: f64,
    pub refresh_hz: f64,
    pub dropped: u64,
    pub cpu_avg_ms: f64,
    pub cpu_p99_ms: f64,
    pub idle_skips: u64,
}

/// Rolling frame-interval and CPU-cost statistics for the render loop.
#[derive(Debug, Default)]
pub struct FrameStats {
    intervals: VecDeque<f64>,
    cpu: VecDeque<f64>,
    frames: u64,
    dropped: u64,
    idle_skips: u64,
    last_present: Option<f64>,
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[idx]
}

fn push(buf: &mut VecDeque<f64>, v: f64) {
    if buf.len() == WINDOW {
        buf.pop_front();
    }
    buf.push_back(v);
}

impl FrameStats {
    /// Records a presented frame at `now_secs`; gaps after idle periods are not counted as drops.
    pub fn record_frame(&mut self, now_secs: f64, cpu_ms: f64, continuous: bool) {
        self.frames += 1;
        push(&mut self.cpu, cpu_ms);
        if let Some(prev) = self.last_present {
            let dt = (now_secs - prev) * 1000.0;
            if continuous && dt < 250.0 {
                push(&mut self.intervals, dt);
                let nominal = self.nominal_ms();
                if nominal > 0.0 && dt > nominal * 1.5 {
                    self.dropped += ((dt / nominal).round() as u64).saturating_sub(1).max(1);
                }
            }
        }
        self.last_present = Some(now_secs);
    }

    /// Call when the loop goes idle so the next frame does not register a huge interval.
    pub fn mark_idle(&mut self) {
        self.idle_skips += 1;
        self.last_present = None;
    }

    fn nominal_ms(&self) -> f64 {
        if self.intervals.len() < 30 {
            return 0.0;
        }
        let mut v: Vec<f64> = self.intervals.iter().copied().collect();
        v.sort_by(f64::total_cmp);
        percentile(&v, 0.25)
    }

    pub fn report(&self) -> FrameStatsReport {
        let mut iv: Vec<f64> = self.intervals.iter().copied().collect();
        iv.sort_by(f64::total_cmp);
        let mut cpu: Vec<f64> = self.cpu.iter().copied().collect();
        cpu.sort_by(f64::total_cmp);
        let p50 = percentile(&iv, 0.5);
        FrameStatsReport {
            frames: self.frames,
            window: iv.len(),
            p50_ms: p50,
            p99_ms: percentile(&iv, 0.99),
            max_ms: iv.last().copied().unwrap_or(0.0),
            refresh_hz: if p50 > 0.0 { 1000.0 / p50 } else { 0.0 },
            dropped: self.dropped,
            cpu_avg_ms: if cpu.is_empty() {
                0.0
            } else {
                cpu.iter().sum::<f64>() / cpu.len() as f64
            },
            cpu_p99_ms: percentile(&cpu, 0.99),
            idle_skips: self.idle_skips,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steady_120hz_reports_120_and_no_drops() {
        let mut s = FrameStats::default();
        for i in 0..200 {
            s.record_frame(i as f64 / 120.0, 0.4, true);
        }
        let r = s.report();
        assert!((r.refresh_hz - 120.0).abs() < 0.5, "{r:?}");
        assert_eq!(r.dropped, 0);
    }

    #[test]
    fn a_skipped_vsync_counts_as_one_drop() {
        let mut s = FrameStats::default();
        let mut t = 0.0;
        for i in 0..100 {
            t += if i == 60 { 2.0 / 60.0 } else { 1.0 / 60.0 };
            s.record_frame(t, 0.3, true);
        }
        assert_eq!(s.report().dropped, 1);
    }

    #[test]
    fn idle_gaps_are_not_drops() {
        let mut s = FrameStats::default();
        for i in 0..60 {
            s.record_frame(i as f64 / 60.0, 0.2, true);
        }
        s.mark_idle();
        s.record_frame(30.0, 0.2, true);
        assert_eq!(s.report().dropped, 0);
    }
}
