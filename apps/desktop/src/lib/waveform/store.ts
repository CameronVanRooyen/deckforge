import type { Waveform } from '$lib/api';
import * as be from '$lib/api/backend';

interface Entry {
  data: Waveform | null;
  inflight: boolean;
  nextTry: number;
  version: number;
}

/** Non-reactive waveform cache polled from rAF loops. Missing data triggers (throttled) fetches. */
class WaveformStore {
  private cache = new Map<number, Entry>();
  private versionCounter = 0;

  get(trackId: number): Waveform | null {
    let e = this.cache.get(trackId);
    if (!e) {
      e = { data: null, inflight: false, nextTry: 0, version: ++this.versionCounter };
      this.cache.set(trackId, e);
    }
    if (!e.data && !e.inflight && performance.now() >= e.nextTry) this.fetch(trackId, e);
    return e.data;
  }

  /** Identity of the currently cached waveform data, used to invalidate rendered tiles. */
  version(trackId: number): number {
    return this.cache.get(trackId)?.version ?? 0;
  }

  invalidate(trackId: number) {
    const e = this.cache.get(trackId);
    if (e) {
      e.data = null;
      e.nextTry = 0;
      e.version = ++this.versionCounter;
    }
  }

  private fetch(trackId: number, e: Entry) {
    e.inflight = true;
    be.backend
      .waveform(trackId)
      .then((w) => {
        if (w) {
          e.data = w;
          e.version = ++this.versionCounter;
        } else {
          e.nextTry = performance.now() + 3000;
        }
      })
      .catch(() => (e.nextTry = performance.now() + 5000))
      .finally(() => (e.inflight = false));
  }
}

export const waveforms = new WaveformStore();
