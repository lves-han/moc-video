import { test } from 'node:test';
import assert from 'node:assert/strict';
import { historyRepository } from '../src/data/history.ts';
import { parseHistoryEntry, historyProgress, historyTime, type HistoryEntry } from '../src/data/history-model.ts';

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
