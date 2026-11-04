import { test } from 'node:test';
import assert from 'node:assert/strict';
import { historyRepository } from '../src/data/history.ts';
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
