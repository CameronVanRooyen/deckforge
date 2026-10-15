<script lang="ts">
  import { engine, cmd } from '$lib/state/engine.svelte';
  import { fxHolds, holdEnd, holdStart, release } from '$lib/state/beatfx.svelte';
  import {
    BEAT_FX_FREEZE_KINDS,
    BEAT_FX_PARAMS,
    BEAT_FX_RELEASE_KINDS,
    BEAT_FX_THROW_KINDS,
    type BeatFxParamSpec,
  } from '$lib/api';
  import Knob from './Knob.svelte';
  import Snowflake from '@lucide/svelte/icons/snowflake';
  import Zap from '@lucide/svelte/icons/zap';
  import Scissors from '@lucide/svelte/icons/scissors';

  interface Props {
    label: string;
    onclose: () => void;
  }
  let { label, onclose }: Props = $props();

  const NOTES: Record<string, string> = {
    pulseVerb: 'Reverb that chops with the beat. Accelerate doubles the chop rate on bar lines.',
    dubTape: 'Tape delay that can run away. Hold Throw to send, Freeze to hold the loop.',
    halo: 'Shimmering reverb. Freeze holds the wash until you let go.',
    ascend: 'One-button build that lands on the next phrase. Release cuts on the downbeat.',
    backwash: 'Reverse swell built from the track ahead. It peaks on the next downbeat.',
  };

  const fx = $derived(engine.snap.beatFx);
  const specs = $derived(BEAT_FX_PARAMS[fx.kind]);
  const knobs = $derived(specs.map((spec, index) => ({ spec, index })).filter((p) => !p.spec.options));
  const choices = $derived(specs.map((spec, index) => ({ spec, index })).filter((p) => p.spec.options));
  const hasThrow = $derived((BEAT_FX_THROW_KINDS as readonly string[]).includes(fx.kind));
  const hasFreeze = $derived((BEAT_FX_FREEZE_KINDS as readonly string[]).includes(fx.kind));
  const hasRelease = $derived((BEAT_FX_RELEASE_KINDS as readonly string[]).includes(fx.kind));
  const isAscend = $derived(fx.kind === 'ascend');
  const throwLabel = $derived(fx.kind === 'backwash' ? 'Arm' : 'Throw');
  const value = (spec: BeatFxParamSpec, index: number) => fx.params[index] ?? spec.default;
  const show = (spec: BeatFxParamSpec, v: number) => `${Math.round(v)}${spec.unit === '%' ? '%' : ''}`;
  const lengthBars = $derived(fx.ascendBars ?? Number(BEAT_FX_PARAMS.ascend[0].options?.[fx.params[0]] ?? 16));
  const barsDone = $derived(fx.ascendProgress === null ? 0 : fx.ascendProgress * lengthBars);

  function press(e: PointerEvent, hold: 'throw' | 'freeze') {
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    holdStart(hold);
  }

  function lift(hold: 'throw' | 'freeze') {
    holdEnd(hold);
  }

  function arm(e: PointerEvent) {
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    if (!fx.enabled) cmd({ type: 'beatFxEnabled', enabled: true });
    else cmd({ type: 'beatFxTrigger', action: 'throwOn' });
  }

  function keydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.stopPropagation();
      onclose();
    }
  }
</script>

<svelte:window onkeydown={keydown} />

