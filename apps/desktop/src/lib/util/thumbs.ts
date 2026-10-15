import * as be from '$lib/api/backend';

const SIZE = 56;
const MAX_ENTRIES = 500;
const MAX_ACTIVE = 4;
const cache = new Map<number, Promise<string | null>>();
const waiting: (() => void)[] = [];
let active = 0;

function schedule<T>(job: () => Promise<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    const start = () => {
      active++;
      job()
        .then(resolve, reject)
        .finally(() => {
          active--;
          waiting.shift()?.();
        });
    };
    if (active < MAX_ACTIVE) start();
    else waiting.push(start);
  });
}

function downscale(url: string): Promise<string | null> {
  return new Promise((resolve) => {
    const img = new Image();
    img.onload = () => {
      const c = document.createElement('canvas');
      c.width = c.height = SIZE;
      const side = Math.min(img.naturalWidth, img.naturalHeight);
      c.getContext('2d')!.drawImage(img, (img.naturalWidth - side) / 2, (img.naturalHeight - side) / 2, side, side, 0, 0, SIZE, SIZE);
      resolve(c.toDataURL('image/jpeg', 0.82));
    };
    img.onerror = () => resolve(null);
    img.src = url;
  });
}

export function thumbnail(trackId: number): Promise<string | null> {
  const hit = cache.get(trackId);
  if (hit) return hit;
  const p = schedule(async () => {
    const url = await be.backend.artwork(trackId).catch(() => null);
    if (!url) return null;
    const small = await downscale(url);
    if (url.startsWith('blob:')) URL.revokeObjectURL(url);
    return small;
  });
  cache.set(trackId, p);
  if (cache.size > MAX_ENTRIES) cache.delete(cache.keys().next().value as number);
  return p;
}
