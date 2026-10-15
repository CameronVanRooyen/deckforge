<script lang="ts">
  import { library } from '$lib/state/library.svelte';
  const c = $derived(library.confirm);
</script>

{#if c}
  <div class="scrim" onpointerdown={() => (library.confirm = null)} role="presentation">
    <div class="card" data-deck={c.deck} role="alertdialog" tabindex="-1" aria-modal="true" aria-label={c.kind === 'eject' ? 'Eject playing track' : 'Replace playing track'} onpointerdown={(e) => e.stopPropagation()}>
      <h2>{c.kind === 'eject' ? `Eject Deck ${c.deck === 0 ? 'A' : 'B'} while playing?` : `Deck ${c.deck === 0 ? 'A' : 'B'} is playing — replace?`}</h2>
      {#if c.kind === 'eject'}
        <p><b>{c.title}</b> is playing on Deck {c.deck === 0 ? 'A' : 'B'}. Ejecting stops it immediately.</p>
      {:else}
        <p>Loading <b>{c.title}</b> will stop the track that's playing on Deck {c.deck === 0 ? 'A' : 'B'}.</p>
      {/if}
      <div class="actions">
        <button class="btn" onclick={() => (library.confirm = null)}>Keep playing</button>
        <button class="btn btn-primary" onclick={() => library.confirmPending()}>{c.kind === 'eject' ? 'Eject track' : 'Replace track'}</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 95;
    display: grid;
    place-items: center;
    background: rgba(0, 0, 0, 0.6);
    animation: fade-in 100ms var(--ease);
  }
  .card {
    position: relative;
    width: min(420px, 92vw);
    padding: 18px 20px 16px;
    border-radius: 10px;
    background: #141416;
    border: 1px solid var(--line-2);
    border-top: 2px solid var(--deck);
    box-shadow: 0 24px 60px rgba(0, 0, 0, 0.6);
    animation: pop-in 120ms var(--ease);
  }
  h2 {
    margin: 0 0 8px;
    font-size: 15px;
    font-weight: 620;
    letter-spacing: -0.01em;
    color: var(--fg);
  }
  p {
    margin: 0 0 18px;
    color: var(--dim);
    font-size: 13px;
    line-height: 1.5;
  }
  b {
    color: var(--fg);
    font-weight: 580;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  .actions .btn {
    height: 32px;
  }
</style>
