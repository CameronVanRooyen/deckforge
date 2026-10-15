<script lang="ts">
  import { engine } from '$lib/state/engine.svelte';
  import { library } from '$lib/state/library.svelte';
  import { PHRASE_STYLE } from '$lib/waveform/phrases';
  import type { DeckIndex } from '$lib/api';

  let { deck }: { deck: DeckIndex } = $props();
  let width = $state(600);
  const d = $derived(engine.snap.decks[deck]);
  const set = $derived(library.deckPhrases[deck]);
  const phrases = $derived(set && d.loaded && set.token === d.trackToken && d.durationSecs > 0 ? set.phrases : []);
</script>

<div class="band" class:has={phrases.length > 0} bind:clientWidth={width} aria-label="Track phrases">
  {#each phrases as p, i (i)}
    {@const s = PHRASE_STYLE[p.kind]}
    <span
      class="ph"
      title={`${s.label} · ${p.bars} bars`}
      style="left:{(p.startSecs / d.durationSecs) * 100}%;width:{((p.endSecs - p.startSecs) / d.durationSecs) * 100}%;--c:{s.color};--e:{0.28 + 0.5 * p.energy}"
    >{#if ((p.endSecs - p.startSecs) / d.durationSecs) * width > 38}{s.label}{/if}</span>
  {/each}
</div>

<style>
  .band {
    position: relative;
    height: 0;
    overflow: hidden;
    background: var(--panel);
    transition: height var(--dur) var(--ease);
  }
  .band.has {
    height: 15px;
  }
  .ph {
    position: absolute;
    top: 2px;
    bottom: 0;
    padding: 0 0 0 4px;
    overflow: hidden;
    white-space: nowrap;
    border-right: 1px solid var(--panel);
    font-size: 9.5px;
    font-weight: 620;
    line-height: 13px;
    color: rgba(9, 9, 11, 0.88);
    background: color-mix(in srgb, var(--c) calc(var(--e) * 100%), var(--panel));
  }
</style>
