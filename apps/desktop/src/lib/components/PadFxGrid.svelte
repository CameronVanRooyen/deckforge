<script lang="ts">
  import { engine, cmd } from '$lib/state/engine.svelte';
  import { fxControl } from '$lib/state/fxcontrol.svelte';
  import { PAD_FX1_BANK, PAD_FX1_SHIFT_BANK, PAD_FX2_FAVOURITES, type DeckIndex, type PadFxSlot } from '$lib/api';
  import { BEAT_FX_LABELS, beatLabel, slotTime, usesPercent } from '$lib/fx/labels';
  import { padFx2Values } from '$lib/fx/hud';

  let { deck }: { deck: DeckIndex } = $props();

  const mode = $derived(fxControl.state.padModes[deck]);
  const shift = $derived(fxControl.shiftLayer(deck));
  const held = $derived(fxControl.state.held[deck]);
  const pad = $derived(engine.snap.padFx[deck]);
  const fx = $derived(engine.snap.beatFx);
  const bank = $derived(shift ? PAD_FX1_SHIFT_BANK : PAD_FX1_BANK);
  const percent = $derived(usesPercent(fx.kind));
  const values = $derived(padFx2Values(percent));
  const current = $derived(percent ? fx.percent : fx.beats);
  const litIndex = $derived(
    values.reduce((best, v, i) => (Math.abs(Math.log(v / current)) < Math.abs(Math.log(values[best] / current)) ? i : best), 0),
  );

  let pressed = $state<{ slot: number; shift: boolean } | null>(null);
  let gated = false;

  const voiceOn = (slot: PadFxSlot) => slot.action.type === 'momentary' && pad.voice?.kind === slot.action.voice.kind;
  const shotOn = (slot: PadFxSlot) => slot.action.type === 'oneShot' && pad.release === slot.action.release;
  const isHeld = (i: number) => (held !== null && held.pad === i && held.shift === shift && held.mode === mode) || (pressed?.slot === i && pressed.shift === shift);

  function down(e: PointerEvent, slot: number) {
    if (e.button !== 0) return;
    e.preventDefault();
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    pressed = { slot, shift };
    if (mode === 'padFx1') return cmd({ type: 'padFx', deck, slot, shift, pressed: true });
    if (shift) return cmd({ type: 'beatFxKind', kind: PAD_FX2_FAVOURITES[slot] });
    if (percent) cmd({ type: 'beatFxPercent', value: values[slot] });
    else cmd({ type: 'beatFxBeats', beats: values[slot] });
    gated = !fx.enabled;
    if (gated) cmd({ type: 'beatFxEnabled', enabled: true });
  }

  function up() {
    if (!pressed) return;
    const p = pressed;
    pressed = null;
    if (mode === 'padFx1') cmd({ type: 'padFx', deck, slot: p.slot, shift: p.shift, pressed: false });
    else if (gated) {
      gated = false;
      cmd({ type: 'beatFxEnabled', enabled: false });
    }
  }
</script>

