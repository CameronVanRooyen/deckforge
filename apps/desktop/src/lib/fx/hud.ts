import {
  BEAT_FX_BEATS,
  BEAT_FX_KIND_BEATS,
  BEAT_FX_PARAMS,
  BEAT_FX_PRIMARY_PARAM,
  type ColorFxKind,
  PAD_FX1_BANK,
  PAD_FX1_SHIFT_BANK,
  PAD_FX2_BEATS,
  PAD_FX2_FAVOURITES,
  PAD_FX2_PERCENTS,
  type BeatFxKind,
  type EngineSnapshot,
  type FxControlState,
  type FxPickup,
} from '$lib/api';
import {
  BEAT_FX_LABELS,
  BEAT_FX_ORDER,
  PAD_MODE_LABELS,
  RELEASE_FX_LABELS,
  SMART_CFX_LABELS,
  SMART_CFX_SIDES,
  beatLabel,
  deckLetter,
  knobParams,
  paramNorm,
  paramText,
  paramValue,
  pct,
  usesPercent,
} from './labels';
import { around } from '$lib/util/around';

export type HudAnchor = 'bar' | 'mixer' | 0 | 1;
export type HudTone = 'fx' | 'shot' | 'smart' | 'deck';

export interface HudPage {
  label: string;
  value: string;
  norm: number;
  active: boolean;
}

export interface HudListItem {
  label: string;
  offset: number;
}

export interface HudPickup {
  value: number;
  knob: number;
  up: boolean;
  text: string;
}

export interface HudIdle {
  kind: string;
  enabled: boolean;
  target: string;
  time: string;
  level: number;
  pads: [string, string];
  cfx: string;
  smart: boolean;
}

export interface HudView {
  tone: HudTone;
  kicker: string;
  name: string;
  value: string;
  norm: number | null;
  progress: number | null;
  sub: string | null;
  pickup: HudPickup | null;
  pages: HudPage[] | null;
  list: HudListItem[] | null;
}

const KIND_LIST_REACH = 3;

function kindList(kind: BeatFxKind): HudListItem[] {
  return around(BEAT_FX_ORDER, BEAT_FX_ORDER.indexOf(kind), KIND_LIST_REACH).map(({ item, offset }) => ({ label: BEAT_FX_LABELS[item], offset }));
}

const COLOR_FX_LABELS: Record<ColorFxKind, string> = { filter: 'Filter', space: 'Space', dubEcho: 'Dub Echo', crush: 'Crush', noise: 'Noise' };

export function hudIdle(snap: EngineSnapshot, fx: FxControlState): HudIdle {
  const b = snap.beatFx;
  const t = beatFxTime(snap);
  return {
    kind: BEAT_FX_LABELS[b.kind],
    enabled: b.enabled,
    target: b.target === 'master' ? 'Master' : `Deck ${b.target.toUpperCase()}`,
    time: t.name === 'Beat' ? `${t.value} beat${b.beats <= 1 ? '' : 's'}` : t.value,
    level: b.level,
    pads: [PAD_MODE_LABELS[fx.padModes[0]], PAD_MODE_LABELS[fx.padModes[1]]],
    cfx: snap.smartCfx.enabled ? SMART_CFX_LABELS[snap.smartCfx.preset] : COLOR_FX_LABELS[snap.colorFx],
    smart: snap.smartCfx.enabled,
  };
}

export function hudAnchor(fx: FxControlState): HudAnchor | null {
  const hud = fx.hud;
  if (!hud) return null;
  if (hud.control === 'smartCfx' || hud.control === 'smartPreset') return 'mixer';
  const deck = hud.deck ?? (fx.knobTarget === 'padLevel' && (hud.control === 'level' || hud.control === 'beats') ? (fx.pickup?.deck ?? fx.held.findIndex((h) => h !== null)) : null);
  return deck === 0 || deck === 1 ? deck : 'bar';
}

function pages(snap: EngineSnapshot, fx: FxControlState): HudPage[] {
  const b = snap.beatFx;
  const levelActive = fx.knobTarget === 'level';
  return [
    { label: 'Level', value: pct(b.level), norm: b.level, active: levelActive },
    ...knobParams(b.kind).map(({ spec, index }) => {
      const v = paramValue(spec, b.params, index);
      return { label: spec.label, value: paramText(spec, v), norm: paramNorm(spec, v), active: !levelActive && fx.focus === index };
    }),
  ];
}

