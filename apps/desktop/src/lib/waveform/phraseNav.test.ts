// @ts-nocheck
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { alignJumpBeats, barsBeats, classify, countdown, finalBar, nextPhraseJumpBeats, wallSecsToNext } from './phraseNav.ts';

const fixtures = JSON.parse(readFileSync(new URL('../../../../../crates/gfx/fixtures/phrase_nav.json', import.meta.url), 'utf8'));

test('matches the shared fixtures used by the Rust helper', () => {
  for (const c of fixtures.cases) {
    const grid = { bpm: c.bpm, firstBeatSecs: c.firstBeatSecs };
    for (const m of c.marks) {
      const phrase = m.kind === 'phrase' ? c.phrases.findIndex((p) => Math.abs(p.startSecs - m.secs) < 1e-6) : -1;
      const got = classify({ secs: m.secs, kind: m.kind, bar: m.bar, phrase }, c.phrases, grid);
      assert.deepEqual([got.level, got.barInPhrase, got.phraseBars], [m.level, m.barInPhrase, m.phraseBars], `${c.name} @${m.secs}`);
    }
    for (const e of c.countdowns) {
      const got = countdown(c.phrases, grid, e.secs);
      assert.deepEqual([got.next, got.remainingBeats], [e.next, e.remainingBeats], `${c.name} @${e.secs}`);
      assert.ok(Math.abs(got.progress - e.progress) < 1e-9);
    }
  }
});

const BAR = 2;
const grid = { bpm: 120, firstBeatSecs: 0 };
const arrangement = (bars) => {
  let t = 0;
  return bars.map((b) => {
    const p = { startSecs: t * BAR, endSecs: (t + b) * BAR };
    t += b;
    return p;
  });
};

test('the countdown reads bars.beats and flags the final bar', () => {
  const ps = arrangement([8, 32, 6]);
  const c = countdown(ps, grid, 38 * BAR - 0.25);
  assert.equal(barsBeats(c), '2.1');
  assert.equal(finalBar(c), false);
  assert.equal(finalBar(countdown(ps, grid, 39.5 * BAR)), true);
});

test('align jumps the incoming deck by whole beats onto the master phrase start', () => {
  const master = { phrases: arrangement([16, 16]), pos: 12 * BAR, rate: 1 };
  const incoming = { phrases: arrangement([8, 8, 8]), pos: 2 * BAR + 0.05, rate: 1, bpm: 120, duration: 60 };
  const jump = alignJumpBeats(incoming, master);
  assert.equal(jump, 8);
  const after = { ...incoming, pos: incoming.pos + jump * 0.5 };
  const lands = wallSecsToNext(after.phrases, after.pos, 1).secs;
  assert.ok(Math.abs(lands - wallSecsToNext(master.phrases, master.pos, 1).secs + 0.05) < 1e-9, 'beat phase is kept');
});

test('align converts the master lead time through both tempos', () => {
  const master = { phrases: arrangement([16, 16]), pos: 14 * BAR, rate: 1 };
  const incoming = { phrases: arrangement([8, 8]), pos: 0, rate: 2, bpm: 120, duration: 60 };
  assert.equal(alignJumpBeats(incoming, master), 16);
});

test('jump to the next phrase keeps the position inside the bar', () => {
  const ps = arrangement([8, 8]);
  assert.equal(nextPhraseJumpBeats(ps, grid, 5 * BAR + 0.5), 12);
  assert.equal(nextPhraseJumpBeats(ps, grid, 7 * BAR + 1.9), 4);
  assert.equal(nextPhraseJumpBeats(ps, grid, 12 * BAR), null);
});
