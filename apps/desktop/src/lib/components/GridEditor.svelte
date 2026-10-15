<script lang="ts">
  import { engine } from '$lib/state/engine.svelte';
  import { library } from '$lib/state/library.svelte';
  import { toast } from '$lib/state/toast.svelte';
  import * as be from '$lib/api/backend';
  import { clocks } from '$lib/ticker';
  import { clamp, errorMessage } from '$lib/util/format';
  import Grid3x3 from '@lucide/svelte/icons/grid-3x3';
  import ChevronLeft from '@lucide/svelte/icons/chevron-left';
  import ChevronRight from '@lucide/svelte/icons/chevron-right';
  import Crosshair from '@lucide/svelte/icons/crosshair';
  import Hand from '@lucide/svelte/icons/hand';
  import type { DeckIndex } from '$lib/api';

  let { deck }: { deck: DeckIndex } = $props();
  // svelte-ignore state_referenced_locally
  const clock = clocks[deck];

  const d = $derived(engine.snap.decks[deck]);
  const track = $derived(library.deckTracks[deck]);

  let open = $state(false);
  let btn: HTMLButtonElement;
  let pop = $state<HTMLDivElement>();
  let pos = $state({ x: 0, y: 0 });
  let bpm = $state(0);
  let first = $state(0);
  let field = $state('');
  let taps: number[] = [];
  let repeat: ReturnType<typeof setInterval> | undefined;

  const round2 = (v: number) => Math.round(v * 100) / 100;
  const beatLen = $derived(bpm > 0 ? 60 / bpm : 0);

  $effect(() => {
    const b = d.hasGrid ? d.gridBpm : (track?.bpm ?? 0);
    const f = d.hasGrid ? d.firstBeatSecs : 0;
    d.trackToken;
    bpm = round2(b);
    first = f;
    field = b > 0 ? round2(b).toFixed(2) : '';
  });

  function toggle() {
    if (open) {
      open = false;
      return;
    }
    const r = btn.getBoundingClientRect();
    pos = { x: Math.min(window.innerWidth - 316, Math.max(8, r.right - 308)), y: r.bottom + 6 };
    open = true;
  }

  function apply(nextBpm = bpm, nextFirst = first) {
    if (!d.loaded) return;
    bpm = clamp(round2(nextBpm), 20, 400);
    first = Math.max(0, nextFirst);
    field = bpm.toFixed(2);
    be.backend.setBeatgrid(d.trackToken, bpm, first).catch((e) => toast.error(`Could not set beatgrid: ${errorMessage(e)}`));
  }

  const step = (delta: number) => apply(bpm + delta);
  const shift = (ms: number) => {
    const len = beatLen || 0.5;
    apply(bpm, ((first + ms / 1000) % len + len) % len);
  };
  const holdStart = (fn: () => void) => {
    fn();
    clearInterval(repeat);
    let n = 0;
    const t = setTimeout(() => {
      repeat = setInterval(() => {
        n++;
        fn();
      }, 70);
    }, 350);
    const stop = () => {
      clearTimeout(t);
      clearInterval(repeat);
      window.removeEventListener('pointerup', stop);
    };
    window.addEventListener('pointerup', stop);
  };

  function setDownbeat() {
    const len = beatLen || 0.5;
    const p = clock.pos;
    apply(bpm, p - Math.floor(p / len) * len);
  }

  function tap() {
    const now = performance.now();
    if (taps.length && now - taps[taps.length - 1] > 2500) taps = [];
    taps.push(now);
    taps = taps.slice(-8);
    if (taps.length >= 3) {
      const avg = (taps[taps.length - 1] - taps[0]) / (taps.length - 1);
      apply(60000 / avg);
    }
  }

  function commitField() {
    const v = parseFloat(field.replace(',', '.'));
    if (Number.isFinite(v) && v > 0) apply(v);
    else field = bpm > 0 ? bpm.toFixed(2) : '';
  }

  $effect(() => {
    if (!open || !pop) return;
    const h = pop.offsetHeight;
    if (pos.y + h > window.innerHeight - 8) pos = { ...pos, y: Math.max(8, window.innerHeight - h - 8) };
  });

  $effect(() => {
    if (!open) return;
    const away = (e: PointerEvent) => {
      const t = e.target as HTMLElement;
      if (!t.closest('.pop') && !btn.contains(t)) open = false;
    };
    const esc = (e: KeyboardEvent) => e.key === 'Escape' && (open = false);
    window.addEventListener('pointerdown', away, true);
    window.addEventListener('keydown', esc);
    return () => {
      window.removeEventListener('pointerdown', away, true);
      window.removeEventListener('keydown', esc);
    };
  });
</script>

<button bind:this={btn} class="trig" class:warn={d.loaded && !d.hasGrid} class:on={open} disabled={!d.loaded} onclick={toggle} title="Edit beatgrid" aria-expanded={open}>
  <Grid3x3 size={12} strokeWidth={2.4} /><span class="lbl">GRID</span>
</button>

