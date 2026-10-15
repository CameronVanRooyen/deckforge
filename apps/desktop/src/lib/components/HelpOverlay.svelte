<script lang="ts">
  import { ui } from '$lib/state/ui.svelte';
  import { SHORTCUT_DOC } from '$lib/shortcuts';
</script>

{#if ui.helpOpen}
  <div class="scrim" onpointerdown={() => (ui.helpOpen = false)} role="presentation">
    <div class="card" role="dialog" aria-modal="true" aria-label="Keyboard shortcuts" onpointerdown={(e) => e.stopPropagation()}>
      <header><h2>Keyboard shortcuts</h2><span>Shortcuts are ignored while typing in a field</span></header>
      <div class="grid">
        {#each SHORTCUT_DOC as g (g.title)}
          <section>
            <h3 style={g.title === 'Deck A' ? 'color:var(--deck-a)' : g.title === 'Deck B' ? 'color:var(--deck-b)' : ''}>{g.title}</h3>
            {#each g.items as it (it.label)}
              <div class="row">
                <span class="keys">{#each it.keys as k, i (i)}{#if k === '–'}<em>–</em>{:else}<kbd>{k}</kbd>{/if}{/each}</span>
                <span class="lbl">{it.label}</span>
              </div>
            {/each}
          </section>
        {/each}
      </div>
    </div>
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 90;
    display: grid;
    place-items: center;
    background: rgba(0, 0, 0, 0.65);
        animation: fade-in 120ms var(--ease);
  }
  .card {
    width: min(820px, 94vw);
    padding: 22px 26px 26px;
    border-radius: 16px;
    background: #141416;
    border: 1px solid var(--line-2);
    box-shadow: 0 30px 90px rgba(0, 0, 0, 0.6);
    animation: pop-in 140ms var(--ease);
  }
  header {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    margin-bottom: 16px;
  }
  h2 {
    margin: 0;
    font-size: 18px;
    letter-spacing: -0.02em;
  }
  header span {
    font-size: 11.5px;
    color: var(--faint);
  }
  .grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 20px 36px;
  }
  h3 {
    margin: 0 0 8px;
    font-size: 12.5px;
    font-weight: 750;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 5px 0;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  }
  .keys {
    width: 124px;
    flex: none;
    display: flex;
    gap: 3px;
    align-items: center;
  }
  kbd {
    min-width: 22px;
    height: 22px;
    padding: 0 6px;
    display: inline-grid;
    place-items: center;
    border-radius: 5px;
    border: 1px solid var(--line-2);
    border-bottom-width: 2px;
    background: var(--raised);
    font: 650 11.5px var(--font-sans);
  }
  em {
    color: var(--faint);
    font-style: normal;
  }
  .lbl {
    font-size: 12.5px;
    color: var(--dim);
  }
</style>
