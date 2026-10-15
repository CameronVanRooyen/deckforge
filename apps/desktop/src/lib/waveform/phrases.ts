import type { Phrase, PhraseKind } from '$lib/api';

export const PHRASE_STYLE: Record<PhraseKind, { color: string; label: string; name: string }> = {
  intro: { color: '#2dd4bf', label: 'Intro', name: 'INTRO' },
  build: { color: '#facc15', label: 'Build', name: 'BUILD' },
  drop: { color: '#f87171', label: 'Drop', name: 'DROP' },
  break: { color: '#818cf8', label: 'Break', name: 'BREAK' },
  outro: { color: '#94a3b8', label: 'Outro', name: 'OUTRO' },
};

export function phraseAt(phrases: Phrase[], secs: number): Phrase | null {
  return phrases.find((p) => secs >= p.startSecs && secs < p.endSecs) ?? null;
}
