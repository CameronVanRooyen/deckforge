import type { Waveform } from '$lib/api';
import { makeGradients, renderColumns, type BandGradients } from './render';

export const TILE_PX = 2048;
const MAX_TILES = 48;

interface Tile {
  canvas: HTMLCanvasElement;
  used: number;
}

/** Lazily pre-rendered waveform tiles (device pixels) so each frame only blits a window. */
export class TileCache {
  private tiles = new Map<string, Tile>();
  private gradients: BandGradients | null = null;
  private gradH = 0;
  private scratch = document.createElement('canvas');

  clear() {
    this.tiles.clear();
    this.gradients = null;
  }

  private key(trackId: number, version: number, ppsDev: number, h: number, idx: number) {
    return `${trackId}:${version}:${ppsDev}:${h}:${idx}`;
  }

  has(trackId: number, version: number, ppsDev: number, h: number, idx: number) {
    return this.tiles.has(this.key(trackId, version, ppsDev, h, idx));
  }

  get(trackId: number, version: number, wf: Waveform, ppsDev: number, h: number, idx: number, frame: number): HTMLCanvasElement {
    const k = this.key(trackId, version, ppsDev, h, idx);
    let t = this.tiles.get(k);
    if (!t) {
      const canvas = document.createElement('canvas');
      canvas.width = TILE_PX;
      canvas.height = h;
      const ctx = canvas.getContext('2d')!;
      if (!this.gradients || this.gradH !== h) {
        this.gradients = makeGradients(this.scratch.getContext('2d')!, h);
        this.gradH = h;
      }
      renderColumns(ctx, wf, TILE_PX, h, (idx * TILE_PX) / ppsDev, 1 / ppsDev, this.gradients);
      t = { canvas, used: frame };
      this.tiles.set(k, t);
      if (this.tiles.size > MAX_TILES) this.evict(frame);
    }
    t.used = frame;
    return t.canvas;
  }

  private evict(frame: number) {
    const entries = [...this.tiles.entries()].sort((a, b) => a[1].used - b[1].used);
    for (const [k, t] of entries) {
      if (this.tiles.size <= MAX_TILES - 8) break;
      if (t.used !== frame) this.tiles.delete(k);
    }
  }
}
