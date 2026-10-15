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
  <section class="height-setting">
    <div class="height-heading">
      <h3>Waveform height</h3>
      <output for="waveform-height">{ui.waveformShare}%</output>
    </div>
    <p>Smaller waveforms give more vertical room to your library.</p>
    <input
      id="waveform-height"
      type="range"
      min="50"
      max="100"
      step="5"
      value={ui.waveformShare}
      aria-label="Waveform height"
      oninput={(event) => ui.setWaveformShare(Number(event.currentTarget.value))}
    />
    <div class="range-labels" aria-hidden="true"><span>Compact</span><span>Full</span></div>
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
  .height-setting {
    display: grid;
    gap: 8px;
    margin-top: 24px;
  }
  .height-heading {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 16px;
  }
  .height-heading output {
    color: var(--text);
    font: 11.5px var(--font-mono);
    font-variant-numeric: tabular-nums;
  }
  p {
    margin: -3px 0 0;
    color: var(--faint);
    font-size: 11.5px;
  }
  input[type='range'] {
    width: 100%;
    accent-color: var(--accent);
  }
  .range-labels {
    display: flex;
    justify-content: space-between;
    color: var(--faint);
    font-size: 10.5px;
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
