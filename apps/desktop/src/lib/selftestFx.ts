import { api, EVENTS, type ControllerAppAction, type FxControlState } from '$lib/api';
import * as be from '$lib/api/backend';
import { engine } from '$lib/state/engine.svelte';

type Report = (message: string) => Promise<unknown>;

const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** Drives DDJ-400 FX control through the real controller mapping using the official MIDI message list. */
export async function runFxSelfTest(report: Report): Promise<void> {
  const seen: { fx: FxControlState | null } = { fx: null };
  const off = await be.on<ControllerAppAction>(EVENTS.controllerAction, (a) => {
    if (a.type === 'fxControl') seen.fx = a;
  });
  const send = async (...msgs: number[][]) => {
    for (const m of msgs) await api.midiInject(m);
    await wait(250);
  };
  const ok = (cond: boolean) => (cond ? 'ok' : 'FAIL');
  try {
    await api.engine({ type: 'beatFxKind', kind: 'echo' });
    await api.engine({ type: 'beatFxTarget', target: 'master' });
    await api.engine({ type: 'beatFxEnabled', enabled: false });
    await api.midiClearLog();

    await send([0x90, 0x6d, 0x7f], [0x90, 0x6d, 0x00]);
    const log = await api.midiLog();
    const pressAt = log.find((e) => e.direction === 'injected' && e.bytes.join() === '144,109,127')?.timeMs;
    const ledAt = log.find((e) => e.direction === 'out' && e.bytes.join() === '144,109,127')?.timeMs;
    const lag = pressAt !== undefined && ledAt !== undefined ? ledAt - pressAt : null;
    await report(`fx pad mode: ${ok(seen.fx?.padModes[0] === 'padFx1')} padModes ${JSON.stringify(seen.fx?.padModes)}, LED lag ${engine.controller?.connected ? `${lag} ms ${ok(lag !== null && lag < 20)}` : 'n/a (no controller connected)'}`);
    const cuesBefore = JSON.stringify(engine.snap.decks[0].hotCues);
    await send([0x97, 0x01, 0x7f], [0x97, 0x01, 0x00]);
    await report(`fx stale hot cue note ignored: ${ok(seen.fx?.padModes[0] === 'padFx1' && JSON.stringify(engine.snap.decks[0].hotCues) === cuesBefore)}`);
    const grid = document.body.innerText;
    await report(`fx pad grid on screen: ${ok(grid.includes('Echo Out') && grid.includes('Vinyl Brake'))}`);

    await send([0x97, 0x10, 0x7f]);
    const held = engine.snap.padFx[0];
    await report(`fx pad 1 held: ${ok(held.voice?.kind === 'echo' && held.sounding)} voice ${JSON.stringify(held.voice)}`);
    await send([0x94, 0x4b, 0x7f], [0x94, 0x4b, 0x00]);
    await report(`fx held pad + BEAT right: ${ok(engine.snap.padFx[0].voice?.beats === 0.75)} beats ${engine.snap.padFx[0].voice?.beats}`);
    await send([0x97, 0x10, 0x00]);
    await report(`fx pad 1 released: ${ok(engine.snap.padFx[0].voice === null)}`);
    await send([0x97, 0x16, 0x7f], [0x97, 0x16, 0x00]);
    await report(`fx pad 7 vinyl brake: ${ok(engine.snap.padFx[0].release === 'vinylBrake')} progress ${engine.snap.padFx[0].releaseProgress.toFixed(2)}`);
    await wait(2500);
    await report(`fx vinyl brake finished: ${ok(engine.snap.padFx[0].release === null)}`);

    await send([0x90, 0x3f, 0x7f], [0x94, 0x6b, 0x7f], [0x94, 0x6b, 0x00]);
    await report(`fx SHIFT + BEAT right pages once: ${ok(seen.fx?.focus === 1)} focus ${seen.fx?.focus}, shift ${seen.fx?.shift}`);
    await send([0x94, 0x66, 0x7f], [0x94, 0x66, 0x00]);
    await send([0xb4, 0x02, 0x7f], [0xb4, 0x22, 0x00]);
    const waiting = seen.fx?.pickup;
    await report(`fx SHIFT + LEVEL far: pickup ${ok(waiting?.target === 'param')} ${JSON.stringify(waiting)}, feedback ${engine.snap.beatFx.params[0]}`);
    await send([0xb4, 0x02, 0x35], [0xb4, 0x22, 0x40], [0xb4, 0x02, 0x50], [0xb4, 0x22, 0x00]);
    await report(`fx SHIFT + LEVEL engaged: ${ok(Math.abs(engine.snap.beatFx.params[0] - 63) < 2)} feedback ${engine.snap.beatFx.params[0].toFixed(1)}, level ${engine.snap.beatFx.level.toFixed(2)}`);
    await send([0x90, 0x3f, 0x00]);

    await api.engine({ type: 'releaseFxConfig', kind: 'echoOut', beats: 2 });
    await api.engine({ type: 'beatFxEnabled', enabled: true });
    await wait(500);
    await send([0x90, 0x3f, 0x7f], [0x94, 0x43, 0x7f], [0x94, 0x43, 0x00], [0x90, 0x3f, 0x00]);
    const rel = engine.snap;
    await report(`fx SHIFT + ON/OFF release: ${ok(!rel.beatFx.enabled && rel.releaseFx.running === 'echoOut')} running ${rel.releaseFx.running} progress ${rel.releaseFx.progress.toFixed(2)}`);
    await wait(2000);
    await report(`fx release finished: ${ok(engine.snap.releaseFx.running === null)} master meter ${engine.snap.master.meterL.toFixed(3)}`);

    await send([0x90, 0x6b, 0x7f], [0x90, 0x6b, 0x00], [0x97, 0x51, 0x7f]);
    await report(`fx PAD FX 2 gate: ${ok(engine.snap.beatFx.enabled && engine.snap.beatFx.beats === 0.25)} beats ${engine.snap.beatFx.beats}`);
    await send([0x97, 0x51, 0x00]);
    await report(`fx PAD FX 2 gate closed: ${ok(!engine.snap.beatFx.enabled)}`);

    await send([0xb6, 0x17, 0x20]);
    await report(`fx channel 1 Color FX knob: ${ok(engine.snap.channels[0].filter < -0.2)} filter ${engine.snap.channels[0].filter.toFixed(2)}`);
    await send([0x90, 0x1b, 0x7f], [0x90, 0x1b, 0x00]);
    await report(`fx back to HOT CUE: ${ok(seen.fx?.padModes[0] === 'hotCue')}`);
  } finally {
    off();
  }
}
