export interface Camelot {
  n: number;
  letter: 'A' | 'B';
}

export type KeyCompat = 'same' | 'adjacent' | 'relative';

const HUES = [170, 135, 100, 58, 32, 4, 336, 302, 268, 232, 206, 186];

export function parseCamelot(key: string | null | undefined): Camelot | null {
  const m = key ? /^(\d{1,2})([ABab])$/.exec(key.trim()) : null;
  if (!m) return null;
  const n = parseInt(m[1], 10);
  return n >= 1 && n <= 12 ? { n, letter: m[2].toUpperCase() as 'A' | 'B' } : null;
}

export function keyHue(key: string | null | undefined): number | null {
  const c = parseCamelot(key);
  return c ? HUES[c.n - 1] : null;
}

export function semitoneShift(tempoRatio: number): number {
  return tempoRatio > 0 ? Math.round(12 * Math.log2(tempoRatio)) : 0;
}

/** Camelot moves seven steps around the wheel per semitone, keeping the A/B letter. */
export function shiftCamelot(key: string, semitones: number): string {
  const c = parseCamelot(key);
  if (!c) return key;
  const n = ((((c.n - 1 + 7 * semitones) % 12) + 12) % 12) + 1;
  return `${n}${c.letter}`;
}

export interface EffectiveKey {
  key: string;
  semitones: number;
}

export function effectiveKey(key: string | null | undefined, tempoRatio: number, keyLock: boolean): EffectiveKey | null {
  if (!key || !parseCamelot(key)) return key ? { key, semitones: 0 } : null;
  const semitones = keyLock ? 0 : semitoneShift(tempoRatio);
  return { key: semitones ? shiftCamelot(key, semitones) : key, semitones };
}

export function keyCompat(reference: string | null | undefined, key: string | null | undefined): KeyCompat | null {
  const a = parseCamelot(reference);
  const b = parseCamelot(key);
  if (!a || !b) return null;
  if (a.n === b.n) return a.letter === b.letter ? 'same' : 'relative';
  const step = Math.min((a.n - b.n + 12) % 12, (b.n - a.n + 12) % 12);
  return step === 1 && a.letter === b.letter ? 'adjacent' : null;
}
