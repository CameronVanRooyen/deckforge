import { cmd, engine } from '$lib/state/engine.svelte';
import { library } from '$lib/state/library.svelte';
import { ui } from '$lib/state/ui.svelte';
import { rangeValue } from '$lib/util/format';
import { holdEnd, holdStart, release } from '$lib/state/beatfx.svelte';
import { alignPhrases, jumpToNextPhrase } from '$lib/state/phraseActions';
import { BEAT_FX_FREEZE_KINDS, BEAT_FX_RELEASE_KINDS, BEAT_FX_THROW_KINDS } from '$lib/api';
import type { DeckIndex } from '$lib/api';

export interface ShortcutGroup {
  title: string;
  items: { keys: string[]; label: string }[];
}

export const SHORTCUT_DOC: ShortcutGroup[] = [
  {
    title: 'Deck A',
    items: [
      { keys: ['Q'], label: 'Play / pause' },
      { keys: ['A'], label: 'Cue (hold)' },
      { keys: ['1', '–', '8'], label: 'Hot cue (hold); Alt deletes' },
      { keys: ['Z'], label: 'Sync on / off' },
      { keys: ['W', 'S'], label: 'Tempo up / down (0.1%)' },
      { keys: ['E', 'D'], label: 'Pitch bend up / down' },
      { keys: ['R'], label: 'Jump to next phrase' },
    ],
  },
  {
    title: 'Deck B',
    items: [
      { keys: ['P'], label: 'Play / pause' },
      { keys: ['L'], label: 'Cue (hold)' },
      { keys: ['⇧', '1', '–', '8'], label: 'Hot cue (hold); ⌥⇧ deletes' },
      { keys: ['M'], label: 'Sync on / off' },
      { keys: ['O', 'K'], label: 'Tempo up / down (0.1%)' },
      { keys: ['I', 'J'], label: 'Pitch bend up / down' },
      { keys: ['U'], label: 'Jump to next phrase' },
    ],
  },
  {
    title: 'Browser',
    items: [
      { keys: ['↑', '↓'], label: 'Move selection' },
      { keys: ['←'], label: 'Load selected to Deck A' },
      { keys: ['→'], label: 'Load selected to Deck B' },
      { keys: ['Tab'], label: 'Switch focus: sidebar / tracks' },
      { keys: ['/'], label: 'Search library' },
    ],
  },
  {
    title: 'Global',
    items: [
      { keys: ['+', '−'], label: 'Waveform zoom in / out' },
      { keys: ['F'], label: 'Toggle quantize' },
      { keys: ['G'], label: 'Align the other deck’s next phrase with the master’s' },
      { keys: ['X'], label: 'Beat FX on / off' },
      { keys: ['⇧', 'X'], label: 'Release Ascend' },
      { keys: ['V'], label: 'Freeze (hold): Dub Tape, Halo' },
      { keys: ['B'], label: 'Throw (hold): Dub Tape; arms Backwash' },
      { keys: ['⌘', ','], label: 'Settings' },
      { keys: ['?'], label: 'This overlay' },
      { keys: ['Esc'], label: 'Close panel or overlay' },
    ],
  },
];

const DIGITS = ['Digit1', 'Digit2', 'Digit3', 'Digit4', 'Digit5', 'Digit6', 'Digit7', 'Digit8'];
const A = { play: 'KeyQ', cue: 'KeyA', sync: 'KeyZ', up: 'KeyW', down: 'KeyS', bendUp: 'KeyE', bendDown: 'KeyD' };
const B = { play: 'KeyP', cue: 'KeyL', sync: 'KeyM', up: 'KeyO', down: 'KeyK', bendUp: 'KeyI', bendDown: 'KeyJ' };

function typing(t: EventTarget | null): boolean {
  const el = t as HTMLElement | null;
  if (!el) return false;
  const tag = el.tagName;
  return tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT' || el.isContentEditable;
}

function nudgeTempo(deck: DeckIndex, dir: 1 | -1) {
  const d = engine.latest.decks[deck];
  if (!d.loaded) return;
  const r = rangeValue(d.tempoRange);
  const ratio = Math.min(1 + r, Math.max(1 - r, d.faderTempoRatio + dir * 0.001));
  cmd({ type: 'setTempo', deck, ratio });
}

