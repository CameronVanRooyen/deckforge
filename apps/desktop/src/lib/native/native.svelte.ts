import { invoke } from '@tauri-apps/api/core';
import { pushThemeStyle, watchTheme } from './styleBridge';

export type SurfaceId = 'scroll-a' | 'scroll-b' | 'overview-a' | 'overview-b';
export type DeckIndex = 0 | 1;

export interface FrameStats {
  frames: number;
  window: number;
  p50Ms: number;
  p99Ms: number;
  maxMs: number;
  refreshHz: number;
  dropped: number;
  cpuAvgMs: number;
  cpuP99Ms: number;
  idleSkips: number;
}

export interface NativeStats {
  active: boolean;
  info: { adapter: string; backend: string; host: string; gpuTiming: boolean };
  frames: FrameStats;
  gpuMs: number | null;
}

const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

/** Reactive availability of the native waveform layer; `ready` flips once the backend has answered. */
export const native = $state({ available: false, ready: false });

let init: Promise<void> | null = null;

export function initNative(): Promise<void> {
  init ??= (async () => {
    if (inTauri) {
      try {
        native.available = await invoke<boolean>('native_waveforms_available');
      } catch {
        native.available = false;
      }
    }
    native.ready = true;
    document.documentElement.toggleAttribute('data-native-waveforms', native.available);
    if (native.available) {
      pushThemeStyle();
      watchTheme();
    }
  })();
  return init;
}

const send = (cmd: string, args?: Record<string, unknown>) => {
  if (native.available) void invoke(cmd, args).catch(() => {});
};

const surfaces = new Map<SurfaceId, HTMLElement>();
let lastKey = '';
let raf = 0;

interface Rect {
  id: SurfaceId;
  x: number;
  y: number;
  w: number;
  h: number;
}

function measure(): Rect[] {
  const out: Rect[] = [];
  for (const [id, el] of surfaces) {
    if (!el.isConnected || el.getClientRects().length === 0) continue;
    const r = el.getBoundingClientRect();
    if (r.width < 1 || r.height < 1) continue;
    out.push({ id, x: r.left, y: r.top, w: r.width, h: r.height });
  }
  return out;
}

function syncRects(force = false) {
  const rects = measure();
  const key = rects.map((r) => `${r.id}:${r.x.toFixed(2)},${r.y.toFixed(2)},${r.w.toFixed(2)},${r.h.toFixed(2)}`).join('|');
  if (!force && key === lastKey) return;
  lastKey = key;
  send('native_set_rects', { rects });
}

function loop() {
  raf = requestAnimationFrame(loop);
  syncRects();
}

/** Registers a placeholder element whose on-screen rectangle the native layer draws into. */
export function registerSurface(id: SurfaceId, el: HTMLElement): () => void {
  surfaces.set(id, el);
  if (!raf) raf = requestAnimationFrame(loop);
  syncRects(true);
  return () => {
    if (surfaces.get(id) === el) surfaces.delete(id);
    syncRects(true);
    if (surfaces.size === 0 && raf) {
      cancelAnimationFrame(raf);
      raf = 0;
    }
  };
}

const lastZoom = new Map<number, number>();

/** Pixels per track second in CSS px, identical to `ZOOM_LEVELS[ui.zoomIndex]`. */
export function setNativeZoom(ppsCss: number, deck?: DeckIndex) {
  const key = deck ?? -1;
  if (lastZoom.get(key) === ppsCss) return;
  lastZoom.set(key, ppsCss);
  send('native_set_zoom', { pps: ppsCss, deck: deck ?? null });
}

let lastBeatmatchView: boolean | undefined;
export function setNativeBeatmatchView(enabled: boolean) {
  if (lastBeatmatchView === enabled) return;
  lastBeatmatchView = enabled;
  send('native_set_beatmatch_view', { enabled });
}

/** Pins the native playhead to `secs` while dragging; `null` releases it. No-op without the native layer. */
export function nativeDragPreview(deck: DeckIndex, secs: number | null) {
  send('native_drag_preview', { deck, secs });
}

export function setNativeLatencyMs(ms: number) {
  send('native_set_latency', { ms });
}

export function reloadNativeTrack(trackId: number) {
  send('native_reload_track', { trackId });
}

export function setNativeStyle(spec: Record<string, unknown>): Promise<string[]> {
  if (!native.available) return Promise.resolve([]);
  return invoke<string[]>('native_set_style', { spec }).catch(() => []);
}

export function nativeFrameStats(): Promise<NativeStats | null> {
  if (!native.available) return Promise.resolve(null);
  return invoke<NativeStats>('native_frame_stats').catch(() => null);
}
