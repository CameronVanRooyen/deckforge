import { initNative, native, nativeFrameStats } from '$lib/native/native.svelte';
import { nativeLayout, nativeOcclusion } from '$lib/native/occlusion';
// Debug-only end-to-end self test, enabled by DECKFORGE_SELFTEST=1 in a debug build of the backend.
// Drives the real UI state stores through the real IPC path and reports each step to the backend log.
import { invoke } from '@tauri-apps/api/core';
import { api } from '$lib/api';
import { engine } from '$lib/state/engine.svelte';
import * as ticker from '$lib/ticker';
import { alignJump, alignPhrases } from '$lib/state/phraseActions';
import { runFxSelfTest } from './selftestFx';

const report = (message: string) => invoke('ui_report', { message }).catch(() => {});
const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

async function until(label: string, check: () => boolean, timeoutMs = 15_000): Promise<boolean> {
  const start = performance.now();
  while (performance.now() - start < timeoutMs) {
    if (check()) return true;
    await wait(100);
  }
  await report(`FAIL timeout waiting for ${label}`);
  return false;
}

function canvasCoverage(): string {
  const canvases = Array.from(document.querySelectorAll('canvas'));
  return canvases
    .filter((c) => c.width > 200 && c.height > 40)
    .map((c) => {
      const ctx = c.getContext('2d');
      if (!ctx) return `${c.width}x${c.height}:webgl`;
      const data = ctx.getImageData(0, 0, c.width, c.height).data;
      let lit = 0;
      for (let i = 3; i < data.length; i += 16) if (data[i] > 0 && data[i - 1] + data[i - 2] + data[i - 3] > 60) lit++;
      return `${c.width}x${c.height}:${((lit / (data.length / 16)) * 100).toFixed(1)}%`;
    })
    .join(' ');
}

const windowErrors: string[] = [];

