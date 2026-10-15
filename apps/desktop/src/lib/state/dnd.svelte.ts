import { library } from './library.svelte';

export const dropTargets = $state<{ deck: number | null; playlist: number | null }>({ deck: null, playlist: null });

class DragState {
  active = $state(false);
  x = $state(0);
  y = $state(0);
  label = $state('');
  ids: number[] = [];
}
export const drag = new DragState();

function targetsAt(x: number, y: number) {
  const el = document.elementFromPoint(x, y);
  const deckEl = el?.closest<HTMLElement>('[data-drop-deck]');
  const plEl = el?.closest<HTMLElement>('[data-drop-playlist]');
  return {
    deck: deckEl ? Number(deckEl.dataset.dropDeck) : null,
    playlist: plEl ? Number(plEl.dataset.dropPlaylist) : null,
  };
}

/** Pointer-based drag (works regardless of the webview's native HTML5 drag-and-drop setting). */
export function startTrackDrag(e: PointerEvent, ids: number[], label: string) {
  if (e.button !== 0) return;
  const sx = e.clientX;
  const sy = e.clientY;
  let started = false;

  const move = (ev: PointerEvent) => {
    if (!started && Math.hypot(ev.clientX - sx, ev.clientY - sy) > 6) {
      started = true;
      drag.active = true;
      drag.ids = ids;
      drag.label = label;
    }
    if (started) {
      drag.x = ev.clientX;
      drag.y = ev.clientY;
      const t = targetsAt(ev.clientX, ev.clientY);
      dropTargets.deck = t.deck;
      dropTargets.playlist = t.playlist;
    }
  };
  const up = (ev: PointerEvent) => {
    window.removeEventListener('pointermove', move);
    window.removeEventListener('pointerup', up);
    window.removeEventListener('pointercancel', up);
    if (started) {
      const t = targetsAt(ev.clientX, ev.clientY);
      if (t.deck !== null && ev.type === 'pointerup') library.loadToDeck(t.deck as 0 | 1, ids[0]);
      else if (t.playlist !== null && ev.type === 'pointerup') library.addToPlaylist(t.playlist, ids);
      suppressClick();
    }
    drag.active = false;
    dropTargets.deck = null;
    dropTargets.playlist = null;
  };
  window.addEventListener('pointermove', move);
  window.addEventListener('pointerup', up);
  window.addEventListener('pointercancel', up);
}

function suppressClick() {
  const stop = (e: Event) => {
    e.stopPropagation();
    e.preventDefault();
  };
  window.addEventListener('click', stop, { capture: true, once: true });
  setTimeout(() => window.removeEventListener('click', stop, { capture: true }), 0);
}
