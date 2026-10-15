<script lang="ts">
  import { clamp } from '$lib/util/format';

  interface Props {
    value: number;
    min?: number;
    max?: number;
    reset?: number;
    bipolar?: boolean;
    size?: number;
    label?: string;
    format?: (v: number) => string;
    onchange: (v: number) => void;
    disabled?: boolean;
    kill?: boolean;
  }
  let {
    value, min = 0, max = 1, reset, bipolar = false, size = 34, label, format, onchange, disabled = false, kill = false,
  }: Props = $props();

  const centre = $derived(reset ?? (min + max) / 2);
  let local = $state<number | null>(null);
  let dragging = $state(false);
  let hover = $state(false);
  let releaseTimer: ReturnType<typeof setTimeout> | undefined;
  const shown = $derived(local ?? value);
  const norm = $derived((shown - min) / (max - min));

  const A0 = -135;
  const A1 = 135;
  const R = 15;
  const pt = (deg: number, r = R) => {
    const rad = (deg * Math.PI) / 180;
    return [20 + r * Math.sin(rad), 20 - r * Math.cos(rad)] as const;
  };
  const arc = (a: number, b: number) => {
    if (Math.abs(b - a) < 0.5) return '';
    const [x1, y1] = pt(a);
    const [x2, y2] = pt(b);
    return `M${x1.toFixed(2)} ${y1.toFixed(2)}A${R} ${R} 0 ${Math.abs(b - a) > 180 ? 1 : 0} ${b > a ? 1 : 0} ${x2.toFixed(2)} ${y2.toFixed(2)}`;
  };
  const angle = $derived(A0 + norm * (A1 - A0));
  const centreAngle = $derived(A0 + ((centre - min) / (max - min)) * (A1 - A0));
  const valuePath = $derived(bipolar ? arc(centreAngle, angle) : arc(A0, angle));
  const tick = $derived([pt(angle, 4), pt(angle, 12.5)]);

  let startY = 0;
  let startVal = 0;
  let pending = 0;
  let raf = 0;

  function push(v: number) {
    local = v;
    pending = v;
    if (!raf) {
      raf = requestAnimationFrame(() => {
        raf = 0;
        onchange(pending);
      });
    }
  }

  function down(e: PointerEvent) {
    if (disabled) return;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    clearTimeout(releaseTimer);
    dragging = true;
    startY = e.clientY;
    startVal = shown;
  }
  function move(e: PointerEvent) {
    if (!dragging) return;
    const span = max - min;
    const fine = e.shiftKey ? 0.2 : 1;
    let v = clamp(startVal + ((startY - e.clientY) / 160) * span * fine, min, max);
    if (bipolar && Math.abs(v - centre) < span * 0.025) v = centre;
    push(v);
  }
  function up() {
    if (!dragging) return;
    dragging = false;
    releaseTimer = setTimeout(() => (local = null), 220);
  }
  function wheel(e: WheelEvent) {
    if (disabled) return;
    e.preventDefault();
    clearTimeout(releaseTimer);
    const step = (max - min) * 0.02 * (e.shiftKey ? 0.25 : 1) * (e.deltaY < 0 ? 1 : -1);
    push(clamp(shown + step, min, max));
    releaseTimer = setTimeout(() => (local = null), 400);
  }
  function key(e: KeyboardEvent) {
    const step = (max - min) * (e.shiftKey ? 0.01 : 0.05);
    if (e.key === 'ArrowUp' || e.key === 'ArrowRight') push(clamp(shown + step, min, max));
    else if (e.key === 'ArrowDown' || e.key === 'ArrowLeft') push(clamp(shown - step, min, max));
    else if (e.key === 'Home' || e.key === 'Enter') push(centre);
    else return;
    e.preventDefault();
    e.stopPropagation();
    releaseTimer = setTimeout(() => (local = null), 400);
  }
  function dbl() {
    if (disabled) return;
    clearTimeout(releaseTimer);
    push(centre);
    releaseTimer = setTimeout(() => (local = null), 400);
  }
</script>

<div class="knob" class:disabled style="--s:{size}px" title="">
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div
    class="face"
    role="slider"
    tabindex="0"
    aria-label={label}
    aria-valuemin={min}
    aria-valuemax={max}
    aria-valuenow={shown}
    onpointerdown={down}
    onpointermove={move}
    onpointerup={up}
    onpointercancel={up}
    onwheel={wheel}
    ondblclick={dbl}
    onkeydown={key}
    onpointerenter={() => (hover = true)}
    onpointerleave={() => (hover = false)}
  >
    <svg viewBox="0 0 40 40" width={size} height={size}>
      <circle cx="20" cy="20" r="12.2" class="cap" />
      <path d={arc(A0, A1)} class="track" />
      {#if valuePath}<path d={valuePath} class="val" class:kill={kill && norm < 0.01} />{/if}
      <line x1={tick[0][0]} y1={tick[0][1]} x2={tick[1][0]} y2={tick[1][1]} class="needle" />
    </svg>
    {#if (hover || dragging) && format}
      <span class="tip num">{format(shown)}</span>
    {/if}
  </div>
</div>

<style>
  .knob {
    display: inline-flex;
    flex-direction: column;
    align-items: center;
    width: var(--s);
    height: var(--s);
    position: relative;
  }
  .face {
    position: relative;
    width: var(--s);
    height: var(--s);
    border-radius: 50%;
    cursor: ns-resize;
    touch-action: none;
  }
  .disabled {
    opacity: 0.4;
    pointer-events: none;
  }
  .cap {
    fill: #18181b;
    stroke: #3f3f46;
    stroke-width: 1;
    transition: fill var(--dur);
  }
  .face:hover .cap {
    fill: #222226;
  }
  .face:active .cap {
    stroke: #52525b;
  }
  .track {
    fill: none;
    stroke: #27272a;
    stroke-width: 3;
    stroke-linecap: round;
  }
  .val {
    fill: none;
    stroke: var(--knob, #d4d4d8);
    stroke-width: 3;
    stroke-linecap: round;
  }
  .val.kill {
    stroke: var(--danger);
  }
  .needle {
    stroke: #fafafa;
    stroke-width: 2;
    stroke-linecap: round;
  }
  .tip {
    position: absolute;
    bottom: calc(100% + 4px);
    left: 50%;
    transform: translateX(-50%);
    padding: 2px 6px;
    border-radius: 5px;
    background: #09090b;
    border: 1px solid var(--line-2);
    font-size: 10.5px;
    font-weight: 600;
    white-space: nowrap;
    z-index: 30;
    pointer-events: none;
  }
</style>
