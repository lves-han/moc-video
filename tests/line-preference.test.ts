import assert from 'node:assert/strict';
import test from 'node:test';
import { captureLine, correspondingEpisode, createLinePreferences, episodeCoordinates, LINE_PREFERENCE_KEY, preferredSelection } from '../src/line-preference.ts';
import { AppController, createAppState, metadataVideoId } from '../src/state.ts';
import { fixture as playerFixture } from './fixtures/t12_player.ts';
import type { DetailsViewData, PlaybackContext, SourceKey } from '../src/models.ts';

const copy = <T>(value: T): T => JSON.parse(JSON.stringify(value));
const META = { source_id: 'a'.repeat(64), source_revision: 'b'.repeat(64) };
const PLAY = { source_id: 'c'.repeat(64), source_revision: 'd'.repeat(64) };
const OTHER = { source_id: 'e'.repeat(64), source_revision: 'f'.repeat(64) };
function details(source = PLAY, direct = false): DetailsViewData {
  const ref = { ...source, data: JSON.stringify(['maccms', '42']) };
  const data: DetailsViewData = { details: { id: '1'.repeat(64), ref, title: 'A stable series', kind: 'series', year: 2026, tags: [], related: [],
    actors: [{ name: 'An actor', role: null, image: null }],
    groups: ['First', 'Second', 'Third'].map((title, group) => ({ id: `g${group}`, title, kind: 'route', dub_status: 'unspecified', season_number: null,
      episodes: [1, 2, 3].map((number, index) => ({ id: `g${group}e${number}`, label: `第0${number}集`, season: null, number: null, index,
        ref: { ...source, data: JSON.stringify(['maccms_episode', '42', String(group), String(index), 'temporary-url-fingerprint']) } })) })) }, images: {}, metadata_ref: null };
  if (source === META || direct) {
    data.metadata_ref = { identity: { config_id: '11111111-1111-4111-8111-111111111111', addon_key: '2'.repeat(64), namespace: direct ? 'maccms' : 'addon:meta',
      type: 'series', meta_id: '42', canonical_id: '42' }, content_ref: ref };
    if (!direct) data.details.groups = [{ id: 'season', title: 'Season 1', kind: 'season', dub_status: 'unspecified', season_number: 1,
      episodes: [1, 2, 3].map((number, index) => ({ id: `v${number}`, label: `第0${number}集`, season: 1, number, index,
        ref: { ...META, data: JSON.stringify(['stremio_video', 'addon', 'series', '42', `v${number}`]) } })) }];
  }
  return data;
}
function context(metadata = details(META), provider = details(), group = 2, number = 1): PlaybackContext {
  const episode = provider.details.groups[group]!.episodes[number - 1]!;
  return { metadata_ref: metadata.metadata_ref!, metadata_video_id: metadataVideoId(metadata.details.groups[0]!.episodes[number - 1]!.ref), match_id: crypto.randomUUID(),
    content_ref: provider.details.ref, group_id: provider.details.groups[group]!.id, episode_id: episode.id, episode_ref: episode.ref, selected_stream_id: null };
}
function memory() {
  const rows = new Map<string, string>(); let getError = false, setError = false;
  const store = createLinePreferences(() => ({ getItem(key) { if (getError) throw new Error('permission'); return rows.get(key) ?? null; },
    setItem(key, value) { if (setError) throw new Error('quota'); rows.set(key, value); } }));
  return { store, rows, readError(value: boolean) { getError = value; }, writeError(value: boolean) { setError = value; } };
}

test('REQ-LINE-PREFERENCE: production null season/number labels map episodes 2/3 with current references', async () => {
  const metadata = details(META), provider = details(), preference = (await captureLine(metadata, provider.details, context(metadata, provider)))!;
  assert.ok(preference);
  for (const n of [2, 3]) {
    const current = copy(provider);
    current.details.groups[2]!.episodes[n - 1]!.ref.data = 'fresh-temporary-reference';
    const selected = await preferredSelection(preference, metadata, metadata.details.groups[0]!.episodes[n - 1]!, current.details);
    assert.equal(selected?.group_id, 'g2'); assert.equal(selected?.episode_id, `g2e${n}`);
  }
});

