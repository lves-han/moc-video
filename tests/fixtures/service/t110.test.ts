import test from 'node:test';
import assert from 'node:assert/strict';
import { AppController, createAppState, defaultSettings, TMDB_CONFIG_ID } from '../../../src/state.ts';
import { ApiFailure } from '../../../src/api.ts';

const ref = { source_id: 'metadata', source_revision: 'revision', data: 'detail' };
const identity = { config_id: TMDB_CONFIG_ID, addon_key: 'addon', namespace: 'tmdb', type: 'series', meta_id: 'tmdb:44', canonical_id: 'tmdb:44' };
const video = 'tt7654321:1:2';
const context = { metadata_ref: { identity, content_ref: ref }, metadata_video_id: video, match_id: 'match', content_ref: { source_id: 'service', source_revision: 'rev', data: 'content' }, group_id: 'group', episode_id: 'episode', episode_ref: { source_id: 'service', source_revision: 'rev', data: JSON.stringify(['service_line', 'round', 'line']) }, selected_stream_id: null };
const resolution = { id: 'resolution', source_id: 'service', source_revision: 'rev', episode_ref: context.episode_ref, streams: [], subtitles: [], expires_at: '2027-01-01T00:00:00Z' };
const line = { line_id: '11111111-1111-4111-8111-111111111111', source_id: 'source', source_name: 'Fixture', line_name: 'Line', episode_label: 'Episode', kind: 'file', probe_status: 'ready', latency_ms: 10 };
const view = (round: string, done: boolean, selected: string | null = null) => ({ round_id: round, lookup_id: null, query_done: done, pending: done ? 0 : 1, pending_confirm: 0, selected_line_id: selected, lines: [line], sources: [{source_id:'source',name:'Fixture',status:'done'}], candidates: [], error: null });
function setup(call: (command: string, input: any) => Promise<any>) {
  const root = createAppState();
  root.settings = { ...defaultSettings(), playback_service: { enabled: true, url: 'http://service', token: 'token', groups: [] } };
  root.details.phase = 'ready';
  root.details.data = { metadata_ref: { identity, content_ref: ref }, details: { groups: [{ episodes: [{ ref: { ...ref, data: JSON.stringify(['stremio_video', 'addon', 'series', 'tmdb:44', video]) } }] }] } } as any;
  const app = new AppController({ call } as any, {} as any, root, null);
  return { app, root };
}

test('automatic selection waits for completed probes and carries the native resolution', async () => {
  const calls: string[] = [];
  const { app, root } = setup(async (command, input) => {
    calls.push(command);
    if (command === 'start_line_round') return view(input.round_id, false);
    if (command === 'get_line_round') return view(input.round_id, true, line.line_id);
    if (command === 'play_line') return { context, resolution };
    throw new Error(command);
  });
  await app.startServiceRound(video);
  assert.deepEqual(calls, ['start_line_round', 'get_line_round', 'play_line']);
  assert.equal(root.route.name, 'playback');
  assert.equal(app.takeServiceResolution(context as any)?.id, resolution.id);
  assert.equal(app.takeServiceResolution(context as any), undefined);
});

test('manual selection wins over a late automatic result', async () => {
  let resume!: (value: unknown) => void;
  let round = ''; const selections: string[] = [];
  const { app, root } = setup(async (command, input) => {
    if (command === 'start_line_round') { round = input.round_id; return view(round, false); }
    if (command === 'get_line_round') return await new Promise(resolve => { resume = resolve; });
    if (command === 'play_line') { selections.push(input.line_id); return { context, resolution }; }
    if (command === 'cancel_line_round') return { ok: true };
    throw new Error(command);
  });
  const running = app.startServiceRound(video);
  await new Promise(resolve => setTimeout(resolve, 220));
  await app.chooseServiceLine(line.line_id);
  resume(view(round, true, 'late-line'));
  await running;
  assert.deepEqual(selections, [line.line_id]);
  assert.equal(root.route.name, 'playback');
});

