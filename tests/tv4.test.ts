import assert from 'node:assert/strict';
import test from 'node:test';
import { AppController, assertContext, createAppState, metadataVideoId } from '../src/state.ts';
import { historyPoster } from '../src/data/history.ts';
import type { AppState, DetailsViewData, PlaybackContext, WatchHistoryRecord } from '../src/models.ts';

const key = { source_id: 'maccms', source_revision: 'r1' }, config = '11111111-1111-4111-8111-111111111111';
const copy = <T>(value: T): T => JSON.parse(JSON.stringify(value));
function fixture() {
  const state = createAppState(); state.runtime.engine_epoch = '22222222-2222-4222-8222-222222222222';
  state.selected_metadata_source = { source_id: 'tmdb', source_revision: 'tmdb-r1' }; state.selected_playback_source = key;
  state.sources.data = { providers: [{ id: key.source_id, source_revision: key.source_revision, kind: 'maccms', runtime_status: 'ready', name: 'MacCMS', content_types: ['variety'],
    origin: { config_id: config, api_url: 'https://example.com/api' }, capabilities: { home: true, search: true, details: true, resolve: true, sequential_home: false } }], plugins: [] } as never;
  const ref = { ...key, data: JSON.stringify(['maccms', 'film']) };
  const data: DetailsViewData = { details: { id: 'a'.repeat(64), title: 'Film', kind: 'other', ref, tags: [], related: [],
    groups: [{ id: 'group', title: 'Line', kind: 'route', dub_status: 'unspecified', episodes: [0, 1].map(index => ({ id: `episode-${index}`, label: String(index + 1), index,
      ref: { ...key, data: JSON.stringify(['maccms_episode', 'film', '0', String(index), 'fingerprint']) } })) }] }, images: {},
    metadata_ref: { content_ref: ref, identity: { config_id: config, addon_key: 'b'.repeat(64), namespace: 'maccms', type: 'other', meta_id: 'film', canonical_id: 'c'.repeat(64) } } };
  const calls: string[] = [];
  const context = (video: string): PlaybackContext => {
    const episode = data.details.groups[0]!.episodes.find(item => metadataVideoId(item.ref) === video)!;
    return { metadata_ref: copy(data.metadata_ref!), metadata_video_id: video, match_id: 'match', content_ref: ref, group_id: 'group', episode_id: episode.id, episode_ref: copy(episode.ref), selected_stream_id: null };
  };
  const app = new AppController({ cancel: async () => {}, call: async (command: string, input: any) => {
    calls.push(command);
    if (command === 'get_details') return copy(data);
    assert.equal(command, 'match_playback'); return { status: 'matched', metadata_ref: input.metadata_ref, video_id: input.video_id,
      candidates: [{ content_ref: ref, title: data.details.title, year: null }], context: context(input.video_id) };
  } } as never, {} as never, state);
  return { state, app, data, context, calls };
}

test('TV4 same source: details, episode switching and stable video IDs retain the TMDB choice', async () => {
  const { state, app, data } = fixture(); app.navigate({ name: 'details', ref: data.details.ref }); await app.loadDetails(data.details.ref);
  assert.equal(state.details.phase, 'ready', state.details.error?.message); const first = metadataVideoId(data.details.groups[0]!.episodes[0]!.ref);
  const result = await app.match(first); assert.equal(result?.status, 'matched'); assertContext(state, result!.context!);
  const next = app.nextVideo(first)!; assert.ok(next); assert.equal((await app.match(next))?.context?.episode_id, 'episode-1');
  assert.equal(state.selected_metadata_source!.source_id, 'tmdb');
  const renewed = copy(data.details.groups[0]!.episodes[0]!.ref); const fields = JSON.parse(renewed.data); fields[4] = 'new-signed-resource'; renewed.data = JSON.stringify(fields);
  assert.equal(metadataVideoId(renewed), first, 'temporary playback entry signatures never enter durable video IDs');
});

test('TV4 same source: namespace, provider, revision, configuration, content and episode bindings remain strict', async () => {
  const { app, state, data, context } = fixture(); await app.loadDetails(data.details.ref);
  const valid = context(metadataVideoId(data.details.groups[0]!.episodes[0]!.ref)); assertContext(state, valid);
  const mutations: ((state: AppState, context: PlaybackContext) => void)[] = [
    (_, c) => { c.metadata_ref.identity.namespace = 'tmdb'; }, (_, c) => { c.metadata_ref.identity.config_id = 'wrong'; },
    (_, c) => { c.episode_ref.source_revision = 'old'; }, (_, c) => { c.content_ref.data = 'different film'; },
    (_, c) => { c.episode_ref.source_id = 'other'; }, (_, c) => { c.group_id = 'other group'; }, (_, c) => { c.episode_id = 'other episode'; },
    s => { s.sources.data!.providers[0]!.kind = 'cloudstream'; }, s => { s.selected_playback_source = { source_id: 'other', source_revision: 'r1' }; },
  ];
  for (const mutate of mutations) { const snapshot = copy(state), value = copy(valid); mutate(snapshot, value); assert.throws(() => assertContext(snapshot, value)); }
  assert.throws(() => app.navigate({ name: 'details', ref: { ...data.details.ref, source_revision: 'old' } }));
  for (const value of [{ id: 'film' }, ['stremio', 'film'], ['maccms'], ['maccms', ''], ['maccms', 5]]) {
    const invalid = { ...data.details.ref, data: JSON.stringify(value) };
    assert.throws(() => app.navigate({ name: 'details', ref: invalid }));
    await assert.rejects(app.loadDetails(invalid));
  }
});

test('TV4 history poster: MacCMS identity resolves through its current configuration and keeps images transient', async () => {
  const { state, data } = fixture(); const record = { content_id: data.details.id, metadata_identity: data.metadata_ref!.identity } as WatchHistoryRecord;
  const calls: any[] = [];
  const result = await historyPoster(record, state.sources.data!.providers, (async (_command: string, input: unknown) => {
    calls.push(input); return { details: { id: record.content_id, poster: { resource_id: 'poster' } }, images: { poster: { url: 'http://127.0.0.1/transient' } } };
  }) as never);
  assert.equal(result, 'http://127.0.0.1/transient'); assert.deepEqual(JSON.parse(calls[0].ref.data), ['maccms', 'film']);
  assert.equal(calls[0].ref.source_revision, 'r1'); assert.equal(await historyPoster(record, [], (() => { throw new Error('unexpected call'); }) as never), null);
});