function pickupView(p: FxPickup | null, format: (norm: number) => string): HudPickup | null {
  if (!p) return null;
  const up = p.knob < p.value;
  return { value: p.value, knob: p.knob, up, text: `Turn ${up ? 'up' : 'down'} to ${format(p.value)}` };
}

function beatFxTime(snap: EngineSnapshot): { name: string; value: string; norm: number } {
  const b = snap.beatFx;
  if (usesPercent(b.kind)) return { name: 'Amount', value: `${Math.round(b.percent)}%`, norm: b.percent / 100 };
  const primary = BEAT_FX_PRIMARY_PARAM[b.kind];
  if (primary !== undefined) {
    const spec = BEAT_FX_PARAMS[b.kind][primary];
    const i = Math.round(paramValue(spec, b.params, primary));
    return { name: 'Length', value: `${spec.options?.[i] ?? i} bars`, norm: i / Math.max(1, (spec.options?.length ?? 1) - 1) };
  }
  const options = BEAT_FX_KIND_BEATS[b.kind] ?? BEAT_FX_BEATS;
  const i = Math.max(0, options.findIndex((o) => Math.abs(o - b.beats) < 1e-6));
  return { name: 'Beat', value: beatLabel(b.beats), norm: i / Math.max(1, options.length - 1) };
}

function deckView(snap: EngineSnapshot, fx: FxControlState, deck: 0 | 1): HudView {
  const hud = fx.hud!;
  const mode = fx.padModes[deck];
  const pad = snap.padFx[deck];
  const kicker = `Deck ${deckLetter(deck)} · ${PAD_MODE_LABELS[mode]}`;
  const base: HudView = { tone: 'deck', kicker, name: '', value: '', norm: null, progress: null, sub: null, pickup: null, pages: null, list: null };
  if (hud.control === 'padMode') return { ...base, kicker: `Deck ${deckLetter(deck)}`, name: 'Pad mode', value: PAD_MODE_LABELS[mode] };
  const voice = pad.voice;
  if (hud.control === 'level' && voice) {
    const pickup = fx.pickup?.target === 'padLevel' && fx.pickup.deck === deck ? pickupView(fx.pickup, pct) : null;
    return { ...base, tone: 'fx', name: `${BEAT_FX_LABELS[voice.kind]} level`, value: pct(voice.level), norm: voice.level, pickup };
  }
  if (hud.control === 'beats' && voice) {
    const pc = usesPercent(voice.kind);
    return { ...base, tone: 'fx', name: `${BEAT_FX_LABELS[voice.kind]} ${pc ? 'amount' : 'beat'}`, value: pc ? `${voice.percent}%` : beatLabel(voice.beats), norm: pc ? voice.percent / 100 : null };
  }
  if (pad.release) {
    return { ...base, tone: 'shot', name: 'Transition', value: RELEASE_FX_LABELS[pad.release], progress: pad.releaseProgress };
  }
  const held = fx.held[deck];
  if (mode === 'padFx2') {
    const b = snap.beatFx;
    if (held?.shift) return { ...base, tone: 'fx', name: 'Favourite', value: BEAT_FX_LABELS[PAD_FX2_FAVOURITES[held.pad] ?? b.kind] };
    const t = beatFxTime(snap);
    return { ...base, tone: 'fx', name: `${BEAT_FX_LABELS[b.kind]} ${t.name.toLowerCase()}`, value: t.value, norm: t.norm, sub: b.enabled ? 'Beat FX on' : 'Beat FX off' };
  }
  if (voice) {
    const pc = usesPercent(voice.kind);
    return { ...base, tone: 'fx', name: 'Held effect', value: BEAT_FX_LABELS[voice.kind], sub: `${pc ? `${voice.percent}%` : `${beatLabel(voice.beats)} beat`} · level ${pct(voice.level)}`, norm: voice.level };
  }
  if (held && mode === 'padFx1') {
    const slot = (held.shift ? PAD_FX1_SHIFT_BANK : PAD_FX1_BANK)[held.pad];
    return { ...base, tone: slot?.action.type === 'oneShot' ? 'shot' : 'fx', name: `Pad ${held.pad + 1}`, value: slot?.label ?? '' };
  }
  return { ...base, name: pad.sounding ? 'Tail ringing out' : 'Released', value: PAD_MODE_LABELS[mode] };
}

