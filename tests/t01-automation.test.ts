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

async function scenario(fault: string, operation = "long", skipAudioSampling = false) {
  let now = 1;
  let stopped = false;
  let focused = fault !== "needs-activation";
  let frame: ((time: number, metadata: object) => void) | undefined;
  let pulse: (() => void) | undefined;
  const rect = { toJSON: () => ({ left: 0, top: 0, width: 960, height: 540 }) };
  const video = Object.assign(new EventTarget(), {
    currentTime: 1, duration: 30, paused: false, ended: false, seeking: false,
    currentSrc: "http://fixture.invalid/video?cap=synthetic-secret",
    srcObject: null as object | null,
    hasAttribute: (name: string) => name === "src" && (!stopped || fault === "stop-src"),
    querySelectorAll: (selector: string) => stopped &&
      ((fault === "stop-source" && selector.includes("source")) ||
       (fault === "stop-track" && selector.includes("track"))) ? [{}] : [],
    readyState: 4, networkState: 1, loop: false, playbackRate: 1, volume: 1, muted: false,
    videoWidth: 1920, videoHeight: 1080, error: null as null | { code: number },
    buffered: { length: 1, start: () => 0, end: () => 30 },
    requestVideoFrameCallback: (callback: typeof frame) => { frame = callback; return 1; },
    cancelVideoFrameCallback: () => { frame = undefined; },
    getVideoPlaybackQuality: () => ({ totalVideoFrames: now / 1000 * 30, droppedVideoFrames: 0 }),
    getBoundingClientRect: () => rect,
  });
  const document = Object.assign(new EventTarget(), {
    visibilityState: focused ? "visible" : "hidden", hasFocus: () => focused,
    querySelector: (selector: string) => selector === '[data-action="stop"]' ? { click: () => {
      stopped = true;
      video.paused = fault !== "stop-playing";
      video.networkState = fault === "stop-network" ? 2 : 0;
      video.readyState = fault === "stop-ready" ? 4 : 0;
      video.buffered.length = fault === "stop-buffer" ? 1 : 0;
      video.videoWidth = fault === "stop-decoder" ? 1920 : 0;
      video.videoHeight = 0;
      video.srcObject = fault === "stop-object" ? {} : null;
    } } : fault === "ui-missing" ? null : ({
      value: String(fault === "ui-stalled" ? 1 : fault === "ui-wrong" ? video.currentTime + 5 : video.currentTime),
    }),
    createElement: () => ({ width: 0, height: 0, toDataURL: () => "synthetic-preview",
      getContext: () => ({ drawImage: () => {}, getImageData: () => ({ data: new Uint8Array([64, now / 5000 % 255, 0, 255]) }) }) }),
  });
  const window = Object.assign(new EventTarget(), { __T01_BOOTSTRAP__: { base: "http://127.0.0.1:1234/", cap: "synthetic-secret" } });
  const player = { setSpeed: () => {}, setVolume: () => {}, stop: () => {}, diagnostics: () => ({
    engine: "native", listening: fault === "stop-listening", hls_instances: fault === "stop-hls" ? 1 : 0,
  }) };
  const results: any[] = [];
  const samples: any[] = [];
  const native = { visible: true, focused, minimized: false };
  const actions: string[] = [];
  let next = 0;
  let audioReads = 0;
  const exports: Record<string, any> = {};
  runInNewContext(source, {
    exports, Event, AbortController, Float32Array, Uint8Array, URL,
    require: () => ({ nativeInvoke: async (_command: string, args: { action: string }) => {
      actions.push(args.action);
      if (args.action === "foreground") {
        if (fault === "activation-error") throw new Error("native activation unavailable");
        if (fault !== "activation-unconfirmed") {
          native.focused = focused = true;
          document.visibilityState = "visible";
        } else native.focused = false;
      }
      if (fault === "native-error" && args.action === "identity" && now > 1) throw new Error("native identity unavailable");
      return { ...native };
    } }),
    window, document, performance: { now: () => now },
    requestAnimationFrame: (callback: typeof pulse) => { pulse = callback; return 1; },
    cancelAnimationFrame: () => { pulse = undefined; },
    setTimeout: (callback: () => void, ms: number) => {
      now += ms;
      if (!stopped || fault === "stop-clock") video.currentTime = (video.currentTime + ms / 1000) % 30;
      if ((!stopped && fault !== "frames") || fault === "stop-frames") {
        frame?.(now, { mediaTime: video.currentTime, presentedFrames: now / 1000 * 30 });
        if (fault === "stop-frames") frame?.(now, { mediaTime: video.currentTime, presentedFrames: now / 1000 * 30 });
      }
      if (fault !== "raf") pulse?.();
      if (fault === "paused" || fault === "native-error") { video.paused = true; video.dispatchEvent(new Event("pause")); }
      if (fault === "media-error") video.error = { code: 3 };
      if (fault === "resolution") video.videoWidth = 1280;
      if (fault === "clock") video.currentTime = 1;
      if (fault === "hidden" || fault === "transient-hidden") {
        document.visibilityState = "hidden";
        document.dispatchEvent(new Event("visibilitychange"));
        if (fault === "transient-hidden") {
          document.visibilityState = "visible";
          document.dispatchEvent(new Event("visibilitychange"));
        }
      }
      if (fault === "blur") { focused = false; window.dispatchEvent(new Event("blur")); }
      if (fault === "native-unfocused") native.focused = false;
      if (fault === "native-hidden") native.visible = false;
      if (fault === "native-minimized") native.minimized = true;
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
        getFloatTimeDomainData: (bins: Float32Array) => { audioReads++; bins.fill(fault === "audio" ? 0 : 0.1); },
        getFloatFrequencyData: (bins: Float32Array) => { audioReads++; bins.fill(-100); bins[19] = -1; },
      }; }
      async close() { this.state = "closed"; }
    },
    fetch: async (url: string, options?: { body: string }) => {
      const path = new URL(url).pathname;
      if (path === "/control/next") return { status: 200, ok: true, json: async () => ++next === 1
        ? { id: 1, op: operation, seconds: 1800, skip_audio_sampling: skipAudioSampling }
        : { id: 2, op: "shutdown", skip_audio_sampling: skipAudioSampling } };
      const value = JSON.parse(options!.body);
      (path === "/control/sample" ? samples : results).push(value);
      return { ok: true };
    },
  });
  await exports.automate(video, player, () => {});
  return { result: results.find(result => result.id === 1), samples, native, actions, audioReads,
    shutdown: results.find(result => result.id === 2) };
}

