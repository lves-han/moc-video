import { Hls, HlsJsAdapter } from "@videojs/hlsjs-video";
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
  let fallback: HlsJsAdapter | undefined;
  let provider: HlsJsAdapter | undefined;
  let listeners: AbortController | undefined;
  let listenerRegistrations = 0;
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

  function safeMediaMessage(message: string) {
    return message.replace(/https?:\/\/[^\s"'<>]+/gi, '<media-resource>')
      .replace(/([?&](?:cap|token|capability|signature|key)=)[^\s"'<>]*/gi, '$1<redacted>');
  }
  function mediaFailure(resource: string, message: string, code?: number, httpStatus?: number) {
    message = safeMediaMessage(message);
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
    listenerRegistrations = 0;
    video.pause();
    if (provider) provider.source = null;
    fallback?.destroy();
    fallback = undefined;
    provider = undefined;
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
    const listen = (target: EventTarget, name: string, callback: EventListener) => {
      target.addEventListener(name, callback, { signal: active.signal });
      listenerRegistrations++;
    };
    lastPosition = -Infinity;
    notify("preparing");
    const fresh = (event: Event) => epoch === current && event.timeStamp >= openedAt;
    const on = (name: string, state: PlayerState, valid: () => boolean = () => true) => listen(video, name, event => {
      if (fresh(event) && valid()) notify(state);
    });
    on("loadedmetadata", "ready", () => video.readyState >= 1);
    on("durationchange", "ready", () => video.readyState >= 1);
    on("playing", "playing", () => !video.paused);
    on("pause", "paused", () => video.paused);
    on("waiting", "buffering");
    on("ended", "ended", () => video.ended);
    listen(video, "timeupdate", event => {
      if (fresh(event) && performance.now() - lastPosition >= 250) {
        lastPosition = performance.now();
        notify("position");
      }
    });
    listen(video, "error", event => {
      if (fresh(event) && video.error) mediaFailure("video", `MediaError ${video.error.code}: ${video.error.message ?? "load failed"}`, video.error.code);
    });
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
      listen(track, "error", event => {
        if (fresh(event)) mediaFailure(subtitle.id, `SUBTITLE_LOAD_FAILED: ${subtitle.id}`);
      });
      video.append(track);
    }
    provider = video.closest?.('hlsjs-video') as unknown as HlsJsAdapter | undefined;
    if (!provider && source.kind === 'hls') {
      if (!video.canPlayType('application/vnd.apple.mpegurl') && !Hls.isSupported()) {
        notify('failed', 'MEDIA_UNSUPPORTED: Video.js cannot play HLS on this device');
        throw new UnsupportedMedia('MEDIA_UNSUPPORTED: Video.js cannot play HLS on this device');
      }
      fallback = new HlsJsAdapter(); fallback.attach(video); provider = fallback;
    }
    if (provider) {
      const original = provider;
      listen(original, 'error', event => {
        const error = original.error;
        if (fresh(event) && error) mediaFailure('video', `Video.js: ${error.message}`, error.code, (error.data as { response?: { code?: number } } | undefined)?.response?.code);
      });
      original.preload = 'auto';
      original.source = { src: source.url, type: source.kind === 'hls' ? 'application/vnd.apple.mpegurl' : 'video/mp4',
        preferPlayback: 'native', engine: { hlsJs: { enableWorker: true, maxBufferLength: 30, maxMaxBufferLength: 60 } } };
      engine = original.engineName === 'hlsjs' ? 'hls.js' : 'native';
    } else {
      engine = 'native'; video.src = source.url; video.load();
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
        const message = blocked ? "AUTOPLAY_BLOCKED: 请点击继续播放" : safeMediaMessage(String(error));
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
    diagnostics: () => ({ epoch, engine: provider?.engineName === "hlsjs" ? "hls.js" : engine, hls_instances: provider?.engine ? 1 : 0, videojs_version: "10.3.0", listening: Boolean(listeners), listener_registrations: listenerRegistrations,
      tracks: video.querySelectorAll("track").length }),
  };
}

export type Player = ReturnType<typeof createPlayer>;
