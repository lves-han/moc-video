import { reactive } from "vue";
import { api, Api, ApiFailure } from "./api.ts";
import { createPlayer, sessionSource, UnsupportedMedia, type Player } from "./player.ts";
import type { AppState, Bridge_ApiError, Bridge_MediaRef, Bridge_Resolution, DetailsViewData,
  Command, CommandInput, PlaybackContext, PlaybackViewSession, PlayerEvent, ResumeSnapshot } from "./models.ts";

export function createAppState(): AppState {
  return reactive<AppState>({
    route: { name: "home" }, selected_metadata_source: null, selected_playback_source: null,
    sources: { phase: "idle", request_epoch: 0, request_id: null, data: null, error: null },
    runtime: { platform: "macos", phase: "unavailable", engine_epoch: null, health: null, error: null,
      cloudstream: { phase: "stopped", engine_epoch: null, display_mode: "none", health: null, error: null } },
    home: { phase: "idle", request_epoch: 0, request_id: null, data: null, error: null },
    details: { phase: "idle", request_epoch: 0, request_id: null, data: null, error: null },
    playback: { phase: "idle", playback_epoch: 0, engine_epoch: null, context: null, resolution: null,
      session: null, position_ms: 0, duration_ms: null, buffered: [], volume: 1, muted: false, speed: 1, subtitle_id: null, error: null },
    settings: { schema_version: 2, last_source_key: null, volume: 1, muted: false, auto_next: true, window_bounds: null, source_configs: [] },
  });
}
export const appState = createAppState();

/** Called with the confirmed match result by the details/matching flow. No URL is accepted. */
export function navigatePlayback(context: PlaybackContext) {
  appState.route = { name: "playback", context };
  window.location.hash = "/playback";
}

const sameRef = (a: Bridge_MediaRef, b: Bridge_MediaRef) => a.source_id === b.source_id && a.source_revision === b.source_revision && a.data === b.data;
const failure = (cause: unknown): Bridge_ApiError => cause instanceof ApiFailure ? cause.error : {
  code: cause instanceof UnsupportedMedia ? "MEDIA_UNSUPPORTED" : "UPSTREAM_FAILED",
  message: cause instanceof Error ? cause.message : String(cause), retryable: !(cause instanceof UnsupportedMedia),
};

