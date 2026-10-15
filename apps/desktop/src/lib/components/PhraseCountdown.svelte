<script lang="ts">
  import { engine } from '$lib/state/engine.svelte';
  import { deckGrid, deckPhrases } from '$lib/waveform/beats';
  import { barSecs, barsBeats, countdown, finalBar, phraseBars } from '$lib/waveform/phraseNav';
  import { PHRASE_STYLE } from '$lib/waveform/phrases';
  import type { DeckIndex } from '$lib/api';

  let { deck, size = 'md' }: { deck: DeckIndex; size?: 'md' | 'sm' } = $props();

  const d = $derived(engine.snap.decks[deck]);
  const view = $derived.by(() => {
    if (!d.loaded || !d.hasGrid || d.gridBpm <= 0) return null;
    const phrases = deckPhrases(deck, d.trackToken);
    if (!phrases.length) return null;
    const grid = deckGrid(d);
    const c = countdown(phrases, grid, d.positionSecs);
    const cur = c ? phrases[c.current] : phrases.find((p) => d.positionSecs >= p.startSecs && d.positionSecs < p.endSecs);
    const len = cur ? phraseBars(cur, grid).bars : 0;
    const bar = cur ? Math.min(len, Math.floor((d.positionSecs - cur.startSecs) / barSecs(grid) + 0.05) + 1) : 0;
    if (!c) {
      return cur ? { next: null, current: PHRASE_STYLE[cur.kind], count: '', final: false, left: 0, progress: (d.positionSecs - cur.startSecs) / (cur.endSecs - cur.startSecs), bar, len } : null;
    }
    return {
      next: PHRASE_STYLE[phrases[c.next].kind],
      current: cur ? PHRASE_STYLE[cur.kind] : null,
      count: barsBeats(c),
      final: finalBar(c),
      left: Math.min(4, c.remainingBeats),
      progress: c.progress,
      bar,
      len,
    };
  });
</script>

{#if view}
  <div
    class="phr {size}"
    class:final={view.final}
    style="--next:{view.next?.color ?? 'var(--faint)'};--now:{view.current?.color ?? 'var(--faint)'}"
    title={view.next ? `${view.next.label} starts in ${view.count} (bars.beats)` : 'Last phrase'}
  >
    <div class="line">
      {#if view.next}
        <b class="name">{view.next.name}</b><span class="in">in</span><span class="count num">{view.count}</span>
      {:else}
        <b class="name now">{view.current?.name}</b><span class="in">last phrase</span>
      {/if}
    </div>
    <div class="meta">
      {#if view.final}
        <span class="pips" aria-label={`${view.left} beats left`}>
          {#each [4, 3, 2, 1] as b (b)}<i class:on={b <= view.left}></i>{/each}
        </span>
      {:else}
        <span class="track"><i style="width:{Math.round(view.progress * 100)}%"></i></span>
      {/if}
      {#if view.current}<span class="pos num"><b>{view.bar}</b>/{view.len}</span>{/if}
    </div>
  </div>
{:else}
  <div class="phr {size} empty"><div class="line"><span class="in">No phrases</span></div><div class="meta"><span class="track"></span></div></div>
{/if}

<style>
  .phr {
    flex: none;
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 5px;
    width: 116px;
    padding: 4px 8px;
    border-radius: var(--radius);
    border: 1px solid transparent;
    transition:
      background var(--dur),
      border-color var(--dur);
  }
  .phr.final {
    background: color-mix(in srgb, var(--next) 14%, transparent);
    border-color: color-mix(in srgb, var(--next) 45%, transparent);
  }
  .line {
    display: flex;
    align-items: baseline;
    gap: 5px;
    white-space: nowrap;
    line-height: 1;
  }
  .name {
    font-size: 12px;
    font-weight: 680;
    letter-spacing: 0.06em;
    color: var(--next);
  }
  .name.now {
    color: var(--now);
  }
  .in {
    font-size: 11px;
    font-weight: 520;
    color: var(--faint);
  }
  .count {
    margin-left: auto;
    font-size: 17px;
    font-weight: 620;
    color: var(--fg);
    letter-spacing: -0.01em;
  }
  .final .count {
    color: var(--next);
  }
  .meta {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 10px;
  }
  .track {
    flex: 1;
    height: 3px;
    border-radius: 2px;
    background: var(--raised-3);
    overflow: hidden;
  }
  .track i {
    display: block;
    height: 100%;
    background: var(--now);
    transition: width 120ms linear;
  }
  .pips {
    flex: 1;
    display: flex;
    gap: 3px;
  }
  .pips i {
    flex: 1;
    height: 5px;
    border-radius: 2px;
    background: var(--raised-3);
  }
  .pips i.on {
    background: var(--next);
  }
  .pos {
    font-size: 10.5px;
    font-weight: 560;
    color: var(--faint);
  }
  .pos b {
    font-weight: 650;
    color: var(--text);
  }
  .empty .in {
    color: var(--faint);
  }
  .sm {
    width: auto;
    min-width: 112px;
    padding: 0 8px;
    gap: 0;
    flex-direction: row;
    align-items: center;
    border-radius: 5px;
    height: 20px;
  }
  .sm .line {
    gap: 4px;
  }
  .sm .name {
    font-size: 10.5px;
  }
  .sm .count {
    font-size: 12.5px;
    margin-left: 2px;
  }
  .sm .meta {
    display: none;
  }
</style>
