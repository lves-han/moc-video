import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { runInNewContext } from "node:vm";
import ts from "typescript";

// Exercise the native harness's real readiness loop. Decoding and all 50 actual
// switches remain required in A1; these doubles cover asynchronous sample order.
const source = ts.transpileModule(readFileSync(new URL("./fixtures/t12_webview.ts", import.meta.url), "utf8"), {
  compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS },
}).outputText;

function fixture(globals: Record<string, unknown> = {}) {
  let now = 0;
  const exports: Record<string, any> = {};
  runInNewContext(source, {
    exports, require: () => ({}), performance: { now: () => now },
    setTimeout(callback: () => void, milliseconds: number) { now += milliseconds; queueMicrotask(callback); },
    ...globals,
  });
  return { wait: exports.waitForEpisodeSample, sampler: exports.createMediaSampler, codec: exports.requireCodecMedia, now: () => now };
}
const red = () => ({ audio_state: "running", audio_rms: .1, audio_peak_hz: 440, pixel: [254, 0, 0, 255], session_bound: true });

test("T12 codec stimulus accepts only the current progressive media, rejecting the preceding HLS case", () => {
  const f = fixture();
  const media_url = "http://127.0.0.1/sessions/current/media?cap=fixture";
  f.codec({ stream_kind: "progressive", media_url }, media_url);
  assert.throws(() => f.codec({ stream_kind: "hls", media_url }, media_url), /progressive MP4/);
  assert.throws(() => f.codec({ stream_kind: "progressive", media_url }, "http://127.0.0.1/sessions/old/media"), /current session/);
  assert.throws(() => f.codec(null, ""), /progressive MP4/);
});

test("T12 observer reuses its PCM/FFT buffers and canvas while returning independent raw values", () => {
  let canvases = 0, allocations = 0, clears = 0, draws = 0;
  const pcm = new Set<Float32Array>(), fft = new Set<Float32Array>();
  const video = { readyState: 2 };
  const pixels = new Uint8ClampedArray([254, 0, 0, 255]);
  const canvas = { width: 0, height: 0, getContext: () => ({
    clearRect() { clears++; pixels.fill(0); },
    drawImage() { draws++; pixels.set([254, 0, 0, 255]); },
    getImageData: () => ({ data: pixels }),
  }) };
  const f = fixture({
    Float32Array: class extends Float32Array { constructor(length: number) { super(length); allocations++; } },
    document: { createElement(name: string) { assert.equal(name, "canvas"); canvases++; return canvas; } },
  });
  const sampler = f.sampler(video, { fftSize: 4096, frequencyBinCount: 2048,
    getFloatTimeDomainData(buffer: Float32Array) { pcm.add(buffer); buffer.fill(.125); },
    getFloatFrequencyData(buffer: Float32Array) { fft.add(buffer); buffer.fill(-100); buffer[75] = -1; },
  }, 48000);
  const first = sampler.read();
  for (let i = 1; i < 500; i++) {
    const value = sampler.read();
    assert.equal(value.audio_rms, .125);
    assert.equal(value.audio_peak_hz, 878.90625);
    assert.deepEqual([...value.pixel], [254, 0, 0, 255]);
    assert.notEqual(value.pixel, first.pixel);
  }
  assert.deepEqual([allocations, canvases, pcm.size, fft.size, clears, draws], [2, 1, 1, 1, 500, 500]);
  assert.deepEqual(JSON.parse(JSON.stringify(sampler.diagnostics())), {
    reads: 500, scratch_buffers: 2, scratch_bytes: 24576, scratch_canvases: 1,
  });
  video.readyState = 0;
  assert.deepEqual([...sampler.read().pixel], [0, 0, 0, 0]);
  assert.deepEqual([...first.pixel], [254, 0, 0, 255]);
  sampler.dispose();
  assert.deepEqual([canvas.width, canvas.height], [0, 0]);
});

test("T12 observer explicitly fails when frame observation has no 2D context", () => {
  const f = fixture({ document: { createElement: () => ({ getContext: () => null }) } });
  assert.throws(() => f.sampler({}, { fftSize: 4096, frequencyBinCount: 2048 }, 48000), /requires a canvas 2D context/);
});

test("T12 sampling waits for nonzero current PCM and current frame, retaining every transient sample", async () => {
  const f = fixture();
  const values = [
    { ...red(), audio_rms: 0, audio_peak_hz: 880 }, // Original failing interval: silent PCM, stale FFT.
    { ...red(), audio_peak_hz: 880 },
    { ...red(), pixel: [0, 0, 254, 255] },
    red(),
  ];
  let index = 0;
  const result = await f.wait(() => values[index++], 2, 30000);
  assert.equal(f.now(), 60);
  assert.equal(result.sample, values[3]);
  assert.deepEqual([...result.observations], values);
});

test("T12 odd episodes require blue frames and the actual 880Hz PCM", async () => {
  const f = fixture();
  const value = { ...red(), audio_peak_hz: 880, pixel: [0, 0, 254, 255] };
  const result = await f.wait(() => value, 1, 30000);
  assert.equal(result.sample, value);
  assert.equal(f.now(), 0);
});

for (const change of [
  { audio_rms: 0, audio_peak_hz: 880 }, { audio_peak_hz: 880 }, { audio_state: "suspended" },
  { session_bound: false }, { pixel: [0, 0, 254, 255] }, { pixel: [0, 0, 0, 255] },
]) {
  test(`T12 unready media fails at the existing deadline and preserves raw samples: ${JSON.stringify(change)}`, async () => {
    const f = fixture();
    await assert.rejects(f.wait(() => ({ ...red(), ...change }), 2, 60), (error: Error) => {
      assert.match(error.message, /Current episode media did not become ready/);
      const diagnostic = JSON.parse(error.message.split(": ", 2)[1]);
      assert.equal(diagnostic.index, 2);
      assert.equal(diagnostic.observations.length, 4);
      assert.deepEqual(diagnostic.observations.at(-1), { ...red(), ...change });
      return true;
    });
    assert.equal(f.now(), 60);
  });
}

test("T12 ready media arriving at the deadline still fails", async () => {
  const f = fixture();
  await assert.rejects(f.wait(() => red(), 2, 0), /Current episode media did not become ready/);
});

test("T12 native HLS retains frame/session readiness while its native audio gate remains in A1", async () => {
  const f = fixture();
  const value = { ...red(), audio_rms: 0 };
  const result = await f.wait(() => value, 51, 30000);
  assert.equal(result.sample, value);
});
