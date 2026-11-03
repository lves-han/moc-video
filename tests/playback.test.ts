import test from "node:test";
import assert from "node:assert/strict";
import { Api, UnknownCommit } from "../src/api.ts";
import { context, deferred, fixture, flush, HOST } from "./fixtures/t12_player.ts";

test("T12 formal PlaybackViewSession binds episode, progressive transport and exact PlayerEvent state", async () => {
  const f = fixture();
  await f.controller.load(context());
  assert.equal(f.root.playback.phase, "preparing");
  assert.equal(f.video.src, f.root.playback.session.media_url);
  assert.equal(f.root.playback.session.episode_ref.data, "one");
  f.ready(); await flush();
  assert.equal(f.root.playback.phase, "playing");
  assert.equal(f.root.playback.duration_ms, 30000);
  assert.equal(f.root.playback.context.selected_stream_id, "a");
  f.video.dispatchEvent(new Event("waiting"));
  assert.equal(f.root.playback.phase, "buffering");
  f.controller.pause();
  assert.equal(f.root.playback.phase, "paused");
  f.controller.seek(99);
  assert.equal(f.video.currentTime, 30);
  f.controller.seek(-2);
  assert.equal(f.video.currentTime, 0);
  f.controller.setVolume(.3); f.controller.setMuted(true); f.controller.setSpeed(1.5);
  assert.equal(f.video.volume, .3); assert.equal(f.video.muted, true); assert.equal(f.video.playbackRate, 1.5);
  f.controller.selectSubtitle("zh"); assert.equal(f.video.textTracks[0].mode, "showing");
  f.controller.selectSubtitle(null); assert.equal(f.video.textTracks[0].mode, "disabled");
  await f.controller.stop();
  assert.equal(f.root.playback.phase, "idle");
});

test("T12 protocol events have schema fields only; legacy API remains compatible", () => {
  const f = fixture(); const legacy: any[] = [], protocol: any[] = [];
  const player = f.playerModule.createPlayer(f.video, (v: unknown) => legacy.push(v), (v: unknown) => protocol.push(v));
  player.open({ kind: "mp4", url: "http://127.0.0.1/video" }, { playback_epoch: 7, engine_epoch: HOST, session_id: "session" });
  assert.deepEqual(Object.keys(protocol.at(-1)).sort(), ["engine_epoch", "playback_epoch", "session_id", "type"]);
  assert.equal(protocol.at(-1).playback_epoch, 7); assert.equal(protocol.at(-1).type, "preparing");
  f.ready(Infinity);
  assert.equal(protocol.at(-1).type, "ready"); assert.equal(protocol.at(-1).duration_ms, null);
  assert.equal(legacy.at(-1).state, "ready"); assert.equal(legacy.at(-1).duration_ms, null);
  player.stop();
});

test("T12 unknown duration is null, seeking fails explicitly, later metadata enables seek", async () => {
  const f = fixture(); await f.controller.load(context());
  f.ready(Infinity); await flush();
  assert.equal(f.root.playback.duration_ms, null);
  assert.equal(f.root.playback.phase, "playing");
  assert.throws(() => f.controller.seek(10), /known duration/);
  f.video.duration = 25; f.video.dispatchEvent(new Event("durationchange"));
  assert.equal(f.root.playback.duration_ms, 25000);
  assert.equal(f.root.playback.phase, "playing");
  await f.controller.stop();
});

test("T12 position events coalesce to at most one update every 250ms", async () => {
  const f = fixture(); await f.controller.load(context()); f.ready(); await flush();
  f.video.currentTime = 1; f.video.dispatchEvent(new Event("timeupdate"));
  assert.equal(f.root.playback.position_ms, 1000);
  for (let i = 0; i < 249; i++) { f.clock(1); f.video.currentTime = 2; f.video.dispatchEvent(new Event("timeupdate")); }
  assert.equal(f.root.playback.position_ms, 1000);
  f.clock(1); f.video.currentTime = 3; f.video.dispatchEvent(new Event("timeupdate"));
  assert.equal(f.root.playback.position_ms, 3000);
  await f.controller.stop();
});

