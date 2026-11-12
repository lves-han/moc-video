import { test } from 'node:test';
import assert from 'node:assert/strict';
import { loadFontBytes, type FontSpec, type FontStore } from '../src/ui/fontCache.ts';
const bytes = Uint8Array.from([0, 1, 2, 3]).buffer;
const sha256 = Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256', bytes)), n => n.toString(16).padStart(2, '0')).join('');
const font: FontSpec = { id: 'test', label: 'Test', family: 'Test', url: 'https://example.test/font', weights: '100 900', bytes: 4, sha256 };
function cache() {
  const files = new Map<string, ArrayBuffer>();
  const store: FontStore = { get: async key => files.get(key), put: async (key, data) => { files.set(key, data); } };
  return { files, store };
}
test('first use downloads, verifies and commits; later offline loads do not request the network', async () => {
  const { store } = cache();
  let requests = 0;
  const download = (async () => { requests++; return new Response(bytes); }) as typeof fetch;
  assert.equal((await loadFontBytes(font, store, download)).source, 'download');
  assert.equal(requests, 1);
  const cached = await loadFontBytes(font, store, (async () => { throw Error('offline'); }) as typeof fetch);
  assert.equal(cached.source, 'local'); assert.deepEqual(cached.bytes, bytes);
});
test('corrupt local font is replaced only by a verified download', async () => {
  const { store, files } = cache(); files.set(`${font.id}:${sha256}`, new Uint8Array([9]).buffer);
  assert.equal((await loadFontBytes(font, store, (async () => new Response(bytes)) as typeof fetch)).source, 'download');
  assert.deepEqual(files.get(`${font.id}:${sha256}`), bytes);
});
test('HTTP failure, altered bytes, oversized bytes and persistence errors are surfaced', async () => {
  for (const response of [new Response('', { status: 503 }), new Response(new Uint8Array([9, 9, 9, 9])), new Response(new Uint8Array(5))]) {
    const { store, files } = cache();
    await assert.rejects(loadFontBytes(font, store, (async () => response) as typeof fetch));
    assert.equal(files.size, 0);
  }
  const store: FontStore = { get: async () => undefined, put: async () => { throw Error('disk full'); } };
  await assert.rejects(loadFontBytes(font, store, (async () => new Response(bytes)) as typeof fetch), /disk full/);
});