test('REQ-LINE-PREFERENCE: reordering, homonymous routes, missing/duplicate/unordered episodes and content replacement require confirmation', async () => {
  const metadata = details(META), provider = details(), preference = (await captureLine(metadata, provider.details, context(metadata, provider)))!;
  const mutations = [
    (d: DetailsViewData) => d.details.groups.reverse(),
    (d: DetailsViewData) => { d.details.groups[1]!.title = 'Third'; },
    (d: DetailsViewData) => d.details.groups[2]!.episodes.splice(1, 1),
    (d: DetailsViewData) => { d.details.groups[2]!.episodes[1]!.label = '第01集'; },
    (d: DetailsViewData) => d.details.groups[2]!.episodes.reverse(),
    (d: DetailsViewData) => { d.details.title = 'Replacement'; },
    (d: DetailsViewData) => { d.details.year = 2025; },
    (d: DetailsViewData) => { d.details.actors![0]!.name = 'Another actor'; },
    (d: DetailsViewData) => { d.details.ref.source_revision = '0'.repeat(64); },
  ];
  for (const mutate of mutations) {
    const current = copy(provider); mutate(current);
    assert.equal(await preferredSelection(preference, metadata, metadata.details.groups[0]!.episodes[1]!, current.details), null);
  }
  const replacement = copy(metadata); replacement.details.title = 'Metadata ID reused';
  assert.equal(await preferredSelection(preference, replacement, replacement.details.groups[0]!.episodes[1]!, provider.details), null);
});

test('REQ-LINE-PREFERENCE: anchored labels reject no-number, duplicate, cross-season ambiguity and similar forgeries', () => {
  const metadata = details(META), provider = details(), target = metadata.details.groups[0]!.episodes[1]!, group = provider.details.groups[2]!;
  for (const label of ['2', 'Episode two', '第02集预告', '第2集$secret', 'S01E02 trailer', '第1季第2集花絮', '第2-3集']) assert.equal(episodeCoordinates({ ...target, number: null, season: null, label }), null);
  for (const label of ['第02集', 'E02', 'EP02', 'Episode 02', 'S01E02', '第1季第2集']) assert.equal(episodeCoordinates({ ...target, number: null, season: null, label })?.number, 2);
  const otherSeason = copy(metadata.details.groups[0]!); otherSeason.id = 'season2'; otherSeason.season_number = 2;
  otherSeason.episodes.forEach(episode => { episode.season = 2; }); metadata.details.groups.push(otherSeason);
  assert.equal(correspondingEpisode(metadata.details, target, group), null);
  group.episodes.forEach((episode, index) => { episode.label = `S01E0${index + 1}`; });
  assert.equal(correspondingEpisode(metadata.details, target, group)?.id, 'g2e2');
  group.episodes[0]!.label = 'S01E02';
  assert.equal(correspondingEpisode(metadata.details, target, group), null);
});

test('REQ-LINE-PREFERENCE: storage survives repository restart, isolates films/revisions and contains only digests', async () => {
  const m = memory(), metadata = details(META), provider = details(), preference = (await captureLine(metadata, provider.details, context(metadata, provider)))!;
  assert.equal(m.store.save(preference, () => true), true);
  const fresh = createLinePreferences(() => ({ getItem: key => m.rows.get(key) ?? null, setItem: (key, value) => { m.rows.set(key, value); } }));
  assert.deepEqual(await fresh.get(metadata.metadata_ref!), preference);
  for (const mutate of [(d: DetailsViewData) => { d.metadata_ref!.identity.canonical_id = 'another'; },
    (d: DetailsViewData) => { d.metadata_ref!.content_ref.source_revision = '0'.repeat(64); },
    (d: DetailsViewData) => { d.metadata_ref!.identity.config_id = '22222222-2222-4222-8222-222222222222'; }]) {
    const other = copy(metadata); mutate(other); assert.equal(await fresh.get(other.metadata_ref!), null);
  }
  const raw = m.rows.get(LINE_PREFERENCE_KEY)!;
  assert.ok(!/http|url|session|resolution|stream|token|fingerprint|actor|series|Third/i.test(raw));
  assert.ok(Object.values(JSON.parse(raw)[0]).every(value => typeof value === 'object' || /^[a-f0-9]{64}$/.test(String(value))));
});

test('REQ-LINE-PREFERENCE: corrupt/unavailable/quota storage preserves confirmed bytes and recovers, stale writes lose', async () => {
  const m = memory(), metadata = details(META), provider = details(), preference = (await captureLine(metadata, provider.details, context(metadata, provider)))!;
  m.store.save(preference, () => true); const before = m.rows.get(LINE_PREFERENCE_KEY)!;
  m.writeError(true); assert.throws(() => m.store.save({ ...preference, line: '0'.repeat(64) }, () => true), /save failed/); assert.equal(m.rows.get(LINE_PREFERENCE_KEY), before);
  m.readError(true); await assert.rejects(m.store.get(metadata.metadata_ref!), /read failed/); m.readError(false); m.writeError(false);
  m.rows.set(LINE_PREFERENCE_KEY, 'damaged'); assert.throws(() => m.store.save(preference, () => true), /damaged/); assert.equal(m.rows.get(LINE_PREFERENCE_KEY), 'damaged');
  m.rows.set(LINE_PREFERENCE_KEY, before); assert.equal(m.store.save({ ...preference, line: '0'.repeat(64) }, () => true), true);
  assert.equal(m.store.save(preference, () => false), false); assert.equal((await m.store.get(metadata.metadata_ref!))?.line, '0'.repeat(64));
});