<div class="grid" class:shift data-mode={mode} role="group" aria-label="Deck {deck === 0 ? 'A' : 'B'} {mode === 'padFx1' ? 'PAD FX 1' : 'PAD FX 2'} pads">
  {#if mode === 'padFx1'}
    {#each bank as slot, i (`${shift}-${i}`)}
      {@const shot = slot.action.type === 'oneShot'}
      {@const running = shotOn(slot)}
      <button
        class="pad"
        class:shot
        class:on={voiceOn(slot)}
        class:running
        class:held={isHeld(i)}
        style={running ? `--p:${pad.releaseProgress}` : ''}
        onpointerdown={(e) => down(e, i)}
        onpointerup={up}
        onpointercancel={up}
        onlostpointercapture={up}
        oncontextmenu={(e) => e.preventDefault()}
        title="{slot.label}: {shot ? 'one-shot transition' : 'hold for the effect'}, {slotTime(slot)}"
      >
        <span class="name">{slot.label}</span>
        <span class="time num">{slotTime(slot)}</span>
      </button>
    {/each}
  {:else if shift}
    {#each PAD_FX2_FAVOURITES as kind, i (kind)}
      <button
        class="pad fav"
        class:on={fx.kind === kind}
        class:held={isHeld(i)}
        onpointerdown={(e) => down(e, i)}
        onpointerup={up}
        onpointercancel={up}
        onlostpointercapture={up}
        oncontextmenu={(e) => e.preventDefault()}
        title="Make {BEAT_FX_LABELS[kind]} the Beat FX"
      >
        <span class="name">{BEAT_FX_LABELS[kind]}</span>
      </button>
    {/each}
  {:else}
    {#each values as v, i (v)}
      <button
        class="pad div"
        class:on={i === litIndex}
        class:gate={i === litIndex && fx.enabled}
        class:held={isHeld(i)}
        onpointerdown={(e) => down(e, i)}
        onpointerup={up}
        onpointercancel={up}
        onlostpointercapture={up}
        oncontextmenu={(e) => e.preventDefault()}
        title="{percent ? `${v}%` : `${beatLabel(v)} beat`}: sets the Beat FX and holds it on while pressed"
      >
        <span class="big num">{percent ? `${v}%` : beatLabel(v)}</span>
      </button>
    {/each}
  {/if}
</div>

<style>
  .grid {
    flex: 1;
    min-width: 0;
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 4px;
  }
  .pad {
    position: relative;
    height: var(--h-pad);
    min-width: 0;
    display: grid;
    align-content: center;
    justify-items: center;
    gap: 1px;
    padding: 0 4px;
    border-radius: 5px;
    border: 1px solid color-mix(in srgb, var(--fx) 22%, var(--line));
    background: linear-gradient(180deg, color-mix(in srgb, var(--fx) 9%, var(--raised)), var(--raised));
    color: var(--dim);
    overflow: hidden;
    isolation: isolate;
    touch-action: none;
    transition:
      background 90ms var(--ease),
      border-color 90ms,
      color 90ms,
      box-shadow 90ms;
  }
  .pad:hover {
    color: var(--fg);
    border-color: color-mix(in srgb, var(--fx) 50%, var(--line));
  }
  .pad.shot {
    border-color: color-mix(in srgb, var(--shot) 28%, var(--line));
    background: linear-gradient(180deg, color-mix(in srgb, var(--shot) 12%, var(--raised)), var(--raised));
  }
  .pad.shot:hover {
    border-color: color-mix(in srgb, var(--shot) 60%, var(--line));
  }
  .shift .pad:not(.shot) {
    border-style: dashed;
  }
  .name {
    max-width: 100%;
    font-size: 10.5px;
    font-weight: 650;
    line-height: 1.1;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .time {
    font-size: 9px;
    line-height: 1.1;
    color: color-mix(in srgb, var(--fx) 45%, var(--faint));
  }
  .shot .time {
    color: color-mix(in srgb, var(--shot) 50%, var(--faint));
  }
  .big {
    font-size: 12.5px;
    font-weight: 650;
  }
  .pad.on {
    color: #fff;
    border-color: color-mix(in srgb, var(--fx) 75%, white);
    background: linear-gradient(180deg, #a78bfa, var(--fx));
    box-shadow: 0 0 14px -3px var(--fx);
  }
  .pad.on .time {
    color: #ede9fe;
  }
  .pad.div.on:not(.gate) {
    background: color-mix(in srgb, var(--fx) 30%, var(--raised));
    box-shadow: none;
  }
  .pad.running {
    color: #fff;
    border-color: var(--shot);
    box-shadow: 0 0 14px -3px var(--shot);
  }
  .pad.running::before {
    content: '';
    position: absolute;
    inset: 0;
    z-index: -1;
    background: linear-gradient(90deg, color-mix(in srgb, var(--shot) 40%, transparent), var(--shot));
    clip-path: inset(0 calc((1 - var(--p, 0)) * 100%) 0 0);
  }
  .pad.running .time {
    color: #fff;
  }
  .pad.held {
    box-shadow:
      inset 0 0 0 2px rgba(255, 255, 255, 0.85),
      0 0 14px -3px var(--fx);
    transform: translateY(1px);
  }
  @media (max-height: 820px) {
    .time {
      display: none;
    }
    .pad {
      padding: 0 2px;
    }
    .name {
      font-size: 9.5px;
      letter-spacing: -0.01em;
    }
  }
</style>