test('candidates use their owning source and can be confirmed', async () => {
  let confirmed = false;
  const candidate = { candidate_id: '22222222-2222-4222-8222-222222222222', source_id: 'source', title: 'Show', year: 2020, kind: 'series' };
  const { app } = setup(async (command, input) => {
    if (command === 'start_line_round') return { ...view(input.round_id, true), lines: [], candidates: [candidate] };
    if (command === 'confirm_line_candidate') { assert.equal(input.source_id, candidate.source_id); confirmed = true; return { ...view(input.round_id, false), candidates: [] }; }
    if (command === 'get_line_round') return view(input.round_id, true, line.line_id);
    if (command === 'play_line') return { context, resolution };
    throw new Error(command);
  });
  const running = app.startServiceRound(video);
  await new Promise(resolve => setTimeout(resolve, 20));
  await app.confirmServiceCandidate(candidate);
  await running;
  assert.equal(confirmed, true);
});

test('migration removes successful sources and retains failed sources for retry', async t => {
  const prior = Object.getOwnPropertyDescriptor(globalThis, 'window');
  const source = (id: string) => ({ kind: 'maccms' as const, config_id: id, name: id, enabled: true, api_url: 'https://example.test/api' });
  const state = createAppState();
  state.settings = { ...defaultSettings(), source_configs: [source('first'), source('second')], playback_service: { enabled: true, url: 'http://service', token: 'token', groups: [] } };
  let stored = JSON.parse(JSON.stringify(state.settings));
  const settings = { update: async (fn: (value: typeof stored) => typeof stored) => stored = fn(stored) };
  Object.defineProperty(globalThis, 'window', { configurable: true, value: { __TAURI__: { core: { invoke: async (_command: string, args: any) => args.action === 'import' ? { results: [{status:'ok'}, {status:'failed'}] } : { sources: [] } } } } });
  t.after(() => { if (prior) Object.defineProperty(globalThis, 'window', prior); else Reflect.deleteProperty(globalThis, 'window'); });
  const app = new AppController({ call: async () => ({}) } as any, settings as any, state, null);
  await app.migratePlaybackSources();
  assert.deepEqual(state.settings.source_configs.map(config => config.config_id), ['second']);
  assert.match(app.serviceSources.error, /待重试/);
});

test('playback reports only naturally advancing frames and current failures', async () => {
  const { fixture, context: makeContext, flush } = await import('../t12_player.ts');
  const f = fixture(); const videoElement = f.video as any;
  let sequence = 0; const frames = new Map<number, (time: number, meta: any) => void>();
  videoElement.requestVideoFrameCallback = (callback: any) => { frames.set(++sequence, callback); return sequence; };
  videoElement.cancelVideoFrameCallback = (id: number) => frames.delete(id);
  videoElement.seeking = false;
  const reports: any[] = [];
  f.intercept(async (command, args, envelope) => { if (command === 'report_line_playback') { reports.push(args.input); return { ...envelope, data: { ok: true, reported: true } }; } return envelope; });
  const ctx = makeContext(); ctx.episode_ref.data = JSON.stringify(['service_line', 'round', 'line']);
  const res = { id:'service-resolution', source_id:'source', source_revision:'rev', episode_ref:ctx.episode_ref, streams:[{id:'a',kind:'progressive',label:'A',playable:true}], subtitles:[] };
  f.resolutions.set(res.id, res);
  await f.controller.load(ctx, { resolution: res as any });
  videoElement.currentSrc = f.root.playback.session.media_url;
  f.ready(); await flush();
  assert.equal(reports.length, 0);
  f.controller.seek(3); videoElement.dispatchEvent(new Event('seeking')); videoElement.dispatchEvent(new Event('seeked'));
  const frame = (wall: number, time: number) => { const [id, cb] = [...frames][0]!; frames.delete(id); cb(wall, {mediaTime:time}); };
  frame(100, 3); assert.equal(reports.length, 0); frame(200, 3.1); await flush();
  assert.equal(reports[0]?.outcome, 'playing');
  f.error(); await flush();
  assert.equal(reports[1]?.outcome, 'failed');
  await f.controller.stop();
});

