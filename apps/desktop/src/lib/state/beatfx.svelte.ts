import { cmd, engine } from './engine.svelte';
import type { FxTriggerAction } from '$lib/api';

/** Which momentary Beat FX gestures are engaged right now, from the UI or the keyboard. */
export const fxHolds = $state({ throw: false, freeze: false });

type Hold = 'throw' | 'freeze';

const ACTIONS: Record<Hold, { on: FxTriggerAction; off: FxTriggerAction }> = {
  throw: { on: 'throwOn', off: 'throwOff' },
  freeze: { on: 'freezeOn', off: 'freezeOff' },
};

/** Presses a hold, switching the effect on first so the gesture never lands on a dormant unit. */
export function holdStart(hold: Hold) {
  if (fxHolds[hold]) return;
  fxHolds[hold] = true;
  if (!engine.latest.beatFx.enabled) cmd({ type: 'beatFxEnabled', enabled: true });
  cmd({ type: 'beatFxTrigger', action: ACTIONS[hold].on });
}

export function holdEnd(hold: Hold) {
  if (!fxHolds[hold]) return;
  fxHolds[hold] = false;
  cmd({ type: 'beatFxTrigger', action: ACTIONS[hold].off });
}

export function release() {
  cmd({ type: 'beatFxTrigger', action: 'release' });
}
