import {
  BEAT_FX_PARAMS,
  BEAT_FX_PERCENT_KINDS,
  type BeatFxKind,
  type BeatFxParamSpec,
  type PadFxSlot,
  type PadMode,
  type ReleaseFxKind,
} from '$lib/api';

export const BEAT_FX_LABELS: Record<BeatFxKind, string> = {
  delay: 'Delay',
  echo: 'Echo',
  upEcho: 'Up Echo',
  downEcho: 'Down Echo',
  pingPong: 'Ping Pong',
  spiral: 'Spiral',
  reverb: 'Reverb',
  flanger: 'Flanger',
  phaser: 'Phaser',
  roll: 'Roll',
  trans: 'Trans',
  filterSweep: 'Filter Sweep',
  pulseVerb: 'Pulse Verb',
  dubTape: 'Dub Tape',
  halo: 'Halo',
  ascend: 'Ascend',
  backwash: 'Backwash',
};

/** FX SELECT order; matches `BeatFxKind::ALL` in the engine. */
export const BEAT_FX_ORDER: readonly BeatFxKind[] = [
  'delay',
  'echo',
  'upEcho',
  'downEcho',
  'pingPong',
  'spiral',
  'reverb',
  'flanger',
  'phaser',
  'roll',
  'trans',
  'filterSweep',
  'pulseVerb',
  'dubTape',
  'halo',
  'ascend',
  'backwash',
];

export const RELEASE_FX_LABELS: Record<ReleaseFxKind, string> = {
  echoOut: 'Echo Out',
  backspin: 'Backspin',
  vinylBrake: 'Vinyl Brake',
  rollOut: 'Roll Out',
};

export const PAD_MODE_LABELS: Record<PadMode, string> = {
  hotCue: 'Hot Cue',
  padFx1: 'PAD FX 1',
  beatJump: 'Beat Jump',
  sampler: 'Sampler',
  keyboard: 'Keyboard',
  padFx2: 'PAD FX 2',
  beatLoop: 'Beat Loop',
  keyShift: 'Key Shift',
};

const SHORT: Record<string, string> = {
  feedback: 'FB',
  filter: 'FLT',
  tone: 'TONE',
  noise: 'NOIS',
  decay: 'DCY',
  bend: 'BEND',
  pitch: 'PTCH',
  resonance: 'RES',
  size: 'SIZE',
  chop: 'CHOP',
  drive: 'DRV',
  wobble: 'WOB',
  shimmer: 'SHIM',
  fifth: '5TH',
};

export const shortParam = (spec: BeatFxParamSpec) => SHORT[spec.id] ?? spec.label.slice(0, 4).toUpperCase();

const FRACTIONS: Record<number, string> = { 0.125: '1/8', 0.25: '1/4', 0.5: '1/2', 0.75: '3/4' };
export const beatLabel = (b: number) => FRACTIONS[b] ?? String(b);

export const usesPercent = (kind: BeatFxKind) => (BEAT_FX_PERCENT_KINDS as readonly string[]).includes(kind);

export const deckLetter = (deck: number) => (deck === 0 ? 'A' : 'B');

export interface KnobParam {
  spec: BeatFxParamSpec;
  index: number;
}

/** Sub-parameters SHIFT + LEVEL/DEPTH can drive: every spec without discrete options. */
export const knobParams = (kind: BeatFxKind): KnobParam[] =>
  BEAT_FX_PARAMS[kind].map((spec, index) => ({ spec, index })).filter((p) => !p.spec.options);

export const paramValue = (spec: BeatFxParamSpec, params: number[], index: number) => params[index] ?? spec.default;
export const paramNorm = (spec: BeatFxParamSpec, v: number) => (v - spec.min) / (spec.max - spec.min);
export const paramText = (spec: BeatFxParamSpec, v: number) => `${Math.round(v)}${spec.unit === '%' ? '%' : ''}`;
export const pct = (v: number) => `${Math.round(v * 100)}%`;

export function slotTime(slot: PadFxSlot): string {
  if (slot.action.type === 'oneShot') return `${beatLabel(slot.action.beats)} beat${slot.action.beats <= 1 ? '' : 's'}`;
  const v = slot.action.voice;
  return usesPercent(v.kind) ? `${v.percent}%` : beatLabel(v.beats);
}
