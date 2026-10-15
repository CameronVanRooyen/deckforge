<script lang="ts">
  import { engine, cmd } from '$lib/state/engine.svelte';
  import { fxControl } from '$lib/state/fxcontrol.svelte';
  import { knobParams, paramNorm, paramText, paramValue, pct, shortParam } from '$lib/fx/labels';
  import ChevronUp from '@lucide/svelte/icons/chevron-up';
  import ChevronDown from '@lucide/svelte/icons/chevron-down';

  interface Column {
    key: string;
    index: number | null;
    short: string;
    label: string;
    norm: number;
    text: string;
    set: (norm: number) => void;
  }

  const fx = $derived(engine.snap.beatFx);
  const ctl = $derived(fxControl.state);
  const columns = $derived<Column[]>([
    {
      key: 'level',
      index: null,
      short: 'LVL',
      label: 'Level / depth',
      norm: fx.level,
      text: pct(fx.level),
      set: (n) => cmd({ type: 'beatFxLevel', value: n }),
    },
    ...knobParams(fx.kind).map(({ spec, index }) => {
      const v = paramValue(spec, fx.params, index);
      return {
        key: spec.id,
        index,
        short: shortParam(spec),
        label: spec.label,
        norm: paramNorm(spec, v),
        text: paramText(spec, v),
        set: (n: number) => cmd({ type: 'beatFxParam', index, value: Math.round(spec.min + n * (spec.max - spec.min)) }),
      };
    }),
  ]);
  const driven = $derived(ctl.knobTarget === 'level' ? 'level' : ctl.knobTarget === 'param' ? ctl.focus : undefined);
  const isDriven = (c: Column) => (c.index === null ? driven === 'level' : driven === c.index);
  const pickupOf = (c: Column) => {
    const p = ctl.pickup;
    if (!p) return null;
    if (c.index === null ? p.target === 'level' : p.target === 'param' && p.index === c.index) return p;
    return null;
  };

  let local = $state<{ key: string; norm: number } | null>(null);
  let raf = 0;
  let pending: { col: Column; norm: number } | null = null;

  function push(col: Column, norm: number) {
    const n = Math.max(0, Math.min(1, norm));
    local = { key: col.key, norm: n };
    pending = { col, norm: n };
    if (!raf)
      raf = requestAnimationFrame(() => {
        raf = 0;
        if (pending) pending.col.set(pending.norm);
      });
  }

  function fromPointer(e: PointerEvent, col: Column) {
    const track = (e.currentTarget as HTMLElement).querySelector('.track')!.getBoundingClientRect();
    push(col, 1 - (e.clientY - track.top) / track.height);
  }

  function down(e: PointerEvent, col: Column) {
    if (e.button !== 0) return;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    fromPointer(e, col);
  }

  function move(e: PointerEvent, col: Column) {
    if (local?.key === col.key) fromPointer(e, col);
  }

  function up() {
    local = null;
  }

  function wheel(e: WheelEvent, col: Column) {
    e.preventDefault();
    push(col, col.norm + (e.deltaY < 0 ? 1 : -1) * (e.shiftKey ? 0.01 : 0.04));
    local = null;
  }

  function key(e: KeyboardEvent, col: Column) {
    const dir = e.key === 'ArrowUp' || e.key === 'ArrowRight' ? 1 : e.key === 'ArrowDown' || e.key === 'ArrowLeft' ? -1 : 0;
    if (!dir) return;
    e.preventDefault();
    push(col, col.norm + dir * (e.shiftKey ? 0.01 : 0.05));
    local = null;
  }
</script>

