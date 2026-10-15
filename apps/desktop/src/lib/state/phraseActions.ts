import { cmd, engine } from '$lib/state/engine.svelte';
import { deckGrid, deckPhrases } from '$lib/waveform/beats';
import { alignJumpBeats, nextPhraseJumpBeats } from '$lib/waveform/phraseNav';
import type { DeckIndex } from '$lib/api';

const ready = (deck: DeckIndex) => {
  const d = engine.latest.decks[deck];
  return d.loaded && d.hasGrid && d.gridBpm > 0 ? d : null;
};

/** Beats to jump so `deck` lands at the same spot inside the first bar of its next phrase. */
export function nextPhraseJump(deck: DeckIndex): number | null {
  const d = ready(deck);
  return d ? nextPhraseJumpBeats(deckPhrases(deck, d.trackToken), deckGrid(d), d.positionSecs) : null;
}

/** The deck that follows the master, with the whole-beat jump that lands its phrase start on the master's next one. */
export function alignJump(): { deck: DeckIndex; beats: number } | null {
  const snap = engine.latest;
  if (snap.masterDeck !== 0 && snap.masterDeck !== 1) return null;
  const master = snap.masterDeck as DeckIndex;
  const deck = (1 - master) as DeckIndex;
  const m = ready(master);
  const d = ready(deck);
  if (!m || !d) return null;
  const beats = alignJumpBeats(
    { phrases: deckPhrases(deck, d.trackToken), pos: d.positionSecs, rate: d.tempoRatio, bpm: d.gridBpm, duration: d.durationSecs },
    { phrases: deckPhrases(master, m.trackToken), pos: m.positionSecs, rate: m.tempoRatio },
  );
  return beats === null ? null : { deck, beats };
}

export function jumpToNextPhrase(deck: DeckIndex) {
  const beats = nextPhraseJump(deck);
  if (beats) cmd({ type: 'beatJump', deck, beats });
}

export function alignPhrases() {
  const a = alignJump();
  if (a && a.beats !== 0) cmd({ type: 'beatJump', deck: a.deck, beats: a.beats });
}
