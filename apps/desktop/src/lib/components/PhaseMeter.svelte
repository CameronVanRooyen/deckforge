<script lang="ts">
  import { engine } from '$lib/state/engine.svelte';
  import { deckGrid, deckPhrases } from '$lib/waveform/beats';
  import { barPosition, phaseOffset } from '$lib/waveform/bars';
  import { wallSecsToNext } from '$lib/waveform/phraseNav';
  import { alignJump, alignPhrases, jumpToNextPhrase, nextPhraseJump } from '$lib/state/phraseActions';
  import PhraseCountdown from './PhraseCountdown.svelte';
  import StepForward from '@lucide/svelte/icons/step-forward';
  import type { DeckIndex } from '$lib/api';

  const decks = [0, 1] as const;
  const snap = $derived(engine.snap);

  const pos = $derived(
    decks.map((i) => {
      const d = snap.decks[i];
      if (!d.loaded || !d.hasGrid || d.gridBpm <= 0) return null;
      return barPosition(deckPhrases(i, d.trackToken), deckGrid(d), d.positionSecs);
    }),
  );
  const beats = $derived(pos.map((p) => (p === null ? null : p.beatsSinceAnchor)));

  const reference = $derived<DeckIndex | null>(snap.masterDeck !== null && beats[snap.masterDeck] !== null ? (snap.masterDeck as DeckIndex) : null);
  const other = $derived(reference === null ? null : ((1 - reference) as DeckIndex));
  const offset = $derived(reference !== null && other !== null && beats[other] !== null ? phaseOffset(beats[reference] as number, beats[other] as number) : null);
  const quality = $derived(offset === null ? 'none' : Math.abs(offset.beats) < 0.06 && offset.bars === 0 ? 'good' : Math.abs(offset.beats) < 0.3 ? 'near' : 'far');

  /** Master beats between the master's next phrase start and the other deck's, at both decks' current tempos. */
  const landing = $derived.by(() => {
    if (reference === null || other === null) return null;
    const m = snap.decks[reference];
    const o = snap.decks[other];
    if (!o.loaded || !o.hasGrid) return null;
    const mine = wallSecsToNext(deckPhrases(reference, m.trackToken), m.positionSecs, m.tempoRatio);
    const theirs = wallSecsToNext(deckPhrases(other, o.trackToken), o.positionSecs, o.tempoRatio);
    if (!mine || !theirs) return null;
    return ((theirs.secs - mine.secs) * m.gridBpm * m.tempoRatio) / 60;
  });
  const together = $derived(landing !== null && Math.abs(landing) < 0.25);
  const align = $derived.by(() => {
    snap;
    return alignJump();
  });
  const jumps = $derived(decks.map((i) => (snap.decks[i].loaded ? nextPhraseJump(i) : null)));

  const letter = (i: number) => (i === 0 ? 'A' : 'B');
  const signed = (v: number, digits: number) => (Math.abs(v) < 0.5 * 10 ** -digits ? (0).toFixed(digits) : `${v < 0 ? '−' : '+'}${Math.abs(v).toFixed(digits)}`);
  const barsBeatsOf = (beatsAway: number) => {
    const n = Math.round(Math.abs(beatsAway));
    return `${beatsAway < 0 ? '−' : '+'}${Math.floor(n / 4)}.${n % 4}`;
  };
  const jumpText = (b: number) => (b % 4 === 0 ? `${Math.abs(b / 4)} bar${Math.abs(b) === 4 ? '' : 's'}` : `${Math.abs(b)} beat${Math.abs(b) === 1 ? '' : 's'}`);
</script>

