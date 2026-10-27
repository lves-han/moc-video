import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { runInNewContext } from "node:vm";
import ts from "typescript";

// Execute the real automation module against deterministic media/DOM doubles.
// These tests verify failure telemetry; real decoding remains the native A1 gate.
const source = ts.transpileModule(readFileSync(new URL("./fixtures/t01/automation.ts", import.meta.url), "utf8"), {
  compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS },
}).outputText;

async function scenario(fault: string) {
  let now = 1;
  let frame: ((time: number, metadata: object) => void) | undefined;
  let pulse: (() => void) | undefined;
  const rect = { toJSON: () => ({ left: 0, top: 0, width: 960, height: 540 }) };
  const video = Object.assign(new EventTarget(), {
    currentTime: 1, duration: 30, paused: false, ended: false, seeking: false,
    readyState: 4, networkState: 1, loop: false, playbackRate: 1, volume: 1, muted: false,
    videoWidth: 1920, videoHeight: 1080, error: null as null | { code: number },
    buffered: { length: 1, start: () => 0, end: () => 30 },
    requestVideoFrameCallback: (callback: typeof frame) => { frame = callback; return 1; },
    cancelVideoFrameCallback: () => { frame = undefined; },
    getVideoPlaybackQuality: () => ({ totalVideoFrames: now / 1000 * 30, droppedVideoFrames: 0 }),
    getBoundingClientRect: () => rect,
  });
  const document = Object.assign(new EventTarget(), {
    visibilityState: "visible", hasFocus: () => true,
    createElement: () => ({ width: 0, height: 0, toDataURL: () => "synthetic-preview",
      getContext: () => ({ drawImage: () => {}, getImageData: () => ({ data: new Uint8Array([64, now / 5000 % 255, 0, 255]) }) }) }),
  });
  const window = Object.assign(new EventTarget(), { __T01_BOOTSTRAP__: { base: "http://127.0.0.1:1234/", cap: "synthetic-secret" } });
  const player = { setSpeed: () => {}, setVolume: () => {}, stop: () => {}, diagnostics: () => ({ engine: "native" }) };
  const results: any[] = [];
  const samples: any[] = [];
  const native = { visible: true, focused: true, minimized: false };
  let next = 0;
  const exports: Record<string, any> = {};
  runInNewContext(source, {
    exports, Event, AbortController, Float32Array, Uint8Array,
    require: () => ({ nativeInvoke: async (_command: string, args: { action: string }) => {
      if (fault === "native-error" && args.action === "identity") throw new Error("native identity unavailable");
      return native;
    } }),
    window, document, performance: { now: () => now },
    requestAnimationFrame: (callback: typeof pulse) => { pulse = callback; return 1; },
    cancelAnimationFrame: () => { pulse = undefined; },
    setTimeout: (callback: () => void, ms: number) => {
      now += ms;
      video.currentTime = (video.currentTime + ms / 1000) % 30;
      if (fault !== "frames") frame?.(now, { mediaTime: video.currentTime, presentedFrames: now / 1000 * 30 });
      if (fault !== "raf") pulse?.();
      if (fault === "paused" || fault === "native-error") { video.paused = true; video.dispatchEvent(new Event("pause")); }
      if (fault === "media-error") video.error = { code: 3 };
      if (fault === "resolution") video.videoWidth = 1280;
      if (fault === "clock") video.currentTime = 1;
      if (fault === "events") {
        document.visibilityState = "hidden";
        document.dispatchEvent(new Event("visibilitychange"));
        for (let i = 0; i < 80; i++) video.dispatchEvent(new Event("seeking"));
        video.paused = true;
      }
      queueMicrotask(callback);
    },
    AudioContext: class {
      state = "running"; sampleRate = 48000; destination = { maxChannelCount: 2 };
      createMediaElementSource() { return { connect() {}, disconnect() {} }; }
      createAnalyser() { return { fftSize: 2048, frequencyBinCount: 1024, connect() {}, disconnect() {},
        getFloatTimeDomainData: (bins: Float32Array) => bins.fill(fault === "audio" ? 0 : 0.1),
        getFloatFrequencyData: (bins: Float32Array) => { bins.fill(-100); bins[19] = -1; },
      }; }
      async close() { this.state = "closed"; }
    },
    fetch: async (url: string, options?: { body: string }) => {
      const path = new URL(url).pathname;
      if (path === "/control/next") return { status: 200, ok: true, json: async () => ++next === 1
        ? { id: 1, op: "long", seconds: 1800 } : { id: 2, op: "shutdown" } };
      const value = JSON.parse(options!.body);
      (path === "/control/sample" ? samples : results).push(value);
      return { ok: true };
    },
  });
  await exports.automate(video, player, () => {});
  return { result: results.find(result => result.id === 1), samples, native };
}

for (const fault of ["paused", "media-error", "frames", "raf", "resolution", "audio", "clock"]) {
  test(`T01 ${fault} failure preserves the exact interval before its assertion`, async () => {
    const { result, samples } = await scenario(fault);
    assert.equal(result.ok, false);
    assert.equal(samples.length, 0);
    const { previous, current, elapsed_ms, index } = result.diagnostics.long;
    assert.equal(index, 0);
    assert.equal(elapsed_ms, 5000);
    assert.equal(previous.position, 1);
    assert.equal(current.time - previous.time, 5000);
    assert.deepEqual(result.diagnostics.native, { visible: true, focused: true, minimized: false });
    assert.equal(result.diagnostics.native_error, null);
    if (fault === "paused") assert.equal(current.paused, true);
    if (fault === "media-error") assert.equal(current.media_error, 3);
    if (fault === "frames") assert.equal(current.frames, previous.frames);
    if (fault === "raf") assert.equal(current.raf, previous.raf);
    if (fault === "resolution") assert.equal(current.width, 1280);
    if (fault === "audio") assert.equal(current.audio_rms, 0);
    if (fault === "clock") assert.equal(current.position, previous.position);
    assert.ok(!JSON.stringify(result).includes("synthetic-secret"));
  });
}

test("T01 native diagnostic failure preserves the original media failure and snapshot", async () => {
  const { result } = await scenario("native-error");
  assert.match(result.error, /Playback or UI stalled/);
  assert.equal(result.diagnostics.long.current.paused, true);
  assert.equal(result.diagnostics.native, null);
  assert.match(result.diagnostics.native_error, /native identity unavailable/);
});

test("T01 event history remains bounded and retains the latest media/visibility state", async () => {
  const { result } = await scenario("events");
  assert.equal(result.diagnostics.long.events.length, 64);
  assert.equal(result.diagnostics.long.current.visibility, "hidden");
  assert.equal(result.diagnostics.long.events.at(-1).type, "seeking");
  assert.equal(result.diagnostics.long.events.at(-1).visibility, "hidden");
});

test("T01 healthy simulated intervals still require all 1800 seconds and changing frames", async () => {
  const { result, samples } = await scenario("healthy");
  assert.equal(result.ok, true);
  assert.equal(result.value.duration_seconds, 1800);
  assert.equal(result.value.media_seconds, 1800);
  assert.equal(result.value.samples, 360);
  assert.equal(samples.length, 360);
  assert.ok(result.value.unique_frames > 2);
});
