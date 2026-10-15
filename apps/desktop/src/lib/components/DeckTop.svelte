<script lang="ts">
  import { engine, cmd } from '$lib/state/engine.svelte';
  import { library } from '$lib/state/library.svelte';
  import { ui } from '$lib/state/ui.svelte';
  import { fmtPercent, fmtTime } from '$lib/util/format';
  import { effectiveKey } from '$lib/util/camelot';
  import { deckGrid, deckPhrases } from '$lib/waveform/beats';
  import { barPosition } from '$lib/waveform/bars';
  import { dropTargets } from '$lib/state/dnd.svelte';
  import OverviewWave from './OverviewWave.svelte';
  import GridEditor from './GridEditor.svelte';
  import KeyBadge from './KeyBadge.svelte';
  import PhraseCountdown from './PhraseCountdown.svelte';
  import Eject from '@lucide/svelte/icons/eject';
  import Disc3 from '@lucide/svelte/icons/disc-3';
  import type { DeckIndex } from '$lib/api';

  let { deck }: { deck: DeckIndex } = $props();
  // svelte-ignore state_referenced_locally
  const letter = deck === 0 ? 'A' : 'B';

  const d = $derived(engine.snap.decks[deck]);
  const track = $derived(library.deckTracks[deck]);
  const art = $derived(library.deckArt[deck]);
  const mode = $derived(ui.timeMode[deck]);
  const eff = $derived(effectiveKey(track?.key, d.tempoRatio, d.keyLock));
  const timeText = $derived(
    !d.loaded ? '--:--.-' : mode === 'remaining' ? `-${fmtTime(Math.max(0, d.durationSecs - d.positionSecs))}` : fmtTime(d.positionSecs),
  );
  const place = $derived(
    d.loaded && d.hasGrid && d.gridBpm > 0 ? barPosition(deckPhrases(deck, d.trackToken), deckGrid(d), d.positionSecs) : null,
  );
  const pitch = $derived(d.tempoRatio - 1);
  let width = $state(600);
  const remainingWarn = $derived(d.loaded && d.durationSecs - d.positionSecs < 30 && d.durationSecs > 60);
</script>

