import assert from 'node:assert/strict';
import test from 'node:test';
import { AppController, createAppState } from '../src/state.ts';
import type { HomeViewData } from '../src/models.ts';

const source = { source_id: 'fixture', source_revision: 'r1' };
const host = '11111111-1111-4111-8111-111111111111';
function setup(status: (id: string) => Promise<unknown>) {
  const state = createAppState(); state.runtime.engine_epoch = host; state.selected_metadata_source = source;
  state.sources.data = { providers: [{ id: source.source_id, source_revision: source.source_revision, runtime_status: 'ready', kind: 'stremio' }], plugins: [] } as never;
  const calls: { command: string; input: any; epoch?: string }[] = [];
  const snapshot = (): HomeViewData => ({ page: { ...source, failed_sections: [], sections: [{ id: 'section', section_ref: 'ref', index: 0, title: 'Fixture', page: 1,
    items: [{ id: 'film', title: 'Film', kind: 'movie', poster: { resource_id: 'poster' }, ref: { ...source, data: 'film' } }], has_next: false, next_page: null }] },
    images: { poster: { resource_id: 'poster', url: 'http://127.0.0.1/image', expires_at: '2030-01-01T00:00:00Z' } } } as HomeViewData);
  const app = new AppController({ cancel: async () => {}, call: async (command: string, input: any, _id?: string, epoch?: string) => {
    calls.push({ command, input, epoch });
    return command === 'get_home' ? snapshot() : status(input.target_id);
  } } as never, {} as never, state);
  state.home.data = snapshot(); state.home.phase = 'ready';
  return { app, state, calls };
}
const expired = (id: string) => ({ kind: 'image', target_id: id, status: 'expired', error: { code: 'SESSION_EXPIRED', message: 'Expired image', retryable: true } });

test('TV3 review: duplicate row image errors query once and refresh its DTO once', async () => {
  let release!: (value: unknown) => void;
  const pending = new Promise(resolve => { release = resolve; });
  let delayed = true;
  const { app, calls } = setup(id => delayed ? pending : Promise.resolve(expired(id)));
  await app.loadRow('ref');
  const first = app.rowImageError('ref', {}, 'poster'), duplicate = app.rowImageError('ref', {}, 'poster');
  assert.equal(calls.filter(c => c.command === 'get_resource_status').length, 1);
  delayed = false; release(expired('poster')); await Promise.all([first, duplicate]);
  assert.equal(calls.filter(c => c.command === 'get_home').length, 2);
  await app.rowImageError('ref', {}, 'poster');
  assert.equal(calls.filter(c => c.command === 'get_home').length, 2, 'recovered DTO gets no second automatic refresh');
  assert.equal(app.catalogRow('ref').error?.code, 'SESSION_EXPIRED');
  assert.ok(calls.filter(c => c.command === 'get_resource_status').every(c => c.epoch === host));
  await app.loadRow('ref'); await app.rowImageError('ref', {}, 'poster');
  assert.equal(calls.filter(c => c.command === 'get_home').length, 4, 'explicit retry starts a fresh attempt');
});

test('TV3 review: late image status loses authority after row reload, locale and host changes', async () => {
  for (const change of ['reload', 'locale', 'host'] as const) {
    let release!: (value: unknown) => void;
    const { app, state, calls } = setup(() => new Promise(resolve => { release = resolve; }));
    await app.loadRow('ref'); const pending = app.rowImageError('ref', {}, 'poster');
    if (change === 'reload') await app.loadRow('ref');
    if (change === 'locale') state.settings.ui_locale = 'zh-CN';
    if (change === 'host') state.runtime.engine_epoch = '22222222-2222-4222-8222-222222222222';
    const before = calls.length; release(expired('poster')); await pending;
    assert.equal(calls.length, before, change);
    assert.equal(app.catalogRow('ref').error, null, change);
  }
});

test('TV3 review: failed diagnostics retain error and reject a mismatched target', async () => {
  for (const mode of ['failure', 'target'] as const) {
    const { app, calls } = setup(async () => { if (mode === 'failure') throw new Error('Status unavailable'); return expired('different'); });
    await app.loadRow('ref'); await app.rowImageError('ref', {}, 'poster'); await app.rowImageError('ref', {}, 'poster');
    assert.match(app.catalogRow('ref').error!.message, mode === 'failure' ? /Status unavailable/ : /target differs/);
    assert.equal(calls.filter(c => c.command === 'get_home').length, 1);
    assert.equal(calls.filter(c => c.command === 'get_resource_status').length, 1);
  }
});