export function installShortcuts(onFocusSearch: () => void): () => void {
  const held = new Map<string, () => void>();

  const releaseAll = () => {
    held.forEach((fn) => fn());
    held.clear();
  };

  const down = (e: KeyboardEvent) => {
    if (e.metaKey && e.code === 'Comma') {
      e.preventDefault();
      ui.settingsOpen = !ui.settingsOpen;
      return;
    }
    if (e.key === 'Escape') {
      if (library.menu) return;
      if (ui.helpOpen) ui.helpOpen = false;
      else if (ui.settingsOpen) ui.settingsOpen = false;
      else if (library.confirm) library.confirm = null;
      return;
    }
    if (library.confirm && e.key === 'Enter') {
      e.preventDefault();
      library.confirmPending();
      return;
    }
    if (typing(e.target) || e.ctrlKey || e.metaKey) return;
    if (e.key === '?' || (e.code === 'Slash' && e.shiftKey)) {
      e.preventDefault();
      ui.helpOpen = !ui.helpOpen;
      return;
    }
    if (ui.helpOpen || library.confirm) return;

    const deckKeys: [DeckIndex, typeof A][] = [[0, A], [1, B]];
    for (const [deck, k] of deckKeys) {
      switch (e.code) {
        case k.play:
          if (!e.repeat) cmd({ type: 'togglePlay', deck });
          e.preventDefault();
          return;
        case k.sync:
          if (!e.repeat) cmd({ type: 'sync', deck, enabled: !engine.latest.decks[deck].syncEnabled });
          return;
        case k.cue:
          if (!e.repeat) {
            cmd({ type: 'cueDown', deck });
            held.set(e.code, () => cmd({ type: 'cueUp', deck }));
          }
          return;
        case k.up:
          nudgeTempo(deck, 1);
          return;
        case k.down:
          nudgeTempo(deck, -1);
          return;
        case k.bendUp:
          cmd({ type: 'nudge', deck, amount: 0.05 });
          return;
        case k.bendDown:
          cmd({ type: 'nudge', deck, amount: -0.05 });
          return;
      }
    }

    const di = DIGITS.indexOf(e.code);
    if (di >= 0) {
      const deck: DeckIndex = e.shiftKey ? 1 : 0;
      e.preventDefault();
      if (e.repeat) return;
      if (e.altKey) cmd({ type: 'hotCueDelete', deck, slot: di });
      else {
        cmd({ type: 'hotCue', deck, slot: di, pressed: true });
        held.set(e.code, () => cmd({ type: 'hotCue', deck, slot: di, pressed: false }));
      }
      return;
    }

    switch (e.code) {
      case 'ArrowUp':
        e.preventDefault();
        library.browse(-1);
        break;
      case 'ArrowDown':
        e.preventDefault();
        library.browse(1);
        break;
      case 'ArrowLeft':
        e.preventDefault();
        if (!e.repeat) library.loadSelected(0);
        break;
      case 'ArrowRight':
        e.preventDefault();
        if (!e.repeat) library.loadSelected(1);
        break;
      case 'Tab':
        e.preventDefault();
        library.toggleFocus();
        break;
      case 'Equal':
      case 'NumpadAdd':
        ui.zoom(1);
        break;
      case 'Minus':
      case 'NumpadSubtract':
        ui.zoom(-1);
        break;
      case 'KeyF':
        if (!e.repeat) cmd({ type: 'quantize', enabled: !engine.latest.quantize });
        break;
      case 'KeyR':
      case 'KeyU':
        if (!e.repeat) jumpToNextPhrase(e.code === 'KeyR' ? 0 : 1);
        break;
      case 'KeyG':
        if (!e.repeat) alignPhrases();
        break;
      case 'KeyX':
        if (e.repeat) break;
        if (e.shiftKey) {
          if ((BEAT_FX_RELEASE_KINDS as readonly string[]).includes(engine.latest.beatFx.kind)) release();
        } else cmd({ type: 'beatFxEnabled', enabled: !engine.latest.beatFx.enabled });
        break;
      case 'KeyV':
        if (!e.repeat && (BEAT_FX_FREEZE_KINDS as readonly string[]).includes(engine.latest.beatFx.kind)) {
          holdStart('freeze');
          held.set(e.code, () => holdEnd('freeze'));
        }
        break;
      case 'KeyB':
        if (!e.repeat && (BEAT_FX_THROW_KINDS as readonly string[]).includes(engine.latest.beatFx.kind)) {
          if (engine.latest.beatFx.kind === 'backwash') cmd({ type: 'beatFxTrigger', action: 'throwOn' });
          else {
            holdStart('throw');
            held.set(e.code, () => holdEnd('throw'));
          }
        }
        break;
      case 'Slash':
        e.preventDefault();
        onFocusSearch();
        break;
    }
  };

  const up = (e: KeyboardEvent) => {
    const fn = held.get(e.code);
    if (fn) {
      fn();
      held.delete(e.code);
    }
  };

  window.addEventListener('keydown', down);
  window.addEventListener('keyup', up);
  window.addEventListener('blur', releaseAll);
  return () => {
    window.removeEventListener('keydown', down);
    window.removeEventListener('keyup', up);
    window.removeEventListener('blur', releaseAll);
    releaseAll();
  };
}
