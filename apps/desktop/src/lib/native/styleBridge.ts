import { setNativeStyle } from './native.svelte';

const SCALARS: Record<string, string> = {
  bg: '--wf-bg',
  deckA: '--deck-a',
  deckB: '--deck-b',
  ghost: '--wf-ghost',
  playedTint: '--wf-played',
  gridBeat: '--wf-grid-beat',
  gridBar: '--wf-grid-bar',
  gridFour: '--wf-grid-four',
  gridShadow: '--wf-grid-shadow',
  gridMarker: '--wf-grid-marker',
  aligned: '--wf-aligned',
  loopColor: '--loop',
  cue: '--cue',
  playhead: '--wf-playhead',
  flagText: '--wf-flag-text',
  barNumber: '--wf-bar-number',
  warn: '--danger',
  hover: '--wf-hover',
};

const GROUPS: Record<string, string[]> = {
  low: ['--wf-low-1', '--wf-low-2', '--wf-low-3'],
  mid: ['--wf-mid-1', '--wf-mid-2', '--wf-mid-3'],
  high: ['--wf-high-1', '--wf-high-2', '--wf-high-3'],
  phrases: ['--phrase-intro', '--phrase-build', '--phrase-drop', '--phrase-break', '--phrase-outro'],
};

const FALLBACKS: Record<string, string> = { bg: '--ink' };

/** Reads the waveform theme from CSS variables; unset variables keep the native defaults. */
export function readThemeStyle(root: HTMLElement = document.documentElement): Record<string, unknown> {
  const css = getComputedStyle(root);
  const value = (name: string) => css.getPropertyValue(name).trim();
  const spec: Record<string, unknown> = { overviewPhraseBand: false };
  for (const [key, variable] of Object.entries(SCALARS)) {
    const v = value(variable) || (FALLBACKS[key] ? value(FALLBACKS[key]) : '');
    if (v) spec[key] = v;
  }
  for (const [key, variables] of Object.entries(GROUPS)) {
    const values = variables.map(value);
    if (values.every(Boolean)) spec[key] = values;
  }
  return spec;
}

export async function pushThemeStyle(): Promise<void> {
  const spec = readThemeStyle();
  if (Object.keys(spec).length === 0) return;
  const errors = await setNativeStyle(spec);
  if (errors.length) console.warn('native style rejected:', errors);
}

let watching = false;
let timer: ReturnType<typeof setTimeout> | undefined;

/** Re-pushes the theme whenever the root element's class or inline style changes. */
export function watchTheme() {
  if (watching || typeof MutationObserver === 'undefined') return;
  watching = true;
  const schedule = () => {
    clearTimeout(timer);
    timer = setTimeout(() => void pushThemeStyle(), 60);
  };
  new MutationObserver(schedule).observe(document.documentElement, { attributes: true, attributeFilter: ['class', 'style', 'data-theme'] });
  window.matchMedia?.('(prefers-color-scheme: dark)').addEventListener('change', schedule);
}
