import assert from 'node:assert/strict';
import test from 'node:test';

type Row = { key: string; savedAt: number; [key: string]: unknown };
function fixture(blocked = false) {
  const legacy = new Map<string, string>();
  const native = new Map<string, Row>();
  let deleted = false, ready = 0, attempts = 0;
  const idb = { deleteDatabase() {
    const request: { onsuccess?: () => void; onblocked?: () => void } = {};
    queueMicrotask(() => { attempts++; if (blocked) request.onblocked?.(); else { deleted = true; request.onsuccess?.(); } });
    return request;
  } };
  const storage = { getItem: (k: string) => legacy.get(k) ?? null,
    setItem: (k: string, v: string) => { legacy.set(k, v); }, removeItem: (k: string) => { legacy.delete(k); } };
  const core = { invoke: async (_command: string, args: { action: string; store: string; key: string; row: Row }) => {
    const id = `${args.store}:${args.key}`;
    if (args.action === 'ready') { ready++; return null; }
    if (args.action === 'write') { native.set(id, args.row); return null; }
    if (args.action === 'read') return native.get(id) ?? null;
    if (args.action === 'delete') { native.delete(id); return null; }
    return [...native.entries()].filter(([key]) => key.startsWith(`${args.store}:`)).map(([, row]) => row);
  } };
  Object.defineProperty(globalThis, 'indexedDB', { configurable: true, value: idb });
  Object.defineProperty(globalThis, 'localStorage', { configurable: true, value: storage });
  Object.defineProperty(globalThis, '__TAURI__', { configurable: true, value: { core } });
  return { legacy, native, deleted: () => deleted, ready: () => ready, attempts: () => attempts, unblock: () => { blocked = false; } };
}
function cleanup() {
  Reflect.deleteProperty(globalThis, 'indexedDB'); Reflect.deleteProperty(globalThis, 'localStorage'); Reflect.deleteProperty(globalThis, '__TAURI__');
}

test('legacy WebView cache is deleted before concurrent native writes', async () => {
  const now = Date.now(), f = fixture();
  f.legacy.set('moc-video.catalog.v1', JSON.stringify([{ key: 'old', savedAt: now }]));
  f.legacy.set('moc-video.metadata.v1', JSON.stringify([{ key: 'old', savedAt: now }]));
  f.legacy.set('moc-video.source-checks.v1', '{"old":1}');
  f.legacy.set('unrelated', 'keep');
  try {
    const cache = await import('../src/data/indexed-cache.ts?cleanup-success');
    await Promise.all(Array.from({ length: 20 }, (_, n) => cache.cacheWrite('details', { key: String(n), savedAt: now })));
    assert.equal(f.deleted(), true); assert.equal(f.ready(), 1); assert.equal(f.attempts(), 1);
    assert.equal(f.legacy.has('moc-video.catalog.v1'), false); assert.equal(f.legacy.has('moc-video.metadata.v1'), false);
    assert.equal(f.legacy.has('moc-video.source-checks.v1'), false);
    assert.equal(f.legacy.get('unrelated'), 'keep'); assert.equal((await cache.cacheRows('details')).length, 20);
    assert.equal(await cache.cacheRead('catalog', 'old'), null);
  } finally { cleanup(); }
});

test('blocked legacy cleanup prevents native writes and can be retried', async () => {
  const f = fixture(true);
  f.legacy.set('moc-video.catalog.v1', JSON.stringify([{ key: 'old', savedAt: Date.now() }]));
  try {
    const cache = await import('../src/data/indexed-cache.ts?cleanup-blocked');
    await assert.rejects(cache.cacheWrite('catalog', { key: 'new', savedAt: Date.now() }), /cleanup blocked/);
    assert.equal(f.ready(), 0); assert.equal(f.native.size, 0); assert.equal(f.deleted(), false);
    f.unblock();
    await cache.cacheWrite('catalog', { key: 'new', savedAt: Date.now() });
    assert.equal(f.deleted(), true); assert.equal(f.ready(), 1); assert.equal(f.native.size, 1);
  } finally { cleanup(); }
});

test('source check records use the shared native cache and survive restart', async () => {
  const f = fixture();
  try {
    const { sourceChecks } = await import('../src/source-health.ts?native');
    const first = sourceChecks();
    first.mark('source', 42, true);
    await new Promise(resolve => setTimeout(resolve, 0));
    await new Promise(resolve => setTimeout(resolve, 0));
    assert.ok(f.native.has('details:source-health'));
    const reopened = sourceChecks();
    await new Promise(resolve => setTimeout(resolve, 0));
    await new Promise(resolve => setTimeout(resolve, 0));
    assert.equal(reopened.has('source'), true);
    assert.equal(reopened.failed('source'), true);
  } finally { cleanup(); }
});
