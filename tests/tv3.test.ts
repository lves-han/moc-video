import assert from 'node:assert/strict';
import test from 'node:test';
import { AppController, createAppState, normalizedFilter } from '../src/state.ts';
import type { Bridge_HomeRequest, HomeViewData } from '../src/models.ts';
const source = { source_id: 'fixture', source_revision: 'r1' };
function data(genre = '', page = 1): HomeViewData {
  return { page: { ...source, failed_sections: [], sections: [{ id: 'section', section_ref: 'ref', index: 0, title: 'Fixture', page,
    items: genre ? [{ id: `${genre}-${page}`, title: genre, kind: 'movie', poster: null, ref: { ...source, data: genre } }] : [],
    has_next: !!genre, next_page: genre ? page + 1 : null, scanned_count: genre ? page * 20 : 0, matched_count: genre ? page : 0 }] }, images: {} };
}
function setup(call: (input: Bridge_HomeRequest) => Promise<HomeViewData>) {
  const state = createAppState(); state.runtime.engine_epoch = '11111111-1111-4111-8111-111111111111';
  state.selected_metadata_source = source;
  state.sources.data = { providers: [{ id: source.source_id, source_revision: source.source_revision, runtime_status: 'ready', kind: 'stremio' }], plugins: [] } as never;
  const cancelled: string[] = [], inputs: Bridge_HomeRequest[] = [];
  const app = new AppController({ cancel: async (id: string) => { cancelled.push(id); }, call: async (command: string, input: Bridge_HomeRequest) => { assert.equal(command, 'get_home'); inputs.push(input); return call(input); } } as never, {} as never, state);
  return { app, state, cancelled, inputs };
}
test('TV-HOME/catalog-pending: descriptions stay ready and rows are pending until requested', async () => {
  const { app, state, inputs } = setup(async input => data(input.catalog_only ? '' : 'Drama', input.page));
  await app.loadCatalog(); assert.equal(inputs.length, 1); assert.equal(inputs[0]!.catalog_only, true);
  assert.equal(state.home.phase, 'ready'); assert.equal(app.catalogRow('ref').phase, 'pending');
  await app.loadRow('ref'); assert.equal(app.catalogRow('ref').phase, 'ready');
  assert.equal(app.catalogRow('ref').data!.page.sections[0]!.items[0]!.title, 'Drama');
});
test('TV-HOME/filtered-row-isolation: same reference has independent concurrent responses and pagination', async () => {
  let release!: (value: HomeViewData) => void;
  const delayed = new Promise<HomeViewData>(resolve => { release = resolve; });
  const { app, cancelled } = setup(async input => input.catalog_only ? data() : input.filter?.genre === 'Drama' && input.page === 1 ? delayed : data(input.filter?.genre, input.page));
  await app.loadCatalog();
  const slow = app.loadRow('ref', { genre: 'Drama', year: '2026' });
  await app.loadRow('ref', { genre: 'Comedy' });
  release(data('Drama')); await slow;
  assert.equal(cancelled.length, 0);
  assert.equal(app.catalogRow('ref', { year: '2026', genre: 'Drama' }).data!.page.sections[0]!.items[0]!.title, 'Drama');
  assert.equal(app.catalogRow('ref', { genre: 'Comedy' }).data!.page.sections[0]!.items[0]!.title, 'Comedy');
  await app.loadRow('ref', { genre: 'Comedy' }, 2);
  assert.equal(app.catalogRow('ref', { genre: 'Comedy' }).data!.page.sections[0]!.items.length, 2);
  assert.equal(app.catalogRow('ref', { genre: 'Drama', year: '2026' }).data!.page.sections[0]!.items.length, 1);
  await assert.rejects(app.loadRow('ref', { genre: 'Drama' }, 2), /continuation/);
});
test('TV-HOME/filtered-row-isolation: retry and real empty results remain local', async () => {
  let fail = true;
  const { app } = setup(async input => { if (input.catalog_only) return data(); if (input.filter?.genre === 'Drama' && fail) throw new Error('fixture unavailable'); return data(input.filter?.genre === 'Drama' ? 'Drama' : ''); });
  await app.loadCatalog(); await app.loadRow('ref', { genre: 'Drama' }); await app.loadRow('ref', { genre: 'Comedy' });
  assert.equal(app.catalogRow('ref', { genre: 'Drama' }).phase, 'failed');
  assert.equal(app.catalogRow('ref', { genre: 'Comedy' }).phase, 'ready');
  assert.equal(app.catalogRow('ref', { genre: 'Comedy' }).data!.page.sections[0]!.items.length, 0);
  fail = false; await app.loadRow('ref', { genre: 'Drama' });
  assert.equal(app.catalogRow('ref', { genre: 'Drama' }).phase, 'ready');
  assert.equal(app.catalogRow('ref', { genre: 'Comedy' }).data!.page.sections[0]!.items.length, 0);
});
test('TV-HOME/filtered-row-isolation: late responses lose authority on locale, source, revision and host changes', async () => {
  for (const change of ['locale', 'source', 'revision', 'host', 'refresh'] as const) {
    let release!: (value: HomeViewData) => void;
    const delayed = new Promise<HomeViewData>(resolve => { release = resolve; });
    const { app, state } = setup(async input => input.catalog_only ? data() : delayed);
    await app.loadCatalog(); const key = app.rowKey('ref', { genre: 'Drama' }); const pending = app.loadRow('ref', { genre: 'Drama' });
    if (change === 'locale') state.settings.ui_locale = 'zh-CN';
    if (change === 'source') state.selected_metadata_source = { ...source, source_id: 'other' };
    if (change === 'revision') state.selected_metadata_source = { ...source, source_revision: 'r2' };
    if (change === 'host') state.runtime.engine_epoch = '22222222-2222-4222-8222-222222222222';
    if (change === 'refresh') await app.loadCatalog();
    release(data('old')); await pending;
    assert.equal(app.rows[key]?.data ?? null, null, change);
    assert.equal(app.catalogRow('ref', { genre: 'Drama' }).phase, 'pending', change);
  }
});
test('TV-HOME/filtered-row-isolation: navigation retains completed filter results', async () => {
  const { app } = setup(async input => input.catalog_only ? data() : data(input.filter?.genre));
  await app.loadCatalog(); await app.loadRow('ref', { genre: 'Drama' });
  app.navigate({ name: 'home' });
  assert.equal(app.catalogRow('ref', { genre: 'Drama' }).data!.page.sections[0]!.items[0]!.title, 'Drama');
  assert.equal(normalizedFilter({ genre: 'Drama', year: '2026' }), normalizedFilter({ year: '2026', genre: 'Drama' }));
});