<div class="phase" role="group" aria-label="Beat and phrase alignment">
  {#each decks as i (i)}
    {@const p = pos[i]}
    <div class="side" data-deck={i} class:rev={i === 1}>
      <b class="tag">{letter(i)}</b>
      <div class="segs" title="Position inside the current bar">
        {#each [1, 2, 3, 4] as b (b)}
          {@const lit = p !== null && p.beat === b}
          <i class:on={lit} class:past={p !== null && p.beat > b} class:down={b === 1} style={lit && p ? `--f:${Math.round(p.fraction * 100)}%` : ''}></i>
        {/each}
      </div>
      <PhraseCountdown deck={i} size="sm" />
      <button class="jump" disabled={!jumps[i]} onclick={() => jumpToNextPhrase(i)} title={`Jump deck ${letter(i)} to its next phrase, keeping the beat (${i === 0 ? 'R' : 'U'})`} aria-label={`Jump deck ${letter(i)} to its next phrase`}>
        <StepForward size={13} strokeWidth={2.2} />
      </button>
    </div>
    {#if i === 0}
      <div class="mid">
        <div class="cell" data-quality={quality} title={other === null ? 'Needs a master deck and two loaded decks' : `Beat phase of deck ${letter(other)} against master deck ${letter(reference!)}`}>
          {#if offset && other !== null && reference !== null}
            <span class="vs">{letter(other)} vs {letter(reference)}</span>
            <span class="num val">{signed(offset.beats, 2)}</span>
            {#if offset.bars !== 0}<span class="num bars">{offset.bars > 0 ? '+' : '−'}{Math.abs(offset.bars)} bar{Math.abs(offset.bars) === 1 ? '' : 's'}</span>{/if}
          {:else}
            <span class="vs">Phase</span><span class="num val">—</span>
          {/if}
        </div>
        <div
          class="cell phrases"
          class:together
          class:apart={landing !== null && !together}
          title={landing === null || other === null || reference === null
            ? 'Next phrase starts: needs a master deck and phrases on both decks'
            : together
              ? `Deck ${letter(other)}'s next phrase starts with deck ${letter(reference)}'s`
              : `Deck ${letter(other)}'s next phrase starts ${barsBeatsOf(landing)} bars.beats ${landing > 0 ? 'after' : 'before'} deck ${letter(reference)}'s`}
        >
          <span class="vs">Phrases</span>
          {#if landing === null}
            <span class="num val">—</span>
          {:else if together}
            <span class="val ok">land together</span>
          {:else}
            <span class="num val">{letter(other!)} {barsBeatsOf(landing)}</span>
          {/if}
          <button
            class="align"
            disabled={!align || align.beats === 0}
            onclick={alignPhrases}
            title={align && align.beats !== 0 ? `Jump deck ${letter(align.deck)} ${align.beats > 0 ? 'forward' : 'back'} ${jumpText(align.beats)} so its next phrase starts with the master's (G)` : 'Phrases already land together'}
          >Align</button>
        </div>
      </div>
    {/if}
  {/each}
</div>

<style>
  .phase {
    flex: none;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
    align-items: center;
    gap: 12px;
    height: 28px;
    background: var(--ink);
  }
  .side {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .side.rev {
    flex-direction: row-reverse;
  }
  .tag {
    flex: none;
    width: 14px;
    height: 14px;
    display: grid;
    place-items: center;
    border-radius: 3px;
    font-size: 9.5px;
    font-weight: 750;
    color: #09090b;
    background: var(--deck);
  }
  .segs {
    flex: 1;
    display: flex;
    gap: 3px;
    min-width: 60px;
  }
  .segs i {
    position: relative;
    flex: 1;
    height: 6px;
    border-radius: 2px;
    background: var(--raised-3);
    overflow: hidden;
  }
  .segs i.past {
    background: #52525b;
  }
  .segs i.on {
    background: linear-gradient(90deg, #e4e4e7 var(--f), #52525b var(--f));
  }
  .segs i.on.down {
    background: linear-gradient(90deg, var(--deck) var(--f), #52525b var(--f));
  }
  .segs i.past.down {
    background: color-mix(in srgb, var(--deck) 55%, #52525b);
  }
  .jump {
    flex: none;
    width: 22px;
    height: 20px;
    display: grid;
    place-items: center;
    border-radius: 5px;
    border: 1px solid var(--line);
    background: var(--panel);
    color: var(--dim);
    transition:
      background var(--dur),
      color var(--dur),
      border-color var(--dur);
  }
  .jump:hover:not(:disabled) {
    color: var(--fg);
    border-color: color-mix(in srgb, var(--deck) 60%, var(--line));
  }
  .jump:disabled {
    opacity: 0.35;
  }
  .mid {
    display: flex;
    gap: 6px;
  }
  .cell {
    display: flex;
    align-items: center;
    gap: 7px;
    height: 22px;
    padding: 0 9px;
    border-radius: var(--radius);
    border: 1px solid var(--line);
    background: var(--panel);
    white-space: nowrap;
    transition: border-color var(--dur);
  }
  .cell:first-child {
    min-width: 132px;
    justify-content: center;
  }
  .vs {
    font-size: 10px;
    font-weight: 560;
    color: var(--faint);
  }
  .val {
    font-size: 11.5px;
    font-weight: 650;
    color: var(--dim);
  }
  .bars {
    font-size: 11px;
    font-weight: 650;
    color: var(--warn);
  }
  .cell[data-quality='good'] {
    border-color: color-mix(in srgb, var(--ok) 45%, var(--line));
  }
  .cell[data-quality='good'] .val {
    color: #86efac;
  }
  .cell[data-quality='near'] .val {
    color: #fcd34d;
  }
  .cell[data-quality='far'] .val {
    color: #fca5a5;
  }
  .phrases {
    padding-right: 3px;
  }
  .phrases.together {
    border-color: color-mix(in srgb, var(--ok) 50%, var(--line));
    background: color-mix(in srgb, var(--ok) 9%, var(--panel));
  }
  .val.ok {
    color: #86efac;
  }
  .phrases.apart .val {
    color: #fcd34d;
  }
  .align {
    height: 16px;
    padding: 0 7px;
    border-radius: 4px;
    border: 1px solid var(--line-2);
    background: var(--raised-2);
    font-size: 10px;
    font-weight: 650;
    color: var(--text);
    transition:
      background var(--dur),
      color var(--dur),
      border-color var(--dur);
  }
  .align:hover:not(:disabled) {
    background: var(--raised-3);
    color: var(--fg);
  }
  .align:disabled {
    opacity: 0.4;
  }
</style>
