// @ts-nocheck
import test from 'node:test';
import assert from 'node:assert/strict';
import { around } from './around.ts';

const ORDER = ['delay', 'echo', 'upEcho', 'downEcho', 'pingPong', 'spiral', 'reverb', 'flanger', 'phaser', 'roll', 'trans', 'filterSweep', 'pulseVerb', 'dubTape', 'halo', 'ascend', 'backwash'];

test('centres the current effect between its FX SELECT neighbours', () => {
  const list = around(ORDER, ORDER.indexOf('reverb'), 3);
  assert.deepEqual(list.map((k) => k.item), ['downEcho', 'pingPong', 'spiral', 'reverb', 'flanger', 'phaser', 'roll']);
  assert.equal(list.find((k) => k.offset === 0).item, 'reverb');
});

test('wraps around both ends like the hardware', () => {
  assert.deepEqual(around(ORDER, 0, 3).map((k) => k.item), ['halo', 'ascend', 'backwash', 'delay', 'echo', 'upEcho', 'downEcho']);
  assert.deepEqual(around(ORDER, ORDER.length - 1, 1).map((k) => k.item), ['ascend', 'backwash', 'delay']);
});
