<script lang="ts">
  import { engine, cmd } from '$lib/state/engine.svelte';
  import { ui } from '$lib/state/ui.svelte';
  import { fmtPercent, hexColor, rangeLabel, rangeValue } from '$lib/util/format';
  import Fader from './Fader.svelte';
  import PadFxGrid from './PadFxGrid.svelte';
  import { fxControl } from '$lib/state/fxcontrol.svelte';
  import { backend } from '$lib/api/backend';
  import ArrowBigUp from '@lucide/svelte/icons/arrow-big-up';
  import Jog from './Jog.svelte';
  import Play from '@lucide/svelte/icons/play';
  import Pause from '@lucide/svelte/icons/pause';
  import ChevronLeft from '@lucide/svelte/icons/chevron-left';
  import ChevronRight from '@lucide/svelte/icons/chevron-right';
  import RotateCcw from '@lucide/svelte/icons/rotate-ccw';
  import type { DeckIndex, TempoRange } from '$lib/api';

  let { deck }: { deck: DeckIndex } = $props();
  const d = $derived(engine.snap.decks[deck]);
  const quantize = $derived(engine.snap.quantize);
  const range = $derived(rangeValue(d.tempoRange));
  const tempoPos = $derived(Math.max(-1, Math.min(1, (d.faderTempoRatio - 1) / range)));
  const sliderVal = $derived(0.5 - tempoPos * 0.5);

  const atCue = $derived(Math.abs(d.positionSecs - d.cueSecs) < 0.04);
  const cueBlink = $derived(d.loaded && !d.playing && !d.previewing && !atCue);
  const playWaiting = $derived(d.loaded && !d.playing && atCue);
  const BEAT_LOOPS = [0.25, 0.5, 1, 2, 4, 8, 16, 32];
  const JUMPS = [1, 2, 4, 8, 16, 32];
  const RANGES: TempoRange[] = ['Six', 'Ten', 'Sixteen', 'Wide'];
  const loopBeats = $derived(
    d.loopInSecs !== null && d.loopOutSecs !== null && d.hasGrid && d.gridBpm > 0 ? ((d.loopOutSecs - d.loopInSecs) * d.gridBpm) / 60 : null,
  );
  const hasLoop = $derived(d.loopInSecs !== null && d.loopOutSecs !== null);
  const jump = $derived(ui.jumpBeats[deck]);

  const beatLabel = (b: number) => (b < 1 ? `1/${Math.round(1 / b)}` : String(b));

  const padMode = $derived(fxControl.state.padModes[deck]);
  const fxPads = $derived(padMode === 'padFx1' || padMode === 'padFx2');
  const PREVIEW_MODES = [0x1b, 0x1e, 0x6b];
  const previewMode = () => backend.midiInject([0x90 + deck, PREVIEW_MODES[(Math.max(0, ['hotCue', 'padFx1', 'padFx2'].indexOf(padMode)) + 1) % 3], 0x7f]);

  let held = $state<boolean[]>(Array(8).fill(false));
  let cueHeld = $state(false);
  let syncTimer: ReturnType<typeof setTimeout> | undefined;
  let syncLong = false;

  function cueDown(e: PointerEvent) {
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    cueHeld = true;
    cmd({ type: 'cueDown', deck });
  }
  function cueUp() {
    if (!cueHeld) return;
    cueHeld = false;
    cmd({ type: 'cueUp', deck });
  }

  function padDown(e: PointerEvent, slot: number) {
    e.preventDefault();
    if (e.shiftKey || e.button === 2) {
      cmd({ type: 'hotCueDelete', deck, slot });
      return;
    }
    if (e.button !== 0) return;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    held[slot] = true;
    cmd({ type: 'hotCue', deck, slot, pressed: true });
  }
  function padUp(slot: number) {
    if (!held[slot]) return;
    held[slot] = false;
    cmd({ type: 'hotCue', deck, slot, pressed: false });
  }

  function syncDown(e: PointerEvent) {
    syncLong = false;
    if (e.shiftKey) {
      syncLong = true;
      cmd({ type: 'setMasterDeck', deck });
      return;
    }
    syncTimer = setTimeout(() => {
      syncLong = true;
      cmd({ type: 'setMasterDeck', deck });
    }, 550);
  }
  function syncUp() {
    clearTimeout(syncTimer);
    if (!syncLong) cmd({ type: 'sync', deck, enabled: !d.syncEnabled });
    syncLong = false;
  }

  let nudgeTimer: ReturnType<typeof setInterval> | undefined;
  function nudgeStart(amount: number) {
    cmd({ type: 'nudge', deck, amount });
    clearInterval(nudgeTimer);
    nudgeTimer = setInterval(() => cmd({ type: 'nudge', deck, amount }), 70);
  }
  function nudgeStop() {
    clearInterval(nudgeTimer);
  }

  function setTempoFromSlider(v: number) {
    const pos = (0.5 - v) * 2;
    cmd({ type: 'setTempo', deck, ratio: 1 + pos * range });
  }
  function cycleRange() {
    const next = RANGES[(RANGES.indexOf(d.tempoRange) + 1) % RANGES.length];
    cmd({ type: 'setTempoRange', deck, range: next });
  }
  function cycleJump(dir: 1 | -1) {
    const i = JUMPS.indexOf(jump);
    ui.setJump(deck, JUMPS[Math.min(JUMPS.length - 1, Math.max(0, i + dir))]);
  }