for (const [label, mse, native, engine] of [["native preferred", true, "maybe", "native"], ["native only", false, "maybe", "native"], ["MSE only", true, "", "hls.js"]] as const) {
  test(`T12 formal HLS selection: ${label}`, async () => {
    const f = fixture({ mse, native, kind: "hls" });
    await f.controller.load(context());
    assert.equal(f.controller.diagnostics().engine, engine);
    assert.equal(f.streams.length, engine === "hls.js" ? 1 : 0);
    await f.controller.stop(); assert.ok(f.streams.every(s => s.destroyed));
  });
}
test("T12 unavailable HLS and unsupported DASH fail with type reason", async () => {
  for (const options of [{ mse: false, native: "", kind: "hls" }, { kind: "dash" }]) {
    const f = fixture(options); await f.controller.load(context());
    assert.equal(f.root.playback.phase, "failed");
    assert.match(f.root.playback.error.message, /MEDIA_UNSUPPORTED/);
    await f.controller.stop();
  }
});

test("T12 autoplay block is recoverable by explicit play and old play rejection is discarded", async () => {
  const f = fixture();
  f.setPlay(async () => { const error = new Error("gesture required"); error.name = "NotAllowedError"; throw error; });
  await f.controller.load(context()); f.ready(); await flush();
  assert.equal(f.controller.ui.autoplayBlocked, true);
  assert.equal(f.calls.filter(c => c.command === "get_resource_status").length, 0);
  f.setPlay(); await f.controller.play();
  assert.equal(f.root.playback.phase, "playing"); assert.equal(f.controller.ui.autoplayBlocked, false);
  const old = deferred<void>(); f.setPlay(() => old.promise);
  const play = f.controller.play();
  await f.controller.load(context("two"));
  const phase = f.root.playback.phase;
  old.reject(new Error("old playback rejected")); await play;
  assert.equal(f.root.playback.phase, phase); assert.equal(f.root.playback.error, null);
  await f.controller.stop();
});

test("T12 old DOM/HLS/track/EOS events and duplicate current EOS have no side effects", async () => {
  const f = fixture({ kind: "hls", native: "", mse: true });
  await f.controller.load(context()); f.ready(); await flush();
  const old = { playback_epoch: f.root.playback.playback_epoch, engine_epoch: HOST, session_id: f.root.playback.session.session_id };
  const oldTrack = f.video.tracks[0], oldHls = f.streams[0];
  await f.controller.load(context("two"));
  const phase = f.root.playback.phase;
  for (const type of ["ended", "position", "failed", "playing", "paused"]) f.controller.receive({ ...old, type, position_ms: 9000, error: { code: "UPSTREAM_FAILED", message: "old", retryable: true } });
  oldTrack.dispatchEvent(new Event("error")); oldHls.error("error", { fatal: true, type: "networkError", details: "old" });
  f.video.dispatchEvent(new Event("ended")); // queued event while the new video has not ended
  assert.equal(f.root.playback.phase, phase); assert.equal(f.ended.length, 0);
  f.ready(); await flush(); f.video.ended = true; f.video.dispatchEvent(new Event("ended")); f.video.dispatchEvent(new Event("ended"));
  assert.equal(f.root.playback.phase, "ended"); assert.equal(f.ended.length, 1);
  await f.controller.stop();
});

test("T12 50 actual controller switches release single transport, tracks, listeners and captured handles", async () => {
  const f = fixture({ native: "", kind: "hls" });
  let count = 0;
  for (let i = 0; i <= 50; i++) {
    const oldTracks = [...f.video.tracks];
    await f.controller.load(context(`episode-${i}`));
    f.ready(); await flush();
    assert.equal(f.sessions.size, 1); assert.equal(f.resolutions.size, 1);
    assert.equal(f.streams.filter(s => !s.destroyed).length, 1);
    assert.equal(f.video.tracks.length, 1);
    if (i) assert.equal(f.listeners(), count);
    count = f.listeners();
    assert.ok(oldTracks.every(t => t.track.mode === "disabled"));
  }
  await f.controller.stop();
  assert.equal(f.sessions.size, 0); assert.equal(f.resolutions.size, 0); assert.equal(f.listeners(), 0);
  assert.equal(f.video.tracks.length, 0); assert.equal(f.video.src, "");
  const order = f.calls.filter(c => ["close_playback", "release_resolution", "resolve_episode"].includes(c.command));
  for (let i = 1; i < order.length; i += 3) assert.deepEqual(order.slice(i, i + 3).map(c => c.command), ["close_playback", "release_resolution", ...(i + 2 < order.length ? ["resolve_episode"] : [])]);
});

