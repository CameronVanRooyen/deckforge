const SAMPLES: [number, number][] = [
  [0.5, 0.5],
  [0.12, 0.5],
  [0.88, 0.5],
  [0.5, 0.2],
  [0.5, 0.8],
];

const painted = (cs: CSSStyleDeclaration) => cs.backgroundImage !== 'none' || !/^(transparent|rgba\(.*,\s*0\))$/.test(cs.backgroundColor);

const describe = (el: Element) => `${el.tagName.toLowerCase()}.${[...el.classList].filter((c) => !c.startsWith('svelte-')).join('.')}`;

/** Elements that paint underneath a native waveform rect and would hide the native layer drawn behind the webview. */
export function nativeOcclusion(): string[] {
  const found = new Set<string>();
  for (const surface of document.querySelectorAll<HTMLElement>('[data-native-surface]')) {
    const r = surface.getBoundingClientRect();
    if (r.width < 1 || r.height < 1) continue;
    for (const [fx, fy] of SAMPLES) {
      const stack = document.elementsFromPoint(r.left + r.width * fx, r.top + r.height * fy);
      const from = stack.findIndex((el) => el === surface || el.contains(surface));
      if (from < 0) continue;
      for (const el of stack.slice(from)) {
        if (el === document.documentElement || el === document.body) continue;
        if (painted(getComputedStyle(el))) found.add(`${surface.dataset.nativeSurface} under ${describe(el)}`);
      }
    }
  }
  return [...found];
}

/** Native rects in CSS px, top to bottom, plus whether any overlaps the FX display. */
export function nativeLayout(): { rects: string; hudClear: boolean } {
  const hud = document.querySelector('[data-fx-hud]')?.getBoundingClientRect();
  const rects = [...document.querySelectorAll<HTMLElement>('[data-native-surface]')].map((el) => ({ id: el.dataset.nativeSurface, r: el.getBoundingClientRect() }));
  const hits = (a: DOMRect, b: DOMRect) => a.left < b.right && b.left < a.right && a.top < b.bottom && b.top < a.bottom;
  return {
    rects: rects
      .sort((a, b) => a.r.top - b.r.top || a.r.left - b.r.left)
      .map(({ id, r }) => `${id}@${Math.round(r.left)},${Math.round(r.top)} ${Math.round(r.width)}x${Math.round(r.height)}`)
      .join(' '),
    hudClear: !hud || rects.every(({ r }) => !hits(r, hud)),
  };
}
