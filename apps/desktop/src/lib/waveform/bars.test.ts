// @ts-nocheck
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { barPosition, gridLines, phaseOffset } from './bars.ts';

const BPM = 128;
const BEAT = 60 / BPM;
const BAR = BEAT * 4;
const grid = { bpm: BPM, firstBeatSecs: 0 };
const near = (a: number, b: number, eps = 1e-6) => assert.ok(Math.abs(a - b) < eps, `${a} vs ${b}`);

const intro = { startSecs: 0, endSecs: 16 * BAR };
const fill = { startSecs: 16 * BAR, endSecs: 18 * BAR };
const drop = { startSecs: 18 * BAR, endSecs: 50 * BAR };
const brk = { startSecs: 50 * BAR, endSecs: 66 * BAR + 2 * BEAT };
const drop2 = { startSecs: 66 * BAR + 2 * BEAT, endSecs: 98 * BAR + 2 * BEAT };
const odd = [intro, fill, drop, brk, drop2];

const bars = (lines) => lines.filter((l) => l.kind !== 'beat');

test('a phrase start is a prominent 4-bar line even when it is not a multiple of 16 beats from the track start', () => {
  const lines = bars(gridLines(odd, grid, drop.startSecs - 1e-6, drop.startSecs + 17 * BAR));
  near(lines[0].secs, drop.startSecs);
  assert.equal(lines[0].kind, 'phrase');
  assert.equal(lines[0].phrase, 2);
  const fours = lines.filter((l) => l.kind === 'four');
  assert.deepEqual(
    fours.map((l) => Math.round(((l.secs - drop.startSecs) / BEAT) * 100) / 100),
    [16, 32, 48, 64],
  );
});

test('4-bar lines count from the phrase start, not from the track start', () => {
  const lattice = 20 * BAR;
  const line = gridLines(odd, grid, lattice - 1e-6, lattice + 1e-6).find((l) => l.kind !== 'beat');
  assert.equal(line.kind, 'bar');
});

test('bar lines run every four beats on the global bar phase', () => {
  const lines = bars(gridLines(odd, grid, fill.startSecs, drop.startSecs + 3 * BAR));
  assert.deepEqual(
    lines.map((l) => l.kind),
    ['phrase', 'bar', 'phrase', 'bar', 'bar', 'bar'],
  );
  lines.slice(2).forEach((l, i) => near(l.secs, drop.startSecs + i * BAR));
});

test('bar numbers are global and count from the first downbeat', () => {
  const lines = bars(gridLines(odd, grid, 14 * BAR, 22 * BAR));
  assert.deepEqual(
    lines.map((l) => l.bar),
    [15, 16, 17, 18, 19, 20, 21, 22, 23],
  );
});

test('an off-bar phrase start snaps to the next downbeat instead of shifting the bars', () => {
  const lines = bars(gridLines(odd, grid, drop2.startSecs - 1e-6, drop2.startSecs + 8 * BAR));
  lines.forEach((l, i) => near(l.secs, 67 * BAR + i * BAR));
  assert.deepEqual(
    lines.slice(0, 2).map((l) => [l.kind, l.bar, l.phrase]),
    [
      ['phrase', 68, 4],
      ['bar', 69, -1],
    ],
  );
});

test('a phrase that is not a whole number of 4-bar groups ends its groups at the next phrase', () => {
  const lines = bars(gridLines(odd, grid, brk.startSecs, 67 * BAR + 1e-6));
  assert.deepEqual(
    lines.filter((l) => l.kind !== 'bar').map((l) => [l.kind, l.bar]),
    [
      ['phrase', 51],
      ['four', 55],
      ['four', 59],
      ['four', 63],
      ['four', 67],
      ['phrase', 68],
    ],
  );
});

test('beat ticks follow the grid and skip bar lines', () => {
  const lines = gridLines(odd, grid, drop.startSecs, drop.startSecs + BAR - 1e-6);
  assert.deepEqual(
    lines.map((l) => l.kind),
    ['phrase', 'beat', 'beat', 'beat'],
  );
  lines.slice(1).forEach((l, i) => near(l.secs, drop.startSecs + (i + 1) * BEAT));
});

test('beat ticks stay on the grid even when a phrase start sits off it', () => {
  const shifted = [{ startSecs: 10.3 * BEAT, endSecs: 100 }];
  const lines = gridLines(shifted, { bpm: BPM, firstBeatSecs: 0 }, 10 * BEAT, 12 * BEAT);
  const ticks = lines.filter((l) => l.kind === 'beat');
  assert.deepEqual(
    ticks.map((l) => Math.round(l.secs / BEAT)),
    [10, 11],
  );
});

