<script lang="ts" module>
  export interface MenuItem {
    label?: string;
    hint?: string;
    onselect?: () => void;
    disabled?: boolean;
    danger?: boolean;
    divider?: boolean;
    heading?: boolean;
  }
</script>

<script lang="ts">
  import { onMount } from 'svelte';

  let { x, y, items, onclose }: { x: number; y: number; items: MenuItem[]; onclose: () => void } = $props();
  let el: HTMLDivElement;
  let pos = $state({ x: 0, y: 0 });

  onMount(() => {
    const r = el.getBoundingClientRect();
    pos = { x: Math.min(x, window.innerWidth - r.width - 8), y: Math.min(y, window.innerHeight - r.height - 8) };
    const down = (e: PointerEvent) => {
      if (!el.contains(e.target as Node)) onclose();
    };
    const key = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopPropagation();
        onclose();
      }
    };
    window.addEventListener('pointerdown', down, true);
    window.addEventListener('keydown', key, true);
    window.addEventListener('blur', onclose);
    return () => {
      window.removeEventListener('pointerdown', down, true);
      window.removeEventListener('keydown', key, true);
      window.removeEventListener('blur', onclose);
    };
  });
</script>

<div bind:this={el} class="menu" role="menu" style="left:{pos.x || x}px;top:{pos.y || y}px">
  {#each items as it, i (i)}
    {#if it.divider}
      <hr />
    {:else if it.heading}
      <div class="head">{it.label}</div>
    {:else}
      <button
        role="menuitem"
        class:danger={it.danger}
        disabled={it.disabled}
        onclick={() => {
          onclose();
          it.onselect?.();
        }}
      >
        <span>{it.label}</span>
        {#if it.hint}<kbd>{it.hint}</kbd>{/if}
      </button>
    {/if}
  {/each}
</div>

<style>
  .menu {
    position: fixed;
    z-index: 100;
    min-width: 200px;
    max-width: 280px;
    padding: 5px;
    border-radius: 10px;
    background: #141416;
        border: 1px solid var(--line-2);
    box-shadow: 0 18px 50px rgba(0, 0, 0, 0.6);
    animation: pop-in 110ms var(--ease);
  }
  button {
    display: flex;
    width: 100%;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    padding: 6px 9px;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: var(--fg);
    font-size: 12.5px;
    text-align: left;
  }
  button:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.08);
  }
  button.danger {
    color: #fca5a5;
  }
  button:disabled {
    opacity: 0.4;
  }
  span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  kbd {
    font: inherit;
    font-size: 10.5px;
    color: var(--faint);
  }
  hr {
    border: 0;
    height: 1px;
    background: var(--line);
    margin: 4px 2px;
  }
  .head {
    padding: 5px 9px 3px;
    font-size: 10px;
    font-weight: 700;
    color: var(--faint);
    letter-spacing: 0.04em;
  }
</style>
