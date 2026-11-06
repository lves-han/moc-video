import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createNativeHistory, entryFromRecord, historyRepository } from '../src/data/history.ts';
import { ApiFailure, SettingsQueue, UnknownCommit, finishBeforeClose } from '../src/api.ts';
import { parseHistoryEntry, historyProgress, historyTime, type HistoryEntry } from '../src/data/history-model.ts';
import { historyCompletionReady } from './fixtures/t13/history-completion.js';

const entry = (changes: Partial<HistoryEntry> = {}): HistoryEntry => ({ content_id:'demo:series:midnight', title:'Letters After Midnight', poster:'/artwork/city.webp', kind:'series', episode:4, episode_total:12, source_id:'demo-cloudstream', source_name:'Cloud Pictures', position_ms:12000, duration_ms:30000, completed:false, updated_at:1000, ...changes });

test('Preview history keeps the latest episode per title and its independent playback source', async () => {
  await historyRepository.clear();
  await historyRepository.save(entry());
  await historyRepository.save(entry({episode:5,source_id:'another-provider',updated_at:2000}));
  await historyRepository.save(entry({updated_at:1500}));
  const rows=await historyRepository.list();
  assert.equal(rows.length,1);
  assert.equal(rows[0].episode,5);
  assert.equal(rows[0].source_id,'another-provider');
  rows[0].episode=1;
  assert.equal((await historyRepository.get(entry().content_id))?.episode,5);
});

test('Preview history sorts recent titles, removes one title and clears the list', async () => {
  await historyRepository.clear();
  await historyRepository.save(entry());
  await historyRepository.save(entry({content_id:'demo:movie:other',kind:'movie',episode:1,episode_total:1,updated_at:3000}));
  assert.equal((await historyRepository.list())[0].content_id,'demo:movie:other');
  await historyRepository.remove('demo:movie:other');
  assert.equal(await historyRepository.get('demo:movie:other'),null);
  assert.equal((await historyRepository.list()).length,1);
  await historyRepository.clear();
  assert.deepEqual(await historyRepository.list(),[]);
});

test('History rejects invalid progress, dates, completion and transient session fields', () => {
  for (const changes of [{position_ms:-1},{position_ms:31000},{duration_ms:0},{episode:0},{episode_total:3},{updated_at:9e15}]) assert.throws(()=>parseHistoryEntry(entry(changes)),TypeError);
  assert.throws(()=>parseHistoryEntry({...entry(),session_url:'http://localhost/media?cap=secret'}),TypeError);
  assert.throws(()=>parseHistoryEntry({...entry(),completed:1}),TypeError);
  assert.equal(historyProgress(entry()),40);
  assert.equal(historyProgress(entry({duration_ms:null})),0);
  assert.equal(historyTime(null),'--:--');
  assert.equal(historyTime(3661000),'1:01:01');
});

function completionEvidence() {
  const binding = { element: 3, epoch: 120, engineEpoch: 'host-current', session: 'session-current',
    resolution: 'resolution-current', match: 'match-current', video: 'midnight:3', source: 'playback-source', metadata: 'metadata-current' };
  return {
    expected: { ...binding, contentId: entry().content_id, episode: 3, historySource: entry().source_id, started: 100, deadline: 12100 },
    current: { ...binding, route: 'playback', phase: 'ended', ended: true },
    nativeEnded: { type: 'ended', trusted: true, ended: true, timestamp: 200, at: 201, playback: { ...binding } },
    committed: { at: 202, playback: { ...binding, phase: 'ended' }, entry: entry({ episode: 3, completed: true }) },
    observedAt: 203,
  };
}
const ready = (evidence: ReturnType<typeof completionEvidence>) => historyCompletionReady(
  evidence.expected, evidence.current, evidence.nativeEnded, evidence.committed, evidence.observedAt);

test('T13 history completion waits for current trusted ended, controller and actual save commit', () => {
  const evidence = completionEvidence();
  assert.equal(ready(evidence), true);
  // A completed entry is accepted independently of position percentage: only
  // the production completion flag and real event/identity chain decide EOS.
  evidence.committed.entry.position_ms = 29800;
  assert.equal(ready(evidence), true);
});