test("T24 automation skips PCM/FFT on stop and failure diagnostics, closes AudioContext", async () => {
  for (const [fault, op] of [['healthy', 'stop'], ['paused', 'long']]) {
    const { result, audioReads, shutdown } = await scenario(fault, op, true);
    assert.equal(audioReads, 0);
    const value = result.ok ? result.value.sample : result.diagnostics.current;
    assert.equal(value.audio_rms, null); assert.equal(value.audio_peak_hz, null);
    assert.equal(value.audio_sample_rate, null);
    assert.equal(shutdown.value.audio_state, 'closed');
  }
  assert.ok((await scenario('healthy', 'stop')).audioReads > 0);
});

test("T01 stop confirms released media while currentSrc retains the selected URL", async () => {
  const { result } = await scenario("healthy", "stop");
  assert.equal(result.ok, true);
  assert.equal(result.value.source_removed, true);
  assert.equal(result.value.sample.current_src_present, true);
  assert.equal(result.value.frames_after_stop, 0);
  assert.ok(!JSON.stringify(result).includes("synthetic-secret"));
});

for (const fault of ["playing", "network", "ready", "buffer", "decoder", "object", "src", "source", "track", "listening", "hls", "clock", "frames"]) {
  test(`T01 stop rejects residual ${fault}`, async () => {
    const { result } = await scenario(`stop-${fault}`, "stop");
    assert.equal(result.ok, false);
    assert.match(result.error, /Video resources remain|Old playback still active/);
  });
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
    assert.equal(current.current_src_present, true);
    assert.equal(current.src_attribute_present, true);
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
  const { result, samples, actions } = await scenario("healthy");
  assert.equal(result.ok, true);
  assert.equal(result.value.duration_seconds, 1800);
  assert.equal(result.value.media_seconds, 1800);
  assert.equal(result.value.samples, 360);
  assert.equal(samples.length, 360);
  assert.ok(result.value.unique_frames > 2);
  assert.equal(actions.filter(action => action === "foreground").length, 1);
  assert.equal(result.value.foreground.confirmed.focused, true);
  assert.ok(samples.every(sample => sample.ui_position === sample.position && sample.native.focused));
});

test("T01 foreground activation restores and reads back an initially hidden WebView", async () => {
  const { result, samples, actions } = await scenario("needs-activation");
  assert.equal(result.ok, true);
  assert.deepEqual(actions.slice(0, 2), ["foreground", "identity"]);
  assert.equal(result.value.foreground.visibility, "visible");
  assert.equal(samples.length, 360);
});

for (const fault of ["activation-error", "activation-unconfirmed"]) {
  test(`T01 ${fault} fails before beginning the measured playback`, async () => {
    const { result, samples } = await scenario(fault);
    assert.equal(result.ok, false);
    assert.match(result.error, /activation/i);
    assert.equal(result.diagnostics.long, null);
    assert.equal(samples.length, 0);
  });
}

for (const fault of ["hidden", "transient-hidden", "blur", "native-unfocused", "native-hidden", "native-minimized"]) {
  test(`T01 ${fault} fails with the foreground interval even while frames and RAF advance`, async () => {
    const { result, samples } = await scenario(fault);
    assert.equal(result.ok, false);
    assert.match(result.error, /Foreground condition lost/);
    const { previous, current } = result.diagnostics.long;
    assert.ok(current.frames > previous.frames && current.raf > previous.raf);
    assert.equal(samples.length, 0);
  });
}

for (const fault of ["ui-stalled", "ui-wrong", "ui-missing"]) {
  test(`T01 ${fault} rejects a broken progress control while media and RAF advance`, async () => {
    const { result, samples } = await scenario(fault);
    assert.equal(result.ok, false);
    assert.match(result.error, /progress UI stalled/);
    const { previous, current } = result.diagnostics.long;
    assert.ok(current.frames > previous.frames && current.raf > previous.raf && current.position !== previous.position);
    assert.equal(samples.length, 0);
  });
}
