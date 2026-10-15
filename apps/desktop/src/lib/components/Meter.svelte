<script lang="ts">
  import { engine } from '$lib/state/engine.svelte';
  import { linearToDb } from '$lib/util/format';
  import CanvasLayer from './CanvasLayer.svelte';

  interface Props {
    source: 0 | 1 | 'master' | 'cue';
    scale?: boolean;
  }
  let { source, scale = false }: Props = $props();

  const DB_MIN = -42;
  const DB_MAX = 3;
  const SEGS = 30;
  const level = [DB_MIN, DB_MIN];
  const peak = [DB_MIN, DB_MIN];
  const hold = [0, 0];
  let last = 0;

  function read(): [number, number] {
    const s = engine.latest;
    if (source === 'master') return [s.master.meterL, s.master.meterR];
    if (source === 'cue') return [s.master.cueMeterL, s.master.cueMeterR];
    const c = s.channels[source];
    return [c.meterL, c.meterR];
  }

  function color(db: number, lit: boolean): string {
    if (db >= -1) return lit ? '#ef4444' : 'rgba(239,68,68,0.12)';
    if (db >= -9) return lit ? '#f59e0b' : 'rgba(245,158,11,0.1)';
    return lit ? '#22c55e' : 'rgba(34,197,94,0.09)';
  }

  function draw(ctx: CanvasRenderingContext2D, W: number, H: number, dpr: number, now: number) {
    const dt = last ? Math.min(0.1, (now - last) / 1000) : 0.016;
    last = now;
    const v = read();
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, W, H);
    const labelW = scale ? 17 * dpr : 0;
    const gap = 2 * dpr;
    const barW = Math.floor((W - labelW - gap) / 2);
    const segH = H / SEGS;
    for (let ch = 0; ch < 2; ch++) {
      const db = Math.max(DB_MIN, linearToDb(v[ch]));
      level[ch] = db > level[ch] ? db : Math.max(db, level[ch] - 38 * dt);
      if (db >= peak[ch]) {
        peak[ch] = db;
        hold[ch] = now + 1100;
      } else if (now > hold[ch]) {
        peak[ch] = Math.max(DB_MIN, peak[ch] - 14 * dt);
      }
      const x = labelW + ch * (barW + gap);
      const peakSeg = Math.min(SEGS - 1, Math.max(0, Math.round(((peak[ch] - DB_MIN) / (DB_MAX - DB_MIN)) * SEGS - 1)));
      for (let i = 0; i < SEGS; i++) {
        const segDb = DB_MIN + ((i + 1) / SEGS) * (DB_MAX - DB_MIN);
        const lit = level[ch] >= segDb - (DB_MAX - DB_MIN) / SEGS / 2 || (i === peakSeg && peak[ch] > DB_MIN + 1);
        const y = H - (i + 1) * segH;
        ctx.fillStyle = color(segDb, lit);
        ctx.fillRect(x, y + dpr * 0.5, barW, Math.max(1, segH - dpr * 1.2));
      }
    }
    if (scale) {
      ctx.fillStyle = 'rgba(161,161,170,0.8)';
      ctx.font = `600 ${8.5 * dpr}px "Geist Variable", system-ui`;
      ctx.textAlign = 'left';
      ctx.textBaseline = 'middle';
      for (const t of H < 90 * dpr ? [0, -12, -36] : [0, -6, -12, -24, -36]) {
        const y = H - ((t - DB_MIN) / (DB_MAX - DB_MIN)) * H;
        ctx.fillText(String(t), 0, Math.min(H - 5 * dpr, Math.max(5 * dpr, y)));
      }
    }
  }
</script>

<CanvasLayer {draw} label="Level meter" />
