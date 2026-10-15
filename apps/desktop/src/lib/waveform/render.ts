import type { Waveform } from '$lib/api';

export const PALETTE = {
  low: ['#6ea8f7', '#3b73d9', '#1e3a8a'],
  mid: ['#e3a13c', '#bf7219', '#6b3a10'],
  high: ['#f4f4f5', '#d4d4d8', '#a1a1aa'],
};

export interface BandGradients {
  low: CanvasGradient;
  mid: CanvasGradient;
  high: CanvasGradient;
}

export function makeGradients(ctx: CanvasRenderingContext2D, h: number, half = false): BandGradients {
  const mk = (stops: string[]) => {
    const g = ctx.createLinearGradient(0, 0, 0, h);
    if (half) {
      g.addColorStop(0, stops[1]);
      g.addColorStop(1, stops[0]);
    } else {
      g.addColorStop(0, stops[2]);
      g.addColorStop(0.42, stops[1]);
      g.addColorStop(0.5, stops[0]);
      g.addColorStop(0.58, stops[1]);
      g.addColorStop(1, stops[2]);
    }
    return g;
  };
  return { low: mk(PALETTE.low), mid: mk(PALETTE.mid), high: mk(PALETTE.high) };
}

const curve = (v: number) => v / 255;

/**
 * Draws a 3-band waveform for columns [0, w). Column i covers [t0 + i*secPerPx, t0 + (i+1)*secPerPx).
 * Bands are layered low → mid → high; `half` raises bars from the bottom edge instead of mirroring.
 */
export function renderColumns(
  ctx: CanvasRenderingContext2D,
  wf: Waveform,
  w: number,
  h: number,
  t0: number,
  secPerPx: number,
  gradients: BandGradients,
  half = false,
) {
  const pps = wf.pointsPerSec;
  const n = wf.low.length;
  const mid = h / 2;
  const amp = half ? h * 0.96 : h * 0.46;
  const bar = (i: number, v: number, scale: number) => {
    const hp = curve(v) * amp * scale;
    if (half) ctx.fillRect(i, h - hp, 1, hp);
    else ctx.fillRect(i, mid - hp, 1, hp * 2);
  };
  for (let i = 0; i < w; i++) {
    const ta = (t0 + i * secPerPx) * pps;
    const tb = (t0 + (i + 1) * secPerPx) * pps;
    let ia = Math.floor(ta);
    let ib = Math.max(ia + 1, Math.ceil(tb));
    if (ib <= 0 || ia >= n) continue;
    if (ia < 0) ia = 0;
    if (ib > n) ib = n;
    let lo = 0, mi = 0, hi = 0, pk = 0;
    for (let k = ia; k < ib; k++) {
      const l = wf.low[k], m = wf.mid[k], hh = wf.high[k], p = wf.peak[k];
      if (l > lo) lo = l;
      if (m > mi) mi = m;
      if (hh > hi) hi = hh;
      if (p > pk) pk = p;
    }
    ctx.fillStyle = 'rgba(161,161,170,0.13)';
    bar(i, pk, 1);
    ctx.fillStyle = gradients.low;
    bar(i, lo, 1);
    ctx.fillStyle = gradients.mid;
    bar(i, mi, 0.82);
    ctx.fillStyle = gradients.high;
    bar(i, hi, 0.62);
  }
}
