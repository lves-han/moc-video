import assert from 'node:assert/strict';
import test from 'node:test';
import { playableSourceGroups } from '../src/state.ts';

test('one source presents its fastest verified line and hides unverified sources', () => {
  const sources = [{ source_id: 'one', name: 'One', status: 'done' }, { source_id: 'two', name: 'Two', status: 'done' }];
  const lines = [
    { source_id: 'one', line_id: 'slow', probe_status: 'ready', latency_ms: 900 },
    { source_id: 'one', line_id: 'fast', probe_status: 'ready', latency_ms: 120 },
    { source_id: 'two', line_id: 'bad', probe_status: 'failed', latency_ms: 10 },
    { source_id: 'two', line_id: 'pending', probe_status: 'testing', latency_ms: 1 },
  ];
  const groups = playableSourceGroups({ sources, lines } as never);
  assert.equal(groups.length, 1);
  assert.equal(groups[0]!.source_id, 'one');
  assert.deepEqual(groups[0]!.lines.map(line => line.line_id), ['fast']);
});

test('sources and lines with actual successful playback take priority', () => {
  const sources = [
    { source_id: 'failed', name: 'Failed', status: 'done', last_playback: 'failed' },
    { source_id: 'new', name: 'New', status: 'done' },
    { source_id: 'verified', name: 'Verified', status: 'done', last_playback: 'playing' },
  ];
  const lines = [
    { source_id: 'failed', line_id: 'failed-source', probe_status: 'ready', latency_ms: 10 },
    { source_id: 'new', line_id: 'new-line', probe_status: 'ready', latency_ms: 20 },
    { source_id: 'verified', line_id: 'fast', probe_status: 'ready', latency_ms: 1, history: { last_ok_at: null } },
    { source_id: 'verified', line_id: 'played', probe_status: 'ready', latency_ms: 900, history: { last_ok_at: 123 } },
  ];
  const groups = playableSourceGroups({ sources, lines } as never);
  assert.deepEqual(groups.map(group => group.source_id), ['verified', 'new', 'failed']);
  assert.equal(groups[0]!.lines[0]!.line_id, 'played');
});
