<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { engine } from '$lib/state/engine.svelte';
  import { toast } from '$lib/state/toast.svelte';
  import * as be from '$lib/api/backend';
  import { errorMessage, hex } from '$lib/util/format';
  import type { MidiLogEntry } from '$lib/api';
  import Trash2 from '@lucide/svelte/icons/trash-2';

  let log = $state.raw<MidiLogEntry[]>([]);
  let follow = $state(true);
  let box: HTMLDivElement;
  const ctl = $derived(engine.controller);

  async function poll() {
    try {
      const next = await be.backend.midiLog();
      log = next.slice(-300);
      if (follow) {
        await tick();
        if (box) box.scrollTop = box.scrollHeight;
      }
    } catch {
      /* transient */
    }
  }
  onMount(() => {
    poll();
    const t = setInterval(poll, 250);
    return () => clearInterval(t);
  });

  async function clearLog() {
    await be.backend.midiClearLog().catch(() => {});
    log = [];
  }

  function inject(bytes: number[]) {
    be.backend.midiInject(bytes).catch((e) => toast.error(`Inject failed: ${errorMessage(e)}`));
  }
  const press = (bytes: number[], release?: number[]) => ({
    down: () => inject(bytes),
    up: () => release && inject(release),
  });
  const note = (status: number, n: number): [number[], number[]] => [[status, n, 0x7f], [status, n, 0x00]];

  const decks = [0, 1] as const;
  const BUTTONS: { label: string; note: number }[] = [
    { label: 'PLAY', note: 0x0b },
    { label: 'CUE', note: 0x0c },
    { label: 'SYNC', note: 0x58 },
    { label: 'HP CUE', note: 0x54 },
    { label: 'SHIFT', note: 0x3f },
    { label: 'LOOP IN', note: 0x10 },
    { label: 'LOOP OUT', note: 0x11 },
    { label: 'RELOOP', note: 0x4d },
  ];

  let xf = $state(8192);
  let faders = $state([16383, 16383]);
  let raf = 0;
  const queue = new Map<string, number[][]>();
  function sendSoon(key: string, msgs: number[][]) {
    queue.set(key, msgs);
    if (!raf)
      raf = requestAnimationFrame(() => {
        raf = 0;
        for (const m of queue.values()) m.forEach(inject);
        queue.clear();
      });
  }
  const msb = (v: number) => v >> 7;
  const lsb = (v: number) => v & 0x7f;
  function setXf(v: number) {
    xf = v;
    sendSoon('xf', [[0xb6, 0x1f, msb(v)], [0xb6, 0x3f, lsb(v)]]);
  }
  function setFader(d: number, v: number) {
    faders[d] = v;
    sendSoon(`f${d}`, [[0xb0 | d, 0x13, msb(v)], [0xb0 | d, 0x33, lsb(v)]]);
  }
  const jog = (d: number, n: number) => inject([0xb0 | d, 0x22, 0x40 + n]);
  const fmtT = (ms: number) => (ms / 1000).toFixed(3);
  const bytesText = (b: number[]) => b.map(hex).join(' ');
</script>

