<script lang="ts">
  import { engine, cmd } from '$lib/state/engine.svelte';
  import {
    BEAT_FX_BEATS,
    BEAT_FX_KIND_BEATS,
    BEAT_FX_FREEZE_KINDS,
    BEAT_FX_PARAMS,
    BEAT_FX_PERCENT_KINDS,
    BEAT_FX_PERCENT_STEPS,
    BEAT_FX_PRIMARY_PARAM,
    BEAT_FX_RELEASE_KINDS,
    BEAT_FX_THROW_KINDS,
    type BeatFxKind,
    type FxTarget,
  } from '$lib/api';
  import BeatFxPanel from './BeatFxPanel.svelte';
  import FxParamStrip from './FxParamStrip.svelte';
  import ReleaseFx from './ReleaseFx.svelte';
  import Knob from './Knob.svelte';
  import SlidersHorizontal from '@lucide/svelte/icons/sliders-horizontal';
  import Select from './Select.svelte';
  import ChevronLeft from '@lucide/svelte/icons/chevron-left';
  import ChevronRight from '@lucide/svelte/icons/chevron-right';
  import Sparkles from '@lucide/svelte/icons/sparkles';

  const KINDS: { value: BeatFxKind; label: string }[] = [
    { value: 'delay', label: 'Delay' },
    { value: 'echo', label: 'Echo' },
    { value: 'upEcho', label: 'Up Echo' },
    { value: 'downEcho', label: 'Down Echo' },
    { value: 'pingPong', label: 'Ping Pong' },
    { value: 'spiral', label: 'Spiral' },
    { value: 'reverb', label: 'Reverb' },
    { value: 'flanger', label: 'Flanger' },
    { value: 'phaser', label: 'Phaser' },
    { value: 'roll', label: 'Roll' },
    { value: 'trans', label: 'Trans' },
    { value: 'filterSweep', label: 'Filter Sweep' },
    { value: 'pulseVerb', label: 'Pulse Verb' },
    { value: 'dubTape', label: 'Dub Tape' },
    { value: 'halo', label: 'Halo' },
    { value: 'ascend', label: 'Ascend' },
    { value: 'backwash', label: 'Backwash' },
  ];
  const TARGETS: { id: FxTarget; label: string }[] = [
    { id: 'a', label: 'A' },
    { id: 'b', label: 'B' },
    { id: 'master', label: 'Master' },
  ];
  const BANDS = [
    { key: 'low', label: 'LOW' },
    { key: 'mid', label: 'MID' },
    { key: 'high', label: 'HI' },
  ] as const;
  const BEAT_LABELS: Record<number, string> = { 0.125: '1/8', 0.25: '1/4', 0.5: '1/2', 0.75: '3/4' };
  const beatLabel = (b: number) => BEAT_LABELS[b] ?? String(b);

  const fx = $derived(engine.snap.beatFx);
  const releasing = $derived(engine.snap.releaseFx.running !== null);
  const options = $derived<readonly number[]>(BEAT_FX_KIND_BEATS[fx.kind] ?? BEAT_FX_BEATS);
  const beatIndex = $derived(
    options.reduce((best, b, i) => (Math.abs(Math.log(b / fx.beats)) < Math.abs(Math.log(options[best] / fx.beats)) ? i : best), 0),
  );
  const primaryParam = $derived(BEAT_FX_PRIMARY_PARAM[fx.kind] ?? null);
  const paramSpec = $derived(primaryParam === null ? null : BEAT_FX_PARAMS[fx.kind][primaryParam]);
  const paramIndex = $derived(primaryParam === null ? 0 : Math.round(fx.params[primaryParam] ?? paramSpec?.default ?? 0));
  const paramLabel = $derived(paramSpec ? `${paramSpec.options?.[paramIndex] ?? ''} bars` : '');
  const stepCount = $derived(primaryParam !== null ? (paramSpec?.options?.length ?? 1) : options.length);
  const stepIndex = $derived(primaryParam !== null ? paramIndex : beatIndex);
  const tunable = $derived(
    BEAT_FX_PARAMS[fx.kind].length > 0 ||
      [BEAT_FX_THROW_KINDS, BEAT_FX_FREEZE_KINDS, BEAT_FX_RELEASE_KINDS].some((kinds) => (kinds as readonly string[]).includes(fx.kind)),
  );
  const kindLabel = $derived(KINDS.find((k) => k.value === fx.kind)?.label ?? '');
  let panelOpen = $state(false);
  $effect(() => {
    if (!tunable) panelOpen = false;
  });
  let root: HTMLElement | undefined;

  function outside(e: PointerEvent) {
    if (panelOpen && root && !root.contains(e.target as Node)) panelOpen = false;
  }

  const usesPercent = $derived((BEAT_FX_PERCENT_KINDS as readonly string[]).includes(fx.kind));
  const percentLabel = $derived(`${Math.round(fx.percent)}%`);

  function toggleBand(key: (typeof BANDS)[number]['key']) {
    const next = { ...fx.bands, [key]: !fx.bands[key] };
    if (!next.low && !next.mid && !next.high) return;
    cmd({ type: 'beatFxBands', ...next });
  }

  function step(dir: 1 | -1, fine = false) {
    if (usesPercent) return stepPercent(dir, fine);
    const i = Math.min(stepCount - 1, Math.max(0, stepIndex + dir));
    if (primaryParam !== null) cmd({ type: 'beatFxParam', index: primaryParam, value: i });
    else cmd({ type: 'beatFxBeats', beats: options[i] });
  }

  function setPercent(value: number) {
    cmd({ type: 'beatFxPercent', value: Math.min(100, Math.max(1, Math.round(value))) });
  }

  function stepPercent(dir: 1 | -1, fine: boolean) {
    const cur = fx.percent;
    if (fine) return setPercent(cur + dir);
    const steps = BEAT_FX_PERCENT_STEPS as readonly number[];
    setPercent(dir > 0 ? (steps.find((s) => s > cur + 1e-9) ?? 100) : ([...steps].reverse().find((s) => s < cur - 1e-9) ?? 1));
  }

  const PX_PER_PERCENT = 3;
  let drag: { x: number; base: number } | null = null;

  function dragStart(e: PointerEvent) {
    drag = { x: e.clientX, base: fx.percent };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }

  function dragMove(e: PointerEvent) {
    if (drag) setPercent(drag.base + (e.clientX - drag.x) / PX_PER_PERCENT);
  }

  function dragEnd() {
    drag = null;
  }

  function percentKey(e: KeyboardEvent) {
    const dir = e.key === 'ArrowUp' || e.key === 'ArrowRight' ? 1 : e.key === 'ArrowDown' || e.key === 'ArrowLeft' ? -1 : 0;
    if (!dir) return;
    e.preventDefault();
    stepPercent(dir, e.shiftKey);
  }
