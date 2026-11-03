import Hls from "hls.js";
import type { Bridge_ApiError, PlaybackViewSession, PlayerEvent as ProtocolEvent } from "./models.ts";

export type PlayerState = "preparing" | "ready" | "playing" | "paused" | "buffering" | "position" | "ended" | "failed" | "stopped";
export type PlayerEvent = { state: PlayerState; epoch: number; position_ms: number; duration_ms: number | null; error?: string };
export type SubtitleSource = { id: string; label: string; language: string | null; url: string };
export type PlaybackSource = { url: string; kind: "mp4" | "hls"; subtitle?: string; subtitles?: readonly SubtitleSource[] };
export type PlayerBinding = { playback_epoch: number; engine_epoch: string | null; session_id: string | null };
export class UnsupportedMedia extends Error {}

export function sessionSource(session: PlaybackViewSession): PlaybackSource {
  if (session.stream_kind !== "progressive" && session.stream_kind !== "hls") throw new UnsupportedMedia(`MEDIA_UNSUPPORTED: ${session.stream_kind}`);
  return { url: session.media_url, kind: session.stream_kind === "progressive" ? "mp4" : "hls", subtitles: session.subtitles };
}

declare global {
  interface Window {
    __TAURI__?: { core: { invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> } };
  }
}

export function nativeInvoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!window.__TAURI__) return Promise.reject(new Error("Tauri host is required"));
  return window.__TAURI__.core.invoke<T>(command, args);
}

export function capabilities(video: HTMLVideoElement) {
  return {
    mp4: video.canPlayType('video/mp4; codecs="avc1.640028, mp4a.40.2"'),
    native_hls: video.canPlayType("application/vnd.apple.mpegurl"),
    mse_hls: Hls.isSupported(),
    hls_version: Hls.version,
  };
}