test('T13 history completion rejects old video, epoch, session and other binding mismatches at every stage', () => {
  for (const stage of ['current', 'nativeEnded', 'committed'] as const) {
    for (const key of ['element', 'epoch', 'engineEpoch', 'session', 'resolution', 'match', 'video', 'source', 'metadata'] as const) {
      const evidence = completionEvidence();
      const binding = stage === 'current' ? evidence.current : evidence[stage].playback;
      Object.assign(binding, { [key]: typeof binding[key] === 'number' ? -1 : 'old-identity' });
      assert.equal(ready(evidence), false, `${stage}/${key}`);
    }
  }
});

test('T13 history completion rejects property-only, synthetic, state-only and uncommitted completion', () => {
  const changes = [
    { nativeEnded: null }, { committed: null },
    { nativeEnded: { ...completionEvidence().nativeEnded, trusted: false } },
    { nativeEnded: { ...completionEvidence().nativeEnded, type: 'timeupdate' } },
    { nativeEnded: { ...completionEvidence().nativeEnded, ended: false } },
    { current: { ...completionEvidence().current, phase: 'playing' } },
    { current: { ...completionEvidence().current, ended: false } },
    { current: { ...completionEvidence().current, route: 'history' } },
  ];
  for (const change of changes) assert.equal(ready(Object.assign(completionEvidence(), change)), false);
  for (const change of [{ completed: false }, { content_id: 'other-title' }, { episode: 2 }, { source_id: 'other-source' }]) {
    const evidence = completionEvidence();
    Object.assign(evidence.committed.entry, change);
    assert.equal(ready(evidence), false, JSON.stringify(change));
  }
  const evidence = completionEvidence();
  evidence.committed.playback.phase = 'playing';
  assert.equal(ready(evidence), false);
});

test('T13 history completion shares one 12s deadline and rejects stale event or pre-event save', () => {
  for (const change of [
    (e: ReturnType<typeof completionEvidence>) => { e.observedAt = 12101; },
    (e: ReturnType<typeof completionEvidence>) => { e.expected.deadline += 12000; },
    (e: ReturnType<typeof completionEvidence>) => { e.nativeEnded.timestamp = 99; },
    (e: ReturnType<typeof completionEvidence>) => { e.nativeEnded.timestamp = Infinity; },
    (e: ReturnType<typeof completionEvidence>) => { e.nativeEnded.at = 99; },
    (e: ReturnType<typeof completionEvidence>) => { e.committed.at = 200; },
    (e: ReturnType<typeof completionEvidence>) => { e.committed.at = 204; },
  ]) {
    const evidence = completionEvidence(); change(evidence); assert.equal(ready(evidence), false);
  }
  const evidence = completionEvidence(); evidence.observedAt = evidence.expected.deadline;
  assert.equal(ready(evidence), true);
});

const nativeRecord = { content_id: 'a'.repeat(64), metadata_identity: { config_id: 'c', addon_key: 'b'.repeat(64), namespace: 'n', type: 'series', meta_id: 'm', canonical_id: 'm' }, video_id: 'v', season_number: 1, episode_number: 2.5, title: 'T', kind: 'other', poster_cache_key: null, source_id: 's', source_name: 'S', position_ms: 10, duration_ms: 20, completed: false, updated_at: 5 } as never;
function nativeFixture() {
  const calls: { command: string; input: any }[] = []; const failures: unknown[] = [];
  let sequence = 4;
  const history = createNativeHistory((async (command: string, input: any) => {
    calls.push({ command, input });
    const failure = failures.shift();
    if (failure) throw failure;
    if (command === 'list_history') return { entries: [nativeRecord], write_sequence: sequence };
    if (input.write_sequence) sequence = input.write_sequence;
    return { ok: true };
  }) as never);
  const snapshot = { ...entryFromRecord(nativeRecord), capture: { session_id: 'x', match_id: 'not-uuid', playback_epoch: 3 } };
  return { calls, failures, history, snapshot, writes: () => calls.filter(call => call.command !== 'list_history').map(call => call.input.write_sequence) };
}
test('REQ-T18: native writes read the Host sequence and keep order through one queue', async () => {
  const { history, snapshot, writes, calls } = nativeFixture();
  await Promise.all([history.save(snapshot), history.remove('a'.repeat(64)), history.clear()]);
  assert.deepEqual(writes(), [5, 6, 7]);
  assert.deepEqual(calls.filter(call => call.command !== 'list_history').map(call => call.command), ['save_history', 'remove_history', 'clear_history']);
  assert.equal(entryFromRecord(nativeRecord).kind, 'series');
  assert.equal(entryFromRecord(nativeRecord).episode, 3);
});
test('REQ-T18: a failed snapshot retries on its sequence and an unknown result rereads it', async () => {
  const { history, snapshot, failures, writes, calls } = nativeFixture();
  await history.list();
  failures.push(new Error('disk'));
  await assert.rejects(history.save(snapshot), /disk/);
  await history.save(snapshot);
  assert.deepEqual(writes(), [5, 5]);
  failures.push(new UnknownCommit('r'));
  const other = { ...snapshot };
  await assert.rejects(history.save(other));
  const before = calls.filter(call => call.command === 'list_history').length;
  await history.clear();
  assert.equal(calls.filter(call => call.command === 'list_history').length, before + 1, 'unknown commit rereads the sequence');
  await assert.rejects(history.save({ ...snapshot, capture: undefined }), /capture/);
  await history.drain();
});