</script>

<svelte:window onpointerdown={outside} />

<div class="fx" class:on={fx.enabled} class:releasing role="group" aria-label="Beat FX" bind:this={root}>
  <Sparkles size={14} strokeWidth={2} class="mark" />
  <Select
    value={fx.kind}
    options={KINDS}
    label="Beat FX type"
    width={104}
    accent="var(--fx)"
    onchange={(kind) => cmd({ type: 'beatFxKind', kind })}
  />

  <div class="beats" role="group" aria-label={usesPercent ? 'Effect amount' : 'Beat division'}>
    <button
      onclick={(e) => step(-1, e.shiftKey)}
      disabled={usesPercent ? fx.percent <= 1 : stepIndex === 0}
      aria-label={usesPercent ? 'Less effect' : primaryParam !== null ? 'Shorter build' : 'Shorter beat division'}
    ><ChevronLeft size={14} strokeWidth={2.4} /></button>
    {#if usesPercent}
      <button
        class="num bv drag"
        title="Effect amount: drag or Shift+click the arrows for 1% steps"
        aria-label="Effect amount in percent"
        onpointerdown={dragStart}
        onpointermove={dragMove}
        onpointerup={dragEnd}
        onpointercancel={dragEnd}
        onkeydown={percentKey}
      >{percentLabel}</button>
    {:else if primaryParam !== null}
      <span class="num bv" title="Build length in bars">{paramLabel}</span>
    {:else}
      <span class="num bv" title="Effect length in beats">{beatLabel(options[beatIndex])}</span>
    {/if}
    <button
      onclick={(e) => step(1, e.shiftKey)}
      disabled={usesPercent ? fx.percent >= 100 : stepIndex === stepCount - 1}
      aria-label={usesPercent ? 'More effect' : primaryParam !== null ? 'Longer build' : 'Longer beat division'}
    ><ChevronRight size={14} strokeWidth={2.4} /></button>
  </div>

  <div class="level">
    <Knob
      value={fx.level}
      size={32}
      reset={0.5}
      label="Beat FX level / depth"
      format={(v) => `${Math.round(v * 100)}%`}
      onchange={(v) => cmd({ type: 'beatFxLevel', value: v })}
    />
    <FxParamStrip />
  </div>

  {#if tunable}
    <button class="tune" class:open={panelOpen} class:held={fx.held} onclick={() => (panelOpen = !panelOpen)} aria-expanded={panelOpen} aria-label="{kindLabel} controls" title="{kindLabel} controls">
      <SlidersHorizontal size={14} strokeWidth={2.2} />
    </button>
  {/if}

  <div class="seg bands" role="group" aria-label="Beat FX frequency bands" title="FX frequency: bands the effect acts on; the rest stay dry">
    {#each BANDS as b (b.key)}
      <button class:active={fx.bands[b.key]} aria-pressed={fx.bands[b.key]} data-band={b.key} onclick={() => toggleBand(b.key)}>{b.label}</button>
    {/each}
  </div>

  <div class="seg" role="group" aria-label="Beat FX target" title="Beat FX target, synced to {fx.bpm.toFixed(1)} BPM">
    {#each TARGETS as t (t.id)}
      <button class:active={fx.target === t.id} data-target={t.id} onclick={() => cmd({ type: 'beatFxTarget', target: t.id })}>{t.label}</button>
    {/each}
  </div>

  <div class="bpm" title="Tempo the effect is synced to">
    <span class="num">{fx.bpm.toFixed(1)}</span><small>BPM</small>
  </div>

  <ReleaseFx />

  <button class="onbtn" class:lit={fx.enabled} onclick={() => cmd({ type: 'beatFxEnabled', enabled: !fx.enabled })} aria-pressed={fx.enabled} title="Beat FX on / off (X)">ON</button>

  {#if panelOpen && tunable}
    <BeatFxPanel label={kindLabel} onclose={() => (panelOpen = false)} />
  {/if}

  {#if releasing}<i class="relbar" style="width:{engine.snap.releaseFx.progress * 100}%"></i>{/if}
</div>

<style>
  .fx {
    position: relative;
    --knob: var(--fx);
    display: flex;
    align-items: center;
    gap: 10px;
    height: 44px;
    padding: 0 8px 0 10px;
    border-radius: var(--radius-lg);
    border: 1px solid var(--line);
    background: var(--raised);
    transition: border-color var(--dur);
  }
  .fx.on {
    border-color: color-mix(in srgb, var(--fx) 45%, var(--line));
  }
  .fx.releasing {
    border-color: color-mix(in srgb, var(--shot) 60%, var(--line));
  }
  .relbar {
    position: absolute;
    left: 6px;
    bottom: -1px;
    max-width: calc(100% - 12px);
    height: 2px;
    border-radius: 1px;
    background: var(--shot);
    box-shadow: 0 0 10px 1px var(--shot);
    pointer-events: none;
  }
  .fx :global(.mark) {
    color: var(--fx);
    flex: none;
  }
  .beats {
    display: flex;
    align-items: center;
    height: 28px;
    border-radius: var(--radius);
    border: 1px solid var(--line);
    background: var(--panel);
  }
  .beats button {
    width: 22px;
    height: 100%;
    border: 0;
    background: transparent;
    color: var(--dim);
    display: grid;
    place-items: center;
    border-radius: 5px;
    transition: background var(--dur), color var(--dur);
  }
  .beats button:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.07);
    color: var(--fg);
  }
  .beats button:disabled {
    opacity: 0.3;
  }
  .bv {
    min-width: 40px;
    text-align: center;
    font-size: 13px;
    font-weight: 650;
    color: var(--fg);
  }
  .beats button.bv {
    width: auto;
    padding: 0 4px;
    color: var(--fg);
    cursor: ew-resize;
    touch-action: none;
    user-select: none;
  }
  .level {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .seg {
    display: flex;
    height: 28px;
    border-radius: var(--radius);
    border: 1px solid var(--line);
    background: var(--panel);
    padding: 2px;
    gap: 2px;
  }
  .seg button {
    min-width: 26px;
    padding: 0 8px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--dim);
    font-size: 11.5px;
    font-weight: 600;
    transition: background var(--dur), color var(--dur);
  }
  .seg button:hover {
    color: var(--fg);
    background: rgba(255, 255, 255, 0.06);
  }
  .seg button.active {
    background: color-mix(in srgb, var(--fx) 24%, var(--panel));
    color: #ddd6fe;
  }
  .seg.bands button {
    padding: 0 6px;
    font-size: 10.5px;
    letter-spacing: 0.03em;
  }
  .bpm {
    display: flex;
    align-items: baseline;
    gap: 3px;
    min-width: 54px;
    color: var(--dim);
  }
  .bpm .num {
    font-size: 13px;
    font-weight: 600;
    color: var(--text);
  }
  .bpm small {
    font-size: 9.5px;
    font-weight: 600;
    color: var(--faint);
  }
  .tune {
    width: 28px;
    height: 28px;
    flex: none;
    display: grid;
    place-items: center;
    border-radius: var(--radius);
    border: 1px solid var(--line);
    background: var(--panel);
    color: var(--dim);
    transition: background var(--dur), color var(--dur), border-color var(--dur);
  }
  .tune:hover,
  .tune.open {
    color: var(--fg);
    border-color: color-mix(in srgb, var(--fx) 55%, var(--line));
  }
  .tune.open {
    background: color-mix(in srgb, var(--fx) 24%, var(--panel));
  }
  .tune.held {
    color: #fff;
    background: var(--fx);
  }
  .tune:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 1px;
  }
  .onbtn {
    width: 54px;
    height: 32px;
    border-radius: var(--radius);
    border: 1px solid var(--line-2);
    background: var(--raised-2);
    color: var(--dim);
    font-weight: 750;
    font-size: 13px;
    letter-spacing: 0.04em;
    transition:
      background var(--dur),
      color var(--dur),
      border-color var(--dur),
      box-shadow var(--dur);
  }
  .onbtn:hover {
    color: var(--fg);
    border-color: var(--faint);
  }
  .onbtn.lit {
    background: var(--fx);
    border-color: color-mix(in srgb, var(--fx) 70%, white);
    color: #fff;
    box-shadow: 0 0 16px -1px color-mix(in srgb, var(--fx) 85%, transparent);
  }
  @media (max-width: 1680px) {
    .fx .bpm {
      display: none;
    }
  }
  @media (max-width: 1440px) {
    .level :global(.knob) {
      display: none;
    }
    .seg button {
      padding: 0 6px;
    }
    .seg.bands button {
      padding: 0 4px;
    }
    .bpm small {
      display: none;
    }
    .fx {
      gap: 7px;
    }
    .bpm {
      min-width: 36px;
    }
  }
</style>