/** One controller owns one video element. stop() releases its whole loading attempt. */
export function createPlayer(video: HTMLVideoElement, emit: (event: PlayerEvent) => void,
  protocol?: (event: ProtocolEvent) => void) {
  let hls: Hls | undefined;
  let listeners: AbortController | undefined;
  let epoch = 0;
  let lastPosition = 0;
  let engine: "native" | "hls.js" = "native";
  let binding: PlayerBinding = { playback_epoch: 0, engine_epoch: null, session_id: null };
  const duration = () => Number.isFinite(video.duration) && video.duration >= 0 ? Math.round(video.duration * 1000) : null;
  const position = () => Number.isFinite(video.currentTime) ? Math.max(0, Math.round(video.currentTime * 1000)) : 0;
  function notify(state: PlayerState, message?: string, error?: Bridge_ApiError) {
    emit({ state, epoch, position_ms: position(), duration_ms: duration(), ...(message ? { error: message } : {}) });
    if (state === "ready") protocol?.({ ...binding, type: state, duration_ms: duration() });
    else if (state === "position") protocol?.({ ...binding, type: state, position_ms: position() });
    else if (state === "buffering") protocol?.({ ...binding, type: state, percent: null });
    else if (state === "failed") protocol?.({ ...binding, type: state, error: error ?? { code: "MEDIA_UNSUPPORTED", message: message!, retryable: false } });
    else protocol?.({ ...binding, type: state });
  }

  function mediaFailure(resource: string, message: string, code?: number, httpStatus?: number) {
    notify("failed", message, {
      code: code === 3 || code === 4 ? "MEDIA_UNSUPPORTED" : httpStatus === 401 ? "UNAUTHORIZED"
        : httpStatus === 410 ? "SESSION_EXPIRED" : httpStatus === 504 ? "UPSTREAM_TIMEOUT" : "UPSTREAM_FAILED",
      message, retryable: code !== 3 && code !== 4,
      details: { resource_kind: resource === "video" ? "media" : "subtitle", resource_key: resource, media_code: code ?? null,
        ...(httpStatus !== undefined ? { http_status: httpStatus } : {}) },
    });
  }

  function stop() {
    epoch++;
    listeners?.abort();
    listeners = undefined;
    video.pause();
    hls?.destroy();
    hls = undefined;
    video.removeAttribute("src");
    Array.from(video.textTracks).forEach(track => { track.mode = "disabled"; });
    video.querySelectorAll("track").forEach(track => track.remove());
    video.load();
    notify("stopped");
  }

  function open(source: PlaybackSource, identity?: PlayerBinding) {
    stop();
    binding = identity ? { ...identity } : { playback_epoch: epoch, engine_epoch: null, session_id: null };
    const current = epoch;
    const openedAt = performance.now();
    const active = new AbortController();
    listeners = active;
    lastPosition = -Infinity;
    notify("preparing");
    const fresh = (event: Event) => epoch === current && event.timeStamp >= openedAt;
    const on = (name: string, state: PlayerState, valid: () => boolean = () => true) => video.addEventListener(name, event => {
      if (fresh(event) && valid()) notify(state);
    }, { signal: active.signal });
    on("loadedmetadata", "ready", () => video.readyState >= 1);
    on("durationchange", "ready", () => video.readyState >= 1);
    on("playing", "playing", () => !video.paused);
    on("pause", "paused", () => video.paused);
    on("waiting", "buffering");
    on("ended", "ended", () => video.ended);
    video.addEventListener("timeupdate", event => {
      if (fresh(event) && performance.now() - lastPosition >= 250) {
        lastPosition = performance.now();
        notify("position");
      }
    }, { signal: active.signal });
    video.addEventListener("error", event => {
      if (fresh(event) && video.error) mediaFailure("video", `MediaError ${video.error.code}: ${video.error.message ?? "load failed"}`, video.error.code);
    }, { signal: active.signal });
    video.crossOrigin = "anonymous";
    video.preload = "auto";
    const subtitles = source.subtitles ?? (source.subtitle ? [{ id: "legacy", label: "样机字幕", language: "zh-CN", url: source.subtitle }] : []);
    for (const [index, subtitle] of subtitles.entries()) {
      const track = document.createElement("track");
      track.kind = "subtitles";
      track.id = subtitle.id;
      track.label = subtitle.label;
      track.srclang = subtitle.language || "und";
      track.src = subtitle.url;
      track.default = index === 0;
      track.addEventListener("error", event => {
        if (fresh(event)) mediaFailure(subtitle.id, `SUBTITLE_LOAD_FAILED: ${subtitle.id}`);
      }, { signal: active.signal });
      video.append(track);
    }
    if (source.kind === "hls" && !video.canPlayType("application/vnd.apple.mpegurl")) {
      if (!Hls.isSupported()) {
        notify("failed", "MEDIA_UNSUPPORTED: HLS requires native HLS or MediaSource");
        throw new UnsupportedMedia("MEDIA_UNSUPPORTED: HLS requires native HLS or MediaSource");
      }
      engine = "hls.js";
      hls = new Hls({ enableWorker: true, maxBufferLength: 30, maxMaxBufferLength: 60 });
      hls.on(Hls.Events.ERROR, (_event, data) => {
        if (current === epoch && data.fatal) mediaFailure("video", `HLS ${data.type}: ${data.details}`,
          data.type === Hls.ErrorTypes.MEDIA_ERROR ? 3 : 2, data.response?.code);
      });
      hls.attachMedia(video);
      hls.loadSource(source.url);
    } else {
      engine = "native";
      video.src = source.url;
      video.load();
    }
    return { epoch, engine };
  }

  return {
    open, stop,
    async play() {
      const current = epoch;
      try { await video.play(); }
      catch (error) {
        if (current !== epoch) return;
        const blocked = typeof error === "object" && error !== null && "name" in error && error.name === "NotAllowedError";
        const message = blocked ? "AUTOPLAY_BLOCKED: 请点击继续播放" : String(error);
        if (!blocked && video.error) mediaFailure("video", `MediaError ${video.error.code}: ${video.error.message ?? message}`, video.error.code);
        else notify("failed", message, { code: "UPSTREAM_FAILED", message, retryable: true, details: { reason: blocked ? "AUTOPLAY_BLOCKED" : "PLAY_FAILED" } });
        throw error;
      }
    },
    pause: () => video.pause(),
    seek(seconds: number) {
      if (!Number.isFinite(seconds) || !Number.isFinite(video.duration)) throw new Error("Seek requires finite time and known duration");
      video.currentTime = Math.max(0, Math.min(seconds, video.duration));
    },
    setVolume(volume: number) {
      if (!Number.isFinite(volume) || volume < 0 || volume > 1) throw new Error("Volume must be in [0, 1]");
      video.volume = volume;
    },
    setSpeed(speed: number) {
      if (!Number.isFinite(speed) || speed <= 0 || speed > 4) throw new Error("Speed must be in (0, 4]");
      video.playbackRate = speed;
    },
    selectSubtitle(index: number | null) {
      if (index !== null && (!Number.isInteger(index) || index < 0 || index >= video.textTracks.length)) throw new Error("Subtitle track is unavailable");
      Array.from(video.textTracks).forEach((track, i) => { track.mode = i === index ? "showing" : "disabled"; });
    },
    fullscreen: () => window.__TAURI__ ? nativeInvoke<boolean>("player_fullscreen") : video.requestFullscreen().then(() => true),
    diagnostics: () => ({ epoch, engine, hls_instances: hls ? 1 : 0, listening: Boolean(listeners), tracks: video.querySelectorAll("track").length }),
  };
}

export type Player = ReturnType<typeof createPlayer>;
