import { library } from '$lib/state/library.svelte';
import type { DeckSnapshot, Phrase } from '$lib/api';
import type { BarGrid } from './bars';

export function deckPhrases(deck: 0 | 1, token: number): Phrase[] {
  const set = library.deckPhrases[deck];
  return set && set.token === token ? set.phrases : [];
}

export function deckGrid(d: DeckSnapshot): BarGrid {
  return { bpm: d.gridBpm, firstBeatSecs: d.firstBeatSecs };
}