<div class="panel" role="group" aria-label="{label} controls">
  <header>
    <b>{label}</b>
    <span>{NOTES[fx.kind] ?? ''}</span>
  </header>

  {#if knobs.length}
    <div class="knobs">
      {#each knobs as { spec, index } (spec.id)}
        <div class="knob">
          <Knob
            value={value(spec, index)}
            min={spec.min}
            max={spec.max}
            reset={spec.default}
            size={38}
            label={spec.label}
            format={(v) => show(spec, v)}
            onchange={(v) => cmd({ type: 'beatFxParam', index, value: Math.round(v) })}
          />
          <span class="cap">{spec.label}</span>
          <span class="num read">{show(spec, value(spec, index))}</span>
        </div>
      {/each}
    </div>
  {/if}

  {#if choices.length}
    <div class="choices">
      {#each choices as { spec, index } (spec.id)}
        <div class="choice">
          <span class="cap">{spec.label}{spec.unit === 'bars' ? ' (bars)' : ''}</span>
          <div class="seg" role="radiogroup" aria-label={spec.label}>
            {#each spec.options ?? [] as option, i (option)}
              <button
                role="radio"
                aria-checked={Math.round(value(spec, index)) === i}
                class:active={Math.round(value(spec, index)) === i}
                onclick={() => cmd({ type: 'beatFxParam', index, value: i })}
              >{option}</button>
            {/each}
          </div>
        </div>
      {/each}
    </div>
  {/if}

  {#if isAscend}
    <div class="ascent" aria-live="off">
      <div class="meter" role="progressbar" aria-valuemin={0} aria-valuemax={1} aria-valuenow={fx.ascendProgress ?? 0} aria-label="Build progress">
        <i style="width:{(fx.ascendProgress ?? 0) * 100}%"></i>
      </div>
      <span class="num">
        {#if fx.ascendProgress === null}
          Idle. Switch on to start the build.
        {:else}
          {`Bar ${Math.min(lengthBars, Math.floor(barsDone) + 1)} of ${Math.round(lengthBars)}`}{fx.ascendLandingBar !== null ? ` · lands on bar ${fx.ascendLandingBar}` : ''}{fx.held ? ' · holding the peak' : ''}
        {/if}
      </span>
    </div>
  {/if}

  {#if hasThrow || hasFreeze || hasRelease}
    <div class="pads">
      {#if hasThrow && fx.kind === 'backwash'}
        <button class="pad" onpointerdown={arm} title="Arm another swell for the next downbeat"><Zap size={15} strokeWidth={2.2} />{throwLabel}</button>
      {:else if hasThrow}
        <button
          class="pad"
          class:lit={fxHolds.throw}
          onpointerdown={(e) => press(e, 'throw')}
          onpointerup={() => lift('throw')}
          onpointercancel={() => lift('throw')}
          onlostpointercapture={() => lift('throw')}
          title="Send while held (B). Set Send to Throw to gate the input."
        ><Zap size={15} strokeWidth={2.2} />Throw</button>
      {/if}
      {#if hasFreeze}
        <button
          class="pad"
          class:lit={fxHolds.freeze}
          onpointerdown={(e) => press(e, 'freeze')}
          onpointerup={() => lift('freeze')}
          onpointercancel={() => lift('freeze')}
          onlostpointercapture={() => lift('freeze')}
          title="Hold the loop while pressed (V)"
        ><Snowflake size={15} strokeWidth={2.2} />Freeze</button>
      {/if}
      {#if hasRelease}
        <button class="pad release" onclick={release} disabled={!fx.enabled} title="Cut the build (Shift+X)"><Scissors size={15} strokeWidth={2.2} />Release</button>
      {/if}
    </div>
  {/if}
</div>

<style>
  .panel {
    position: absolute;
    top: calc(100% + 10px);
    left: 50%;
    transform: translateX(-50%);
    z-index: 40;
    width: max-content;
    max-width: min(640px, 92vw);
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 12px 14px 14px;
    border-radius: var(--radius-lg);
    border: 1px solid color-mix(in srgb, var(--fx) 40%, var(--line-2));
    background: var(--raised);
    box-shadow: 0 18px 40px -12px rgba(0, 0, 0, 0.7);
    --knob: var(--fx);
  }
  header {
    display: flex;
    align-items: baseline;
    gap: 10px;
    min-width: 0;
  }
  header b {
    font-size: 13px;
    font-weight: 650;
    color: var(--fg);
    flex: none;
  }
  header span {
    font-size: 11.5px;
    color: var(--dim);
    line-height: 1.35;
    max-width: 46ch;
  }
  .knobs {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 14px;
  }
  .knob {
    display: grid;
    justify-items: center;
    gap: 1px;
    width: 52px;
  }
  .cap {
    font-size: 10.5px;
    font-weight: 560;
    color: var(--faint);
    white-space: nowrap;
  }
  .read {
    font-size: 11px;
    color: var(--text);
  }
  .choices {
    display: flex;
    flex-wrap: wrap;
    gap: 8px 16px;
  }
  .choice {
    display: grid;
    gap: 4px;
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
    min-width: 30px;
    padding: 0 9px;
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
  .ascent {
    display: grid;
    gap: 6px;
  }
  .ascent .num {
    font-size: 11.5px;
    color: var(--dim);
  }
  .meter {
    height: 6px;
    border-radius: 3px;
    background: var(--panel);
    border: 1px solid var(--line);
    overflow: hidden;
  }
  .meter i {
    display: block;
    height: 100%;
    background: linear-gradient(90deg, color-mix(in srgb, var(--fx) 55%, var(--panel)), var(--fx));
  }
  .pads {
    display: flex;
    gap: 8px;
  }
  .pad {
    flex: 1;
    min-width: 92px;
    height: 40px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 7px;
    border-radius: var(--radius);
    border: 1px solid var(--line-2);
    background: var(--raised-2);
    color: var(--text);
    font-size: 12.5px;
    font-weight: 650;
    touch-action: none;
    user-select: none;
    transition: background var(--dur), border-color var(--dur), color var(--dur), box-shadow var(--dur);
  }
  .pad:hover:not(:disabled) {
    border-color: var(--faint);
  }
  .pad:active:not(:disabled),
  .pad.lit {
    background: var(--fx);
    border-color: color-mix(in srgb, var(--fx) 70%, white);
    color: #fff;
    box-shadow: 0 0 14px -2px color-mix(in srgb, var(--fx) 85%, transparent);
  }
  .pad:disabled {
    opacity: 0.4;
  }
  .pad:focus-visible,
  .seg button:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 1px;
  }
</style>
