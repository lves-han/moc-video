import Hls from "hls.js";

export type PlayerState = "preparing" | "ready" | "playing" | "paused" | "buffering" | "position" | "ended" | "failed" | "stopped";
export type PlayerEvent = { state: PlayerState; epoch: number; position_ms: number; duration_ms: number | null; error?: string };
export type PlaybackSource = { url: string; kind: "mp4" | "hls"; subtitle?: string };

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
export function createPlayer(video: HTMLVideoElement, emit: (event: PlayerEvent) => void) {
  let hls: Hls | undefined;
  let listeners: AbortController | undefined;
  let epoch = 0;
  let lastPosition = 0;
  let engine: "native" | "hls.js" = "native";
  const notify = (state: PlayerState, error?: string) => emit({
    state, epoch, position_ms: Math.round(video.currentTime * 1000),
    duration_ms: Number.isFinite(video.duration) ? Math.round(video.duration * 1000) : null,
    ...(error ? { error } : {}),
  });

  function stop() {
    epoch++;
    listeners?.abort();
    listeners = undefined;
    video.pause();
    hls?.destroy();
    hls = undefined;
    video.removeAttribute("src");
    video.querySelectorAll("track").forEach(track => track.remove());
    video.load();
    notify("stopped");
  }

  function open(source: PlaybackSource) {
    stop();
    const current = epoch;
    const active = new AbortController();
    listeners = active;
    lastPosition = 0;
    notify("preparing");
    const on = (name: string, state: PlayerState) => video.addEventListener(name, () => {
      if (epoch === current) notify(state);
    }, { signal: active.signal });
    on("loadedmetadata", "ready");
    on("playing", "playing");
    on("pause", "paused");
    on("waiting", "buffering");
    on("ended", "ended");
    video.addEventListener("timeupdate", () => {
      if (epoch === current && performance.now() - lastPosition >= 250) {
        lastPosition = performance.now();
        notify("position");
      }
    }, { signal: active.signal });
    video.addEventListener("error", () => {
      if (epoch === current) notify("failed", `MediaError ${video.error?.code}: ${video.error?.message ?? "load failed"}`);
    }, { signal: active.signal });
    video.crossOrigin = "anonymous";
    video.preload = "auto";
    if (source.subtitle) {
      const track = document.createElement("track");
      track.kind = "subtitles";
      track.label = "样机字幕";
      track.srclang = "zh-CN";
      track.src = source.subtitle;
      track.default = true;
      video.append(track);
    }
    // WebKit native HLS bypasses MediaElementAudioSourceNode (WebKit bug 306493).
    // Prefer the supported MSE path to keep media-element audio routing consistent.
    if (source.kind === "hls" && Hls.isSupported()) {
      engine = "hls.js";
      hls = new Hls({ enableWorker: true, maxBufferLength: 30, maxMaxBufferLength: 60 });
      hls.on(Hls.Events.ERROR, (_event, data) => {
        if (current === epoch && data.fatal) notify("failed", `HLS ${data.type}: ${data.details}`);
      });
      hls.attachMedia(video);
      hls.loadSource(source.url);
    } else {
      if (source.kind === "hls" && !video.canPlayType("application/vnd.apple.mpegurl")) {
        notify("failed", "MEDIA_UNSUPPORTED: HLS requires native HLS or MediaSource");
        throw new Error("MEDIA_UNSUPPORTED: HLS requires native HLS or MediaSource");
      }
      engine = "native";
      video.src = source.url;
      video.load();
    }
    return { epoch, engine };
  }

  return {
    open, stop,
    async play() {
      try { await video.play(); }
      catch (error) {
        notify("failed", error instanceof Error && error.name === "NotAllowedError"
          ? "AUTOPLAY_BLOCKED: 请点击继续播放" : String(error));
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