test('explicit line selection keeps the completed list open', async () => {
  const selections: string[] = [];
  const { app, root } = setup(async (command, input) => {
    if (command === 'start_line_round') return view(input.round_id, true, line.line_id);
    if (command === 'play_line') { selections.push(input.line_id); return { context, resolution }; }
    throw new Error(command);
  });
  await app.startServiceRound(video, false, false);
  assert.equal(app.serviceRound.open, true);
  assert.deepEqual(selections, []);
  await app.chooseServiceLine(line.line_id);
  assert.deepEqual(selections, [line.line_id]);
  assert.equal(root.route.name, 'playback');
});

test('metadata add-on identity selects its registered service source without a global playback source', async () => {
  let received: any;
  const { app, root } = setup(async (command, input) => {
    if (command === 'start_line_round') { received = input; return view(input.round_id, true, line.line_id); }
    if (command === 'play_line') return { context, resolution };
    throw new Error(command);
  });
  root.settings.playback_service!.groups = [{ config_id:'33333333-3333-4333-8333-333333333333',status:'ready',addons:[{ original_url:'https://fixture.test/manifest.json',source_id:'registered-addon',addon_key:'addon',localized:false }] }];
  root.details.data!.metadata_ref!.identity.config_id = '33333333-3333-4333-8333-333333333333';
  root.selected_playback_source = null;
  await app.startServiceRound(video);
  assert.equal(received.metadata_source_id, 'registered-addon');
  assert.equal(received.video_id, video);
  assert.equal(root.route.name, 'playback');
});

test('expired line restarts lookup for the same episode', async () => {
  const rounds: string[] = [];
  let plays = 0;
  const { app, root } = setup(async (command, input) => {
    if (command === 'start_line_round') { rounds.push(input.round_id); assert.equal(input.video_id, video); return view(input.round_id, true, line.line_id); }
    if (command === 'cancel_line_round') return { ok: true };
    if (command === 'play_line') { if (++plays === 1) throw new ApiFailure({code:'SESSION_EXPIRED',message:'expired',retryable:true}); return { context, resolution }; }
    throw new Error(command);
  });
  await app.startServiceRound(video);
  assert.equal(rounds.length, 2); assert.notEqual(rounds[0], rounds[1]);
  assert.equal(root.route.name, 'playback');
});

test('late lookup after cancellation is retired', async () => {
  let complete!: (value: unknown) => void;
  let round = ''; let cancelled = 0;
  const { app } = setup(async (command, input) => {
    if (command === 'start_line_round') { round = input.round_id; return await new Promise(resolve => { complete = resolve; }); }
    if (command === 'cancel_line_round') { cancelled++; return {ok:true}; }
    throw new Error(command);
  });
  const pending = app.startServiceRound(video);
  await new Promise(resolve => setTimeout(resolve, 0));
  app.invalidatePlayback(); complete(view(round, true, line.line_id)); await pending;
  assert.ok(cancelled >= 1); assert.equal(app.serviceRound.open, false);
});

test('migration keeps local configuration if service changes during import', async t => {
  const prior = Object.getOwnPropertyDescriptor(globalThis, 'window');
  const state = createAppState();
  const local = { kind:'maccms' as const, config_id:'first', name:'First', enabled:true, api_url:'https://example.test/api' };
  state.settings = { ...defaultSettings(), source_configs:[local], playback_service:{enabled:true,url:'http://service',token:'token',groups:[]} };
  let stored = JSON.parse(JSON.stringify(state.settings));
  const settings = { update: async (fn: (value: typeof stored) => typeof stored) => stored = fn(stored) };
  Object.defineProperty(globalThis, 'window', { configurable:true, value:{ __TAURI__:{ core:{ invoke:async (_command:string,args:any) => {
    if (args.action === 'import') { stored.playback_service.url = 'http://other'; return {results:[{status:'ok'}]}; }
    return {sources:[]};
  } } } } });
  t.after(() => { if (prior) Object.defineProperty(globalThis, 'window', prior); else Reflect.deleteProperty(globalThis, 'window'); });
  const app = new AppController({call:async () => ({})} as any, settings as any, state, null);
  await app.migratePlaybackSources();
  assert.equal(stored.source_configs.length, 1);
  assert.match(app.serviceSources.error, /变化/);
});
