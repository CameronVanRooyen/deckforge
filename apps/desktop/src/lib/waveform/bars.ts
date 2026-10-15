export interface BarPhrase {
  startSecs: number;
  endSecs: number;
}

export interface BarGrid {
  bpm: number;
  firstBeatSecs: number;
}

export type GridLineKind = 'phrase' | 'four' | 'bar' | 'beat';

export interface GridLine {
  secs: number;
  kind: GridLineKind;
  /** Continuous 1-based bar number; 0 for beat ticks. */
  bar: number;
  /** Index of the phrase that owns a `phrase` line, otherwise -1. */
  phrase: number;
}

export interface BarPosition {
  /** Global 1-based bar number; bar 1 starts at the first downbeat of the track. */
  bar: number;
  beat: number;
  fraction: number;
  toFourBar: number;
  /** Fractional beats since the downbeat that starts the surrounding phrase. */
  beatsSinceAnchor: number;
}

const BEATS_PER_BAR = 4;
const FOUR_BAR_BEATS = 16;
const EDGE_BEATS = 0.02;
const TICK_MERGE_BEATS = 0.1;

const mod = (n: number, m: number) => ((n % m) + m) % m;

/** Global bar lattice: bar k starts at origin + k * bar; grid beat 0 is a downbeat and bar 0 is the first at or after 0 s. */
interface Lattice {
  origin: number;
  beat: number;
  bar: number;
}

function latticeOf(grid: BarGrid): Lattice {
  const beat = 60 / grid.bpm;
  const bar = beat * BEATS_PER_BAR;
  const origin = grid.firstBeatSecs - Math.floor((grid.firstBeatSecs + EDGE_BEATS * beat) / bar) * bar;
  return { origin, beat, bar };
}

const barSecs = (l: Lattice, k: number) => l.origin + k * l.bar;
const firstBarAtOrAfter = (l: Lattice, secs: number) => Math.ceil((secs - l.origin) / l.bar - EDGE_BEATS / BEATS_PER_BAR);

interface Region {
  anchor: number;
  next: number | null;
  phrase: number;
}

function regionsOf(phrases: BarPhrase[], l: Lattice): Region[] {
  const anchors: [number, number][] = [];
  phrases.forEach((p, i) => {
    const k = firstBarAtOrAfter(l, p.startSecs);
    const last = anchors[anchors.length - 1];
    if (last && last[0] >= k) last[1] = i;
    else anchors.push([k, i]);
  });
  if (anchors.length === 0) return [{ anchor: 0, next: null, phrase: -1 }];
  const regions: Region[] = [{ anchor: anchors[0][0], next: anchors[0][0], phrase: -1 }];
  anchors.forEach(([k, i], n) => regions.push({ anchor: k, next: n + 1 < anchors.length ? anchors[n + 1][0] : null, phrase: i }));
  return regions;
}

function regionOfBar(regions: Region[], k: number): Region {
  for (let i = regions.length - 1; i >= 1; i--) if (k >= regions[i].anchor) return regions[i];
  return regions[0];
}

function barLineKind(region: Region, k: number): GridLineKind {
  if (region.phrase >= 0 && k === region.anchor) return 'phrase';
  return mod(k - region.anchor, BEATS_PER_BAR) === 0 ? 'four' : 'bar';
}

const validGrid = (grid: BarGrid) => grid.bpm > 0 && Number.isFinite(grid.bpm) && Number.isFinite(grid.firstBeatSecs);

/**
 * Bar lines and beat ticks inside [fromSecs, toSecs], sorted by time. Bars sit on one global phase (grid beat 0 is a
 * downbeat) and are numbered from the first downbeat of the track; 4-bar lines count from each phrase start.
 */
export function gridLines(phrases: BarPhrase[], grid: BarGrid, fromSecs: number, toSecs: number): GridLine[] {
  if (!validGrid(grid) || !(toSecs >= fromSecs)) return [];
  const l = latticeOf(grid);
  const regions = regionsOf(phrases, l);
  const lines: GridLine[] = [];
  const k0 = Math.ceil((fromSecs - l.origin) / l.bar - 1e-9);
  const k1 = Math.floor((toSecs - l.origin) / l.bar + 1e-9);
  for (let k = k0; k <= k1; k++) {
    const region = regionOfBar(regions, k);
    const kind = barLineKind(region, k);
    lines.push({ secs: barSecs(l, k), kind, bar: k + 1, phrase: kind === 'phrase' ? region.phrase : -1 });
  }
  const n0 = Math.ceil((fromSecs - grid.firstBeatSecs) / l.beat - 1e-9);
  const n1 = Math.floor((toSecs - grid.firstBeatSecs) / l.beat + 1e-9);
  for (let n = n0; n <= n1; n++) {
    const secs = grid.firstBeatSecs + n * l.beat;
    const offset = (secs - l.origin) / l.beat;
    const nearest = Math.round(offset);
    const onBarLine = Math.abs(offset - nearest) <= TICK_MERGE_BEATS && mod(nearest, BEATS_PER_BAR) === 0;
    if (!onBarLine) lines.push({ secs, kind: 'beat', bar: 0, phrase: -1 });
  }
  return lines.sort((a, b) => a.secs - b.secs);
}

/** Global bar and beat for a position, and beats to the next 4-bar line of its phrase (or the next phrase), or null without a grid. */
export function barPosition(phrases: BarPhrase[], grid: BarGrid, secs: number): BarPosition | null {
  if (!validGrid(grid)) return null;
  const l = latticeOf(grid);
  const regions = regionsOf(phrases, l);
  const bt = (secs - l.origin) / l.beat;
  const whole = Math.floor(bt + 1e-9);
  const region = regionOfBar(regions, Math.floor(whole / BEATS_PER_BAR));
  const since = (secs - barSecs(l, region.anchor)) / l.beat;
  const toMultiple = Math.ceil(FOUR_BAR_BEATS - mod(since, FOUR_BAR_BEATS) - 1e-6);
  const toRegionEnd = region.next === null ? Infinity : Math.ceil((barSecs(l, region.next) - secs) / l.beat - 1e-6);
  return {
    bar: Math.floor(whole / BEATS_PER_BAR) + 1,
    beat: mod(whole, BEATS_PER_BAR) + 1,
    fraction: bt - whole,
    toFourBar: Math.max(0, Math.min(toMultiple, toRegionEnd)),
    beatsSinceAnchor: since,
  };
}

const wrap = (x: number, period: number) => mod(x + period / 2, period) - period / 2;

export interface PhaseOffset {
  beats: number;
  bars: number;
}

/** How far deck `b` sits ahead of reference deck `a`: a beat offset inside the bar plus whole bars inside the 4-bar phrase. */
export function phaseOffset(beatsA: number, beatsB: number): PhaseOffset {
  const beats = wrap(beatsB - beatsA, BEATS_PER_BAR);
  const bars = Math.round((wrap(beatsB - beatsA, FOUR_BAR_BEATS) - beats) / BEATS_PER_BAR);
  return { beats, bars };
}