function appFixture(direct = false) {
  const metadata = details(direct ? PLAY : META, direct), provider = direct ? metadata : details(), m = memory(), state = createAppState();
  state.runtime.phase = 'ready'; state.runtime.engine_epoch = '11111111-1111-4111-8111-111111111111';
  state.selected_metadata_source = META; state.selected_playback_source = PLAY;
  state.details.data = metadata; state.details.phase = 'ready'; state.route = { name: 'details', ref: metadata.details.ref };
  const source = (key: SourceKey, kind: 'maccms' | 'stremio') => ({ id: key.source_id, source_revision: key.source_revision, name: kind, kind, runtime_status: 'ready',
    origin: { config_id: metadata.metadata_ref!.identity.config_id }, capabilities: { details: true, home: true, resolve: kind === 'maccms', search: true } });
  state.sources.data = { providers: [source(PLAY, 'maccms'), source(META, 'stremio'), source(OTHER, 'maccms')], plugins: [] } as never;
  const calls: { command: string; input: any }[] = [];
  let playable = true, hold: (() => Promise<void>) | null = null;
  const client = { cancel: async () => {}, call: async (command: string, input: any) => {
    calls.push({ command, input: copy(input) });
    if (command === 'get_details') { if (hold) await hold(); return copy(provider); }
    if (command === 'resolve_episode') return { id: crypto.randomUUID(), streams: [{ playable }] };
    if (command !== 'match_playback') return { ok: true };
    const number = direct ? Number(JSON.parse(input.video_id)[3]) + 1 : Number(input.video_id.slice(1));
    const group = input.selection ? provider.details.groups.find(g => g.id === input.selection.group_id)!
      : provider.details.groups[direct ? Number(JSON.parse(input.video_id)[2]) : 0]!;
    const episode = input.selection ? group.episodes.find(e => e.id === input.selection.episode_id)! : group.episodes[number - 1]!;
    const ctx = context(metadata, provider, provider.details.groups.indexOf(group), number);
    Object.assign(ctx, { metadata_video_id: input.video_id, episode_id: episode.id, episode_ref: episode.ref });
    return { status: direct || input.selection ? 'matched' : 'needs_confirmation', metadata_ref: metadata.metadata_ref, video_id: input.video_id,
      candidates: [{ content_ref: provider.details.ref, title: provider.details.title }], context: direct || input.selection ? ctx : null };
  } };
  const app = new AppController(client as never, { update: async (change: any) => change(state.settings) } as never, state, m.store);
  const video = (number: number) => metadataVideoId(metadata.details.groups[0]!.episodes[number - 1]!.ref);
  const confirmed = async (group = 2, number = 1) => {
    await app.match(video(number));
    if (direct) await app.match(metadataVideoId(provider.details.groups[group]!.episodes[number - 1]!.ref), null, false);
    else { await app.loadCandidate(provider.details.ref); await app.match(video(number), { content_ref: provider.details.ref, group_id: provider.details.groups[group]!.id, episode_id: provider.details.groups[group]!.episodes[number - 1]!.id }); }
  };
  const advance = () => {
    const ctx = state.playback.context!;
    state.playback.session = { session_id: crypto.randomUUID() } as never;
    app.rememberLine(ctx, state.playback.playback_epoch, state.playback.session!.session_id);
  };
  return { app, state, m, metadata, provider, calls, video, confirmed, advance, playable(value: boolean) { playable = value; }, hold(fn: (() => Promise<void>) | null) { hold = fn; } };
}

test('REQ-LINE-PREFERENCE: manual confirmation and resolve alone save nothing; current successful choice drives next episodes', async () => {
  const f = appFixture(); await f.confirmed();
  assert.equal(await f.m.store.get(f.metadata.metadata_ref!), null);
  f.advance(); assert.equal(f.app.lineMemory.saved, true);
  for (const n of [2, 3]) { const result = await f.app.match(f.video(n)); assert.equal(result?.context?.group_id, 'g2'); assert.equal(result?.context?.metadata_video_id, f.video(n)); }
  const resolves = f.calls.filter(call => call.command === 'resolve_episode');
  assert.equal(resolves.length, 2); assert.ok(resolves.every(call => JSON.parse(call.input.episode_ref.data)[2] === '2'));
});

