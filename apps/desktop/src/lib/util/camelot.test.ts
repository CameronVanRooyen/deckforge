// @ts-nocheck
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { effectiveKey, keyCompat, parseCamelot, semitoneShift, shiftCamelot } from './camelot.ts';

test('parses camelot keys', () => {
  assert.deepEqual(parseCamelot('8A'), { n: 8, letter: 'A' });
  assert.deepEqual(parseCamelot('12b'), { n: 12, letter: 'B' });
  assert.equal(parseCamelot('13A'), null);
  assert.equal(parseCamelot('Am'), null);
  assert.equal(parseCamelot(null), null);
});

test('semitone shift from tempo ratio', () => {
  assert.equal(semitoneShift(1), 0);
  assert.equal(semitoneShift(1.059463), 1);
  assert.equal(semitoneShift(0.943874), -1);
  assert.equal(semitoneShift(1.02), 0);
  assert.equal(semitoneShift(1.16), 3);
});

test('camelot shifts seven steps per semitone', () => {
  assert.equal(shiftCamelot('8A', 1), '3A');
  assert.equal(shiftCamelot('8A', -1), '1A');
  assert.equal(shiftCamelot('8A', 2), '10A');
  assert.equal(shiftCamelot('8B', 12), '8B');
  assert.equal(shiftCamelot('1A', -1), '6A');
});

test('key lock keeps the original key', () => {
  assert.deepEqual(effectiveKey('8A', 1.1, true), { key: '8A', semitones: 0 });
  assert.deepEqual(effectiveKey('8A', 1.0595, false), { key: '3A', semitones: 1 });
  assert.equal(effectiveKey(null, 1, false), null);
});

test('compatibility classes', () => {
  assert.equal(keyCompat('8A', '8A'), 'same');
  assert.equal(keyCompat('8A', '9A'), 'adjacent');
  assert.equal(keyCompat('1A', '12A'), 'adjacent');
  assert.equal(keyCompat('8A', '8B'), 'relative');
  assert.equal(keyCompat('8A', '9B'), null);
  assert.equal(keyCompat('8A', '10A'), null);
  assert.equal(keyCompat(null, '8A'), null);
});