test('before the first phrase the 4-bar groups count back from it', () => {
  const phrases = [{ startSecs: 2 * BAR, endSecs: 100 }];
  const lines = bars(gridLines(phrases, grid, -2 * BAR, 3 * BAR));
  assert.deepEqual(
    lines.map((l) => [l.kind, l.bar]),
    [
      ['four', -1],
      ['bar', 0],
      ['bar', 1],
      ['bar', 2],
      ['phrase', 3],
      ['bar', 4],
    ],
  );
});

test('a pickup intro keeps bar 1 on the first downbeat of the grid', () => {
  const g = { bpm: BPM, firstBeatSecs: 3 * BEAT };
  const intro = { startSecs: 0, endSecs: 3 * BEAT + 8 * BAR };
  const lines = bars(gridLines([intro], g, 0, 2 * BAR));
  assert.deepEqual(
    lines.map((l) => [l.kind, l.bar, Math.round(l.secs / BEAT)]),
    [
      ['phrase', 1, 3],
      ['bar', 2, 7],
    ],
  );
  const pickup = barPosition([intro], g, BEAT + 0.01);
  assert.deepEqual([pickup.bar, pickup.beat], [0, 3]);
});

test('without phrases bars count from the first downbeat of the grid', () => {
  const g = { bpm: 120, firstBeatSecs: 0.18 };
  const lines = bars(gridLines([], g, 0, 10));
  assert.equal(lines[0].kind, 'four');
  near(lines[0].secs, 0.18);
  assert.equal(lines[0].bar, 1);
  assert.deepEqual(
    lines.filter((l) => l.kind === 'four').map((l) => l.bar),
    [1, 5],
  );
});

test('an invalid grid draws nothing', () => {
  assert.deepEqual(gridLines(odd, { bpm: 0, firstBeatSecs: 0 }, 0, 10), []);
  assert.equal(barPosition(odd, { bpm: 0, firstBeatSecs: 0 }, 5), null);
});

test('bar position counts from the phrase start', () => {
  const p = barPosition(odd, grid, drop.startSecs + 5.5 * BEAT);
  assert.equal(p.bar, 19 + 1);
  assert.equal(p.beat, 2);
  near(p.fraction, 0.5);
  assert.equal(p.toFourBar, 11);
  near(p.beatsSinceAnchor, 5.5);
});

test('beats to the next 4-bar line stop at the next phrase start', () => {
  const p = barPosition(odd, grid, fill.startSecs + 1.5 * BEAT);
  assert.equal(p.bar, 17);
  assert.equal(p.beat, 2);
  assert.equal(p.toFourBar, 7);
});

test('position agrees with the drawn lines', () => {
  for (const l of bars(gridLines(odd, grid, 10, 120))) {
    const p = barPosition(odd, grid, l.secs + 1e-4);
    assert.equal(p.bar, l.bar, `bar at ${l.secs}`);
    assert.equal(p.beat, 1);
    if (l.kind === 'phrase' || l.kind === 'four') assert.ok(p.toFourBar >= 15 || p.toFourBar >= 1);
  }
});

test('phase offset uses beats counted from each deck own phrase start', () => {
  const a = barPosition(odd, grid, drop.startSecs + 2 * BEAT).beatsSinceAnchor;
  const b = barPosition(odd, grid, 67 * BAR + 2.25 * BEAT).beatsSinceAnchor;
  const o = phaseOffset(a, b);
  near(o.beats, 0.25);
  assert.equal(o.bars, 0);
});

test('shared fixtures describe the same lines', () => {
  const fixtures = JSON.parse(readFileSync(new URL('../../../../../crates/gfx/fixtures/bar_lines.json', import.meta.url), 'utf8'));
  assert.ok(fixtures.cases.length > 0);
  for (const c of fixtures.cases) {
    const got = gridLines(c.phrases, { bpm: c.bpm, firstBeatSecs: c.firstBeatSecs }, c.fromSecs, c.toSecs);
    assert.equal(got.length, c.lines.length, `${c.name}: line count`);
    got.forEach((l, i) => {
      near(l.secs, c.lines[i].secs, 1e-9);
      assert.equal(l.kind, c.lines[i].kind, `${c.name} #${i}`);
      assert.equal(l.bar, c.lines[i].bar, `${c.name} #${i}`);
      assert.equal(l.phrase, c.lines[i].phrase, `${c.name} #${i}`);
    });
    for (const p of c.positions) {
      const got = barPosition(c.phrases, { bpm: c.bpm, firstBeatSecs: c.firstBeatSecs }, p.secs);
      assert.equal(got.bar, p.bar, `${c.name} bar @${p.secs}`);
      assert.equal(got.beat, p.beat, `${c.name} beat @${p.secs}`);
      assert.equal(got.toFourBar, p.toFourBar, `${c.name} to4 @${p.secs}`);
      near(got.beatsSinceAnchor, p.beatsSinceAnchor, 1e-9);
    }
  }
});