<div class="strip" class:shift={ctl.shift} role="group" aria-label="Beat FX parameters">
  {#if ctl.shift}<span class="shiftchip">SHIFT</span>{/if}
  {#each columns as col (col.key)}
    {@const shown = local?.key === col.key ? local.norm : col.norm}
    {@const pick = pickupOf(col)}
    <div
      class="col"
      class:driven={isDriven(col)}
      class:page={col.index !== null && col.index === ctl.focus}
      class:pick={!!pick}
      role="slider"
      tabindex="0"
      aria-label={col.label}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Math.round(shown * 100)}
      aria-valuetext={col.text}
      title="{col.label}: {col.text}{col.index !== null && col.index === ctl.focus ? ' (SHIFT + LEVEL/DEPTH)' : ''}"
      onpointerdown={(e) => down(e, col)}
      onpointermove={(e) => move(e, col)}
      onpointerup={up}
      onpointercancel={up}
      onwheel={(e) => wheel(e, col)}
      onkeydown={(e) => key(e, col)}
    >
      <span class="track">
        <i class="fill" style="height:{shown * 100}%"></i>
        {#if pick}
          <b class="ghost" style="bottom:{pick.value * 100}%"></b>
          <u class="knob" style="bottom:{pick.knob * 100}%"></u>
        {/if}
      </span>
      <span class="lbl">
        {#if pick}
          {#if pick.knob < pick.value}<ChevronUp size={11} strokeWidth={3} />{:else}<ChevronDown size={11} strokeWidth={3} />{/if}
        {:else}{col.short}{/if}
      </span>
    </div>
  {/each}
</div>

<style>
  .strip {
    position: relative;
    display: flex;
    align-items: stretch;
    gap: 1px;
    height: 38px;
    padding: 2px 3px;
    border-radius: var(--radius);
    background: var(--panel);
    border: 1px solid var(--line);
    transition: border-color var(--dur);
  }
  .strip.shift {
    border-color: color-mix(in srgb, var(--fx) 60%, var(--line));
  }
  .shiftchip {
    position: absolute;
    top: -8px;
    left: 50%;
    translate: -50% 0;
    padding: 1px 6px;
    border-radius: 999px;
    background: var(--fx);
    color: #fff;
    font-size: 8.5px;
    font-weight: 800;
    letter-spacing: 0.08em;
    line-height: 12px;
    box-shadow: 0 0 10px -1px var(--fx);
    pointer-events: none;
  }
  .col {
    position: relative;
    width: 22px;
    display: grid;
    grid-template-rows: 1fr 11px;
    justify-items: center;
    gap: 2px;
    padding-top: 2px;
    border-radius: 4px;
    cursor: ns-resize;
    touch-action: none;
    outline: none;
    transition: background var(--dur);
  }
  .col:hover,
  .col:focus-visible {
    background: rgba(255, 255, 255, 0.05);
  }
  .col:focus-visible {
    box-shadow: inset 0 0 0 1px var(--ring);
  }
  .track {
    position: relative;
    width: 7px;
    border-radius: 3px;
    background: rgba(255, 255, 255, 0.08);
  }
  .fill {
    position: absolute;
    inset: auto 0 0 0;
    border-radius: inherit;
    background: #52525b;
    transition: background var(--dur);
  }
  .page .fill {
    background: color-mix(in srgb, var(--fx) 55%, #52525b);
  }
  .driven .fill {
    background: linear-gradient(0deg, color-mix(in srgb, var(--fx) 60%, #1e1036), #c4b5fd);
    box-shadow: 0 0 9px -1px var(--fx);
  }
  .pick .fill {
    opacity: 0.4;
    box-shadow: none;
  }
  .ghost {
    position: absolute;
    left: -4px;
    right: -4px;
    height: 2px;
    margin-bottom: -1px;
    border-radius: 1px;
    background: var(--cue);
    box-shadow: 0 0 6px var(--cue);
  }
  .knob {
    position: absolute;
    left: -2px;
    right: -2px;
    height: 2px;
    margin-bottom: -1px;
    background: #fff;
    opacity: 0.8;
  }
  .lbl {
    display: grid;
    place-items: center;
    font-size: 8.5px;
    font-weight: 750;
    letter-spacing: 0.02em;
    line-height: 11px;
    color: var(--faint);
    white-space: nowrap;
  }
  .page .lbl {
    color: color-mix(in srgb, var(--fx) 60%, var(--dim));
  }
  .driven .lbl {
    color: #ddd6fe;
  }
  .pick .lbl {
    color: var(--cue);
    animation: blink 0.8s steps(1) infinite;
  }
</style>
