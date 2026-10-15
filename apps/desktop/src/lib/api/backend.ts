import { api, on as tauriOn } from './index';
import type { UnlistenFn } from '@tauri-apps/api/event';

export type Backend = typeof api;
export type OnFn = <T>(event: string, handler: (payload: T) => void) => Promise<UnlistenFn>;

export const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

export let backend: Backend = api;
export let on: OnFn = tauriOn;

/** Resolves the backend once; swaps in the isolated preview mock only outside Tauri. */
export async function initBackend(): Promise<void> {
  if (inTauri) return;
  const mock = await import('./mock');
  backend = mock.mockApi;
  on = mock.mockOn;
}