<div class="wrap">
  <section class="card">
    <div class="top">
      <i class="led" data-state={ctl?.connected ? 'ok' : 'bad'}></i>
      <b>{ctl?.connected ? 'Controller connected' : 'Controller not connected'}</b>
      <span class="map">{ctl?.mapping ?? '—'}</span>
    </div>
    <div class="kv">
      <span>Input</span><b>{ctl?.inputPort ?? 'none'}</b>
      <span>Output</span><b>{ctl?.outputPort ?? 'none'}</b>
      <span>Messages</span><b class="num">{ctl?.messagesIn ?? 0} in · {ctl?.messagesOut ?? 0} out</b>
      {#if ctl?.lastError}<span>Last error</span><b class="bad">{ctl.lastError}</b>{/if}
    </div>
    {#if ctl && ctl.availableInputs.length}
      <p class="ports">Detected inputs: {ctl.availableInputs.join(', ')}</p>
    {/if}
  </section>

  <section class="log">
    <div class="lh">
      <h3>MIDI log</h3>
      <label class="fol"><input type="checkbox" bind:checked={follow} />Follow</label>
      <button class="btn" onclick={clearLog}><Trash2 size={13} />Clear</button>
    </div>
    <div class="tbl" bind:this={box}>
      <div class="r h"><span>Time</span><span>Dir</span><span>Bytes</span><span>Summary</span></div>
      {#each log as e, i (i + ':' + e.timeMs)}
        <div class="r" data-dir={e.direction}>
          <span class="num">{fmtT(e.timeMs)}</span><span class="dir">{e.direction}</span><span class="mono">{bytesText(e.bytes)}</span><span>{e.summary}</span>
        </div>
      {:else}
        <p class="empty">No MIDI traffic yet. Move a control on the DDJ-400, or use the virtual controller below.</p>
      {/each}
    </div>
  </section>

  <section class="virtual">
    <h3>Virtual DDJ-400</h3>
    <p class="muted">Sends DDJ-400 MIDI messages through the same mapping path as the hardware.</p>
    <div class="vdecks">
      {#each decks as d (d)}
        <div class="vd" data-deck={d}>
          <b class="vh">Deck {d + 1}</b>
          <div class="vrow">
            {#each BUTTONS as b (b.label)}
              {@const m = note(0x90 | d, b.note)}
              <button class="vb" onpointerdown={() => press(m[0], m[1]).down()} onpointerup={() => press(m[0], m[1]).up()} onpointerleave={() => {}}>{b.label}</button>
            {/each}
          </div>
          <div class="vrow pads">
            {#each Array(8) as _, p (p)}
              {@const m = note(d === 0 ? 0x97 : 0x99, p)}
              <button class="vb pad" onpointerdown={() => press(m[0], m[1]).down()} onpointerup={() => press(m[0], m[1]).up()}>{p + 1}</button>
            {/each}
          </div>
          <div class="vrow">
            <button class="vb" onclick={() => { inject([0x96, 0x46 + d, 0x7f]); inject([0x96, 0x46 + d, 0x00]); }}>LOAD {d + 1}</button>
            <span class="lab">Jog</span>
            {#each [-5, -1, 1, 5] as n (n)}<button class="vb sm" onclick={() => jog(d, n)}>{n > 0 ? '+' : ''}{n}</button>{/each}
          </div>
          <label class="vs"><span>Fader {d + 1}</span><input type="range" min="0" max="16383" value={faders[d]} oninput={(e) => setFader(d, +e.currentTarget.value)} /><span class="num">{faders[d]}</span></label>
        </div>
      {/each}
    </div>
    <div class="vrow center">
      <span class="lab">Browse</span>
      <button class="vb sm" onclick={() => inject([0xb6, 0x40, 0x7f])}>−1</button>
      <button class="vb sm" onclick={() => inject([0xb6, 0x40, 0x01])}>+1</button>
      <label class="vs grow"><span>Crossfader</span><input type="range" min="0" max="16383" value={xf} oninput={(e) => setXf(+e.currentTarget.value)} /><span class="num">{xf}</span></label>
    </div>
  </section>
</div>

<style>
  .wrap {
    padding: 14px 20px 30px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  h3 {
    margin: 0;
    font-size: 12px;
    font-weight: 720;
    color: var(--dim);
  }
  .card {
    border: 1px solid var(--line);
    background: rgba(255, 255, 255, 0.02);
    border-radius: 10px;
    padding: 12px;
  }
  .top {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 8px;
  }
  .map {
    margin-left: auto;
    font-size: 11.5px;
    color: var(--faint);
  }
  .kv {
    display: grid;
    grid-template-columns: 90px 1fr;
    gap: 5px 10px;
    font-size: 12px;
  }
  .kv span {
    color: var(--faint);
  }
  .kv b {
    font-weight: 560;
  }
  .bad {
    color: var(--danger);
  }
  .ports,
  .muted {
    margin: 8px 0 0;
    font-size: 11.5px;
    color: var(--faint);
  }
  .lh {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 8px;
  }
  .lh h3 {
    flex: 1;
  }
  .fol {
    display: flex;
    gap: 5px;
    align-items: center;
    font-size: 11.5px;
    color: var(--dim);
  }
  .tbl {
    height: 220px;
    overflow: auto;
    border: 1px solid var(--line);
    border-radius: 9px;
    background: #09090b;
    font-size: 11.5px;
  }
  .r {
    display: grid;
    grid-template-columns: 62px 58px 118px 1fr;
    gap: 8px;
    padding: 3px 10px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.03);
  }
  .r.h {
    position: sticky;
    top: 0;
    background: var(--panel);
    color: var(--faint);
    font-weight: 650;
  }
  .mono {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text);
  }
  .dir {
    font-weight: 650;
  }
  [data-dir='in'] .dir {
    color: #6ee7a0;
  }
  [data-dir='out'] .dir {
    color: #fdba74;
  }
  [data-dir='injected'] .dir {
    color: #c4b5fd;
  }
  .empty {
    padding: 20px;
    margin: 0;
    text-align: center;
    color: var(--faint);
  }
  .vdecks {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
    margin: 10px 0;
  }
  .vd {
    padding: 10px;
    border-radius: 10px;
    border: 1px solid var(--line);
    background: transparent;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .vh {
    color: var(--deck);
    font-size: 12px;
  }
  .vrow {
    display: flex;
    gap: 5px;
    align-items: center;
    flex-wrap: wrap;
  }
  .vrow.center {
    padding: 0 2px;
  }
  .pads {
    display: grid;
    grid-template-columns: repeat(8, 1fr);
  }
  .vb {
    height: 26px;
    padding: 0 9px;
    border-radius: 6px;
    border: 1px solid var(--line-2);
    background: var(--raised);
    color: var(--fg);
    font-size: 11px;
    font-weight: 700;
  }
  .vb:active {
    background: var(--deck, var(--fg));
    color: #09090b;
  }
  .vb.sm {
    padding: 0;
    width: 30px;
  }
  .vb.pad {
    padding: 0;
  }
  .lab {
    font-size: 11px;
    color: var(--faint);
    margin: 0 2px;
  }
  .vs {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 11px;
    color: var(--faint);
  }
  .vs input {
    flex: 1;
    min-width: 0;
    accent-color: #a1a1aa;
  }
  .vs .num {
    width: 38px;
    text-align: right;
  }
  .grow {
    flex: 1;
  }
</style>