export function createPlaybackController(root: AppState = appState, client: Pick<Api, "call"> = api,
  ended: (context: PlaybackContext, epoch: number) => void = () => {}) {
  const state = root.playback;
  const ui = reactive({ autoplayBlocked: false, diagnostic: "", action: "" as "" | "retry" | "reconnect", imageError: "" });
  let player: Player | undefined;
  let video: HTMLVideoElement | undefined;
  let tail: Promise<void> = Promise.resolve();
  let pendingResume: ResumeSnapshot | null = null;
  let wantsPlaying = false;
  let recovered = false;
  let reconnected = false;
  let endedEpoch = -1;
  const queried = new Set<string>();
  const retiring: { session: PlaybackViewSession | null; resolution: Bridge_Resolution | null }[] = [];
  const pendingRequests = new Set<string>();
  let imageAttempt: DetailsViewData | null = null;
  const queriedImages = new Set<string>();
  let refreshedImageEpoch = -1;

  function current(epoch: number, session?: PlaybackViewSession) {
    return epoch === state.playback_epoch && (!session || state.session?.session_id === session.session_id
      && state.engine_epoch === root.runtime.engine_epoch && state.context !== null && sameRef(state.context.episode_ref, session.episode_ref));
  }

  async function request<C extends Command>(command: C, input: CommandInput<C>, host?: string) {
    const id = crypto.randomUUID();
    pendingRequests.add(id);
    try { return await client.call(command, input, id, host); }
    finally { pendingRequests.delete(id); }
  }

  async function drainRetired() {
    while (retiring.length) {
      const old = retiring[0]!;
      if (old.session) {
        await client.call("close_playback", { session_id: old.session.session_id });
        old.session = null;
      }
      if (old.resolution) {
        await client.call("release_resolution", { resolution_id: old.resolution.id });
        old.resolution = null;
      }
      retiring.shift();
    }
  }

  function fail(error: Bridge_ApiError) {
    state.phase = "failed";
    state.error = error;
    ui.action = error.code === "UNAUTHORIZED" ? "reconnect" : error.retryable || ["SESSION_EXPIRED", "UPSTREAM_FAILED", "UPSTREAM_TIMEOUT"].includes(error.code) ? "retry" : "";
  }

  function snapshot(): ResumeSnapshot | null {
    return state.context ? { context: structuredCloneContext(state.context), position_ms: state.position_ms, was_playing: wantsPlaying } : null;
  }

  // Vue's reactive proxies cannot be passed directly to structuredClone.
  function structuredCloneContext(context: PlaybackContext): PlaybackContext { return JSON.parse(JSON.stringify(context)); }

  async function connect(prepare = false) {
    const runtime = await client.call(prepare ? "runtime_prepare" : "runtime_status", prepare ? {
      target: root.sources.data?.providers.find(s => s.id === state.context?.episode_ref.source_id)?.kind === "cloudstream" ? "cloudstream" : "application",
    } : {});
    if (runtime.phase !== "ready" || !runtime.engine_epoch) throw new Error(runtime.error?.message ?? "Host is not ready");
    root.runtime = runtime;
    state.engine_epoch = runtime.engine_epoch;
  }

  function load(context: PlaybackContext, options: { stream?: string; resume?: ResumeSnapshot | null; recovery?: boolean } = {}): Promise<void> {
    const epoch = ++state.playback_epoch;
    player?.stop();
    const keep = options.stream ? state.resolution : null;
    retiring.push({ session: state.session, resolution: keep ? null : state.resolution });
    state.session = null;
    state.resolution = keep;
    state.context = structuredCloneContext(context);
    state.phase = "resolving";
    state.error = null;
    state.position_ms = options.resume?.position_ms ?? 0;
    state.duration_ms = null;
    state.buffered = [];
    ui.autoplayBlocked = false;
    ui.diagnostic = "";
    ui.action = "";
    queried.clear();
    pendingResume = options.resume ?? { context: structuredCloneContext(context), position_ms: 0, was_playing: true };
    wantsPlaying = pendingResume.was_playing;
    if (!options.recovery) { recovered = false; reconnected = false; }
    // Cancellation is advisory; identity checks and captured cleanup still run on late results.
    for (const id of pendingRequests) void client.call("cancel_request", { target_request_id: id }).catch(error => {
      if (current(epoch)) ui.diagnostic = `取消请求失败：${failure(error).message}`;
    });
    tail = tail.then(async () => {
      await drainRetired();
      if (!current(epoch)) return;
      if (!player) throw new Error("Playback view is not mounted");
      if (root.runtime.phase !== "ready" || !root.runtime.engine_epoch) await connect();
      if (!current(epoch)) return;
      const host = root.runtime.engine_epoch!;
      state.engine_epoch = host;
      const resolution = keep ?? await request("resolve_episode", { episode_ref: context.episode_ref, match_id: context.match_id }, host) as Bridge_Resolution;
      if (!current(epoch) || root.runtime.engine_epoch !== host) {
        if (!keep) { retiring.push({ session: null, resolution }); await drainRetired(); }
        return;
      }
      if (!sameRef(resolution.episode_ref, context.episode_ref) || resolution.source_id !== context.episode_ref.source_id || resolution.source_revision !== context.episode_ref.source_revision) {
        retiring.push({ session: null, resolution }); await drainRetired();
        throw new Error("Resolution episode/source binding differs");
      }
      state.resolution = resolution;
      const stream = options.stream ? resolution.streams.find(s => s.id === options.stream) :
        resolution.streams.find(s => s.id === context.selected_stream_id && s.playable) ?? resolution.streams.find(s => s.playable);
      if (!stream || !stream.playable) throw new ApiFailure({ code: "MEDIA_UNSUPPORTED", message: stream?.reason ?? "No playable stream", retryable: false });
      const session = await request("open_playback", { resolution_id: resolution.id, stream_id: stream.id }, host) as PlaybackViewSession;
      if (!current(epoch) || root.runtime.engine_epoch !== host) {
        const owner = retiring.find(old => old.resolution?.id === resolution.id);
        if (owner) owner.session = session;
        else retiring.push({ session, resolution: null });
        await drainRetired(); return;
      }
      if (session.resolution_id !== resolution.id || session.stream_id !== stream.id || session.source_id !== resolution.source_id
        || session.source_revision !== resolution.source_revision || !sameRef(session.episode_ref, context.episode_ref) || session.stream_kind !== stream.kind) {
        retiring.push({ session, resolution: null }); await drainRetired();
        throw new Error("Playback session episode/stream binding differs");
      }
      state.session = session;
      state.context!.selected_stream_id = stream.id;
      player.open(sessionSource(session), { playback_epoch: epoch, engine_epoch: host, session_id: session.session_id });
      player.setVolume(state.volume);
      player.setSpeed(state.speed);
      video!.muted = state.muted;
      const index = session.subtitles.findIndex(s => s.id === state.subtitle_id);
      player.selectSubtitle(index < 0 ? null : index);
      if (index < 0) state.subtitle_id = null;
    }).catch(cause => { if (current(epoch)) fail(failure(cause)); });
    return tail;
  }

  async function diagnose(original: Bridge_ApiError, epoch: number, session: PlaybackViewSession) {
    const key = String(original.details?.resource_key ?? "video");
    if (!current(epoch, session) || queried.has(key)) return;
    queried.add(key);
    const host = state.engine_epoch!;
    try {
      const status = await request("get_resource_status", { kind: "playback", target_id: session.session_id }, host);
      if (!current(epoch, session)) return;
      if (status.kind !== "playback" || status.target_id !== session.session_id) throw new Error("Resource status target differs");
      if (!["ready", "expired", "failed"].includes(status.state) || status.state === "ready" && status.error !== null) throw new Error("Resource status state differs");
      const error = status.state === "ready" ? original : status.error;
      if (!error) throw new Error("Resource status error is missing");
      if (error.code === "SESSION_EXPIRED" && !recovered) {
        recovered = true;
        const resume = snapshot();
        if (resume) await load(resume.context, { resume, recovery: true });
      } else if (error.code === "UNAUTHORIZED" && !reconnected) {
        reconnected = true;
        await reconnect(epoch);
      } else fail({ ...error, details: { ...error.details, original_error: original } });
    } catch (cause) {
      if (current(epoch, session)) {
        const query = failure(cause);
        if (query.code === "UNAUTHORIZED") {
          if (!reconnected) { reconnected = true; await reconnect(epoch); }
          else fail({ ...query, details: { ...query.details, original_error: original } });
          return;
        }
        ui.diagnostic = `资源状态查询失败：${query.message}`;
        fail({ ...original, retryable: true, details: { ...original.details, query_error: query } });
      }
    }
  }

  function receive(event: PlayerEvent) {
    if (!state.session || !current(event.playback_epoch, state.session) || event.session_id !== state.session.session_id || event.engine_epoch !== state.engine_epoch || event.engine_epoch !== root.runtime.engine_epoch) return;
    if (event.type === "position") state.position_ms = event.position_ms;
    else if (event.type === "ready") {
      state.duration_ms = event.duration_ms;
      if (video) state.position_ms = Math.max(0, Math.round(video.currentTime * 1000));
      if (!["playing", "paused", "buffering"].includes(state.phase)) state.phase = "ready";
      if (pendingResume && event.duration_ms === null && pendingResume.position_ms > 0) ui.diagnostic = "恢复播放位置正在等待媒体时长";
      if (pendingResume && (event.duration_ms !== null || pendingResume.position_ms === 0)) {
        const resume = pendingResume;
        pendingResume = null;
        if (ui.diagnostic === "恢复播放位置正在等待媒体时长") ui.diagnostic = "";
        if (event.duration_ms !== null) {
          player!.seek(resume.position_ms / 1000);
          state.position_ms = Math.min(event.duration_ms, resume.position_ms);
        }
        if (resume.was_playing) void play();
      }
    } else if (event.type === "failed") {
      fail(event.error);
      if (event.error.details?.reason === "AUTOPLAY_BLOCKED") { ui.autoplayBlocked = true; ui.action = ""; }
      else if (event.error.details?.resource_kind) void diagnose(event.error, event.playback_epoch, state.session!);
    } else if (event.type === "stopped") state.phase = "idle";
    else {
      state.phase = event.type;
      if (event.type === "playing") { wantsPlaying = true; state.error = null; ui.autoplayBlocked = false; ui.action = ""; }
      if (event.type === "paused") wantsPlaying = false;
      if (event.type === "ended" && endedEpoch !== event.playback_epoch && state.context) {
        endedEpoch = event.playback_epoch;
        wantsPlaying = false;
        ended(structuredCloneContext(state.context), event.playback_epoch);
      }
    }
    if (video) state.buffered = Array.from({ length: video.buffered.length }, (_, i) => ({ start_ms: Math.round(video!.buffered.start(i) * 1000), end_ms: Math.round(video!.buffered.end(i) * 1000) }));
  }

  async function play() {
    wantsPlaying = true;
    try { await player?.play(); }
    catch { /* The current PlayerEvent owns the visible failure, including autoplay. */ }
  }
  function pause() { wantsPlaying = false; player?.pause(); }
  function seek(seconds: number) { player?.seek(seconds); if (video) state.position_ms = Math.round(video.currentTime * 1000); }
  async function stop() {
    ++state.playback_epoch;
    player?.stop();
    pendingResume = null;
    wantsPlaying = false;
    retiring.push({ session: state.session, resolution: state.resolution });
    state.session = null; state.resolution = null; state.context = null;
    state.phase = "idle"; state.position_ms = 0; state.duration_ms = null; state.buffered = [];
    state.error = null; ui.action = ""; ui.autoplayBlocked = false;
    for (const id of pendingRequests) void client.call("cancel_request", { target_request_id: id }).catch(cause => { ui.diagnostic = failure(cause).message; });
    tail = tail.then(drainRetired).catch(cause => { fail(failure(cause)); });
    await tail;
  }
  async function reconnect(epoch = state.playback_epoch) {
    const resume = snapshot();
    ui.action = "reconnect";
    try {
      await connect(true);
      if (!current(epoch)) return;
      if (resume) await load(resume.context, { resume, recovery: true });
    } catch (cause) { if (current(epoch)) fail(failure(cause)); }
  }

  async function imageError(data: DetailsViewData, resourceId: string, pageEpoch: number) {
    const active = () => root.details.data === data && root.details.request_epoch === pageEpoch && data.images[resourceId]?.resource_id === resourceId;
    if (!active()) return;
    if (imageAttempt !== data) { imageAttempt = data; queriedImages.clear(); }
    if (queriedImages.has(resourceId)) return;
    queriedImages.add(resourceId);
    const host = root.runtime.engine_epoch;
    if (!host) { ui.imageError = "图片状态查询需要已连接的宿主"; return; }
    try {
      const status = await client.call("get_resource_status", { kind: "image", target_id: resourceId }, crypto.randomUUID(), host);
      if (!active() || root.runtime.engine_epoch !== host) return;
      if (status.kind !== "image" || status.target_id !== resourceId) throw new Error("Image status target differs");
      if (status.error?.code === "SESSION_EXPIRED" && refreshedImageEpoch !== pageEpoch) {
        refreshedImageEpoch = pageEpoch;
        const refreshed = await client.call("get_details", { ref: data.details.ref }, crypto.randomUUID(), host);
        if (!active() || root.runtime.engine_epoch !== host) return;
        if (!sameRef(refreshed.details.ref, data.details.ref)) throw new Error("Image DTO source revision differs");
        root.details.data = refreshed;
      } else if (status.error?.code === "UNAUTHORIZED") {
        ui.imageError = status.error.message;
        await reconnect();
      } else ui.imageError = status.error?.message ?? "图片加载失败，请重试";
    } catch (cause) {
      if (active()) {
        const error = failure(cause);
        ui.imageError = `图片状态查询失败：${error.message}`;
        if (error.code === "UNAUTHORIZED") await reconnect();
      }
    }
  }

  async function retryImages() {
    const data = root.details.data;
    const epoch = root.details.request_epoch;
    const host = root.runtime.engine_epoch;
    if (!data || !host) return;
    try {
      const refreshed = await client.call("get_details", { ref: data.details.ref }, crypto.randomUUID(), host);
      if (root.details.data !== data || root.details.request_epoch !== epoch || root.runtime.engine_epoch !== host) return;
      if (!sameRef(refreshed.details.ref, data.details.ref)) throw new Error("Image DTO source revision differs");
      root.details.data = refreshed;
      ui.imageError = "";
      refreshedImageEpoch = -1;
    } catch (cause) { if (root.details.data === data) ui.imageError = `图片重试失败：${failure(cause).message}`; }
  }

  return {
    state, ui, load, stop, play, pause, seek, reconnect, imageError, retryImages, receive,
    attach(element: HTMLVideoElement) {
      if (player) throw new Error("Playback controller already owns a video");
      video = element;
      player = createPlayer(element, () => {}, receive);
    },
    retry() { const resume = snapshot(); return resume ? load(resume.context, { resume }) : retiring.length ? stop() : Promise.resolve(); },
    selectStream(id: string) { const resume = snapshot(); return resume ? load(resume.context, { stream: id, resume }) : Promise.resolve(); },
    setVolume(value: number) { player?.setVolume(value); state.volume = value; },
    setMuted(value: boolean) { state.muted = value; if (video) video.muted = value; },
    setSpeed(value: number) { player?.setSpeed(value); state.speed = value; },
    selectSubtitle(id: string | null) {
      const index = id === null ? null : state.session?.subtitles.findIndex(s => s.id === id);
      if (index === undefined || index === -1) throw new Error("Subtitle track is unavailable");
      player?.selectSubtitle(index); state.subtitle_id = id;
    },
    fullscreen: () => player?.fullscreen(),
    diagnostics: () => ({ ...player?.diagnostics(), retiring: retiring.length, pending_requests: pendingRequests.size }),
    settled: () => tail,
  };
}
export type PlaybackController = ReturnType<typeof createPlaybackController>;