test('REQ-T18: a refused snapshot leaves the failed one and its sequence for retry', async () => {
  const { history, snapshot, failures, writes } = nativeFixture();
  await history.list();
  failures.push(new Error('disk'));
  await assert.rejects(history.save(snapshot));
  failures.push(new ApiFailure({ code: 'SESSION_EXPIRED', message: 'closed', retryable: false } as never));
  await assert.rejects(history.save({ ...snapshot }));
  await history.save(snapshot);
  assert.deepEqual(writes(), [5, 6, 5]);
});

test('REQ-T18: reading history never lowers the sequence after a failed write', async () => {
  const { history, snapshot, failures, writes } = nativeFixture();
  await history.list();
  failures.push(new Error('disk'));
  await assert.rejects(history.save(snapshot));
  await history.list();
  await history.save({ ...snapshot });
  assert.deepEqual(writes(), [5, 6], 'the new snapshot takes a new number');
  await history.save(snapshot);
  assert.deepEqual(writes(), [5, 6, 7], 'superseded by the newer write, so it is no longer a same-sequence retry');
});
test('R43/REQ-T18: closing the settings queue waits for what is queued', async () => {
  const saved: unknown[] = []; let release: () => void = () => {};
  const held = new Promise<void>(resolve => { release = resolve; });
  const queue = new SettingsQueue({ call: async (command: string, input: unknown) => {
    if (command === 'get_settings') return { volume: 1 };
    await held; saved.push(input); return input;
  } } as never);
  await queue.initialize();
  const pending = queue.update(settings => ({ ...settings, volume: .42 }));
  let closed = false;
  const closing = queue.close().then(() => { closed = true; });
  await new Promise(resolve => setTimeout(resolve, 10));
  assert.equal(closed, false);
  await assert.rejects(queue.update(settings => settings), /closing/);
  release();
  await Promise.all([pending, closing]);
  assert.deepEqual(saved, [{ volume: .42 }]);
});

test('R43/REQ-T18: closing waits for settings, keeps the window when they fail, and takes changes again', async () => {
  const steps: string[] = [];
  const settings = { close: async () => { steps.push('close'); }, reopen: () => steps.push('reopen') };
  const release = async () => { steps.push('release'); };
  const abort = async () => { steps.push('abort'); };
  await finishBeforeClose({ saveHistory: async () => { steps.push('history'); throw new Error('history keeps its own notice'); }, saveSettings: async () => { steps.push('settings'); }, settings, release, abort });
  assert.deepEqual(steps, ['history', 'settings', 'close', 'release']);
  steps.length = 0;
  await assert.rejects(finishBeforeClose({ saveHistory: async () => {}, saveSettings: async () => { throw new Error('unresolved'); }, settings, release, abort }), /unresolved/);
  assert.deepEqual(steps, ['reopen', 'abort'], 'the window is not released and the next close starts over');
  steps.length = 0;
  await assert.rejects(finishBeforeClose({ saveHistory: async () => {}, saveSettings: async () => {}, settings: { close: async () => { throw new Error('paused'); }, reopen: () => steps.push('reopen') }, release, abort }), /paused/);
  assert.deepEqual(steps, ['reopen', 'abort']);
  const queue = new SettingsQueue({ call: async (command: string, input: unknown) => command === 'get_settings' ? { volume: 1 } : input } as never);
  await queue.initialize();
  await queue.close();
  await assert.rejects(queue.update(value => value), /closing/);
  queue.reopen();
  assert.equal((await queue.update(value => ({ ...value, volume: .5 }))).volume, .5);
});
