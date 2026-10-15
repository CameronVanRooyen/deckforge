<script lang="ts">
  import { engine, cmd } from '$lib/state/engine.svelte';
  import { RELEASE_FX_BEATS, RELEASE_FX_KINDS, type ReleaseFxKind } from '$lib/api';
  import { RELEASE_FX_LABELS, beatLabel } from '$lib/fx/labels';
  import AudioWaveform from '@lucide/svelte/icons/audio-waveform';
  import RotateCcw from '@lucide/svelte/icons/rotate-ccw';
  import Disc3 from '@lucide/svelte/icons/disc-3';
  import ChevronDown from '@lucide/svelte/icons/chevron-down';

  const ICONS = { echoOut: AudioWaveform, backspin: RotateCcw, vinylBrake: Disc3, rollOut: AudioWaveform } as const;
  const HINTS: Record<ReleaseFxKind, string> = {
    echoOut: 'Cuts the dry signal and lets the echo ring out',
    backspin: 'Spins the track backwards to a stop',
    vinylBrake: 'Slows the platter to a halt',
    rollOut: 'Rolls the last beat while it fades',
  };

  const rel = $derived(engine.snap.releaseFx);
  const running = $derived(rel.running !== null);
  const Icon = $derived(ICONS[rel.running ?? rel.kind]);
  let open = $state(false);
  let root: HTMLElement | undefined;

  const beatsText = (b: number) => `${beatLabel(b)} beat${b <= 1 ? '' : 's'}`;

  function configure(kind: ReleaseFxKind, beats: number) {
    cmd({ type: 'releaseFxConfig', kind, beats });
  }

  function away(e: PointerEvent) {
    if (open && root && !root.contains(e.target as Node)) open = false;
  }

  function keydown(e: KeyboardEvent) {
    if (open && e.key === 'Escape') {
      e.stopPropagation();
      open = false;
    }
  }
</script>

<svelte:window onpointerdown={away} onkeydown={keydown} />

