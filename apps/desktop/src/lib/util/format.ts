export const clamp = (v: number, lo: number, hi: number) => (v < lo ? lo : v > hi ? hi : v);
export const lerp = (a: number, b: number, t: number) => a + (b - a) * t;

export function fmtTime(secs: number, showTenths = true): string {
  if (!Number.isFinite(secs)) return '--:--';
  const neg = secs < 0;
  const s = Math.abs(secs);
  const m = Math.floor(s / 60);
  const sec = Math.floor(s % 60);
  const t = Math.floor((s * 10) % 10);
  const base = `${m}:${sec.toString().padStart(2, '0')}`;
  return `${neg ? '-' : ''}${base}${showTenths ? `.${t}` : ''}`;
}

export function fmtDuration(secs: number): string {
  return fmtTime(secs, false);
}

export function fmtBpm(bpm: number | null | undefined, digits = 1): string {
  return bpm && bpm > 0 ? bpm.toFixed(digits) : '—';
}

export function fmtPercent(ratio: number, digits = 2): string {
  const p = (ratio - 1) * 100;
  const s = Math.abs(p) < 0.005 ? 0 : p;
  return `${s >= 0 ? '+' : '−'}${Math.abs(s).toFixed(digits)}%`;
}

export function hexColor(c: number): string {
  return '#' + (c & 0xffffff).toString(16).padStart(6, '0');
}

export function rgbOf(c: number): [number, number, number] {
  return [(c >> 16) & 255, (c >> 8) & 255, c & 255];
}

export function rangeValue(range: string): number {
  switch (range) {
    case 'Six': return 0.06;
    case 'Ten': return 0.1;
    case 'Sixteen': return 0.16;
    default: return 1;
  }
}

export function rangeLabel(range: string): string {
  switch (range) {
    case 'Six': return '±6%';
    case 'Ten': return '±10%';
    case 'Sixteen': return '±16%';
    default: return 'WIDE';
  }
}

export function linearToDb(v: number): number {
  return v <= 1e-5 ? -100 : 20 * Math.log10(v);
}

export function errorMessage(e: unknown): string {
  if (typeof e === 'string') return e;
  if (e instanceof Error) return e.message;
  try { return JSON.stringify(e); } catch { return String(e); }
}

export const hex = (b: number) => b.toString(16).toUpperCase().padStart(2, '0');