test("T12 same episode stream change keeps Resolution and closes old Session first", async () => {
  const f = fixture(); await f.controller.load(context()); f.ready(); await flush();
  f.controller.pause(); f.controller.seek(12);
  const resolution = f.root.playback.resolution.id;
  await f.controller.selectStream("b"); f.ready(); await flush();
  assert.equal(f.root.playback.resolution.id, resolution);
  assert.equal(f.root.playback.session.stream_id, "b");
  assert.equal(f.root.playback.phase, "ready"); assert.equal(f.video.paused, true); assert.equal(f.video.currentTime, 12);
  assert.equal(f.calls.filter(c => c.command === "resolve_episode").length, 1);
  assert.equal(f.calls.filter(c => c.command === "release_resolution").length, 0);
  await f.controller.stop();
});

for (const target of ["video", "track"] as const) test(`T12 native ${target} error queries once per attempt and preserves actual MediaError`, async () => {
  const f = fixture(); await f.controller.load(context());
  if (target === "video") { f.error(3, "message mentions 401 and 502"); f.error(3, "message mentions 401 and 502"); }
  else { f.video.tracks[0].dispatchEvent(new Event("error")); f.video.tracks[0].dispatchEvent(new Event("error")); }
  await flush();
  assert.equal(f.calls.filter(c => c.command === "get_resource_status").length, 1);
  assert.equal(f.root.playback.error.code, target === "video" ? "MEDIA_UNSUPPORTED" : "UPSTREAM_FAILED");
  assert.equal(f.calls.filter(c => c.command === "runtime_prepare").length, 0);
  if (target === "video") assert.match(f.root.playback.error.message, /401 and 502/);
  await f.controller.stop();
});

test("T12 expired playback resolves/opens once, restores clamped position and paused state", async () => {
  const f = fixture(); await f.controller.load(context()); f.ready(); await flush();
  f.controller.pause(); f.controller.seek(12); f.status("expired", "SESSION_EXPIRED");
  f.error(); await flush(); await f.controller.settled(); f.ready(8); await flush();
  assert.equal(f.video.currentTime, 8); assert.equal(f.video.paused, true);
  assert.equal(f.calls.filter(c => c.command === "resolve_episode").length, 2);
  f.error(); await flush();
  assert.equal(f.root.playback.error.code, "SESSION_EXPIRED");
  assert.equal(f.calls.filter(c => c.command === "resolve_episode").length, 2);
  await f.controller.stop();
});
test("T12 expired playing session restores playing intent, volume, speed and subtitle", async () => {
  const f = fixture(); await f.controller.load(context()); f.ready(); await flush();
  f.controller.seek(6); f.controller.setVolume(.4); f.controller.setSpeed(2); f.controller.selectSubtitle("zh");
  f.status("expired", "SESSION_EXPIRED"); f.error(); await flush(); await f.controller.settled(); f.ready(); await flush();
  assert.equal(f.video.currentTime, 6); assert.equal(f.video.paused, false); assert.equal(f.video.volume, .4);
  assert.equal(f.video.playbackRate, 2); assert.equal(f.video.textTracks[0].mode, "showing");
  await f.controller.stop();
});

test("T12 unknown recovered duration shows actual position while waiting to restore seek", async () => {
  const f = fixture(); await f.controller.load(context()); f.ready(); await flush();
  f.controller.pause(); f.controller.seek(9); f.status("expired", "SESSION_EXPIRED");
  f.error(); await flush(); await f.controller.settled(); f.ready(Infinity); await flush();
  assert.equal(f.root.playback.duration_ms, null); assert.equal(f.root.playback.position_ms, 0);
  assert.equal(f.video.paused, true); assert.match(f.controller.ui.diagnostic, /等待媒体时长/);
  f.video.duration = 25; f.video.dispatchEvent(new Event("durationchange"));
  assert.equal(f.video.currentTime, 9); assert.equal(f.root.playback.position_ms, 9000); assert.equal(f.controller.ui.diagnostic, "");
  await f.controller.stop();
});
test("T12 gateway UNAUTHORIZED reconnects once; UPSTREAM_FAILED/TIMEOUT offer retry; codec keeps type", async () => {
  for (const code of ["UNAUTHORIZED", "UPSTREAM_FAILED", "UPSTREAM_TIMEOUT", "MEDIA_UNSUPPORTED"]) {
    const f = fixture(); await f.controller.load(context()); f.status("failed", code); f.error(2, "opaque browser message");
    await flush(); await f.controller.settled();
    if (code === "UNAUTHORIZED") {
      assert.equal(f.calls.filter(c => c.command === "runtime_prepare").length, 1);
      f.error(); await flush(); assert.equal(f.calls.filter(c => c.command === "runtime_prepare").length, 1);
      assert.equal(f.controller.ui.action, "reconnect");
    } else {
      assert.equal(f.root.playback.error.code, code);
      if (code !== "MEDIA_UNSUPPORTED") assert.equal(f.controller.ui.action, "retry");
    }
    await f.controller.stop();
  }
});

