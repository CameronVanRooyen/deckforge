const SNAP_SECS = 0.03;
const SMOOTHING_TAU = 0.08;
const MAX_EXTRAPOLATION = 2;
const RESYNC_GAP = 0.25;
const DRAG_RELEASE_TIMEOUT = 0.35;
const DRAG_RELEASE_TOLERANCE = 0.04;

export interface DeckSample {
  pos: number;
  rate: number;
  moving: boolean;
  token: number;
  /** Host time in seconds at which `pos` was valid. */
  at: number;
}

type Drag = { kind: 'none' } | { kind: 'active'; secs: number; at: number } | { kind: 'releasing'; secs: number; anchor: number; since: number };

/** Same extrapolation algorithm as the native renderer's `DeckClock`, for the canvas fallback. */
export class SmoothClock {
  private sample: DeckSample | null = null;
  private shown = 0;
  private lastNow: number | null = null;
  private drag: Drag = { kind: 'none' };

  push(sample: DeckSample) {
    this.sample = sample;
  }

  setDrag(secs: number | null, now: number) {
    if (secs !== null) this.drag = { kind: 'active', secs, at: now };
    else if (this.drag.kind === 'active') this.drag = { kind: 'releasing', secs: this.drag.secs, anchor: this.drag.at, since: now };
  }

  get settling(): boolean {
    return this.drag.kind !== 'none';
  }

  get moving(): boolean {
    return this.drag.kind === 'active' || (this.sample?.moving ?? false);
  }

  private target(now: number): number | null {
    const s = this.sample;
    if (!s) return null;
    const age = Math.min(Math.max(now - s.at, 0), MAX_EXTRAPOLATION);
    return s.moving ? s.pos + age * s.rate : s.pos;
  }

  position(now: number): number {
    const previousNow = this.lastNow;
    this.lastNow = now;
    const d = this.drag;
    if (d.kind === 'active') {
      this.shown = d.secs;
      return d.secs;
    }
    if (d.kind === 'releasing') {
      const rate = this.sample?.moving ? this.sample.rate : 0;
      const held = d.secs + Math.max(now - d.anchor, 0) * rate;
      const s = this.sample;
      const converged = s !== null && s.at >= d.since && Math.abs((this.target(now) ?? s.pos) - held) < DRAG_RELEASE_TOLERANCE;
      this.shown = held;
      if (converged || now - d.since > DRAG_RELEASE_TIMEOUT) this.drag = { kind: 'none' };
      else return held;
    }
    const sample = this.sample;
    const target = this.target(now);
    if (!sample || target === null) return this.shown;
    const dt = previousNow === null ? Infinity : Math.max(now - previousNow, 0);
    const predicted = this.shown + Math.min(dt, RESYNC_GAP) * sample.rate;
    const error = target - predicted;
    if (!sample.moving || dt > RESYNC_GAP || Math.abs(error) > SNAP_SECS) {
      this.shown = target;
    } else {
      const next = predicted + error * (1 - Math.exp(-dt / SMOOTHING_TAU));
      this.shown = sample.rate >= 0 ? Math.max(next, this.shown) : Math.min(next, this.shown);
    }
    return this.shown;
  }
}