function barView(snap: EngineSnapshot, fx: FxControlState, sticky: boolean): HudView {
  const hud = fx.hud!;
  const b = snap.beatFx;
  const kind = BEAT_FX_LABELS[b.kind];
  const base: HudView = { tone: 'fx', kicker: kind, name: '', value: '', norm: null, progress: null, sub: null, pickup: null, pages: null, list: null };
  const showPages = hud.control === 'page' || (sticky && (hud.control === 'param' || hud.control === 'level'));
  const pageList = showPages ? pages(snap, fx) : null;
  if (hud.control === 'release') {
    const r = snap.releaseFx;
    const target = b.target === 'master' ? 'Master' : `Deck ${b.target.toUpperCase()}`;
    return {
      ...base,
      tone: 'shot',
      kicker: 'Release FX',
      name: r.running ? 'Firing' : 'Selected',
      value: RELEASE_FX_LABELS[r.running ?? r.kind],
      progress: r.running ? r.progress : null,
      sub: `${beatLabel(r.beats)} beat${r.beats <= 1 ? '' : 's'} on ${target}`,
    };
  }
  if (hud.control === 'kind') {
    const t = beatFxTime(snap);
    return { ...base, kicker: 'Beat FX', name: 'Type', value: kind, sub: `${t.name} ${t.value}`, list: kindList(b.kind) };
  }
  if (hud.control === 'onOff') return { ...base, name: 'Beat FX', value: b.enabled ? 'On' : 'Off', sub: `Target ${b.target === 'master' ? 'Master' : `Deck ${b.target.toUpperCase()}`}` };
  if (hud.control === 'beats') return { ...base, ...beatFxTime(snap), pages: pageList };
  if (fx.knobTarget === 'param' && fx.focus !== null && (hud.control === 'param' || hud.control === 'page' || hud.control === 'level')) {
    const spec = BEAT_FX_PARAMS[b.kind][fx.focus];
    if (spec) {
      const v = paramValue(spec, b.params, fx.focus);
      const fmt = (n: number) => paramText(spec, spec.min + n * (spec.max - spec.min));
      const pickup = fx.pickup?.target === 'param' && fx.pickup.index === fx.focus ? pickupView(fx.pickup, fmt) : null;
      return { ...base, name: spec.label, value: paramText(spec, v), norm: paramNorm(spec, v), pickup, pages: pageList };
    }
  }
  const pickup = fx.pickup?.target === 'level' ? pickupView(fx.pickup, pct) : null;
  return { ...base, name: 'Level', value: pct(b.level), norm: b.level, pickup, pages: pageList };
}

function mixerView(snap: EngineSnapshot, fx: FxControlState): HudView {
  const s = snap.smartCfx;
  const [l, r] = SMART_CFX_SIDES[s.preset];
  const arming = fx.smartArming.some(Boolean);
  return {
    tone: 'smart',
    kicker: 'Smart CFX',
    name: fx.hud?.control === 'smartPreset' ? 'Preset' : s.enabled ? 'On' : 'Off · Color FX',
    value: SMART_CFX_LABELS[s.preset],
    norm: null,
    progress: null,
    sub: arming ? 'Return the CFX knob to centre to arm it' : `${l}  ◂ ▸  ${r}`,
    pickup: null,
    pages: null,
    list: null,
  };
}

export function hudView(snap: EngineSnapshot, fx: FxControlState, anchor: HudAnchor, sticky: boolean): HudView | null {
  if (!fx.hud) return null;
  if (anchor === 'mixer') return mixerView(snap, fx);
  if (anchor === 'bar') return barView(snap, fx, sticky);
  return deckView(snap, fx, anchor);
}

export const padFx2Values = (kindUsesPercent: boolean): readonly number[] => (kindUsesPercent ? PAD_FX2_PERCENTS : PAD_FX2_BEATS);
