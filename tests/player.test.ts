import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { runInNewContext } from "node:vm";
import ts from "typescript";

const moduleSource = ts.transpileModule(readFileSync(new URL("../src/player.ts", import.meta.url), "utf8"), {
  compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS },
}).outputText;

function playerFixture(mse: boolean, nativeHls: string) {
  const streams: any[] = [];
  class Hls {
    static isSupported = () => mse;
    static Events = { ERROR: "error" };
    source = "";
    destroyed = false;
    onError?: (event: string, data: object) => void;
    constructor(_config: object) { streams.push(this); }
    on(_event: string, listener: typeof this.onError) { this.onError = listener; }
    attachMedia(_video: object) {}
    loadSource(url: string) { this.source = url; }
    destroy() { this.destroyed = true; }
  }
  const video = Object.assign(new EventTarget(), {
    currentTime: 0, duration: 30, src: "", crossOrigin: "", preload: "",
    pause() {}, load() {}, canPlayType: () => nativeHls,
    removeAttribute(name: string) { if (name === "src") this.src = ""; },
    querySelectorAll: () => [],
  });
  const exports: Record<string, any> = {};
  runInNewContext(moduleSource, { exports, require: () => ({ default: Hls }), AbortController, performance });
  const events: any[] = [];
  const player = exports.createPlayer(video, (event: unknown) => events.push(event));
  return { player, video, streams, events };
}

test("HLS uses the supported MSE transport when native HLS is unavailable", () => {
  const { player, streams } = playerFixture(true, "");
  assert.equal(player.open({ kind: "hls", url: "https://fixture.invalid/index.m3u8" }).engine, "hls.js");
  assert.equal(streams[0].source, "https://fixture.invalid/index.m3u8");
});

test("HLS uses native playback when MSE is unavailable", () => {
  const { player, video, streams } = playerFixture(false, "maybe");
  assert.equal(player.open({ kind: "hls", url: "https://fixture.invalid/index.m3u8" }).engine, "native");
  assert.equal(video.src, "https://fixture.invalid/index.m3u8");
  assert.equal(streams.length, 0);
});

test("unavailable HLS transports fail explicitly", () => {
  const { player, events } = playerFixture(false, "");
  assert.throws(() => player.open({ kind: "hls", url: "https://fixture.invalid/index.m3u8" }), /MEDIA_UNSUPPORTED/);
  assert.equal(events.at(-1).state, "failed");
});

test("switching HLS to MP4 destroys the old transport and rejects late errors", () => {
  const { player, video, streams, events } = playerFixture(true, "");
  player.open({ kind: "hls", url: "https://fixture.invalid/index.m3u8" });
  player.open({ kind: "mp4", url: "https://fixture.invalid/clip.mp4" });
  assert.equal(streams[0].destroyed, true);
  assert.equal(video.src, "https://fixture.invalid/clip.mp4");
  const count = events.length;
  streams[0].onError("error", { fatal: true, type: "old", details: "late" });
  assert.equal(events.length, count);
  player.stop();
  assert.equal(video.src, "");
  assert.equal(player.diagnostics().hls_instances, 0);
  assert.equal(player.diagnostics().listening, false);
});

test("HLS keeps the native path when native and MSE are both available", () => {
  const { player, video, streams } = playerFixture(true, "maybe");
  assert.equal(player.open({ kind: "hls", url: "https://fixture.invalid/index.m3u8" }).engine, "native");
  assert.equal(video.src, "https://fixture.invalid/index.m3u8");
  assert.equal(streams.length, 0);
});
