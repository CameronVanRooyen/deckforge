<script lang="ts">
  import { ui, type DeckView } from '$lib/state/ui.svelte';

  const choices: { id: DeckView; label: string; description: string }[] = [
    { id: 'detailed', label: 'Detailed', description: 'Show the full beat grid and bar counts.' },
    { id: 'beatmatch', label: 'Beatmatch', description: 'Emphasize each bar’s first beat and keep phrase sections visible.' },
  ];
</script>

<div class="wrap">
  <section>
    <h3>Deck view</h3>
    <div class="choices" role="group" aria-label="Deck view">
      {#each choices as choice (choice.id)}
        <button
          class="choice"
          class:selected={ui.deckView === choice.id}
          aria-pressed={ui.deckView === choice.id}
          onclick={() => ui.setDeckView(choice.id)}
        >
          <span class="name">{choice.label}</span>
          <span class="description">{choice.description}</span>
        </button>
      {/each}
    </div>
  </section>
</div>

<style>
  .wrap {
    padding: 20px;
  }
  section {
    display: grid;
    gap: 12px;
  }
  h3 {
    margin: 0;
    color: var(--fg);
    font-size: 13px;
    font-weight: 650;
  }
  .choices {
    display: grid;
    gap: 8px;
  }
  .choice {
    display: grid;
    gap: 4px;
    padding: 12px;
    text-align: left;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--raised);
    color: var(--dim);
  }
  .choice:hover {
    border-color: var(--line-2);
    color: var(--text);
  }
  .choice.selected {
    border-color: var(--accent);
    background: var(--raised-2);
    color: var(--fg);
  }
  .name {
    font-size: 12.5px;
    font-weight: 650;
  }
  .description {
    font-size: 11.5px;
    color: var(--faint);
  }
</style>