<div class="rel" class:running bind:this={root} style="--p:{running ? rel.progress : 0}">
  <button class="cfg" class:open onclick={() => (open = !open)} aria-expanded={open} aria-label="Release FX: {RELEASE_FX_LABELS[rel.kind]}, {beatsText(rel.beats)}" title="Release FX type and length">
    <span class="kind">{RELEASE_FX_LABELS[rel.running ?? rel.kind]}</span>
    <span class="len num">{running ? `${Math.round(rel.progress * 100)}%` : beatsText(rel.beats)}</span>
    <ChevronDown size={11} strokeWidth={2.6} class="chev" />
  </button>
  <button class="fire" class:hot={running} onclick={() => cmd({ type: 'releaseFx' })} aria-label="Fire {RELEASE_FX_LABELS[rel.kind]}" title="Fire {RELEASE_FX_LABELS[rel.kind]} and switch Beat FX off (SHIFT + FX ON/OFF)">
    <Icon size={15} strokeWidth={2.3} />
  </button>

  {#if open}
    <div class="menu" role="dialog" aria-label="Release FX">
      <span class="cap">Release FX</span>
      <div class="kinds" role="radiogroup" aria-label="Release FX type">
        {#each RELEASE_FX_KINDS as k (k)}
          {@const KIcon = ICONS[k]}
          <button role="radio" aria-checked={rel.kind === k} class:active={rel.kind === k} onclick={() => configure(k, rel.beats)} title={HINTS[k]}>
            <KIcon size={16} strokeWidth={2.1} />
            <span>{RELEASE_FX_LABELS[k]}</span>
          </button>
        {/each}
      </div>
      <span class="cap">Length in beats</span>
      <div class="seg" role="radiogroup" aria-label="Release FX length">
        {#each RELEASE_FX_BEATS as b (b)}
          <button role="radio" aria-checked={rel.beats === b} class:active={rel.beats === b} onclick={() => configure(rel.kind, b)}>{beatLabel(b)}</button>
        {/each}
      </div>
      <p class="hint">{HINTS[rel.kind]}. Fires on the Beat FX target; SHIFT + FX ON/OFF does the same on the DDJ-400.</p>
    </div>
  {/if}
</div>

<style>
  .rel {
    position: relative;
    display: flex;
    height: 32px;
    border-radius: var(--radius);
    border: 1px solid color-mix(in srgb, var(--shot) 30%, var(--line));
    background: var(--panel);
    isolation: isolate;
    z-index: 3;
  }
  .rel::before {
    content: '';
    position: absolute;
    inset: 0;
    z-index: -1;
    border-radius: inherit;
    background: linear-gradient(90deg, color-mix(in srgb, var(--shot) 18%, transparent), color-mix(in srgb, var(--shot) 55%, transparent));
    clip-path: inset(0 calc((1 - var(--p)) * 100%) 0 0);
  }
  .rel.running {
    border-color: var(--shot);
    box-shadow: 0 0 18px -3px var(--shot);
    animation: rel-in 0.45s var(--ease);
  }
  @keyframes rel-in {
    from {
      box-shadow: 0 0 0 4px color-mix(in srgb, var(--shot) 70%, transparent);
    }
  }
  .cfg {
    position: relative;
    display: grid;
    grid-template-columns: auto 10px;
    align-items: center;
    column-gap: 3px;
    padding: 0 6px 0 8px;
    border: 0;
    background: transparent;
    text-align: left;
    border-radius: var(--radius) 0 0 var(--radius);
    transition: background var(--dur);
  }
  .cfg:hover,
  .cfg.open {
    background: rgba(255, 255, 255, 0.05);
  }
  .kind {
    grid-column: 1;
    font-size: 10.5px;
    font-weight: 650;
    line-height: 1.1;
    color: var(--text);
    white-space: nowrap;
  }
  .len {
    grid-column: 1;
    font-size: 9.5px;
    line-height: 1.1;
    color: color-mix(in srgb, var(--shot) 60%, var(--dim));
  }
  .cfg :global(.chev) {
    grid-column: 2;
    grid-row: 1 / span 2;
    color: var(--faint);
  }
  .running .kind {
    color: #fff;
  }
  .running .len {
    color: #fff;
  }
  .fire {
    width: 32px;
    display: grid;
    place-items: center;
    border: 0;
    border-left: 1px solid color-mix(in srgb, var(--shot) 30%, var(--line));
    border-radius: 0 var(--radius) var(--radius) 0;
    background: transparent;
    color: var(--shot);
    transition: background var(--dur), color var(--dur);
  }
  .fire:hover {
    background: color-mix(in srgb, var(--shot) 22%, transparent);
    color: #fff;
  }
  .fire:active {
    background: var(--shot);
    color: #2a0530;
  }
  .fire.hot {
    color: #fff;
  }
  .fire.hot :global(svg) {
    animation: spin 0.8s linear infinite reverse;
  }
  @media (max-width: 1440px) {
    .kind {
      display: none;
    }
    .len {
      grid-row: 1 / span 2;
      font-size: 10.5px;
      font-weight: 600;
    }
  }
  .menu {
    position: absolute;
    top: calc(100% + 10px);
    right: 0;
    z-index: 45;
    width: 262px;
    display: grid;
    gap: 7px;
    padding: 12px;
    border-radius: var(--radius-lg);
    border: 1px solid color-mix(in srgb, var(--shot) 40%, var(--line-2));
    background: var(--raised);
    box-shadow: 0 18px 40px -12px rgba(0, 0, 0, 0.7);
    animation: pop-in 0.12s var(--ease);
  }
  .cap {
    font-size: 10.5px;
    font-weight: 600;
    color: var(--faint);
  }
  .kinds {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 5px;
  }
  .kinds button {
    height: 52px;
    display: grid;
    place-items: center;
    align-content: center;
    gap: 4px;
    border-radius: var(--radius);
    border: 1px solid var(--line);
    background: var(--panel);
    color: var(--dim);
    font-size: 11px;
    font-weight: 600;
    transition: background var(--dur), color var(--dur), border-color var(--dur);
  }
  .kinds button:hover {
    color: var(--fg);
    border-color: var(--line-2);
  }
  .kinds button.active {
    color: #fff;
    border-color: var(--shot);
    background: color-mix(in srgb, var(--shot) 22%, var(--panel));
  }
  .seg {
    display: grid;
    grid-template-columns: repeat(5, 1fr);
    gap: 2px;
    height: 28px;
    padding: 2px;
    border-radius: var(--radius);
    border: 1px solid var(--line);
    background: var(--panel);
  }
  .seg button {
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--dim);
    font-family: var(--font-mono);
    font-size: 11.5px;
    font-weight: 600;
  }
  .seg button:hover {
    color: var(--fg);
    background: rgba(255, 255, 255, 0.06);
  }
  .seg button.active {
    color: #fff;
    background: color-mix(in srgb, var(--shot) 35%, var(--panel));
  }
  .hint {
    margin: 2px 0 0;
    font-size: 11px;
    line-height: 1.4;
    color: var(--dim);
  }
  button:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 1px;
  }
</style>
