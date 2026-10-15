<script lang="ts">
  import { clamp } from '$lib/util/format';

  interface Props {
    value: number;
    onchange: (v: number) => void;
    orientation?: 'vertical' | 'horizontal';
    reset?: number;
    detent?: number;
    capSize?: number;
    label?: string;
    ticks?: number[];
    fill?: 'none' | 'from-start' | 'from-centre';
    thin?: boolean;
  }
  let {
    value, onchange, orientation = 'vertical', reset, detent, capSize = 26, label, ticks = [], fill = 'from-start', thin = false,
  }: Props = $props();

  let el: HTMLDivElement;
  let local = $state<number | null>(null);
  let dragging = $state(false);
  let grab = 0;
  let releaseTimer: ReturnType<typeof setTimeout> | undefined;
  let raf = 0;
  let pending = 0;
  const vertical = $derived(orientation === 'vertical');
  const shown = $derived(local ?? value);

  function push(v: number) {
    if (detent !== undefined && Math.abs(v - detent) < 0.018) v = detent;
    local = v;
    pending = v;
    if (!raf) {
      raf = requestAnimationFrame(() => {
        raf = 0;
        onchange(pending);
      });
    }
  }

  function valueAt(e: PointerEvent): number {
    const r = el.getBoundingClientRect();
    const span = (vertical ? r.height : r.width) - capSize;
    const pos = vertical ? r.bottom - e.clientY - capSize / 2 : e.clientX - r.left - capSize / 2;
    return clamp((pos - grab) / span, 0, 1);
  }

  function down(e: PointerEvent) {
    el.setPointerCapture(e.pointerId);
    clearTimeout(releaseTimer);
    dragging = true;
    const r = el.getBoundingClientRect();
    const span = (vertical ? r.height : r.width) - capSize;
    const capCentre = shown * span + capSize / 2;
    const ptr = vertical ? r.bottom - e.clientY : e.clientX - r.left;
    grab = Math.abs(ptr - capCentre) <= capSize / 2 + 3 ? ptr - capCentre : 0;
    push(valueAt(e));
  }
  function move(e: PointerEvent) {
    if (dragging) push(valueAt(e));
  }
  function up() {
    if (!dragging) return;
    dragging = false;
    releaseTimer = setTimeout(() => (local = null), 220);
  }
  function dbl() {
    if (reset === undefined) return;
    clearTimeout(releaseTimer);
    push(reset);
    releaseTimer = setTimeout(() => (local = null), 400);
  }
  function key(e: KeyboardEvent) {
    const step = e.shiftKey ? 0.01 : 0.05;
    if (e.key === 'ArrowUp' || e.key === 'ArrowRight') push(clamp(shown + step, 0, 1));
    else if (e.key === 'ArrowDown' || e.key === 'ArrowLeft') push(clamp(shown - step, 0, 1));
    else return;
    e.preventDefault();
    e.stopPropagation();
    clearTimeout(releaseTimer);
    releaseTimer = setTimeout(() => (local = null), 400);
  }
  function wheel(e: WheelEvent) {
    e.preventDefault();
    push(clamp(shown + (e.deltaY < 0 ? 0.02 : -0.02), 0, 1));
    clearTimeout(releaseTimer);
    releaseTimer = setTimeout(() => (local = null), 400);
  }

  const capStyle = $derived(
    vertical
      ? `bottom: calc(${shown} * (100% - ${capSize}px)); height:${capSize}px`
      : `left: calc(${shown} * (100% - ${capSize}px)); width:${capSize}px`,
  );
  const fillStyle = $derived.by(() => {
    if (fill === 'none') return 'display:none';
    const a = fill === 'from-centre' ? Math.min(shown, 0.5) : 0;
    const b = fill === 'from-centre' ? Math.max(shown, 0.5) : shown;
    const inset = capSize / 2;
    return vertical
      ? `bottom: calc(${a} * (100% - ${capSize}px) + ${inset}px); height: calc(${b - a} * (100% - ${capSize}px))`
      : `left: calc(${a} * (100% - ${capSize}px) + ${inset}px); width: calc(${b - a} * (100% - ${capSize}px))`;
  });
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div
  bind:this={el}
  class="fader"
  class:v={vertical}
  class:h={!vertical}
  class:thin
  role="slider"
  tabindex="0"
  aria-label={label}
  aria-valuemin={0}
  aria-valuemax={1}
  aria-valuenow={shown}
  aria-orientation={orientation}
  onpointerdown={down}
  onpointermove={move}
  onpointerup={up}
  onpointercancel={up}
  ondblclick={dbl}
  onkeydown={key}
  onwheel={wheel}
>
  <div class="groove"></div>
  {#each ticks as t (t)}
    <i class="tick" style={vertical ? `bottom: calc(${t} * (100% - ${capSize}px) + ${capSize / 2}px)` : `left: calc(${t} * (100% - ${capSize}px) + ${capSize / 2}px)`}></i>
  {/each}
  <div class="fill" style={fillStyle}></div>
  <div class="cap" class:active={dragging} style={capStyle}><b></b></div>
</div>

<style>
  .fader {
    position: relative;
    touch-action: none;
    cursor: grab;
  }
  .fader:active {
    cursor: grabbing;
  }
  .v {
    width: 30px;
    height: 100%;
  }
  .h {
    height: 30px;
    width: 100%;
  }
  .groove {
    position: absolute;
    background: #09090b;
    border: 1px solid var(--line);
    border-radius: 4px;
  }
  .v .groove {
    left: 50%;
    width: 6px;
    transform: translateX(-50%);
    top: 0;
    bottom: 0;
  }
  .h .groove {
    top: 50%;
    height: 6px;
    transform: translateY(-50%);
    left: 0;
    right: 0;
  }
  .thin .groove {
    width: 4px;
  }
  .fill {
    position: absolute;
    background: #52525b;
    border-radius: 3px;
  }
  .v .fill {
    left: 50%;
    width: 4px;
    margin-left: -2px;
  }
  .h .fill {
    top: 50%;
    height: 4px;
    margin-top: -2px;
    --fill-dir: 90deg;
  }
  .tick {
    position: absolute;
    background: #3f3f46;
  }
  .v .tick {
    left: 3px;
    right: 3px;
    height: 1px;
  }
  .h .tick {
    top: 3px;
    bottom: 3px;
    width: 1px;
  }
  .cap {
    position: absolute;
    border-radius: 5px;
    background: linear-gradient(180deg, #3a3a40, #26262b);
    border: 1px solid #52525b;
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.55), inset 0 1px 0 rgba(255, 255, 255, 0.08);
    display: flex;
    align-items: center;
    justify-content: center;
    transition: border-color 120ms;
  }
  .v .cap {
    left: 1px;
    right: 1px;
  }
  .h .cap {
    top: 1px;
    bottom: 1px;
  }
  .cap b {
    display: block;
    background: var(--deck, #fafafa);
    border-radius: 1px;
  }
  .v .cap b {
    width: calc(100% - 8px);
    height: 2px;
  }
  .h .cap b {
    height: calc(100% - 8px);
    width: 2px;
  }
  .cap.active,
  .fader:hover .cap {
    border-color: #a1a1aa;
  }
</style>
