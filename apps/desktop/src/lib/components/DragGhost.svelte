<script lang="ts">
  import { drag, dropTargets } from '$lib/state/dnd.svelte';
  import Music from '@lucide/svelte/icons/music';
  const hint = $derived(dropTargets.deck !== null ? `Load to Deck ${dropTargets.deck === 0 ? 'A' : 'B'}` : dropTargets.playlist !== null ? 'Add to playlist' : '');
</script>

{#if drag.active}
  <div class="ghost" style="transform:translate({drag.x + 14}px,{drag.y + 10}px)">
    <Music size={13} /><span>{drag.label}</span>
    {#if hint}<b>{hint}</b>{/if}
  </div>
{/if}

<style>
  .ghost {
    position: fixed;
    left: 0;
    top: 0;
    z-index: 200;
    pointer-events: none;
    display: flex;
    align-items: center;
    gap: 7px;
    max-width: 300px;
    padding: 6px 10px;
    border-radius: 8px;
    background: #18181b;
    border: 1px solid var(--line-2);
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.5);
    font-size: 12px;
    font-weight: 600;
  }
  span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  b {
    color: var(--ok);
    font-size: 11px;
    white-space: nowrap;
  }
</style>
