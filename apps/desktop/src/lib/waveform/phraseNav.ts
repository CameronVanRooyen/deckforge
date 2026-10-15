// Mirrors crates/gfx/src/phrase_nav.rs; crates/gfx/fixtures/phrase_nav.json keeps them equal.
import type { BarGrid, BarPhrase, GridLine } from './bars';

const BEATS_PER_BAR = 4;
const SNAP_BARS = 0.05;
const EDGE_SECS = 1e-6;

export type BarLevel = 'bar' | 'four' | 'eight' | 'sixteen' | 'phrase';
export const LEVEL_RANK: Record<BarLevel, number> = { bar: 0, four: 1, eight: 2, sixteen: 3, phrase: 4 };

export interface BarMark {
  level: BarLevel;
  /** 1-based bar inside the containing phrase. */
  barInPhrase: number;
  /** Phrase length in bars; 0 outside any phrase. */
  phraseBars: number;
  phrase: number;
}

export const barSecs = (grid: BarGrid) => (60 / grid.bpm) * BEATS_PER_BAR;

/** Length in bars, rounded when within a twentieth of a bar of a whole number. */
export function phraseBars(p: BarPhrase, grid: BarGrid): { bars: number; whole: boolean } {
  const bars = (p.endSecs - p.startSecs) / barSecs(grid);
  const whole = Math.abs(bars - Math.round(bars)) < SNAP_BARS;
  return { bars: whole ? Math.round(bars) : Math.ceil(bars), whole };
}

export function phraseIndexAt(phrases: BarPhrase[], secs: number): number {
  for (let i = phrases.length - 1; i >= 0; i--) {
    const p = phrases[i];
    if (secs + EDGE_SECS >= p.startSecs && secs < p.endSecs - EDGE_SECS) return i;
  }
  return -1;
}

export function nextPhraseIndex(phrases: BarPhrase[], secs: number): number {
  return phrases.findIndex((p) => p.startSecs > secs + EDGE_SECS);
}

/** Classifies a non-beat grid line by its position inside the containing phrase. */
export function classify(line: GridLine, phrases: BarPhrase[], grid: BarGrid): BarMark {
  const bar = barSecs(grid);
  if (line.kind === 'phrase' && phrases[line.phrase]) {
    return { level: 'phrase', barInPhrase: 1, phraseBars: phraseBars(phrases[line.phrase], grid).bars, phrase: line.phrase };
  }
  const i = phraseIndexAt(phrases, line.secs);
  let k: number;
  let len = 0;
  let nextStart: number;
  if (i >= 0) {
    const p = phrases[i];
    k = Math.floor((line.secs - p.startSecs) / bar + SNAP_BARS);
    len = phraseBars(p, grid).bars;
    nextStart = p.endSecs;
  } else {
    const n = nextPhraseIndex(phrases, line.secs);
    k = line.bar - 1;
    nextStart = n >= 0 ? phrases[n].startSecs : Infinity;
  }
  const crowded = nextStart - line.secs < bar * (1 - SNAP_BARS);
  let level: BarLevel = 'bar';
  if (k <= 0 || crowded) level = 'bar';
  else if (k % 16 === 0 && (len === 0 || len > 16)) level = 'sixteen';
  else if (k % 8 === 0 && (len === 0 || len > 8)) level = 'eight';
  else if (k % 4 === 0) level = 'four';
  return { level, barInPhrase: k + 1, phraseBars: len, phrase: i };
}

export interface Countdown {
  current: number;
  next: number;
  /** Beats from the position to the next phrase start. */
  beats: number;
  /** Whole beats still to play before the change, shown as `bars.beats`. */
  remainingBeats: number;
  /** 0..1 through the current phrase, 0 before the first phrase. */
  progress: number;
}

export function countdown(phrases: BarPhrase[], grid: BarGrid, secs: number): Countdown | null {
  if (!(grid.bpm > 0)) return null;
  const next = nextPhraseIndex(phrases, secs);
  if (next < 0) return null;
  const beats = (phrases[next].startSecs - secs) / (60 / grid.bpm);
  const current = phraseIndexAt(phrases, secs);
  const p = phrases[current];
  const progress = p ? Math.min(1, Math.max(0, (secs - p.startSecs) / Math.max(1e-9, p.endSecs - p.startSecs))) : 0;
  return { current, next, beats, remainingBeats: Math.max(0, Math.ceil(beats - 1e-6)), progress };
}

export const barsBeats = (c: Countdown) => `${Math.floor(c.remainingBeats / 4)}.${c.remainingBeats % 4}`;
export const finalBar = (c: Countdown) => c.remainingBeats <= 4;

/** Wall-clock seconds until a deck reaches its next phrase start at its current playback rate. */
export function wallSecsToNext(phrases: BarPhrase[], pos: number, rate: number): { next: number; secs: number } | null {
  const next = nextPhraseIndex(phrases, pos);
  if (next < 0) return null;
  return { next, secs: (phrases[next].startSecs - pos) / Math.max(0.05, rate) };
}

/**
 * Whole-beat jump for the incoming deck that puts one of its phrase starts on the master's next phrase start.
 * Picks the smallest jump; `null` when either deck has no phrase ahead or the target falls outside the track.
 */
export function alignJumpBeats(
  incoming: { phrases: BarPhrase[]; pos: number; rate: number; bpm: number; duration: number },
  master: { phrases: BarPhrase[]; pos: number; rate: number },
): number | null {
  const target = wallSecsToNext(master.phrases, master.pos, master.rate);
  if (!target || !(incoming.bpm > 0)) return null;
  const beat = 60 / incoming.bpm;
  const lead = target.secs * Math.max(0.05, incoming.rate);
  let best: number | null = null;
  for (const p of incoming.phrases) {
    const jump = Math.round((p.startSecs - incoming.pos - lead) / beat);
    const landing = incoming.pos + jump * beat;
    if (landing < 0 || landing >= incoming.duration) continue;
    if (best === null || Math.abs(jump) < Math.abs(best) || (Math.abs(jump) === Math.abs(best) && jump > best)) best = jump;
  }
  return best;
}

/** Whole-bar jump that lands at the same spot inside the first bar of the next phrase. */
export function nextPhraseJumpBeats(phrases: BarPhrase[], grid: BarGrid, pos: number): number | null {
  const c = countdown(phrases, grid, pos);
  if (!c) return null;
  const jump = Math.ceil(c.beats / BEATS_PER_BAR - 1e-6) * BEATS_PER_BAR;
  return jump > 0 ? jump : BEATS_PER_BAR;
}
