import { cmd, engine } from '$lib/state/engine.svelte';
import { clocks } from '$lib/ticker';
import { clamp } from '$lib/util/format';
import type { DeckIndex } from '$lib/api';

/** Receives the position being dragged to on every move, then `null` when the gesture ends. */
export type PreviewFn = (deck: DeckIndex, secs: number | null) => void;

const releaseTimers: (ReturnType<typeof setTimeout> | undefined)[] = [undefined, undefined];

export const canvasPreview: PreviewFn = (deck, secs) => {
  clearTimeout(releaseTimers[deck]);
  if (secs !== null) clocks[deck].hold = secs;
  else releaseTimers[deck] = setTimeout(() => (clocks[deck].hold = null), 140);
};

/** rAF-throttled `seek` commands plus a preview position for the renderer in use. */
export class SeekDrag {
  private target: number | null = null;
  private raf = 0;
  constructor(
    readonly deck: DeckIndex,
    private onPreview: PreviewFn = canvasPreview,
  ) {}

  to(secs: number) {
    const t = clamp(secs, 0, engine.latest.decks[this.deck].durationSecs);
    this.onPreview(this.deck, t);
    this.target = t;
    if (!this.raf) this.raf = requestAnimationFrame(() => this.flush());
  }

  private flush() {
    this.raf = 0;
    if (this.target === null) return;
    cmd({ type: 'seek', deck: this.deck, secs: this.target });
    this.target = null;
  }

  end() {
    if (this.raf) {
      cancelAnimationFrame(this.raf);
      this.raf = 0;
    }
    this.flush();
    this.onPreview(this.deck, null);
  }
}

export interface GestureHandlers {
  onpointerdown: (e: PointerEvent) => void;
  onpointermove: (e: PointerEvent) => void;
  onpointerup: (e: PointerEvent) => void;
  onpointercancel: (e: PointerEvent) => void;
}

/** Dragging by dx pixels moves the playhead by -dx * secsPerPx (grab the waveform and pull it). */
export function scrubGesture(deck: DeckIndex, secsPerPx: () => number, onPreview?: PreviewFn): GestureHandlers & { wheel: (dxPx: number) => void } {
  const drag = new SeekDrag(deck, onPreview);
  let active = false;
  let startX = 0;
  let startPos = 0;
  let wheelPos: number | null = null;
  let wheelTimer: ReturnType<typeof setTimeout> | undefined;
  const loaded = () => engine.latest.decks[deck].loaded;
  const end = () => {
    if (!active) return;
    active = false;
    drag.end();
  };
  return {
    onpointerdown(e) {
      if (e.button !== 0 || !loaded()) return;
      (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
      active = true;
      startX = e.clientX;
      startPos = clocks[deck].pos;
    },
    onpointermove(e) {
      if (active) drag.to(startPos - (e.clientX - startX) * secsPerPx());
    },
    onpointerup: end,
    onpointercancel: end,
    wheel(dxPx) {
      if (!loaded()) return;
      wheelPos = (wheelPos ?? clocks[deck].pos) + dxPx * secsPerPx();
      drag.to(wheelPos);
      clearTimeout(wheelTimer);
      wheelTimer = setTimeout(() => {
        wheelPos = null;
        drag.end();
      }, 120);
    },
  };
}

/** Pointer x maps straight to a track position via `toSecs`; press seeks, dragging keeps seeking. */
export function absoluteGesture(deck: DeckIndex, toSecs: (clientX: number, rect: DOMRect) => number, onPreview?: PreviewFn): GestureHandlers {
  const drag = new SeekDrag(deck, onPreview);
  let active = false;
  const seek = (e: PointerEvent) => drag.to(toSecs(e.clientX, (e.currentTarget as HTMLElement).getBoundingClientRect()));
  const end = () => {
    if (!active) return;
    active = false;
    drag.end();
  };
  return {
    onpointerdown(e) {
      if (e.button !== 0 || !engine.latest.decks[deck].loaded) return;
      (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
      active = true;
      seek(e);
    },
    onpointermove(e) {
      if (active) seek(e);
    },
    onpointerup: end,
    onpointercancel: end,
  };
}
