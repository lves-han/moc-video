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

function fixture() {
  let now = 0;
  const exports: Record<string, any> = {};
  runInNewContext(source, {
    exports, require: () => ({}), performance: { now: () => now },
    setTimeout(callback: () => void, milliseconds: number) { now += milliseconds; queueMicrotask(callback); },
  });
  return { wait: exports.waitForEpisodeSample, now: () => now };
}
const red = () => ({ audio_state: "running", audio_rms: .1, audio_peak_hz: 440, pixel: [254, 0, 0, 255], session_bound: true });

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
