<script lang="ts">
  import { toast } from '$lib/state/toast.svelte';
  import X from '@lucide/svelte/icons/x';
</script>

<div class="toasts" aria-live="polite">
  {#each toast.items as t (t.id)}
    <div class="t" data-kind={t.kind} role="status">
      <i></i><span>{t.text}</span>
      <button onclick={() => toast.dismiss(t.id)} aria-label="Dismiss"><X size={13} /></button>
    </div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    left: 50%;
    bottom: 18px;
    transform: translateX(-50%);
    z-index: 120;
    display: flex;
    flex-direction: column;
    gap: 8px;
    align-items: center;
    pointer-events: none;
  }
  .t {
    pointer-events: auto;
    display: flex;
    align-items: center;
    gap: 10px;
    max-width: min(640px, 92vw);
    padding: 9px 10px 9px 12px;
    border-radius: 10px;
    background: #141416;
    border: 1px solid var(--line-2);
    box-shadow: 0 16px 40px rgba(0, 0, 0, 0.55);
    font-size: 12.5px;
    animation: pop-in 140ms var(--ease);
  }
  i {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--dim);
    flex: none;
  }
  [data-kind='ok'] i {
    background: var(--ok);
    box-shadow: 0 0 8px var(--ok);
  }
  [data-kind='error'] i {
    background: var(--danger);
    box-shadow: 0 0 8px var(--danger);
  }
  span {
    overflow-wrap: anywhere;
  }
  button {
    border: 0;
    background: transparent;
    color: var(--faint);
    display: grid;
    place-items: center;
  }
</style>
