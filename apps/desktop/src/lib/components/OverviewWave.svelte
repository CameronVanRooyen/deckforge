<script lang="ts">
  import { OverviewRenderer } from '$lib/waveform/draw';
  import { absoluteGesture, canvasPreview } from '$lib/waveform/seek';
  import NativeSurface from '$lib/native/NativeSurface.svelte';
  import { native, nativeDragPreview } from '$lib/native/native.svelte';
  import { engine } from '$lib/state/engine.svelte';
  import CanvasLayer from './CanvasLayer.svelte';
  import PhraseBand from './PhraseBand.svelte';
  import type { DeckIndex } from '$lib/api';

  let { deck }: { deck: DeckIndex } = $props();
  // svelte-ignore state_referenced_locally
  const renderer = new OverviewRenderer(deck);
  // svelte-ignore state_referenced_locally
  const gesture = absoluteGesture(deck, (x, rect) => {
    const d = engine.latest.decks[deck];
    return Math.min(1, Math.max(0, (x - rect.left) / rect.width)) * d.durationSecs;
  }, (d, secs) => (native.available ? nativeDragPreview : canvasPreview)(d, secs));
  let hoverX = $state<number | null>(null);
</script>

<div class="ov" data-deck={deck}>
  <div class="box">
    <div class="layer">
      <NativeSurface id={deck === 0 ? 'overview-a' : 'overview-b'}>
        {#snippet fallback()}
          <CanvasLayer label={`Deck ${deck === 0 ? 'A' : 'B'} track overview`} draw={(ctx, W, H, dpr, now) => renderer.drawBase(ctx, W, H, dpr, now)} />
        {/snippet}
      </NativeSurface>
    </div>
    {#if native.ready && !native.available}
      <div class="layer">
        <CanvasLayer label="" draw={(ctx, W, H, dpr, now) => renderer.drawOverlay(ctx, W, H, dpr, now)} />
      </div>
    {/if}
    {#if hoverX !== null}<i class="hover" style="left:{hoverX}px"></i>{/if}
    <div
      class="hit"
      role="presentation"
      onpointerdown={gesture.onpointerdown}
      onpointermove={(e) => {
        hoverX = e.clientX - e.currentTarget.getBoundingClientRect().left;
        gesture.onpointermove(e);
      }}
      onpointerup={gesture.onpointerup}
      onpointercancel={gesture.onpointercancel}
      onpointerleave={() => (hoverX = null)}
    ></div>
  </div>
  <PhraseBand {deck} />
</div>

<style>
  .ov {
    display: flex;
    flex-direction: column;
  }
  .box {
    position: relative;
    height: 44px;
    overflow: hidden;
    background: var(--wave-bg, #0b0b0d);
    border: 1px solid var(--line);
    border-radius: var(--radius);
  }
  @media (max-height: 820px) {
    .box {
      height: 36px;
    }
  }
  .layer {
    position: absolute;
    inset: 0;
  }
  .hit {
    position: absolute;
    inset: 0;
    cursor: pointer;
    touch-action: none;
  }
  .hover {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 1px;
    background: rgba(250, 250, 250, 0.35);
    pointer-events: none;
  }
</style>
