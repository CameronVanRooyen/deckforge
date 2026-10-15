<script lang="ts">
  import { onMount } from 'svelte';
  import { onFrame } from '$lib/ticker';

  interface Props {
    draw: (ctx: CanvasRenderingContext2D, W: number, H: number, dpr: number, now: number) => void;
    class?: string;
    onresize?: () => void;
    onpointerdown?: (e: PointerEvent, rect: DOMRect) => void;
    onpointermove?: (e: PointerEvent, rect: DOMRect) => void;
    onpointerup?: (e: PointerEvent, rect: DOMRect) => void;
    onpointerleave?: (e: PointerEvent) => void;
    onwheel?: (e: WheelEvent) => void;
    label?: string;
  }
  let { draw, class: cls = '', onresize, onpointerdown, onpointermove, onpointerup, onpointerleave, onwheel, label }: Props = $props();

  let canvas: HTMLCanvasElement;
  let W = 0;
  let H = 0;
  let dpr = 1;

  onMount(() => {
    const ctx = canvas.getContext('2d', { alpha: true })!;
    const resize = () => {
      const r = canvas.getBoundingClientRect();
      dpr = window.devicePixelRatio || 1;
      const w = Math.max(1, Math.round(r.width * dpr));
      const h = Math.max(1, Math.round(r.height * dpr));
      if (w !== W || h !== H) {
        W = canvas.width = w;
        H = canvas.height = h;
        onresize?.();
      }
    };
    resize();
    const ro = new ResizeObserver(resize);
    ro.observe(canvas);
    const off = onFrame((now) => {
      if (W > 1) draw(ctx, W, H, dpr, now);
    });
    return () => {
      off();
      ro.disconnect();
    };
  });

  const rect = () => canvas.getBoundingClientRect();
</script>

<div class="cl">
<canvas
  bind:this={canvas}
  class={cls}
  aria-label={label}
  role="img"
  style="position:absolute;inset:0;display:block;width:100%;height:100%;touch-action:none"
  onpointerdown={(e) => onpointerdown?.(e, rect())}
  onpointermove={(e) => onpointermove?.(e, rect())}
  onpointerup={(e) => onpointerup?.(e, rect())}
  onpointerleave={(e) => onpointerleave?.(e)}
  onwheel={(e) => onwheel?.(e)}
></canvas>
</div>

<style>
  .cl {
    position: relative;
    width: 100%;
    height: 100%;
    min-height: 0;
    min-width: 0;
    overflow: hidden;
  }
</style>