</script>

<section class="ctl" data-deck={deck} class:rev={deck === 1} class:off={!d.loaded}>
  <div class="tempo">
    <button class="rng num" onclick={cycleRange} title="Tempo range (click to cycle)">{rangeLabel(d.tempoRange)}</button>
    <div class="slider">
      <div class="scale" aria-hidden="true"><span>−</span><span></span><span>0</span><span></span><span>+</span></div>
      <Fader value={sliderVal} onchange={setTempoFromSlider} reset={0.5} detent={0.5} capSize={26} label="Tempo" fill="none" ticks={[0, 0.25, 0.5, 0.75, 1]} />
    </div>
    <div class="tv num">{fmtPercent(d.faderTempoRatio, 1)}</div>
    <button class="reset" disabled={!d.loaded} onclick={() => cmd({ type: 'setTempo', deck, ratio: 1 })} title="Reset tempo to 0%" aria-label="Reset tempo"><RotateCcw size={12} strokeWidth={2.2} /></button>
  </div>

  <div class="side">
    <Jog {deck} />
    <div class="row jump">
      <button class="chip icon" disabled={!d.loaded} onclick={() => cmd({ type: 'beatJump', deck, beats: -jump })} title={`Jump back ${jump} beats`} aria-label="Beat jump back"><ChevronLeft size={14} strokeWidth={2.4} /></button>
      <button
        class="chip num size"
        onclick={() => cycleJump(1)}
        oncontextmenu={(e) => {
          e.preventDefault();
          cycleJump(-1);
        }}
        onwheel={(e) => {
          e.preventDefault();
          cycleJump(e.deltaY < 0 ? 1 : -1);
        }}
        title="Beat jump size (click up, right-click down)">{jump}</button>
      <button class="chip icon" disabled={!d.loaded} onclick={() => cmd({ type: 'beatJump', deck, beats: jump })} title={`Jump forward ${jump} beats`} aria-label="Beat jump forward"><ChevronRight size={14} strokeWidth={2.4} /></button>
    </div>
    <div class="row bend">
      <button class="chip icon" onpointerdown={() => nudgeStart(-0.05)} onpointerup={nudgeStop} onpointerleave={nudgeStop} disabled={!d.loaded} title="Pitch bend −" aria-label="Pitch bend minus"><ChevronLeft size={14} strokeWidth={2.4} /></button>
      <span class="bl">Bend</span>
      <button class="chip icon" onpointerdown={() => nudgeStart(0.05)} onpointerup={nudgeStop} onpointerleave={nudgeStop} disabled={!d.loaded} title="Pitch bend +" aria-label="Pitch bend plus"><ChevronRight size={14} strokeWidth={2.4} /></button>
    </div>
  </div>

  <div class="main">
    <div class="transport">
      <button
        class="tbtn cue"
        class:lit={cueHeld || d.previewing || (d.loaded && atCue && !d.playing)}
        class:blink={cueBlink}
        disabled={!d.loaded}
        onpointerdown={cueDown}
        onpointerup={cueUp}
        onpointercancel={cueUp}
        aria-label="Cue">CUE</button>
      <button class="tbtn play" class:playing={d.playing && !d.previewing} class:waiting={playWaiting} disabled={!d.loaded} onclick={() => cmd({ type: 'togglePlay', deck })} aria-label={d.playing ? 'Pause' : 'Play'}>
        {#if d.playing && !d.previewing}<Pause size={18} fill="currentColor" strokeWidth={0} />{:else}<Play size={18} fill="currentColor" strokeWidth={0} />{/if}
      </button>
    </div>

    <div class="modes">
      <button
        class="mode sync"
        data-on={d.syncEnabled}
        disabled={!d.loaded}
        onpointerdown={syncDown}
        onpointerup={syncUp}
        onpointerleave={() => clearTimeout(syncTimer)}
        title="Sync (hold or Shift-click to make master)"
        aria-pressed={d.syncEnabled}>Sync</button>
      <button class="mode master" data-on={d.isMaster} disabled={!d.loaded} onclick={() => cmd({ type: 'setMasterDeck', deck })} title="Make this the master tempo deck" aria-pressed={d.isMaster}>Master</button>
      <button class="mode lock" data-on={d.keyLock} disabled={!d.loaded} onclick={() => cmd({ type: 'setKeyLock', deck, enabled: !d.keyLock })} title="Key lock (master tempo)" aria-pressed={d.keyLock}>Key lock</button>
      <button class="mode quant" data-on={quantize} onclick={() => cmd({ type: 'quantize', enabled: !quantize })} title="Quantize (F)" aria-pressed={quantize}>Quant</button>
    </div>

    <div class="padzone">
      {#if fxPads || engine.preview}
        <div class="ptab" class:lit={fxPads} class:shift={fxControl.shiftLayer(deck)}>
          {#if engine.preview}
            <button class="plabel" onclick={previewMode} title="Preview only: cycle the pad mode the DDJ-400 would set">{fxPads ? (padMode === 'padFx1' ? 'FX 1' : 'FX 2') : 'CUE'}</button>
          {:else}
            <span class="plabel" title={padMode === 'padFx1' ? 'PAD FX 1' : 'PAD FX 2'}>{padMode === 'padFx1' ? 'FX 1' : 'FX 2'}</span>
          {/if}
          {#if fxPads}
            <button class="peek" class:on={fxControl.peek[deck]} onclick={() => fxControl.togglePeek(deck)} aria-pressed={fxControl.peek[deck]} title="Show the SHIFT layer ({padMode === 'padFx1' ? 'more effects' : 'favourite effects'})" aria-label="Show the SHIFT pad layer"><ArrowBigUp size={12} strokeWidth={2.4} /></button>
          {/if}
        </div>
      {/if}
      {#if fxPads}
        <PadFxGrid {deck} />
      {:else}
        <div class="pads">
          {#each d.hotCues as cue, i (i)}
            <button
              class="pad"
              class:set={!!cue}
              class:held={held[i]}
              style={cue ? `--pc:${hexColor(cue.color)}` : ''}
              disabled={!d.loaded}
              onpointerdown={(e) => padDown(e, i)}
              onpointerup={() => padUp(i)}
              onpointercancel={() => padUp(i)}
              oncontextmenu={(e) => e.preventDefault()}
              title={cue ? `Hot cue ${i + 1} — Shift/right-click to delete` : `Set hot cue ${i + 1}`}>{i + 1}</button>
          {/each}
        </div>
      {/if}
    </div>

    <div class="loops">
      <button class="chip" disabled={!d.loaded} onclick={() => cmd({ type: 'loopIn', deck })}>IN</button>
      <button class="chip" disabled={!d.loaded || d.loopInSecs === null} onclick={() => cmd({ type: 'loopOut', deck })}>OUT</button>
      <button class="chip reloop" data-on={d.loopActive} disabled={!d.loaded || !hasLoop} onclick={() => cmd({ type: 'reloopExit', deck })}>{d.loopActive ? 'EXIT' : 'RELOOP'}</button>
      <button class="chip" disabled={!d.loaded || !hasLoop} onclick={() => cmd({ type: 'loopScale', deck, factor: 0.5 })} title="Halve loop">½</button>
      <button class="chip" disabled={!d.loaded || !hasLoop} onclick={() => cmd({ type: 'loopScale', deck, factor: 2 })} title="Double loop">×2</button>
    </div>

    <div class="beatloops">
      {#each BEAT_LOOPS as b (b)}
        <button class="chip num" data-on={d.loopActive && loopBeats !== null && Math.abs(loopBeats - b) < 0.02} disabled={!d.loaded || !d.hasGrid} onclick={() => cmd({ type: 'beatLoop', deck, beats: b })}>{beatLabel(b)}</button>
      {/each}
    </div>
  </div>
</section>

<style>
  .ctl {
    position: relative;
    --h-main: 40px;
    --h-mode: 22px;
    --h-pad: 30px;
    --h-row: 24px;
    display: flex;
    gap: 10px;
    padding: 10px;
    border: 1px solid var(--line);
    border-radius: var(--radius-lg);
    background: var(--panel);
    min-width: 0;
  }
  .ctl.rev {
    flex-direction: row-reverse;
  }
  .tempo {
    width: 42px;
    flex: none;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 5px;
  }
  .rng {
    height: 22px;
    width: 100%;
    border-radius: 5px;
    border: 1px solid var(--line);
    background: var(--raised);
    font-size: 10px;
    font-weight: 650;
    color: var(--dim);
    transition:
      color var(--dur),
      border-color var(--dur);
  }
  .rng:hover {
    color: var(--fg);
    border-color: var(--line-2);
  }
  .slider {
    flex: 1;
    min-height: 70px;
    width: 100%;
    display: flex;
    justify-content: center;
    position: relative;
  }
  .scale {
    position: absolute;
    left: 0;
    top: 13px;
    bottom: 13px;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    font-size: 9px;
    font-weight: 650;
    color: var(--faint);
    pointer-events: none;
    width: 8px;
    text-align: center;
    line-height: 1;
  }
  .rev .scale {
    left: auto;
    right: 0;
  }
  .slider :global(.fader) {
    margin: 0 auto;
  }
  .tv {
    font-size: 10.5px;
    font-weight: 600;
    color: var(--dim);
  }
  .reset {
    width: 100%;
    height: 20px;
    border-radius: 5px;
    border: 1px solid var(--line);
    background: transparent;
    color: var(--faint);
    display: grid;
    place-items: center;
    transition:
      color var(--dur),
      border-color var(--dur),
      background var(--dur);
  }
  .reset:hover:not(:disabled) {
    color: var(--fg);
    border-color: var(--line-2);
    background: var(--raised);
  }
  .reset:disabled {
    opacity: 0.35;
  }

  .side {
    flex: none;
    width: 104px;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    align-items: stretch;
    gap: 6px;
  }
  .side :global(.jog) {
    align-self: center;
  }
  .row {
    display: flex;
    gap: 4px;
    height: var(--h-row);
    align-items: stretch;
  }
  .row .icon {
    width: 28px;
    padding: 0;
  }
  .row .size {
    flex: 1;
    color: var(--fg);
  }
  .bl {
    flex: 1;
    display: grid;
    place-items: center;
    font-size: 10.5px;
    font-weight: 560;
    color: var(--faint);
  }

  .main {
    flex: 1;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    gap: 6px;
    min-width: 0;
  }
  .transport {
    display: flex;
    gap: 6px;
    height: var(--h-main);
  }
  .tbtn {
    border-radius: var(--radius-lg);
    border: 1px solid var(--line-2);
    background: var(--raised-2);
    font-weight: 700;
    font-size: 12px;
    letter-spacing: 0.05em;
    color: var(--dim);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    transition:
      background var(--dur) var(--ease),
      color var(--dur),
      border-color var(--dur);
  }
  .tbtn:disabled {
    opacity: 0.35;
  }
  .tbtn:not(:disabled):hover {
    background: var(--raised-3);
    color: var(--fg);
  }
  .tbtn:not(:disabled):active {
    background: #313137;
  }
  .cue {
    flex: 1;
    color: var(--cue);
    border-color: color-mix(in srgb, var(--cue) 38%, var(--line));
  }
  .cue:not(:disabled):hover {
    color: var(--cue);
    border-color: var(--cue);
  }
  .cue.lit {
    background: var(--cue);
    color: #1c1203;
    border-color: var(--cue);
  }
  .cue.lit:not(:disabled):hover {
    background: #fbbf24;
    color: #1c1203;
  }
  .cue.blink {
    animation: blink 0.9s steps(1) infinite;
  }
  .play {
    flex: 1.5;
    color: var(--play);
    border-color: color-mix(in srgb, var(--play) 38%, var(--line));
  }
  .play:not(:disabled):hover {
    color: var(--play);
    border-color: var(--play);
  }
  .play.playing {
    background: var(--play);
    color: #052e16;
    border-color: var(--play);
    animation: pulse-ring 1.6s var(--ease) infinite;
  }
  .play.playing:not(:disabled):hover {
    background: #4ade80;
    color: #052e16;
  }
  .play.waiting {
    animation: blink 1.1s steps(1) infinite;
  }

  .modes {
    display: grid;
    grid-template-columns: 1fr 1.25fr 1.35fr 1.1fr;
    gap: 4px;
    height: var(--h-mode);
  }
  .mode {
    min-width: 0;
    padding: 0 2px;
    border-radius: 5px;
    border: 1px solid var(--line);
    background: transparent;
    font-size: 10px;
    font-weight: 650;
    color: var(--dim);
    white-space: nowrap;
    transition:
      background var(--dur),
      color var(--dur),
      border-color var(--dur);
  }
  .mode:hover:not(:disabled) {
    background: var(--raised);
    color: var(--fg);
    border-color: var(--line-2);
  }
  .mode:disabled {
    opacity: 0.35;
  }
  .mode[data-on='true'] {
    color: #fff;
    background: var(--sync);
    border-color: var(--sync);
  }
  .mode.sync[data-on='true'] {
    background: color-mix(in srgb, var(--sync) 22%, var(--panel));
    border-color: color-mix(in srgb, var(--sync) 70%, transparent);
    color: #93c5fd;
  }
  .mode.lock[data-on='true'],
  .mode.quant[data-on='true'] {
    background: rgba(250, 250, 250, 0.1);
    border-color: var(--line-2);
    color: var(--fg);
  }

  .padzone {
    display: flex;
    gap: 4px;
  }
  .rev .padzone {
    flex-direction: row-reverse;
  }
  .ptab {
    flex: none;
    width: 20px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .plabel {
    flex: 1;
    min-height: 0;
    display: grid;
    place-items: center;
    writing-mode: vertical-rl;
    rotate: 180deg;
    padding: 0;
    border-radius: 5px;
    border: 1px solid var(--line);
    background: var(--raised);
    color: var(--faint);
    font-size: 9px;
    font-weight: 750;
    letter-spacing: 0.06em;
    white-space: nowrap;
  }
  button.plabel:hover {
    color: var(--fg);
    border-color: var(--line-2);
  }
  .ptab.lit .plabel {
    color: #fff;
    border-color: color-mix(in srgb, var(--deck) 70%, transparent);
    background: linear-gradient(0deg, color-mix(in srgb, var(--deck) 45%, var(--raised)), color-mix(in srgb, var(--fx) 45%, var(--raised)));
  }
  .peek {
    flex: none;
    height: 20px;
    display: grid;
    place-items: center;
    padding: 0;
    border-radius: 5px;
    border: 1px solid var(--line);
    background: var(--raised);
    color: var(--faint);
    transition:
      color var(--dur),
      background var(--dur),
      border-color var(--dur);
  }
  .peek:hover {
    color: var(--fg);
    border-color: var(--line-2);
  }
  .peek.on,
  .ptab.shift .peek {
    color: #fff;
    background: var(--fx);
    border-color: var(--fx);
  }
  .pads {
    flex: 1;
    min-width: 0;
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 4px;
  }
  .pad {
    height: var(--h-pad);
    border-radius: 5px;
    border: 1px solid var(--line);
    background: var(--raised);
    color: var(--faint);
    font-family: var(--font-mono);
    font-weight: 650;
    font-size: 11.5px;
    transition:
      background 90ms var(--ease),
      border-color 90ms,
      color 90ms,
      filter 90ms;
  }
  .pad:disabled {
    opacity: 0.35;
  }
  .pad:not(.set):not(:disabled):hover {
    border-color: var(--line-2);
    color: var(--dim);
    background: var(--raised-2);
  }
  .pad.set {
    color: #09090b;
    background: var(--pc);
    border-color: color-mix(in srgb, var(--pc) 60%, white);
  }
  .pad.set:hover {
    filter: brightness(1.12);
  }
  .pad.held {
    filter: brightness(1.4);
    transform: translateY(1px);
  }

  .loops,
  .beatloops {
    display: grid;
    gap: 4px;
    height: var(--h-row);
  }
  .loops {
    grid-template-columns: 1fr 1fr 1.7fr 1fr 1fr;
  }
  .beatloops {
    grid-template-columns: repeat(8, 1fr);
    gap: 3px;
  }
  .chip {
    height: 100%;
    min-width: 0;
    padding: 0 4px;
    border-radius: 5px;
    border: 1px solid var(--line);
    background: var(--raised);
    font-size: 10.5px;
    font-weight: 650;
    color: var(--dim);
    transition:
      background var(--dur),
      color var(--dur),
      border-color var(--dur);
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }
  .chip:hover:not(:disabled) {
    color: var(--fg);
    border-color: var(--line-2);
    background: var(--raised-2);
  }
  .chip:active:not(:disabled) {
    background: var(--raised-3);
  }
  .chip:disabled {
    opacity: 0.35;
  }
  .chip[data-on='true'] {
    color: #1a2e05;
    background: var(--loop);
    border-color: var(--loop);
  }
  .beatloops .chip {
    padding: 0;
    font-size: 10px;
  }

  @media (max-height: 820px) {
    .ctl {
      --h-main: 34px;
      --h-mode: 20px;
      --h-pad: 22px;
      --h-row: 22px;
      padding: 8px;
    }
    .side {
      width: 92px;
    }
    .side :global(.jog) {
      width: 88px !important;
      height: 88px !important;
    }
  }
</style>
