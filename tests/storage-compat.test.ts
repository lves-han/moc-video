import assert from 'node:assert/strict';
import test from 'node:test';
import { readProductStorage } from '../src/storage-compat.ts';

test('the renamed product reads existing preferences and favors new values', () => {
  const rows = new Map([['yingxiang-video.lines.v1', 'saved-lines']]);
  const store = { getItem: (key: string) => rows.get(key) ?? null };
  assert.equal(readProductStorage(store, 'moc-video.lines.v1'), 'saved-lines');
  rows.set('moc-video.lines.v1', 'new-lines');
  assert.equal(readProductStorage(store, 'moc-video.lines.v1'), 'new-lines');
  assert.equal(rows.get('yingxiang-video.lines.v1'), 'saved-lines');
  assert.equal(readProductStorage(store, 'another-key'), null);
});
