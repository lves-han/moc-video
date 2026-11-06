import { videoJsAdapter } from "./videojs_adapter.ts";
import { readFileSync } from "node:fs";
import { runInNewContext } from "node:vm";
import ts from "typescript";
import * as vue from "vue";
import * as apiModule from "../../src/api.ts";
import type { PlaybackContext, Bridge_ApiError } from "../../src/models.ts";

const compile = (path: string) => ts.transpileModule(readFileSync(new URL(path, import.meta.url), "utf8"), {
  compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS },
}).outputText;
const playerSource = compile("../../src/player.ts");
const stateSource = compile("../../src/state.ts");
export const HOST = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
export const context = (episode = "one"): PlaybackContext => ({
  metadata_ref: { identity: { config_id: HOST, addon_key: "addon", namespace: "addon", type: "series", meta_id: "show", canonical_id: "show" }, content_ref: { source_id: "metadata", source_revision: "meta-rev", data: "show" } },
  metadata_video_id: episode, match_id: `match-${episode}`, content_ref: { source_id: "source", source_revision: "rev", data: "show" },
  group_id: "group", episode_id: episode, episode_ref: { source_id: "source", source_revision: "rev", data: episode }, selected_stream_id: null,
});
export const deferred = <T>() => {
  let resolve!: (value: T) => void;
  let reject!: (cause: unknown) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
};
export const flush = async () => { for (let i = 0; i < 30; i++) await Promise.resolve(); };

export function fixture(options: { mse?: boolean; native?: string; kind?: string } = {}) {
  const streams: any[] = [];
  const listeners = new Set<EventListenerOrEventListenerObject>();
  let now = 0;
  class Target extends EventTarget {
    override addEventListener(name: string, listener: EventListenerOrEventListenerObject | null, opts?: AddEventListenerOptions | boolean) {
      super.addEventListener(name, listener, opts);
      if (listener) {
        listeners.add(listener);
        if (typeof opts === "object") opts.signal?.addEventListener("abort", () => listeners.delete(listener), { once: true });
      }
    }
  }
  class Hls {
    static isSupported = () => options.mse ?? true;
    static Events = { ERROR: "error" };
    static ErrorTypes = { MEDIA_ERROR: "mediaError" };
    destroyed = false;
    error?: (event: string, data: any) => void;
    constructor(_config: object) { streams.push(this); }
    on(_name: string, callback: typeof this.error) { this.error = callback; }
    attachMedia(_element: object) {}
    loadSource(_url: string) {}
    destroy() { this.destroyed = true; }
  }
  let playResult: (() => Promise<void>) | undefined;
  const video = Object.assign(new Target(), {
    currentTime: 0, duration: NaN, paused: true, ended: false, readyState: 0, networkState: 0,
    src: "", muted: false, volume: 1, playbackRate: 1, error: null as null | { code: number; message: string },
    buffered: { length: 0, start: () => 0, end: () => 30 }, tracks: [] as any[],
    canPlayType: () => options.native ?? "maybe",
    async play() {
      if (playResult) return playResult();
      this.paused = false; this.dispatchEvent(new Event("playing"));
    },
    pause() { if (!this.paused) { this.paused = true; this.dispatchEvent(new Event("pause")); } },
    load() { this.currentTime = 0; this.duration = NaN; this.readyState = 0; this.ended = false; this.error = null; },
    removeAttribute(name: string) { if (name === "src") this.src = ""; },
    querySelectorAll() { return [...this.tracks]; },
    append(track: any) { this.tracks.push(track); },
  });
  Object.defineProperty(video, "textTracks", { get: () => video.tracks.map(t => t.track) });
  const document = { createElement() {
    return Object.assign(new Target(), { track: { mode: "disabled" },
      remove() { const index = video.tracks.indexOf(this); if (index >= 0) video.tracks.splice(index, 1); },
    });
  } };
  const globals = { AbortController, performance: { now: () => now }, document, Error, crypto, console };
  const playerModule: Record<string, any> = {};
  runInNewContext(playerSource, { ...globals, exports: playerModule, require: () => ({ Hls, HlsJsAdapter: videoJsAdapter(Hls, Target) }) });
  const stateModule: Record<string, any> = {};
  runInNewContext(stateSource, { ...globals, exports: stateModule, require: (path: string) =>
    path === "vue" ? vue : path.includes("api") ? apiModule : playerModule });
  const root = stateModule.createAppState();
  root.runtime.phase = "ready"; root.runtime.engine_epoch = HOST;
  const calls: { command: string; input: any; request_id: string }[] = [];
  const resolutions = new Map<string, any>();
  const sessions = new Map<string, any>();
  let counter = 0;
  let status = { state: "ready", error: null as Bridge_ApiError | null };
  let intercept: ((command: string, args: any, data: any) => Promise<unknown> | unknown) | undefined;
  const client = new apiModule.Api(async (command, args) => {
    calls.push({ command, input: args.input, request_id: args.request_id });
    const input = args.input as any;
    let data: any = { ok: true };
    if (command === "resolve_episode") {
      data = { id: `res-${++counter}`, ...input.episode_ref, source_id: "source", source_revision: "rev", episode_ref: input.episode_ref,
        streams: ["a", "b"].map(id => ({ id, kind: options.kind ?? "progressive", label: id, playable: true })), subtitles: [] };
      resolutions.set(data.id, data);
    } else if (command === "open_playback") {
      const res = resolutions.get(input.resolution_id);
      data = { session_id: `session-${++counter}`, resolution_id: res.id, stream_id: input.stream_id,
        source_id: res.source_id, source_revision: res.source_revision, episode_ref: res.episode_ref, stream_kind: options.kind ?? "progressive",
        media_url: `http://127.0.0.1/media/${counter}`, mime_type: "video/mp4", subtitles: [{ id: "zh", url: "http://127.0.0.1/subtitle", label: "中文", language: "zh-CN" }] };
      sessions.set(data.session_id, data);
    } else if (command === "close_playback") sessions.delete(input.session_id);
    else if (command === "release_resolution") resolutions.delete(input.resolution_id);
    else if (command === "get_resource_status") data = { ...input, ...status };
    else if (command === "runtime_status" || command === "runtime_prepare") data = { ...root.runtime, health: { status: "ready", media_gateway_ready: true } };
    else if (command === "get_details") data = { ...root.details.data, images: { replacement: { resource_id: "replacement", url: "http://127.0.0.1/new-image" } } };
    const envelope = { request_id: args.request_id, engine_epoch: HOST, data };
    return intercept ? await intercept(command, args, envelope) : envelope;
  });
  const ended: unknown[] = [];
  const controller = stateModule.createPlaybackController(root, client, (...args: unknown[]) => ended.push(args));
  controller.attach(video);
  return {
    root, controller, video, streams, calls, sessions, resolutions, ended, playerModule,
    listeners: () => listeners.size,
    clock(ms: number) { now += ms; },
    setPlay(fn?: () => Promise<void>) { playResult = fn; },
    intercept(fn?: typeof intercept) { intercept = fn; },
    status(state: string, code?: string) { status = { state, error: code ? { code, message: code, retryable: false } as Bridge_ApiError : null }; },
    ready(duration = 30) { video.duration = duration; video.readyState = 4; video.dispatchEvent(new Event("loadedmetadata")); },
    error(code = 2, message = "native network failure") { video.error = { code, message }; video.dispatchEvent(new Event("error")); },
  };
}