test('REQ-LINE-PREFERENCE: manual override, unavailable remembered line and stale Session cannot replace success', async () => {
  const f = appFixture(); await f.confirmed(); f.advance(); const saved = f.m.rows.get(LINE_PREFERENCE_KEY);
  const old = copy(f.state.playback.context!), oldSession = f.state.playback.session!.session_id, oldEpoch = f.state.playback.playback_epoch;
  f.playable(false); assert.equal((await f.app.match(f.video(2)))?.status, 'needs_confirmation');
  assert.equal(f.m.rows.get(LINE_PREFERENCE_KEY), saved);
  await f.app.loadCandidate(f.provider.details.ref);
  await f.app.match(f.video(2), { content_ref: f.provider.details.ref, group_id: 'g0', episode_id: 'g0e2' });
  f.app.rememberLine(old, oldEpoch, oldSession); assert.equal(f.m.rows.get(LINE_PREFERENCE_KEY), saved);
  f.advance(); f.playable(true); assert.equal((await f.app.match(f.video(3)))?.context?.group_id, 'g0');
});

test('REQ-LINE-PREFERENCE: late preference reads and detail results lose to a new manual source', async () => {
  const f = appFixture(); await f.confirmed(); f.advance();
  let release!: () => void, entered = false;
  const gate = new Promise<void>(resolve => { release = resolve; });
  f.hold(async () => { entered = true; await gate; });
  const pending = f.app.match(f.video(2));
  while (!entered) await new Promise(resolve => setTimeout(resolve, 1));
  await f.app.selectSource(OTHER.source_id, 'playback');
  release(); await pending;
  assert.equal(f.state.selected_playback_source?.source_id, OTHER.source_id);
  assert.equal(f.state.playback.context, null);
});

test('REQ-LINE-PREFERENCE: same-source route preference preserves requested history video and stops at the group boundary', async () => {
  const f = appFixture(true); await f.confirmed(); f.advance();
  const result = await f.app.match(f.video(2));
  assert.equal(result?.context?.group_id, 'g2'); assert.equal(result?.context?.metadata_video_id, f.video(2));
  assert.equal(f.app.nextVideo(f.video(3)), null);
});

test('REQ-LINE-PREFERENCE: late preference reads cannot replace a manual source or publish stale diagnostics', async () => {
  const f = appFixture(); await f.confirmed(); f.advance();
  const get = f.m.store.get;
  let release!: () => void, entered = false;
  const gate = new Promise<void>(resolve => { release = resolve; });
  f.m.store.get = async metadata => { entered = true; await gate; return get(metadata); };
  const pending = f.app.match(f.video(2));
  while (!entered) await new Promise(resolve => setTimeout(resolve, 1));
  await f.app.selectSource(OTHER.source_id, 'playback');
  release(); await pending;
  assert.equal(f.state.selected_playback_source?.source_id, OTHER.source_id);
  assert.equal(f.state.playback.context, null);
  assert.equal(f.app.lineMemory.error, '');
});

test('REQ-LINE-PREFERENCE: only current presented frames advancing naturally trigger success, seeks and late callbacks do not', () => {
  const f = playerFixture(), video = f.video as any;
  let sequence = 0; const callbacks = new Map<number, (now: number, metadata: any) => void>();
  video.requestVideoFrameCallback = (callback: any) => { callbacks.set(++sequence, callback); return sequence; };
  video.cancelVideoFrameCallback = (id: number) => callbacks.delete(id);
  video.currentSrc = 'http://media'; video.seeking = false;
  const successes: any[] = [];
  const player = f.playerModule.createPlayer(video, () => {}, undefined, (binding: any) => successes.push(binding));
  const binding = { playback_epoch: 7, engine_epoch: 'host', session_id: 'current' };
  player.open({ url: 'http://media', kind: 'mp4' }, binding); video.readyState = 4; video.paused = false;
  const frame = (wall: number, time: number) => { const [id, callback] = [...callbacks][0]!; callbacks.delete(id); callback(wall, { mediaTime: time }); };
  frame(100, 0); assert.equal(successes.length, 0);
  video.seeking = true; video.dispatchEvent(new Event('seeking')); frame(200, 20); video.seeking = false; video.dispatchEvent(new Event('seeked'));
  frame(300, 20); assert.equal(successes.length, 0); frame(400, 20.1); assert.deepEqual(copy(successes), [binding]);
  const late = [...callbacks.values()][0]!; player.stop(); late(6000, 21); assert.equal(successes.length, 1); assert.equal(callbacks.size, 0);
});