test("T12 structured hls.js HTTP codes use the same once-only diagnosis and recovery", async () => {
  for (const [status, expected] of [[401, "UNAUTHORIZED"], [502, "UPSTREAM_FAILED"], [504, "UPSTREAM_TIMEOUT"]] as const) {
    const f = fixture({ native: "", kind: "hls" }); await f.controller.load(context());
    f.streams[0].error("error", { fatal: true, type: "networkError", details: "opaque", response: { code: status } });
    await flush(); await f.controller.settled();
    assert.equal(f.calls.filter(c => c.command === "get_resource_status").length, 1);
    if (expected === "UNAUTHORIZED") assert.equal(f.calls.filter(c => c.command === "runtime_prepare").length, 1);
    else assert.equal(f.root.playback.error.code, expected);
    await f.controller.stop();
  }
});

test("T12 unauthorized status-query envelope invokes engine reconnect", async () => {
  const f = fixture(); await f.controller.load(context());
  f.intercept((command, _args, value) => command === "get_resource_status" ? {
    request_id: value.request_id, engine_epoch: HOST, error: { code: "UNAUTHORIZED", message: "reconnect engine", retryable: true },
  } : value);
  f.error(); await flush(); await f.controller.settled();
  assert.equal(f.calls.filter(c => c.command === "runtime_prepare").length, 1);
  await f.controller.stop();
});

test("T12 old status response is discarded after episode, source revision or host changes", async () => {
  for (const change of ["episode", "revision", "host"]) {
    const f = fixture(); await f.controller.load(context());
    const late = deferred<unknown>(); let envelope: any;
    f.intercept((command, _args, data) => { if (command === "get_resource_status") { envelope = data; return late.promise; } return data; });
    f.error(); await flush();
    if (change === "episode") await f.controller.load(context("two"));
    else if (change === "revision") f.root.playback.context.episode_ref.source_revision = "new-rev";
    else f.root.runtime.engine_epoch = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
    const before = JSON.stringify(f.root.playback);
    late.resolve({ ...envelope, data: { ...envelope.data, state: "expired", error: { code: "SESSION_EXPIRED", message: "old", retryable: true } } });
    await flush(); await f.controller.settled();
    assert.equal(JSON.stringify(f.root.playback), before);
    await f.controller.stop();
  }
});

test("T12 resource query identity/target/transport failures retain original diagnostics and retry", async () => {
  for (const fault of ["request", "host", "target", "transport"]) {
    const f = fixture(); await f.controller.load(context());
    f.intercept((command, _args, envelope) => {
      if (command !== "get_resource_status") return envelope;
      if (fault === "transport") throw new Error("offline");
      if (fault === "request") return { ...envelope, request_id: crypto.randomUUID() };
      if (fault === "host") return { ...envelope, engine_epoch: "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb" };
      return { ...envelope, data: { ...envelope.data, target_id: "wrong-target" } };
    });
    f.error(3, "native decoder failure"); await flush();
    assert.match(f.root.playback.error.message, /native decoder failure/);
    assert.match(f.controller.ui.diagnostic, /查询失败/); assert.equal(f.controller.ui.action, "retry");
    await f.controller.stop();
  }
});