export async function runSelfTest(): Promise<void> {
  const enabled = await invoke<boolean>('selftest_requested').catch(() => false);
  if (!enabled) return;
  addEventListener('error', (e) => windowErrors.push(`${e.message} ${e.filename}:${e.lineno}`));
  addEventListener('unhandledrejection', (e) => windowErrors.push(`rejection: ${String(e.reason)}`));
  await report(`start: window ${innerWidth}x${innerHeight}, dpr ${devicePixelRatio}`);
  if (!(await until('first engine snapshot', () => engine.live))) return;
  await report(`snapshot live: rate ${engine.snap.sampleRate}, cueAvailable ${engine.snap.cueAvailable}`);
  await initNative();
  await wait(300);
  const layout = nativeLayout();
  const blocked = nativeOcclusion();
  await report(`native rects: ${layout.rects}; fx display clear of them: ${layout.hudClear ? 'ok' : 'FAIL'}`);
  await report(`native occlusion: ${!native.available ? 'n/a (canvas waveforms)' : blocked.length === 0 ? 'ok' : `FAIL ${blocked.join(', ')}`}`);
  await until('audio status', () => engine.audio !== null, 4000);
  await report(`audio: ${engine.audio?.state} ${engine.audio?.active?.deviceName} ${engine.audio?.active?.channels}ch`);

  const tracks = await api.tracks();
  await report(`library: ${tracks.length} tracks, analysed ${tracks.filter((t) => t.analysis === 'done').length}`);
  const ready = tracks.filter((t) => t.analysis === 'done' && t.bpm);
  if (ready.length < 2) {
    await report('FAIL need two analysed tracks');
    return;
  }
  const [a, b] = ready;
  await api.loadTrack(0, a.id, true);
  await api.loadTrack(1, b.id, true);
  if (!(await until('both decks loaded', () => engine.snap.decks[0].trackToken === a.id && engine.snap.decks[1].trackToken === b.id))) return;
  await report(`loaded A="${a.title}" (${engine.snap.decks[0].gridBpm} bpm), B="${b.title}" (${engine.snap.decks[1].gridBpm} bpm)`);

  await api.engine({ type: 'masterLevel', value: 0.12 });
  await api.engine({ type: 'fader', channel: 0, value: 1 });
  await api.engine({ type: 'fader', channel: 1, value: 0 });
  await api.engine({ type: 'pfl', channel: 1, enabled: true });
  await api.engine({ type: 'play', deck: 0 });
  await api.engine({ type: 'play', deck: 1 });
  await api.engine({ type: 'sync', deck: 1, enabled: true });
  await wait(3000);
  await api.engine({ type: 'setMasterDeck', deck: 0 });
  await wait(300);
  const before = alignJump();
  alignPhrases();
  await wait(600);
  const after = alignJump();
  await report(`phrase align: jump ${before ? `${before.beats} beats on ${before.deck ? 'B' : 'A'}` : 'n/a'}, then ${after ? after.beats : 'n/a'}`);
  await wait(4000);
  const s = engine.snap;
  await report(
    `playing A=${s.decks[0].playing} pos ${s.decks[0].positionSecs.toFixed(2)}s, B=${s.decks[1].playing} pos ${s.decks[1].positionSecs.toFixed(2)}s, ` +
      `sync B=${s.decks[1].syncEnabled} bpm A ${s.decks[0].bpm.toFixed(2)} B ${s.decks[1].bpm.toFixed(2)}, ` +
      `master meter ${s.master.meterL.toFixed(3)}, cue meter ${s.master.cueMeterL.toFixed(3)}, ch2 meter ${s.channels[1].meterL.toFixed(3)}`,
  );
  const wf = await api.waveform(a.id);
  await report(`waveform A: ${wf ? `${wf.peak.length} points @ ${wf.pointsPerSec.toFixed(1)}/s` : 'missing'}`);
  const rafFrames = ticker.frameCount;
  for (let i = 0; i < 30; i++) {
    ticker.stepOnce();
    await wait(20);
  }
  await report(`raf frames before manual stepping: ${rafFrames}; document.visibilityState ${document.visibilityState}`);
  await report(`canvases: ${canvasCoverage()}; frames ${ticker.frameCount}; frame errors ${JSON.stringify(ticker.frameErrors.slice(0, 3))}; window errors ${JSON.stringify(windowErrors.slice(0, 3))}`);
  const ns = await nativeFrameStats();
  await report(`native: ${native.available ? `on ${ns?.info.backend} ${ns?.frames.refreshHz.toFixed(0)}Hz p50 ${ns?.frames.p50Ms.toFixed(2)}ms p99 ${ns?.frames.p99Ms.toFixed(2)}ms dropped ${ns?.frames.dropped}/${ns?.frames.frames}` : 'off'}`);
  await api.engine({ type: 'beatFxKind', kind: 'spiral' });
  await api.engine({ type: 'beatFxTarget', target: 'master' });
  await api.engine({ type: 'beatFxLevel', value: 0.5 });
  await api.engine({ type: 'beatFxEnabled', enabled: true });
  await wait(1500);
  const fx = engine.snap;
  await report(`beat fx ${fx.beatFx.kind} on=${fx.beatFx.enabled} target ${fx.beatFx.target} bpm ${fx.beatFx.bpm.toFixed(1)} master meter ${fx.master.meterL.toFixed(3)} limiting ${fx.master.limiting}`);
  await api.engine({ type: 'beatFxEnabled', enabled: false });
  const ph = await api.phrases(a.id);
  await report(`phrases A: ${ph.map((p) => `${p.kind}:${p.bars}`).join(' ')}; key A: ${a.key ?? '—'} ${a.keyName ?? ''}`);
  const text = document.body.innerText;
  await report(`dom: deck titles visible=${text.includes(a.title) && text.includes(b.title)}, buttons=${document.querySelectorAll('button').length}`);
  await api.engine({ type: 'hotCue', deck: 0, slot: 0, pressed: true });
  await api.engine({ type: 'hotCue', deck: 0, slot: 0, pressed: false });
  await wait(400);
  await report(`hot cue 1 on A: ${JSON.stringify(engine.snap.decks[0].hotCues[0])}`);
  await runFxSelfTest(report);
  await api.engine({ type: 'pause', deck: 0 });
  await api.engine({ type: 'pause', deck: 1 });
  await wait(300);
  await report(`done: paused A=${!engine.snap.decks[0].playing} B=${!engine.snap.decks[1].playing}`);
}
