import test from 'node:test';
import assert from 'node:assert/strict';
import { context, deferred, fixture, flush } from './fixtures/t12_player.ts';

function localSession(id = 'local-session') {
  return { session_id:id, resolution_id:'local-resolution', stream_id:'file', source_id:'local', source_revision:'local',
    episode_ref:{source_id:'local',source_revision:'local',data:'local-file'}, stream_kind:'progressive', mime_type:'video/mp4',
    media_url:'http://127.0.0.1/local-video', subtitles:[], expires_at:'2030-01-01T00:00:00Z' };
}

test('local video uses the shared player and restores its saved position', async () => {
  const f = fixture(); const session = localSession(); f.sessions.set(session.session_id, session);
  await f.controller.loadLocal(session, 7000);
  assert.equal(f.video.src, session.media_url);
  f.ready(30); await flush();
  assert.equal(f.video.currentTime, 7);
  assert.equal(f.root.playback.phase, 'playing');
  assert.equal(f.root.playback.session.session_id, session.session_id);
  await f.controller.stop();
  assert.equal(f.sessions.size, 0);
});

test('local resume near the end starts from the beginning', async () => {
  const f = fixture(); const session = localSession(); f.sessions.set(session.session_id, session);
  await f.controller.loadLocal(session, 27000); f.ready(30); await flush();
  assert.equal(f.video.currentTime, 0);
  await f.controller.stop();
});

test('cancelling a queued local session closes the captured session', async () => {
  const f = fixture(); await f.controller.load(context());
  const gate = deferred<void>(); let blocked = false;
  f.intercept(async (command, _args, envelope) => { if (command === 'close_playback' && !blocked) { blocked = true; await gate.promise; } return envelope; });
  const session = localSession(); f.sessions.set(session.session_id, session);
  const opening = f.controller.loadLocal(session, 0);
  await flush();
  const closing = f.controller.stop(); gate.resolve();
  await Promise.all([opening, closing]);
  assert.equal(f.sessions.size, 0);
});
