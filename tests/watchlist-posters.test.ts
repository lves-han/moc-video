import test from 'node:test';
import assert from 'node:assert/strict';
import { loadWatchlistPosters } from '../src/data/watchlist-posters.ts';

test('watchlist posters load concurrently with a limit and complete despite a failed title', async () => {
  let active = 0, peak = 0; const started: number[] = [], finished: number[] = [], release: (() => void)[] = [];
  const work = loadWatchlistPosters([0, 1, 2, 3, 4, 5], () => true, async entry => {
    started.push(entry); peak = Math.max(peak, ++active);
    await new Promise<void>(resolve => release[entry] = resolve);
    try { if (entry === 1) throw Error('unavailable'); } catch { /* one missing poster does not stop the others */ }
    finished.push(entry); active--;
  });
  assert.deepEqual(started, [0, 1, 2, 3]);
  release[1]!(); await new Promise(resolve => setTimeout(resolve, 0));
  assert.deepEqual(started, [0, 1, 2, 3, 4]);
  release[0]!(); await new Promise(resolve => setTimeout(resolve, 0));
  release[2]!(); release[3]!(); release[4]!(); release[5]!();
  await work;
  assert.equal(peak, 4); assert.equal(finished.length, 6);
});

test('watchlist stops scheduling new posters when the view changes', async () => {
  let active = true; const started: number[] = [], release: (() => void)[] = [];
  const work = loadWatchlistPosters([0, 1, 2, 3, 4], () => active, async entry => {
    started.push(entry); await new Promise<void>(resolve => release[entry] = resolve);
  });
  active = false; for (const resolve of release) resolve();
  await work; assert.deepEqual(started, [0, 1, 2, 3]);
});