{#if open}
  <div bind:this={pop} class="pop" data-deck={deck} style="left:{pos.x}px;top:{pos.y}px" role="dialog" aria-label="Beatgrid editor">
    <div class="hd"><b>Beatgrid · Deck {deck === 0 ? 'A' : 'B'}</b><span>{d.hasGrid ? 'Locked after your edits' : 'No grid yet — enter a BPM'}</span></div>

    <div class="row">
      <span class="l">BPM</span>
      <button class="s" onclick={() => step(-0.01)} onpointerdown={(e) => { e.preventDefault(); holdStart(() => step(-0.01)); }} aria-label="BPM minus 0.01">−.01</button>
      <input
        class="num"
        type="text"
        inputmode="decimal"
        bind:value={field}
        onblur={commitField}
        onkeydown={(e) => {
          e.stopPropagation();
          if (e.key === 'Enter') e.currentTarget.blur();
        }}
        aria-label="BPM"
      />
      <button class="s" onpointerdown={(e) => { e.preventDefault(); holdStart(() => step(0.01)); }} aria-label="BPM plus 0.01">+.01</button>
    </div>
    <div class="row">
      <span class="l"></span>
      <button class="s w" onclick={() => apply(bpm / 2)}>÷ 2</button>
      <button class="s w" onclick={() => apply(bpm * 2)}>× 2</button>
      <button class="s w" onclick={tap} title="Tap along with the beat"><Hand size={12} />Tap</button>
    </div>

    <div class="row">
      <span class="l">Shift</span>
      <button class="s" onpointerdown={(e) => { e.preventDefault(); holdStart(() => shift(-5)); }} aria-label="Shift grid earlier 5 ms"><ChevronLeft size={14} strokeWidth={2.6} />5 ms</button>
      <span class="off num">{(first * 1000).toFixed(0)} ms</span>
      <button class="s" onpointerdown={(e) => { e.preventDefault(); holdStart(() => shift(5)); }} aria-label="Shift grid later 5 ms">5 ms<ChevronRight size={14} strokeWidth={2.6} /></button>
    </div>

    <button class="down" onclick={setDownbeat} disabled={bpm <= 0}><Crosshair size={14} strokeWidth={2.4} />Set downbeat here</button>
    <p class="hint">Drops the first beat at the playhead. Pause on a kick drum, then press it.</p>
  </div>
{/if}

<style>
  .trig {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    height: 22px;
    padding: 0 8px;
    border-radius: 6px;
    border: 1px solid var(--line-2);
    background: var(--raised);
    color: var(--dim);
    font-size: 10px;
    font-weight: 760;
    letter-spacing: 0.05em;
    transition: all 110ms var(--ease);
  }
  .trig:hover:not(:disabled),
  .trig.on {
    color: var(--fg);
    border-color: var(--deck);
  }
  .trig.warn {
    color: #fcd34d;
    border-color: color-mix(in srgb, var(--warn) 45%, var(--line));
    background: color-mix(in srgb, var(--warn) 10%, transparent);
  }
  .trig:disabled {
    opacity: 0.35;
  }
  .pop {
    position: fixed;
    z-index: 110;
    width: 300px;
    padding: 12px;
    border-radius: 12px;
    background: #141416;
    border: 1px solid var(--line-2);
    box-shadow: 0 20px 50px rgba(0, 0, 0, 0.6);
    display: flex;
    flex-direction: column;
    gap: 8px;
    animation: pop-in 120ms var(--ease);
  }
  .hd {
    display: flex;
    flex-direction: column;
    gap: 1px;
    margin-bottom: 2px;
  }
  .hd b {
    font-size: 13px;
    color: var(--deck);
  }
  .hd span,
  .hint {
    font-size: 11px;
    color: var(--faint);
  }
  .hint {
    margin: 0;
    line-height: 1.4;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .l {
    width: 38px;
    font-size: 11px;
    font-weight: 650;
    color: var(--dim);
  }
  .s {
    flex: none;
    height: 28px;
    padding: 0 8px;
    min-width: 46px;
    border-radius: 7px;
    border: 1px solid var(--line-2);
    background: var(--raised-2);
    font-size: 11.5px;
    font-weight: 700;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 3px;
  }
  .s.w {
    flex: 1;
  }
  .s:hover {
    border-color: var(--deck);
  }
  .s:active {
    background: var(--deck);
    color: #09090b;
  }
  input {
    flex: 1;
    min-width: 0;
    height: 28px;
    text-align: center;
    border-radius: 7px;
    border: 1px solid var(--line-2);
    background: #09090b;
    font-size: 14px;
    font-weight: 650;
    outline: none;
  }
  input:focus {
    border-color: var(--deck);
    box-shadow: 0 0 0 3px rgba(var(--deck-rgb), 0.16);
  }
  .off {
    flex: 1;
    text-align: center;
    font-size: 12.5px;
    font-weight: 600;
  }
  .down {
    height: 32px;
    border-radius: 8px;
    border: 0;
    background: var(--fg);
    color: #09090b;
    font-weight: 750;
    font-size: 12.5px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
  }
  .down:disabled {
    opacity: 0.4;
  }
</style>
