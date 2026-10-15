import type { DeckIndex, FxControlState } from '$lib/api';

export const FX_HUD_MS = 1200;

const IDLE: FxControlState = {
  shift: false,
  knobTarget: 'level',
  focus: null,
  pickup: null,
  padModes: ['hotCue', 'hotCue'],
  held: [null, null],
  smartArming: [false, false],
  hud: null,
};

/** Controller-side FX state from `fxControl` app actions, plus the transient HUD timing derived from it. */
class FxControlStore {
  state = $state.raw<FxControlState>(IDLE);
  /** True for FX_HUD_MS after the last FX hardware gesture. */
  recent = $state(false);
  /** A gesture happened while SHIFT was held; the HUD stays open until SHIFT is released. */
  sticky = $state(false);
  /** On-screen peek at a deck's SHIFT pad layer. */
  peek = $state<[boolean, boolean]>([false, false]);
  private timer: ReturnType<typeof setTimeout> | undefined;

  get hudOpen(): boolean {
    return this.state.hud !== null && (this.recent || this.sticky);
  }

  shiftLayer(deck: DeckIndex): boolean {
    return this.state.shift || this.peek[deck];
  }

  togglePeek(deck: DeckIndex) {
    const next: [boolean, boolean] = [...this.peek];
    next[deck] = !next[deck];
    this.peek = next;
  }

  ingest(next: FxControlState) {
    const prev = this.state;
    this.state = next;
    if (next.hud !== null && next.hud.seq !== prev.hud?.seq) {
      this.flash();
      if (next.shift) this.sticky = true;
    }
    if (!next.shift && this.sticky) {
      this.sticky = false;
      this.flash();
    }
  }

  private flash() {
    this.recent = true;
    clearTimeout(this.timer);
    this.timer = setTimeout(() => (this.recent = false), FX_HUD_MS);
  }
}

export const fxControl = new FxControlStore();
