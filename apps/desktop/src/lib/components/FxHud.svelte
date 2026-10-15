<script lang="ts">
  import { fade } from 'svelte/transition';
  import { engine } from '$lib/state/engine.svelte';
  import { FX_HUD_MS, fxControl } from '$lib/state/fxcontrol.svelte';
  import { hudAnchor, hudIdle, hudView } from '$lib/fx/hud';
  import ChevronsUp from '@lucide/svelte/icons/chevrons-up';
  import ChevronsDown from '@lucide/svelte/icons/chevrons-down';

  const anchor = $derived(fxControl.hudOpen ? hudAnchor(fxControl.state) : null);
  const view = $derived(anchor === null ? null : hudView(engine.snap, fxControl.state, anchor, fxControl.sticky));
  const idle = $derived(hudIdle(engine.snap, fxControl.state));
  const side = $derived(anchor === 0 ? 'a' : anchor === 1 ? 'b' : null);
  const wordy = $derived(view !== null && view.value.length > 9);
  const split = $derived(view !== null && (view.list !== null || view.pages !== null));
  const meterNorm = $derived(view && view.norm !== null ? Math.max(0, Math.min(1, view.pickup ? view.pickup.knob : view.norm)) : null);
</script>

<section class="dock" class:live={view !== null} class:on={idle.enabled} data-tone={view?.tone ?? 'idle'} data-side={side} data-deck={side === 'b' ? 1 : side === 'a' ? 0 : undefined} data-fx-hud aria-label="FX display">
  {#if view}
    <div class="layer active" class:split role="status" aria-live="polite" in:fade={{ duration: 90 }} out:fade={{ duration: 160 }}>
      <div class="main">
        <div class="head">
          <span class="kicker">{view.kicker}</span>
          <span class="name">{view.name}</span>
        </div>
        <div class="value" class:num={/\d/.test(view.value)} class:wordy>{view.value}</div>
        {#if meterNorm !== null || view.pickup}
          <div class="meter">
            {#if meterNorm !== null}<i class="fill" class:loose={!!view.pickup} style="width:{meterNorm * 100}%"></i>{/if}
            {#if view.pickup}<b class="ghost" style="left:{view.pickup.value * 100}%"></b>{/if}
          </div>
        {/if}
        {#if view.progress !== null}
          <div class="progress"><i style="width:{view.progress * 100}%"></i></div>
        {/if}
        {#if view.pickup}
          <div class="pickup">
            {#if view.pickup.up}<ChevronsUp size={13} strokeWidth={2.4} />{:else}<ChevronsDown size={13} strokeWidth={2.4} />{/if}
            <span>{view.pickup.text}</span>
          </div>
        {:else if view.sub}
          <div class="sub">{view.sub}</div>
        {/if}
      </div>

      {#if view.list}
        <ol class="kinds" aria-label="Beat FX types">
          {#each view.list as k (k.label)}
            <li class:active={k.offset === 0} style="--d:{Math.abs(k.offset)}">{k.label}</li>
          {/each}
        </ol>
      {:else if view.pages}
        <ol class="pages" aria-label="Parameter pages">
          {#each view.pages as p (p.label)}
            <li class:active={p.active}>
              <span class="pl">{p.label}</span>
              <span class="pb"><i style="width:{p.norm * 100}%"></i></span>
              <span class="pv num">{p.value}</span>
            </li>
          {/each}
        </ol>
      {/if}

      {#if fxControl.sticky}
        <i class="hold sticky" title="Held open while SHIFT is down"></i>
      {:else}
        {#key fxControl.state.hud?.seq}<i class="hold" style="--ms:{FX_HUD_MS}ms"></i>{/key}
      {/if}
    </div>
  {:else}
    <div class="layer home" in:fade={{ duration: 160 }}>
      <div class="hrow">
        <span class="lab">Beat FX</span>
        <span class="state" class:lit={idle.enabled}>{idle.enabled ? 'On' : 'Off'}</span>
        <span class="tgt">on {idle.target}</span>
      </div>
      <div class="krow">
        <span class="kind">{idle.kind}</span>
        <span class="time num">{idle.time}</span>
      </div>
      <div class="lrow">
        <div class="lvl"><i style="width:{idle.level * 100}%"></i></div>
        <span class="lv num">{Math.round(idle.level * 100)}%</span>
      </div>
      <div class="frow">
        <span class="pm" data-deck="0"><b>A</b>{idle.pads[0]}</span>
        <span class="cfx">CFX {idle.cfx}</span>
        <span class="pm" data-deck="1">{idle.pads[1]}<b>B</b></span>
      </div>
    </div>
  {/if}
</section>

<style>
  .dock {
    --tone: var(--fx);
    position: relative;
    min-width: 0;
    min-height: 104px;
    overflow: hidden;
    border-radius: var(--radius-lg);
    border: 1px solid var(--line);
    background: var(--panel);
    transition:
      border-color 160ms var(--ease),
      background-color 160ms var(--ease);
  }
  .dock[data-tone='shot'] {
    --tone: var(--shot);
  }
  .dock[data-tone='deck'] {
    --tone: var(--deck, var(--fx));
  }
  .dock.on:not(.live) {
    border-color: color-mix(in srgb, var(--fx) 40%, var(--line));
    background: radial-gradient(80% 120% at 0% 0%, color-mix(in srgb, var(--fx) 12%, transparent), transparent 70%), var(--panel);
  }
  .dock.live {
    border-color: color-mix(in srgb, var(--tone) 55%, var(--line-2));
    background:
      radial-gradient(90% 140% at var(--glow-x, 0%) 0%, color-mix(in srgb, var(--tone) 24%, transparent), transparent 70%),
      var(--panel);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--tone) 18%, transparent);
  }
  .dock[data-side='b'] {
    --glow-x: 100%;
  }
  .dock[data-side]::before {
    content: '';
    position: absolute;
    top: 10px;
    bottom: 10px;
    width: 3px;
    border-radius: 2px;
    background: var(--deck);
    box-shadow: 0 0 10px var(--deck);
  }
  .dock[data-side='a']::before {
    left: 0;
  }
  .dock[data-side='b']::before {
    right: 0;
  }
  .layer {
    position: absolute;
    inset: 0;
    padding: 9px 12px 10px;
  }

  .active {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 12px;
  }
  .active.split {
    grid-template-columns: minmax(0, 1.05fr) minmax(0, 1fr);
  }
  .main {
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 6px;
    min-width: 0;
  }
  .head {
    display: flex;
    align-items: baseline;
    gap: 7px;
    min-width: 0;
    white-space: nowrap;
  }
  .kicker {
    flex: none;
    font-size: 11px;
    font-weight: 620;
    color: color-mix(in srgb, var(--tone) 70%, var(--dim));
  }
  .name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    font-size: 12.5px;
    font-weight: 600;
    color: var(--text);
  }
  .value {
    font-size: 30px;
    line-height: 1;
    font-weight: 640;
    letter-spacing: -0.015em;
    color: #fff;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .value.wordy {
    font-size: 22px;
  }
  .split .value {
    font-size: 26px;
  }
  .split .value.wordy {
    font-size: 19px;
  }
  .meter,
  .progress {
    position: relative;
    flex: none;
    height: 6px;
    border-radius: 3px;
    background: rgba(255, 255, 255, 0.07);
  }
  .meter .fill,
  .progress i {
    position: absolute;
    inset: 0 auto 0 0;
    border-radius: inherit;
    background: linear-gradient(90deg, color-mix(in srgb, var(--tone) 40%, transparent), var(--tone));
    box-shadow: 0 0 10px -1px var(--tone);
  }
  .meter .loose {
    opacity: 0.45;
    box-shadow: none;
  }
  .ghost {
    position: absolute;
    top: -4px;
    bottom: -4px;
    width: 3px;
    margin-left: -1.5px;
    border-radius: 2px;
    background: var(--cue);
    box-shadow: 0 0 8px var(--cue);
  }
  .sub {
    font-size: 11.5px;
    color: var(--dim);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .pickup {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 11.5px;
    font-weight: 650;
    color: var(--cue);
    white-space: nowrap;
  }
  .pickup :global(svg) {
    flex: none;
    animation: nudge 0.9s var(--ease) infinite alternate;
  }
  @keyframes nudge {
    to {
      translate: 0 -2px;
    }
  }

  .kinds,
  .pages {
    list-style: none;
    margin: 0;
    padding: 0 0 0 12px;
    border-left: 1px solid rgba(255, 255, 255, 0.07);
    display: flex;
    flex-direction: column;
    justify-content: center;
    min-width: 0;
  }
  .kinds li {
    padding: 0 7px;
    height: 12.5px;
    display: flex;
    align-items: center;
    border-radius: 4px;
    font-size: 10.5px;
    font-weight: 580;
    color: var(--dim);
    white-space: nowrap;
    overflow: hidden;
    opacity: calc(1 - var(--d) * 0.26);
  }
  .kinds li.active {
    height: 19px;
    margin: 1px 0;
    font-size: 12.5px;
    font-weight: 680;
    color: #fff;
    background: color-mix(in srgb, var(--tone) 34%, transparent);
    box-shadow: inset 2px 0 0 var(--tone);
  }
  .pages {
    gap: 2px;
  }
  .pages li {
    display: grid;
    grid-template-columns: minmax(0, 56px) minmax(0, 1fr) 34px;
    align-items: center;
    gap: 7px;
    padding: 2px 6px;
    border-radius: 4px;
    color: var(--faint);
  }
  .pages li.active {
    color: var(--fg);
    background: color-mix(in srgb, var(--tone) 24%, transparent);
  }
  .pl {
    font-size: 11px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .pb {
    position: relative;
    height: 3px;
    border-radius: 2px;
    background: rgba(255, 255, 255, 0.08);
  }
  .pb i {
    position: absolute;
    inset: 0 auto 0 0;
    border-radius: inherit;
    background: currentColor;
  }
  .active .pb i {
    background: var(--tone);
  }
  .pv {
    font-size: 10.5px;
    text-align: right;
  }
  .hold {
    position: absolute;
    left: 12px;
    right: 12px;
    bottom: 0;
    height: 2px;
    border-radius: 1px;
    background: color-mix(in srgb, var(--tone) 70%, transparent);
    transform-origin: left center;
    animation: drain var(--ms) linear forwards;
  }
  .hold.sticky {
    animation: none;
    background: var(--tone);
  }
  @keyframes drain {
    to {
      transform: scaleX(0);
    }
  }

  .home {
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    gap: 6px;
  }
  .hrow,
  .krow,
  .frow {
    display: flex;
    align-items: center;
    gap: 7px;
    min-width: 0;
    white-space: nowrap;
  }
  .lab {
    font-size: 11px;
    font-weight: 600;
    color: var(--faint);
  }
  .state {
    height: 15px;
    padding: 0 6px;
    display: inline-flex;
    align-items: center;
    border-radius: 4px;
    border: 1px solid var(--line);
    font-size: 9.5px;
    font-weight: 700;
    color: var(--faint);
  }
  .state.lit {
    border-color: var(--fx);
    background: var(--fx);
    color: #fff;
  }
  .tgt {
    margin-left: auto;
    font-size: 11px;
    font-weight: 560;
    color: var(--dim);
  }
  .kind {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    font-size: 21px;
    font-weight: 640;
    letter-spacing: -0.01em;
    color: var(--fg);
  }
  .time {
    margin-left: auto;
    font-size: 15px;
    font-weight: 600;
    color: var(--text);
  }
  .lrow {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .lv {
    font-size: 10.5px;
    font-weight: 600;
    color: var(--faint);
  }
  .lvl {
    flex: 1;
    position: relative;
    height: 3px;
    border-radius: 2px;
    background: rgba(255, 255, 255, 0.07);
  }
  .lvl i {
    position: absolute;
    inset: 0 auto 0 0;
    border-radius: inherit;
    background: color-mix(in srgb, var(--fx) 80%, transparent);
  }
  .frow {
    justify-content: space-between;
    font-size: 10.5px;
    font-weight: 600;
    color: var(--dim);
  }
  .pm {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
  .pm b {
    width: 14px;
    height: 14px;
    display: grid;
    place-items: center;
    border-radius: 3px;
    font-size: 9.5px;
    font-weight: 750;
    color: #09090b;
    background: var(--deck);
  }
  .cfx {
    color: var(--faint);
  }
  @media (min-height: 821px) {
    .kinds li {
      height: 14.5px;
      font-size: 11px;
    }
    .kinds li.active {
      height: 22px;
      font-size: 13.5px;
    }
    .value {
      font-size: 34px;
    }
    .split .value {
      font-size: 29px;
    }
    .pages {
      gap: 4px;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .hold,
    .pickup :global(svg) {
      animation: none;
    }
  }
</style>
