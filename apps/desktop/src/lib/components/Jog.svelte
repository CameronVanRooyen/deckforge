<script lang="ts">
  import { engine } from '$lib/state/engine.svelte';
  import { clocks } from '$lib/ticker';
  import { hexColor } from '$lib/util/format';
  import { SeekDrag } from '$lib/waveform/seek';
  import { DECK_RGB } from '$lib/waveform/draw';
  import CanvasLayer from './CanvasLayer.svelte';
  import type { DeckIndex } from '$lib/api';

  let { deck, size = 104 }: { deck: DeckIndex; size?: number } = $props();
  // svelte-ignore state_referenced_locally
  const drag = new SeekDrag(deck);
  const REV_PER_SEC = 100 / 60 / 3;
  const TAU = Math.PI * 2;
  const FONT = '"Geist Mono Variable", ui-monospace, monospace';

  let el: HTMLDivElement;
  let scrubbing = false;
  let lastAngle = 0;
  let travelled = 0;
  let startPos = 0;

  const angleOf = (e: PointerEvent) => {
    const r = el.getBoundingClientRect();
    return Math.atan2(e.clientY - (r.top + r.height / 2), e.clientX - (r.left + r.width / 2));
  };

  function down(e: PointerEvent) {
    if (e.button !== 0 || !engine.latest.decks[deck].loaded) return;
    el.setPointerCapture(e.pointerId);
    scrubbing = true;
    lastAngle = angleOf(e);
    travelled = 0;
    startPos = clocks[deck].pos;
  }
  function move(e: PointerEvent) {
    if (!scrubbing) return;
    const a = angleOf(e);
    let delta = a - lastAngle;
    if (delta > Math.PI) delta -= TAU;
    else if (delta < -Math.PI) delta += TAU;
    travelled += delta;
    lastAngle = a;
    drag.to(startPos + travelled / TAU / REV_PER_SEC);
  }
  function up() {
    if (!scrubbing) return;
    scrubbing = false;
    drag.end();
  }

  function draw(ctx: CanvasRenderingContext2D, W: number, H: number, dpr: number) {
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, W, H);
    const d = engine.latest.decks[deck];
    const rgb = DECK_RGB[deck];
    const deckCol = `rgb(${rgb[0]},${rgb[1]},${rgb[2]})`;
    const c = W / 2;
    const ringW = 4 * dpr;
    const R = c - 2 * dpr - ringW / 2;
    const pos = clocks[deck].pos;
    const frac = d.loaded && d.durationSecs > 0 ? pos / d.durationSecs : 0;
    const top = -Math.PI / 2;

    ctx.lineWidth = ringW;
    ctx.lineCap = 'butt';
    ctx.strokeStyle = '#27272a';
    ctx.beginPath();
    ctx.arc(c, c, R, 0, TAU);
    ctx.stroke();
    if (d.loaded) {
      ctx.strokeStyle = deckCol;
      ctx.globalAlpha = d.jogTouched ? 1 : 0.85;
      ctx.beginPath();
      ctx.arc(c, c, R, top, top + frac * TAU);
      ctx.stroke();
      ctx.globalAlpha = 1;
      const tick = (t: number, col: string, len: number) => {
        const a = top + (t / d.durationSecs) * TAU;
        ctx.strokeStyle = col;
        ctx.lineWidth = 2 * dpr;
        ctx.beginPath();
        ctx.moveTo(c + Math.cos(a) * (R - ringW / 2 - dpr), c + Math.sin(a) * (R - ringW / 2 - dpr));
        ctx.lineTo(c + Math.cos(a) * (R + ringW / 2 + len), c + Math.sin(a) * (R + ringW / 2 + len));
        ctx.stroke();
      };
      d.hotCues.forEach((h) => h && tick(h.positionSecs, hexColor(h.color), 0));
      tick(d.cueSecs, '#f59e0b', 0);
    }

    const r = R - ringW / 2 - 5 * dpr;
    ctx.fillStyle = '#111113';
    ctx.strokeStyle = '#2f2f35';
    ctx.lineWidth = dpr;
    ctx.beginPath();
    ctx.arc(c, c, r, 0, TAU);
    ctx.fill();
    ctx.stroke();
    ctx.strokeStyle = '#1b1b1f';
    for (const k of [0.8, 0.62]) {
      ctx.beginPath();
      ctx.arc(c, c, r * k, 0, TAU);
      ctx.stroke();
    }

    const angle = d.loaded ? pos * REV_PER_SEC * TAU + top : top;
    const ca = Math.cos(angle);
    const sa = Math.sin(angle);
    ctx.lineCap = 'round';
    ctx.strokeStyle = d.loaded ? '#fafafa' : '#3f3f46';
    ctx.lineWidth = 2.6 * dpr;
    ctx.beginPath();
    ctx.moveTo(c + ca * r * 0.52, c + sa * r * 0.52);
    ctx.lineTo(c + ca * r * 0.93, c + sa * r * 0.93);
    ctx.stroke();
    ctx.fillStyle = d.loaded ? deckCol : '#3f3f46';
    ctx.beginPath();
    ctx.arc(c - ca * r * 0.88, c - sa * r * 0.88, 2.2 * dpr, 0, TAU);
    ctx.fill();

    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    if (d.loaded && d.hasGrid && d.gridBpm > 0) {
      const bt = ((pos - d.firstBeatSecs) * d.gridBpm) / 60;
      const beat = ((Math.floor(bt) % 4) + 4) % 4;
      const bar = Math.max(1, Math.floor(bt / 4) + 1);
      ctx.fillStyle = beat === 0 ? deckCol : '#e4e4e7';
      ctx.font = `650 ${19 * dpr}px ${FONT}`;
      ctx.fillText(String(beat + 1), c, c - 3 * dpr);
      ctx.fillStyle = '#71717a';
      ctx.font = `600 ${8.5 * dpr}px "Geist Variable", system-ui`;
      ctx.fillText(`bar ${bar}`, c, c + 11 * dpr);
    } else {
      ctx.fillStyle = '#52525b';
      ctx.font = `600 ${15 * dpr}px ${FONT}`;
      ctx.fillText('—', c, c);
    }
  }
</script>

<div
  bind:this={el}
  class="jog"
  role="presentation"
  style="width:{size}px;height:{size}px"
  onpointerdown={down}
  onpointermove={move}
  onpointerup={up}
  onpointercancel={up}
>
  <CanvasLayer label={`Deck ${deck === 0 ? 'A' : 'B'} jog wheel`} {draw} />
</div>

<style>
  .jog {
    position: relative;
    flex: none;
    border-radius: 50%;
    cursor: grab;
    touch-action: none;
  }
  .jog:active {
    cursor: grabbing;
  }
</style>
