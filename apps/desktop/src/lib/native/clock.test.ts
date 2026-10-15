// @ts-nocheck
import test from 'node:test';
import assert from 'node:assert/strict';
import { SmoothClock } from './clock.ts';

const sample = (pos: number, rate: number, at: number, moving = true) => ({ pos, rate, moving, token: 1, at });

test('advances smoothly between sparse snapshots', () => {
  const c = new SmoothClock();
  c.push(sample(10, 1, 0));
  let prev = c.position(0);
  for (let i = 1; i <= 120; i++) {
    const p = c.position(i / 120);
    assert.ok(Math.abs(p - prev - 1 / 120) < 1e-9);
    prev = p;
  }
});

test('noisy snapshot timing never moves backwards', () => {
  const c = new SmoothClock();
  let seed = 12345;
  const rnd = () => ((seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0) >>> 8) / 2 ** 24;
  const truth = (t: number) => 30 + t * 1.04;
  let prev = -Infinity;
  let next = 0;
  let worst = 0;
  for (let i = 0; i < 2400; i++) {
    const now = i / 240;
    if (now >= next) {
      const stamped = Math.max(now + (rnd() - 0.5) * 0.004, 0);
      c.push(sample(truth(stamped), 1.04, stamped));
      next += 0.003 + rnd() * 0.004;
    }
    const p = c.position(now);
    assert.ok(p >= prev, `backwards at ${i}`);
    prev = p;
    if (now > 0.5) worst = Math.max(worst, Math.abs(p - truth(now)));
  }
  assert.ok(worst < 0.006, `worst ${worst}`);
});

test('seeks snap and pauses freeze', () => {
  const c = new SmoothClock();
  c.push(sample(5, 1, 0));
  c.position(0);
  c.position(0.01);
  c.push(sample(60, 1, 0.02));
  assert.ok(Math.abs(c.position(0.02) - 60) < 1e-9);
  c.push(sample(61, 1, 0.1, false));
  assert.equal(c.position(0.3), 61);
});

test('drag pins, then hands back to the engine without a jump', () => {
  const c = new SmoothClock();
  c.push(sample(10, 1, 0, false));
  assert.equal(c.position(0), 10);
  c.setDrag(42, 0.1);
  assert.equal(c.position(0.11), 42);
  c.setDrag(null, 0.14);
  assert.equal(c.position(0.15), 42);
  c.push(sample(42, 1, 0.17, false));
  assert.equal(c.position(0.18), 42);
  assert.equal(c.settling, false);
});
