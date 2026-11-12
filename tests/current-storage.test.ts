import assert from 'node:assert/strict';
import test from 'node:test';
import { sourceChecks } from '../src/source-health.ts';
import { metadataCache } from '../src/metadata-cache.ts';
import { catalogCache } from '../src/catalog-cache.ts';

test('current caches do not read the previous product namespace', () => {
  const requested: string[] = [];
  const old = new Map([['yingxiang-video.source-checks.v1', '{"old":42}']]);
  const store = { getItem: (key: string) => { requested.push(key); return old.get(key) ?? null; }, setItem() {} };
  assert.equal(sourceChecks(() => store).has('old'), false);
  const source = { source_id: 's', source_revision: 'r' };
  assert.equal(metadataCache(() => store).get({ ...source, data: 'id' }, 'en', 'host'), null);
  assert.equal(catalogCache(() => store).get(source, 'section', {}, 'en', 'host'), null);
  assert.ok(requested.every(key => key.startsWith('moc-video.')));
  assert.equal(old.get('yingxiang-video.source-checks.v1'), '{"old":42}');
});