test("T12 late open is closed before captured Resolution release and next resolve", async () => {
  const f = fixture(); const old = deferred<unknown>(); let envelope: any;
  f.intercept((command, _args, value) => { if (command === "open_playback" && !envelope) { envelope = value; return old.promise; } return value; });
  const first = f.controller.load(context("one")); await flush();
  const second = f.controller.load(context("two"));
  old.resolve(envelope); await first; await second;
  assert.equal(f.sessions.size, 1); assert.equal(f.resolutions.size, 1);
  assert.equal(f.root.playback.session.episode_ref.data, "two");
  assert.deepEqual(f.calls.filter(c => c.command !== "cancel_request").map(c => c.command), ["resolve_episode", "open_playback", "close_playback", "release_resolution", "resolve_episode", "open_playback"]);
  await f.controller.stop();
});

test("T12 cleanup failure stays retryable and its exact old handle is retried before new resolve", async () => {
  const f = fixture(); await f.controller.load(context()); let failed = false;
  f.intercept((command, _args, value) => {
    if (command === "close_playback" && !failed) { failed = true; return { request_id: value.request_id, engine_epoch: HOST, error: { code: "UPSTREAM_FAILED", message: "close failed", retryable: true } }; }
    return value;
  });
  await f.controller.load(context("two"));
  assert.equal(f.root.playback.phase, "failed");
  assert.equal(f.calls.filter(c => c.command === "resolve_episode").length, 1);
  await f.controller.retry();
  assert.equal(f.calls.filter(c => c.command === "close_playback").length, 2);
  assert.equal(f.root.playback.session.episode_ref.data, "two");
  await f.controller.stop();
});

function imageData() {
  return { details: { ref: { source_id: "metadata", source_revision: "meta-rev", data: "show" } }, images: { poster: { resource_id: "poster", url: "http://127.0.0.1/image" } }, metadata_ref: null };
}
test("T12 native image LRU expiry queries once and refreshes only its owning DTO once", async () => {
  const f = fixture(); f.root.details.data = imageData(); f.root.details.request_epoch = 4; f.status("expired", "SESSION_EXPIRED");
  const dto = f.root.details.data;
  await Promise.all([f.controller.imageError(dto, "poster", 4), f.controller.imageError(dto, "poster", 4)]);
  assert.equal(f.calls.filter(c => c.command === "get_resource_status").length, 1);
  assert.equal(f.calls.filter(c => c.command === "get_details").length, 1);
  assert.equal(f.calls.find(c => c.command === "get_resource_status")!.input.kind, "image");
  const replacement = f.root.details.data;
  await f.controller.imageError(replacement, "replacement", 4);
  assert.equal(f.calls.filter(c => c.command === "get_details").length, 1);
  assert.match(f.controller.ui.imageError, /SESSION_EXPIRED/);
});
test("T12 old image queries and refreshed DTOs cannot replace a newer page", async () => {
  for (const phase of ["query", "refresh"]) {
    const f = fixture(); f.root.details.data = imageData(); f.root.details.request_epoch = 4; f.status("expired", "SESSION_EXPIRED");
    const late = deferred<unknown>(); let saved: any;
    f.intercept((command, _args, value) => {
      if (command === (phase === "query" ? "get_resource_status" : "get_details")) { saved = value; return late.promise; }
      return value;
    });
    const pending = f.controller.imageError(f.root.details.data, "poster", 4); await flush();
    f.root.details.request_epoch = 5; f.root.details.data = { ...imageData(), metadata_ref: "new-page" };
    late.resolve(saved); await pending;
    assert.equal(f.root.details.data.metadata_ref, "new-page"); assert.equal(f.controller.ui.imageError, "");
  }
});
test("T12 image ready and query failures preserve visible retry diagnostics", async () => {
  const f = fixture(); f.root.details.data = imageData();
  await f.controller.imageError(f.root.details.data, "poster", 0); assert.match(f.controller.ui.imageError, /加载失败/);
  f.root.details.data = imageData();
  f.intercept((command, _args, value) => { if (command === "get_resource_status") throw new Error("query offline"); return value; });
  await f.controller.imageError(f.root.details.data, "poster", 0); assert.match(f.controller.ui.imageError, /查询失败/);
});

test("T12 API rejects changed Host epoch and retains the 3s read-only resource budget", async () => {
  const api = new Api(async (_command, args) => ({ request_id: args.request_id, engine_epoch: HOST, data: { ok: true } }));
  assert.equal(api.budget("get_resource_status", { kind: "playback", target_id: "session" }), 3000);
  await assert.rejects(api.call("close_playback", { session_id: "session" }, crypto.randomUUID(), "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb"), UnknownCommit);
});
