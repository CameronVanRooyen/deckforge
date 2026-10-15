<script lang="ts">
  import { ui, ZOOM_LEVELS } from '$lib/state/ui.svelte';
  import ZoomIn from '@lucide/svelte/icons/zoom-in';
  import ZoomOut from '@lucide/svelte/icons/zoom-out';
</script>

<div class="zoom" role="group" aria-label="Waveform zoom">
  <button onclick={() => ui.zoom(-1)} disabled={ui.zoomIndex === 0} aria-label="Zoom out (−)" title="Zoom out (−)"><ZoomOut size={14} strokeWidth={2.2} /></button>
  <div class="dots" aria-hidden="true">{#each ZOOM_LEVELS as _, i (i)}<i class:on={i <= ui.zoomIndex}></i>{/each}</div>
  <button onclick={() => ui.zoom(1)} disabled={ui.zoomIndex === ZOOM_LEVELS.length - 1} aria-label="Zoom in (+)" title="Zoom in (+)"><ZoomIn size={14} strokeWidth={2.2} /></button>
</div>

<style>
  .zoom {
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 2px;
    border-radius: var(--radius-lg);
    background: #141416;
    border: 1px solid var(--line-2);
    opacity: 0.7;
    transition: opacity var(--dur);
  }
  .zoom:hover,
  .zoom:focus-within {
    opacity: 1;
  }
  button {
    width: 24px;
    height: 24px;
    border-radius: 5px;
    border: 0;
    background: transparent;
    color: var(--dim);
    display: grid;
    place-items: center;
    transition: background var(--dur), color var(--dur);
  }
  button:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.08);
    color: var(--fg);
  }
  button:disabled {
    opacity: 0.3;
  }
  .dots {
    display: flex;
    gap: 3px;
    padding: 0 3px;
  }
  .dots i {
    width: 4px;
    height: 12px;
    border-radius: 2px;
    background: var(--raised-3);
  }
  .dots i.on {
    background: var(--dim);
  }
</style>