<section class="card" data-deck={deck} data-drop-deck={deck} class:drop={dropTargets.deck === deck} class:empty={!d.loaded} class:narrow={width < 470} class:tight={width < 410} bind:clientWidth={width}>
  <div class="hd">
    {#if art}<div class="washbox"><img class="wash" src={art} alt="" draggable="false" /></div>{/if}
    <div class="id">
      <span class="letter">{letter}</span>
      <div class="art" class:blank={!art}>
        {#if art}
          <img src={art} alt="" draggable="false" />
        {:else}
          <Disc3 size={14} strokeWidth={1.6} />
        {/if}
      </div>
      <div class="titles">
        <span class="title" title={track?.title}>{track?.title ?? (d.loaded ? 'Loading…' : 'No track loaded')}</span>
        <span class="artist" title={track?.artist}>{track?.artist ?? (d.loaded ? '' : `Drag a track here, or press ${deck === 0 ? '←' : '→'} on the selection`)}</span>
      </div>
      <div class="flags">
        <span class="flag master" class:on={d.isMaster}>Master</span>
        <span class="flag sync" class:on={d.syncEnabled}>Sync</span>
        <button class="flag lock" class:on={d.keyLock} disabled={!d.loaded} onclick={() => cmd({ type: 'setKeyLock', deck, enabled: !d.keyLock })} title="Key lock (master tempo)">Key lock</button>
      </div>
      <button class="eject" disabled={!d.loaded} onclick={() => library.eject(deck)} aria-label={`Eject deck ${letter}`} title="Eject track"><Eject size={14} strokeWidth={2} /></button>
    </div>

    <div class="stats">
      <div class="stat key">
        {#if eff}
          <KeyBadge value={eff.key} semitones={eff.semitones} original={track?.key} name={eff.semitones ? null : track?.keyName} size="lg" />
        {:else}
          <KeyBadge value={null} size="lg" />
        {/if}
      </div>

      <div class="tempo">
        <div class="stat">
          <div class="big num">{d.loaded && d.hasGrid ? d.bpm.toFixed(1) : '—'}<small>BPM</small></div>
          <div class="sub num" class:pos={pitch > 0.00005} class:neg={pitch < -0.00005}>{d.loaded ? fmtPercent(d.tempoRatio, 1) : '—'}</div>
        </div>
        <GridEditor {deck} />
      </div>

      <PhraseCountdown {deck} />

      <button class="stat time" onclick={() => ui.toggleTime(deck)} title="Click to toggle elapsed / remaining">
        <div class="big num" class:warn={remainingWarn}>{timeText}</div>
        <div class="place num">
          <span title="Bar and beat in the track">{place ? `${place.bar}.${place.beat}` : '—'}</span>
        </div>
      </button>
    </div>
  </div>

  <div class="ovwrap">
    <OverviewWave {deck} />
  </div>
</section>

<style>
  .card {
    position: relative;
    min-width: 0;
    display: flex;
    flex-direction: column;
    border: 1px solid var(--line);
    border-radius: var(--radius-lg);
    transition: border-color var(--dur);
  }
  .card.drop {
    border-color: var(--deck);
  }
  .washbox {
    position: absolute;
    inset: 0;
    overflow: hidden;
    border-radius: inherit;
    pointer-events: none;
  }
  .wash {
    position: absolute;
    inset: -40px;
    width: calc(100% + 80px);
    height: calc(100% + 80px);
    object-fit: cover;
    filter: blur(28px) saturate(1.5);
    opacity: 0.32;
    pointer-events: none;
    mask-image: linear-gradient(90deg, #000 0%, rgba(0, 0, 0, 0.55) 55%, rgba(0, 0, 0, 0.25) 100%);
  }
  .hd {
    flex: 1;
    justify-content: center;
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px 12px 7px;
    min-width: 0;
    border-radius: 7px 7px 0 0;
    background:
      linear-gradient(90deg, rgba(var(--deck-rgb), 0.13), transparent 46%),
      var(--panel);
  }
  .hd::after {
    content: '';
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: 3px;
    background: var(--deck);
  }
  .card[data-deck='1'] .hd::after {
    left: auto;
    right: 0;
  }
  .id,
  .stats {
    position: relative;
  }
  .id {
    display: flex;
    align-items: center;
    gap: 7px;
    min-width: 0;
    height: 22px;
  }
  .letter {
    flex: none;
    width: 20px;
    height: 20px;
    display: grid;
    place-items: center;
    border-radius: 5px;
    font-weight: 750;
    font-size: 12px;
    color: #09090b;
    background: var(--deck);
  }
  .art {
    position: relative;
    flex: none;
    width: 22px;
    height: 22px;
    border-radius: 5px;
    overflow: hidden;
    background: var(--raised);
    border: 1px solid var(--line-2);
    display: grid;
    place-items: center;
    color: var(--faint);
  }
  .art img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .titles {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: baseline;
    gap: 8px;
    white-space: nowrap;
    overflow: hidden;
  }
  .title {
    flex: 0 1 auto;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    font-size: 14px;
    font-weight: 640;
    letter-spacing: -0.01em;
    color: var(--fg);
  }
  .empty .title {
    color: var(--dim);
    font-weight: 540;
  }
  .artist {
    flex: 1 1 0;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    font-size: 12px;
    color: var(--dim);
  }
  .flags {
    flex: none;
    display: flex;
    gap: 4px;
  }
  .flag {
    flex: none;
    white-space: nowrap;
    height: 18px;
    padding: 0 6px;
    display: inline-flex;
    align-items: center;
    border-radius: 4px;
    border: 1px solid var(--line);
    background: rgba(9, 9, 11, 0.35);
    font-size: 9.5px;
    font-weight: 650;
    letter-spacing: 0.01em;
    color: var(--faint);
    transition:
      background var(--dur),
      color var(--dur),
      border-color var(--dur);
  }
  .flag.master.on {
    background: var(--sync);
    border-color: var(--sync);
    color: #fff;
  }
  .flag.sync.on {
    background: color-mix(in srgb, var(--sync) 18%, transparent);
    border-color: color-mix(in srgb, var(--sync) 60%, transparent);
    color: #93c5fd;
  }
  .flag.lock.on {
    background: rgba(250, 250, 250, 0.1);
    border-color: var(--line-2);
    color: var(--text);
  }
  button.flag:hover:not(:disabled) {
    border-color: var(--line-2);
    color: var(--dim);
  }
  button.flag.on:hover:not(:disabled) {
    color: var(--fg);
  }
  button.flag:disabled {
    opacity: 0.5;
  }
  .stats {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
    height: 40px;
  }
  .stat {
    flex: none;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    justify-content: center;
    gap: 3px;
  }
  .stat.key {
    min-width: 42px;
    align-items: flex-start;
  }
  .tempo {
    flex: none;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .big {
    font-size: 21px;
    font-weight: 620;
    line-height: 1;
    color: var(--fg);
  }
  .big small {
    margin-left: 3px;
    font-size: 9.5px;
    font-weight: 600;
    color: var(--faint);
    letter-spacing: 0;
  }
  .sub {
    font-size: 11.5px;
    font-weight: 600;
    color: var(--dim);
  }
  .sub.pos {
    color: #fdba74;
  }
  .sub.neg {
    color: #7dd3fc;
  }
  .stats > :global(.phr) {
    margin-left: auto;
  }
  .time {
    padding: 0;
    border: 0;
    background: transparent;
    min-width: 78px;
    text-align: right;
  }
  .time .big.warn {
    color: var(--danger);
  }
  .time:hover .big {
    color: #fff;
  }
  .place {
    display: flex;
    gap: 8px;
    font-size: 11.5px;
    font-weight: 600;
    color: var(--text);
  }
  .eject {
    flex: none;
    width: 24px;
    height: 20px;
    border-radius: 5px;
    border: 1px solid var(--line);
    background: rgba(9, 9, 11, 0.35);
    color: var(--dim);
    display: grid;
    place-items: center;
    transition:
      background var(--dur),
      color var(--dur),
      border-color var(--dur);
  }
  .eject:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.07);
    color: var(--fg);
    border-color: var(--line-2);
  }
  .eject:disabled {
    opacity: 0.3;
  }
  .ovwrap {
    border: 0 solid var(--panel);
    border-width: 0 8px 7px;
    border-radius: 0 0 7px 7px;
  }
  .narrow .tempo :global(.trig .lbl) {
    display: none;
  }
  .narrow .tempo :global(.trig) {
    padding: 0 5px;
  }
  .tight .art {
    display: none;
  }
  .tight .stats {
    gap: 6px;
  }
  .tight .stats > :global(.phr) {
    width: 104px;
    padding-inline: 6px;
  }
  .tight .time {
    min-width: 0;
  }
  .tight .big {
    font-size: 18px;
  }
  @media (max-height: 820px) {
    .hd {
      padding: 6px 10px 5px;
      gap: 4px;
    }
    .stats {
      height: 36px;
    }
    .big {
      font-size: 20px;
    }
    .ovwrap {
      border-width: 0 6px 5px;
    }
  }
</style>
