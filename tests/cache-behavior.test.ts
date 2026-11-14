import assert from 'node:assert/strict';
import test from 'node:test';
import { metadataCache, METADATA_TTL } from '../src/metadata-cache.ts';
import { sourceChecks, SOURCE_CHECK_INTERVAL } from '../src/source-health.ts';
import { catalogCache } from '../src/catalog-cache.ts';
import { BackgroundRefresh } from '../src/background-refresh.ts';

function storage() {
  const rows = new Map<string, string>();
  return { rows, getItem: (key: string) => rows.get(key) ?? null, setItem: (key: string, value: string) => { rows.set(key, value); } };
}

test('source checks survive restart and become due after four hours', () => {
  const store = storage(), first = sourceChecks(() => store), start = 1000;
  assert.equal(first.has('one'), false);
  first.mark('one', start);
  const reopened = sourceChecks(() => store);
  assert.equal(reopened.due('one', start + SOURCE_CHECK_INTERVAL - 1), false);
  assert.equal(reopened.due('one', start + SOURCE_CHECK_INTERVAL), true);
  assert.equal(reopened.due('one', start - 1), true);
  reopened.mark('bad', start, true);
  const third = sourceChecks(() => store);
  assert.equal(third.failed('bad'), true);
  assert.equal(third.due('bad', start + SOURCE_CHECK_INTERVAL - 1), false);
  third.mark('bad', start + SOURCE_CHECK_INTERVAL);
  assert.equal(third.failed('bad'), false);
});

test('source check scheduling remains usable when storage is unavailable', () => {
  const checks = sourceChecks(() => { throw new Error('unavailable'); });
  checks.mark('one', 42);
  assert.equal(checks.due('one', 42 + SOURCE_CHECK_INTERVAL), true);
});

test('film details survive restart for seven days without saving gateway URLs', () => {
  const store = storage(), first = metadataCache(() => store);
  const ref = { source_id: 'source', source_revision: 'revision', data: '["stremio","film"]' };
  const value = { details: { ref, id: 'film', title: 'Film', groups: [], related: [], tags: [], kind: 'movie' },
    metadata_ref: { identity: { config_id: 'config', addon_key: 'addon', namespace: 'stremio', type: 'movie', meta_id: 'film', canonical_id: 'film' }, content_ref: ref },
    images: { poster: { url: 'http://127.0.0.1:1234/secret-capability' } } } as never;
  first.put(ref, 'zh-CN', 'host-one', value, 1000);
  assert.equal(first.get(ref, 'zh-CN', 'host-one', 1001)?.imagesReady, true);
  assert.equal([...store.rows.values()].join('').includes('secret-capability'), false);
  const reopened = metadataCache(() => store);
  const cached = reopened.get(ref, 'zh-CN', 'host-two', 1000 + METADATA_TTL - 1);
  assert.equal(cached?.value.details.title, 'Film');
  assert.deepEqual(cached?.value.images, {});
  assert.equal(cached?.stale, false);
  assert.equal(reopened.get(ref, 'zh-CN', 'host-two', 1000 + METADATA_TTL)?.stale, true);
  assert.deepEqual(reopened.due('zh-CN', 1000 + METADATA_TTL - 1), []);
  assert.deepEqual(reopened.due('zh-CN', 1000 + METADATA_TTL), [ref]);
  assert.equal(reopened.get({ ...ref, source_revision: 'changed' }, 'zh-CN', 'host-two'), null);
});

test('catalog rows reuse safe cards, isolate filters and refresh image access after restart', () => {
  const store = storage(), first = catalogCache(() => store);
  const source = { source_id: 's', source_revision: 'r' }, ref = { ...source, data: 'film' };
  const section = { id: 'section', title: 'Films', section_ref: 'old-host-handle', items: [{ id: 'film', title: 'Film', ref, kind: 'movie' }],
    page: 1, index: 0, has_next: true, next_page: 2 } as never;
  first.put(source, 'section', { genre: 'drama' }, 'en', 'host-one', section, { poster: { url: 'http://127.0.0.1/cap' } } as never, 1000);
  assert.equal(first.get(source, 'section', { genre: 'drama' }, 'en', 'host-one', 1001)?.hasNext, true);
  assert.equal([...store.rows.values()].join('').includes('127.0.0.1'), false);
  const reopened = catalogCache(() => store);
  const value = reopened.get(source, 'section', { genre: 'drama' }, 'en', 'host-two', 1001);
  assert.equal(value?.items[0]?.title, 'Film');
  assert.equal(value?.hasNext, false);
  assert.equal(value?.imagesReady, false);
  assert.equal(reopened.get(source, 'section', { genre: 'comedy' }, 'en', 'host-two'), null);
  assert.equal(reopened.get({ ...source, source_revision: 'other' }, 'section', { genre: 'drama' }, 'en', 'host-two'), null);
});

