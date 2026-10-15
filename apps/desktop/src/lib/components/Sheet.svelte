<script lang="ts">
  import type { Snippet } from 'svelte';
  import X from '@lucide/svelte/icons/x';

  let { open, title, onclose, children, width = 560 }: { open: boolean; title: string; onclose: () => void; children: Snippet; width?: number } = $props();
</script>

{#if open}
  <div class="scrim" onpointerdown={onclose} role="presentation"></div>
  <aside class="sheet" style="width:min({width}px, 96vw)" role="dialog" aria-label={title} aria-modal="true">
    <header>
      <h2>{title}</h2>
      <button class="btn btn-ghost" style="width:28px;padding:0" onclick={onclose} aria-label="Close"><X size={16} /></button>
    </header>
    <div class="content">{@render children()}</div>
  </aside>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 60;
    background: rgba(0, 0, 0, 0.55);
        animation: fade-in 120ms var(--ease);
  }
  .sheet {
    position: fixed;
    top: 0;
    right: 0;
    bottom: 0;
    z-index: 61;
    display: flex;
    flex-direction: column;
    background: var(--panel);
    border-left: 1px solid var(--line-2);
    box-shadow: -20px 0 60px rgba(0, 0, 0, 0.5);
    animation: sheet-in 160ms var(--ease);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 18px 10px 20px;
    border-bottom: 1px solid var(--line);
  }
  h2 {
    margin: 0;
    font-size: 16px;
    font-weight: 700;
    letter-spacing: -0.01em;
  }
  .content {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }
</style>