test('film details containing URLs stay in memory only', () => {
  const store = storage(), cache = metadataCache(() => store);
  const ref = { source_id: 's', source_revision: 'r', data: 'id' };
  const value = { details: { ref, title: 'Film', plot: 'https://example.invalid/?token=secret' }, metadata_ref: { identity: {}, content_ref: ref }, images: {} } as never;
  cache.put(ref, 'en', 'host', value);
  assert.equal(store.rows.size, 0);
  assert.equal(cache.get(ref, 'en', 'host')?.value.details.title, 'Film');
});

test('new URL-bearing data invalidates older persistent catalog and details', async () => {
  const store = storage(), source = { source_id: 's', source_revision: 'r' }, ref = { ...source, data: 'id' };
  const catalog = catalogCache(() => store);
  const section = (title: string) => ({ id: 'section', title: 'Films', section_ref: 'handle', items: [{ id: 'id', title, ref }], page: 1, has_next: false, next_page: null }) as never;
  await catalog.putPersistent(source, 'section', {}, 'en', 'host', section('old'), {}, 1000);
  await catalog.putPersistent(source, 'section', {}, 'en', 'host', section('https://example.invalid/new'), {}, 2000);
  assert.equal((await catalogCache(() => store).getPersistent(source, 'section', {}, 'en', 'other', 2001)), null);
  const details = metadataCache(() => store);
  const value = (title: string) => ({ details: { ref, id: 'id', title, groups: [], related: [], tags: [] },
    metadata_ref: { identity: { config_id: 'c', addon_key: 'a', namespace: 'n', type: 'movie', meta_id: 'id', canonical_id: 'id' }, content_ref: ref }, images: {} }) as never;
  await details.putPersistent(ref, 'en', 'host', value('old'), 1000);
  await details.putPersistent(ref, 'en', 'host', value('https://example.invalid/new'), 2000);
  assert.equal(await metadataCache(() => store).getPersistent(ref, 'en', 'other', 2001), null);
});

test('cache invalidation reports storage write failures', async () => {
  const base = storage(), source = { source_id: 's', source_revision: 'r' }, ref = { ...source, data: 'id' };
  let fail = false;
  const store = { getItem: base.getItem, setItem: (key: string, value: string) => { if (fail) throw new Error('storage denied'); base.setItem(key, value); } };
  const cache = catalogCache(() => store);
  const section = (title: string) => ({ id: 'section', title: 'Films', section_ref: 'handle', items: [{ id: 'id', title, ref }], page: 1, has_next: false, next_page: null }) as never;
  await cache.putPersistent(source, 'section', {}, 'en', 'host', section('old'), {}, 1000);
  fail = true;
  await assert.rejects(cache.putPersistent(source, 'section', {}, 'en', 'host', section('https://example.invalid/new'), {}, 2000), /storage denied/);
  assert.equal(cache.get(source, 'section', {}, 'en', 'host', 2001)?.items[0]?.title, 'https://example.invalid/new');
});

test('film details retain the latest 200 titles', () => {
  const store = storage(), cache = metadataCache(() => store);
  for (let index = 0; index <= 200; index++) {
    const ref = { source_id: 's', source_revision: 'r', data: `${index}` };
    const value = { details: { ref, id: `${index}`, title: 'Film', groups: [], related: [], tags: [], kind: 'movie' },
      metadata_ref: { identity: { config_id: 'c', addon_key: 'a', namespace: 'n', type: 'movie', meta_id: `${index}`, canonical_id: `${index}` }, content_ref: ref }, images: {} } as never;
    cache.put(ref, 'en', 'host', value, index);
  }
  const reopened = metadataCache(() => store);
  assert.equal(reopened.get({ source_id: 's', source_revision: 'r', data: '0' }, 'en', 'host'), null);
  assert.ok(reopened.get({ source_id: 's', source_revision: 'r', data: '1' }, 'en', 'host'));
  assert.ok(reopened.get({ source_id: 's', source_revision: 'r', data: '200' }, 'en', 'host'));
});

test('background refresh waits for idle and never runs two updates together', async t => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  let idle = false, running = 0, maximum = 0, finished = 0;
  let release: (() => void) | undefined;
  const queue = new BackgroundRefresh(() => idle);
  queue.enqueue('first', async () => { running++; maximum = Math.max(maximum, running); await new Promise<void>(resolve => { release = resolve; }); running--; finished++; });
  queue.enqueue('second', async () => { running++; maximum = Math.max(maximum, running); running--; finished++; });
  t.mock.timers.tick(2000);
  assert.equal(running, 0);
  idle = true; t.mock.timers.tick(2000);
  assert.equal(running, 1);
  t.mock.timers.tick(10000);
  assert.equal(finished, 0);
  release!(); await Promise.resolve(); await Promise.resolve();
  t.mock.timers.tick(3000); await Promise.resolve(); await Promise.resolve();
  assert.equal(finished, 2);
  assert.equal(maximum, 1);
  queue.clear();
});
