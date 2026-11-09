import { reactive } from "vue";
import { api, Api, ApiFailure, settingsQueue, SettingsQueue } from "./api.ts";
import { createPlayer, sessionSource, UnsupportedMedia, type Player } from "./player.ts";
import { t } from "./i18n.ts";
import type { HttpSourceConfig, AppState, Bridge_ApiError, Bridge_MediaRef, Bridge_Resolution, DetailsViewData,
  Command, CommandInput, PlaybackContext, PlaybackViewSession, PlayerEvent, ResumeSnapshot,
  AppSource, Bridge_HomeRequest, HomeViewData, MetadataRef, PlaybackMatchResult,
  PlaybackMatchSelection, Route, RuntimeStatus, Settings, SourceKey } from "./models.ts";

export const defaultSettings = (): Settings => ({ schema_version: 2, last_source_key: null,
  last_metadata_config_id: null, last_playback_source_key: null, ui_locale: "en", color_palette: "lime",
  volume: 1, muted: false, auto_next: true, window_bounds: null, source_configs: [] });

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
    settings: defaultSettings(),
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
  ended: (context: PlaybackContext, epoch: number) => void = () => {},
  runtimeChanged: (runtime: RuntimeStatus) => void = runtime => { root.runtime = runtime; }) {
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
  let guestEpoch: string | null | undefined;

  function current(epoch: number, session?: PlaybackViewSession) {
    return epoch === state.playback_epoch && (guestEpoch === undefined || guestEpoch === root.runtime.cloudstream.engine_epoch)
      && (!root.selected_playback_source || !state.context || sameSource(root.selected_playback_source, state.context.episode_ref))
      && (!session || state.session?.session_id === session.session_id
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
    const changed = root.runtime.engine_epoch !== null && root.runtime.engine_epoch !== runtime.engine_epoch;
    runtimeChanged(runtime);
    if (changed) throw new ApiFailure({ code: "ENGINE_NOT_READY", message: "Host restarted; reload sources and confirm playback again", retryable: true });
    state.engine_epoch = runtime.engine_epoch;
  }

  function load(context: PlaybackContext, options: { stream?: string; resume?: ResumeSnapshot | null; recovery?: boolean } = {}): Promise<void> {
    context = structuredCloneContext(context);
    if (options.resume && !sameContext(options.resume.context, context)) return Promise.reject(new Error("Resume context differs from playback context"));
    const epoch = ++state.playback_epoch;
    guestEpoch = root.sources.data?.providers.find(source => source.id === context.episode_ref.source_id)?.kind === "cloudstream"
      ? root.runtime.cloudstream.engine_epoch : undefined;
    player?.stop();
    const keep = options.stream ? state.resolution : null;
    // Stream handles belong to one Resolution; a fresh resolve signs new IDs.
    if (!keep) {
      context.selected_stream_id = null;
      if (root.route.name === "playback" && root.route.context.match_id === context.match_id) root.route.context.selected_stream_id = null;
    }
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
      const selected = options.stream ?? context.selected_stream_id;
      const stream = selected ? resolution.streams.find(s => s.id === selected) : resolution.streams.find(s => s.playable);
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
      if (root.route.name === "playback" && root.route.context.match_id === context.match_id) root.route.context.selected_stream_id = stream.id;
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
    if (!current(epoch, session)) return;
    if (queried.has(key)) {
      // Repeated native errors must retain the pending/completed diagnosis and
      // its recovery action. After successful playback, a new failure is visible
      // again, while this loading attempt's status query remains once-only.
      if (state.error) state.phase = "failed";
      else fail(original);
      return;
    }
    queried.add(key);
    fail(original);
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
          const position = Math.min(event.duration_ms, resume.position_ms);
          if (video!.currentTime !== position / 1000) player!.seek(position / 1000);
          state.position_ms = position;
        }
        if (resume.was_playing) void play();
      }
    } else if (event.type === "failed") {
      if (event.error.details?.reason === "AUTOPLAY_BLOCKED") { fail(event.error); ui.autoplayBlocked = true; ui.action = ""; }
      else if (event.error.details?.resource_kind) void diagnose(event.error, event.playback_epoch, state.session!);
      else fail(event.error);
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
    if (root.route.name === "playback") root.route = root.details.data
      ? { name: "details", ref: { ...root.details.data.details.ref } } : { name: "home" };
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
    diagnostics: () => ({ ...player?.diagnostics(), retiring: retiring.length, pending_requests: pendingRequests.size,
      queried_resources: queried.size, queried_images: queriedImages.size }),
    settled: () => tail,
  };
}
export type PlaybackController = ReturnType<typeof createPlaybackController>;

// App DTOs are JSON values; this also removes nested Vue proxies in route/context.
const clone = <T>(value: T): T => JSON.parse(JSON.stringify(value));
const page = () => ({ phase: "idle" as const, request_epoch: 0, request_id: null, data: null, error: null });
export const sameSource = (a: SourceKey | Bridge_MediaRef | null, b: SourceKey | Bridge_MediaRef | null) =>
  a !== null && b !== null && a.source_id === b.source_id && a.source_revision === b.source_revision;
const sameMetadata = (a: MetadataRef, b: MetadataRef) => sameRef(a.content_ref, b.content_ref)
  && (["config_id", "addon_key", "namespace", "type", "meta_id", "canonical_id"] as const).every(key => a.identity[key] === b.identity[key]);
const sameContext = (a: PlaybackContext, b: PlaybackContext) => sameMetadata(a.metadata_ref, b.metadata_ref)
  && sameRef(a.content_ref, b.content_ref) && sameRef(a.episode_ref, b.episode_ref)
  && (["metadata_video_id", "match_id", "group_id", "episode_id", "selected_stream_id"] as const).every(key => a[key] === b[key]);
function requireBinding(condition: unknown, message: string): asserts condition {
  if (!condition) throw new ApiFailure({ code: "BAD_REQUEST", message, retryable: false });
}

export type SourceDraft = { kind: "maccms" | "stremio"; name: string; urls: string };
const draftUrls = (draft: SourceDraft) => draft.urls.split("\n").map(url => url.trim()).filter(Boolean);
// SPEC 18.1: at most 32 configurations and 16 manifests, anonymous http(s) URLs only.
export function validateSourceDraft(draft: SourceDraft, others: number) {
  const errors: string[] = [], urls = draftUrls(draft);
  if (others >= 32) errors.push(t("At most 32 source configurations are allowed."));
  if (!draft.name.trim()) errors.push(t("Enter a name for this source."));
  if (!urls.length) errors.push(t("Enter a URL."));
  if (draft.kind === "maccms" && urls.length > 1) errors.push(t("MacCMS takes exactly one API URL."));
  if (urls.length > 16) errors.push(t("At most 16 manifest URLs are allowed."));
  for (const url of urls) {
    let parsed: URL;
    try { parsed = new URL(url); } catch { errors.push(t("Not a full URL: {url}", { url: redactDiagnostics(url) })); continue; }
    if (!["http:", "https:"].includes(parsed.protocol) || parsed.username || parsed.password || parsed.hash) errors.push(t("Use http(s) without credentials or a fragment: {url}", { url: redactDiagnostics(url) }));
    else if (draft.kind === "stremio" && !parsed.pathname.endsWith("/manifest.json")) errors.push(t("A Stremio URL must end in /manifest.json: {url}", { url: redactDiagnostics(url) }));
  }
  return errors;
}
// Diagnostics leave the app: URLs keep only origin and path, and credential-shaped values are removed.
export function redactDiagnostics(text: string) {
  return text.replace(/\b(https?:\/\/)(?:[^\s/@]*@)?([^\s/?#]+)([^\s?#]*)[^\s]*/gi, "$1$2$3")
    .replace(/\b(cap|token|capability|authorization|bearer|cookie|password|secret)(["'=:\s]+)[^\s&"',;]+/gi, "$1$2<redacted>");
}

// Host's Stremio video reference is ["stremio_video", addon_key, type, meta_id, video_id];
// the session-only browser preview publishes { id, video_id }.
export function metadataVideoId(reference: Bridge_MediaRef) {
  try {
    const fields = JSON.parse(reference.data);
    const value = Array.isArray(fields) ? fields.length === 5 && fields[0] === "stremio_video" && fields[4] : fields?.video_id;
    return typeof value === "string" ? value : "";
  } catch { return ""; }
}

export function assertContext(state: AppState, context: PlaybackContext) {
  const metadata = state.sources.data?.providers.find(source => source.id === state.selected_metadata_source?.source_id);
  const provider = state.sources.data?.providers.find(source => source.id === state.selected_playback_source?.source_id);
  requireBinding(metadata?.kind === "stremio" && metadata.runtime_status === "ready" && sameSource(state.selected_metadata_source, context.metadata_ref.content_ref)
    && metadata.source_revision === context.metadata_ref.content_ref.source_revision
    && metadata.origin.config_id === context.metadata_ref.identity.config_id, "Metadata source/configuration binding differs");
  requireBinding(provider && provider.kind !== "stremio" && provider.runtime_status === "ready"
    && provider.capabilities.resolve && provider.source_revision === context.episode_ref.source_revision
    && sameSource(state.selected_playback_source, context.episode_ref)
    && sameSource(context.content_ref, context.episode_ref)
    && context.metadata_ref.content_ref.source_id !== context.episode_ref.source_id, "Playback source/revision binding differs");
  requireBinding(context.match_id.length > 0 && context.group_id.length > 0 && context.episode_id.length > 0, "Missing confirmed match/episode identity");
  const details = state.details.data;
  requireBinding(details?.metadata_ref && sameMetadata(details.metadata_ref, context.metadata_ref)
    && sameRef(details.details.ref, context.metadata_ref.content_ref), "Current details and playback metadata differ");
  if (state.route.name === "details") requireBinding(sameRef(state.route.ref, details.details.ref), "Route and details differ");
  requireBinding(details.details.groups.some(group => group.episodes.some(episode => {
    return sameSource(episode.ref, context.metadata_ref.content_ref) && metadataVideoId(episode.ref) === context.metadata_video_id;
  })), "Video is absent from current metadata details");
}

export function assertPlayback(state: AppState) {
  const { context, resolution, session } = state.playback;
  if (context) {
    assertContext(state, context);
    requireBinding(state.playback.engine_epoch === state.runtime.engine_epoch, "Playback host epoch differs");
  }
  if (state.route.name === "playback") requireBinding(context && sameContext(state.route.context, context), "Route and playback context differ");
  if (resolution) {
    requireBinding(context && sameRef(context.episode_ref, resolution.episode_ref)
      && sameSource(resolution.episode_ref, resolution), "Resolution belongs to another episode/source");
    requireBinding(context.selected_stream_id === null || resolution.streams.some(stream => stream.id === context.selected_stream_id && stream.playable), "Selected stream is absent or unplayable");
  } else requireBinding(!context?.selected_stream_id, "Selected stream requires a Resolution");
  if (session) {
    const stream = resolution?.streams.find(stream => stream.id === session.stream_id);
    requireBinding(context && resolution && context.selected_stream_id === session.stream_id
      && resolution.id === session.resolution_id && sameSource(resolution, session)
      && sameRef(resolution.episode_ref, session.episode_ref) && stream?.playable
      && stream.kind === session.stream_kind, "Session/Resolution/stream binding differs");
  }
}

// Validate before mutating the current snapshot. Titles and opaque references never determine order.
export function mergeHome(current: HomeViewData | null, incoming: HomeViewData, request: Bridge_HomeRequest): HomeViewData {
  const full = request.page === 1 && request.section_ref == null;
  const source = { source_id: incoming.page.source_id, source_revision: incoming.page.source_revision };
  requireBinding(sameSource(source, request), "Home response source/revision differs");
  const positions = [...incoming.page.sections, ...incoming.page.failed_sections].map(section => section.index);
  requireBinding(positions.every(index => Number.isSafeInteger(index) && index >= 0) && new Set(positions).size === positions.length, "Duplicate/invalid home section index");
  requireBinding(incoming.page.sections.every(section => section.page === request.page
    && section.items.every(item => sameSource(item.ref, request))), "Home page/card source differs");
  let result: HomeViewData;
  if (full) result = clone(incoming);
  else {
    requireBinding(current && sameSource(current.page, source) && request.section_ref, "Local home request requires its current snapshot");
    const existing = [...current.page.sections, ...current.page.failed_sections].find(section => section.section_ref === request.section_ref);
    requireBinding(existing && positions.length === 1, "Local response must target one registered section");
    const returned = [...incoming.page.sections, ...incoming.page.failed_sections][0]!;
    requireBinding(returned.index === existing.index && returned.section_ref === request.section_ref, "Local response section/index differs");
    result = clone(current);
    const section = incoming.page.sections[0];
    if (section) {
      const previous = result.page.sections.find(item => item.index === section.index);
      if (request.page >= 2) requireBinding(previous && previous.id === section.id && previous.next_page === request.page, "Pagination identity/page differs");
      const replacement = clone(section);
      if (request.page >= 2) {
        const items = new Map(previous!.items.map(item => [item.id, item]));
        for (const item of replacement.items) items.set(item.id, item);
        replacement.items = [...items.values()];
      }
      result.page.sections = result.page.sections.filter(item => item.index !== section.index).concat(replacement);
      result.page.failed_sections = result.page.failed_sections.filter(item => item.index !== section.index);
    } else {
      // A failed local request preserves its cards and attaches a local error at the controller.
      return result;
    }
    result.images = { ...result.images, ...clone(incoming.images) };
  }
  result.page.sections.sort((a, b) => a.index - b.index);
  result.page.failed_sections.sort((a, b) => a.index - b.index);
  const images = new Set(result.page.sections.flatMap(section => section.items.flatMap(item =>
    [item.poster, item.backdrop].flatMap(image => image ? [image.resource_id] : []))));
  result.images = Object.fromEntries(Object.entries(result.images).filter(([key]) => images.has(key)));
  return result;
}


type Ticket = { id: string; source: SourceKey | null; host: string | null; guest: string | null | undefined; locale: string; epoch: number };
type LocalRequest = { request_id: string | null; phase: "loading" | "ready" | "failed"; error: Bridge_ApiError | null };
export type HomeFilter = NonNullable<Bridge_HomeRequest["filter"]>;
export type CatalogRow = { phase: "pending" | "loading" | "ready" | "failed"; data: HomeViewData | null; error: Bridge_ApiError | null };
export const normalizedFilter = (filter: Bridge_HomeRequest["filter"]) =>
  JSON.stringify(Object.entries(filter ?? {}).filter(([, value]) => value != null).sort(([a], [b]) => a.localeCompare(b)));
export class AppController {
  readonly rows = reactive<Record<string, CatalogRow>>({});
  private catalogGeneration = 0;
  private catalogMode = false;
  private homeFilters = new Map<string, Bridge_HomeRequest["filter"]>();
  readonly state: AppState;
  readonly sections = reactive<Record<string, LocalRequest>>({});
  readonly diagnostics = reactive<Bridge_ApiError[]>([]);
  readonly homeImage = reactive({ error: "" });
  readonly detailsImage = reactive({ error: "" });
  readonly matching = reactive({ result: null as PlaybackMatchResult | null, details: null as DetailsViewData | null,
    loading: false, pending: null as string | null, error: null as Bridge_ApiError | null });
  private requests = new Map<string, Ticket>();
  private imageAttempts = { home: { data: null as object | null, queried: new Set<string>(), refreshed: false },
    details: { data: null as object | null, queried: new Set<string>(), refreshed: false } };
  private runtimeSequence = 0;
  private sourceSequence = 0;
  private confirmation: PlaybackMatchResult | null = null;
  private settings: SettingsQueue;
  private stopMedia: (() => Promise<void>) | null = null;
  private retryRetirement: (() => Promise<void>) | null = null;
  private cleanup: Promise<void> = Promise.resolve();
  private api: Api;
  constructor(client: Api = api, settings: SettingsQueue = settingsQueue, initial: AppState = appState) {
    this.api = client; this.state = initial; this.settings = settings;
  }
  bindPlayback(controller: PlaybackController) {
    const stop = async () => {
      await controller.stop();
      if (controller.diagnostics().retiring) throw new ApiFailure(controller.state.error ?? {
        code: "UPSTREAM_FAILED", message: "Playback resources are awaiting cleanup", retryable: true,
      });
    };
    this.stopMedia = stop;
    return () => { if (this.stopMedia === stop) this.stopMedia = null; };
  }
  async retryCleanup() {
    if (!this.retryRetirement) return;
    this.cleanup = this.retryRetirement();
    await this.cleanup;
    this.retryRetirement = null;
    this.diagnostics.splice(0);
  }
  get awaitingCleanup() { return this.retryRetirement !== null; }
  private ticket(scope: string, source: SourceKey | null, epoch: number): Ticket {
    this.cancel(scope);
    const provider = this.state.sources.data?.providers.find(item => item.id === source?.source_id);
    const value = { id: crypto.randomUUID(), source: source && clone(source), host: this.state.runtime.engine_epoch,
      guest: provider?.kind === "cloudstream" ? this.state.runtime.cloudstream.engine_epoch : undefined,
      locale: this.state.settings.ui_locale ?? "en", epoch };
    this.requests.set(scope, value);
    return value;
  }
  private current(scope: string, ticket: Ticket) {
    const provider = this.state.sources.data?.providers.find(item => item.id === ticket.source?.source_id);
    return this.requests.get(scope) === ticket && ticket.host === this.state.runtime.engine_epoch
      && (!["home", "details"].includes(scope) && !scope.startsWith("section:") || ticket.locale === (this.state.settings.ui_locale ?? "en"))
      && (ticket.guest === undefined || ticket.guest === this.state.runtime.cloudstream.engine_epoch)
      && (!["home", "details"].includes(scope) && !scope.startsWith("section:") || sameSource(ticket.source, this.state.selected_metadata_source))
      && (!["match", "candidate"].includes(scope) || sameSource(ticket.source, this.state.selected_playback_source))
      && (!["match", "candidate"].includes(scope) || ticket.epoch === this.state.playback.playback_epoch)
      && (!ticket.source || (provider?.source_revision === ticket.source.source_revision && provider.runtime_status === "ready"));
  }
  private cancel(scope: string) {
    const ticket = this.requests.get(scope);
    this.requests.delete(scope);
    if (ticket) void this.api.cancel(ticket.id).catch(error => this.diagnostics.push(failure(error)));
  }
  private cancelPages() {
    for (const scope of [...this.requests.keys()]) if (scope === "home" || scope === "details" || scope.startsWith("section:")) this.cancel(scope);
    this.state.home.request_epoch++;
    this.state.details.request_epoch++;
    this.state.home.request_id = this.state.details.request_id = null;
    if (this.state.home.phase === "loading") this.state.home.phase = this.state.home.data ? "ready" : "idle";
    if (this.state.details.phase === "loading") this.state.details.phase = this.state.details.data ? "ready" : "idle";
    for (const key of Object.keys(this.sections)) delete this.sections[key];
    for (const row of Object.values(this.rows)) if (row.phase === "loading") row.phase = row.data ? "ready" : "pending";
  }
  rowKey(sectionRef: string, filter: HomeFilter = {}) {
    return JSON.stringify([this.state.selected_metadata_source, this.state.runtime.engine_epoch,
      this.state.settings.ui_locale ?? "en", this.catalogGeneration, sectionRef, normalizedFilter(filter)]);
  }
  catalogRow(sectionRef: string, filter: HomeFilter = {}) {
    const key = this.rowKey(sectionRef, filter);
    return this.rows[key] ??= { phase: "pending", data: null, error: null };
  }
  async loadRow(sectionRef: string, filter: HomeFilter = {}, pageNumber = 1) {
    const source = this.state.selected_metadata_source;
    requireBinding(source && this.state.runtime.engine_epoch, "Row requires a metadata source and host epoch");
    const key = this.rowKey(sectionRef, filter), row = this.catalogRow(sectionRef, filter);
    requireBinding(pageNumber === 1 || row.data?.page.sections[0]?.next_page === pageNumber, "Row pagination requires its current continuation");
    const scope = `section:row:${key}`, ticket = this.ticket(scope, source, this.state.home.request_epoch);
    const input = { ...clone(source), section_ref: sectionRef, page: pageNumber, filter: clone(filter) };
    const valid = () => this.current(scope, ticket) && key === this.rowKey(sectionRef, filter);
    row.phase = "loading"; row.error = null;
    try {
      const response = await this.api.call("get_home", input, ticket.id, ticket.host ?? undefined);
      if (!valid()) return;
      const initial = pageNumber === 1 ? this.state.home.data : row.data;
      const merged = mergeHome(initial, response, input);
      const error = response.page.failed_sections[0]?.error;
      if (error) throw new ApiFailure(error);
      // A row owns only its own images and continuation, even when another row
      // uses the same opaque section reference with a different filter.
      const section = merged.page.sections.find(section => section.section_ref === sectionRef)!;
      row.data = { page: { ...merged.page, sections: [section], failed_sections: [] }, images: merged.images };
      row.phase = "ready";
    } catch (cause) { if (valid()) { row.phase = "failed"; row.error = failure(cause); } }
    finally { if (this.requests.get(scope) === ticket) this.requests.delete(scope); }
  }
  async rowImageError(sectionRef: string, filter: HomeFilter, id: string) {
    const key = this.rowKey(sectionRef, filter), row = this.catalogRow(sectionRef, filter), snapshot = row.data;
    if (!snapshot?.images[id]) return;
    const status = await this.api.call("get_resource_status", { kind: "image", target_id: id });
    if (key !== this.rowKey(sectionRef, filter) || row.data !== snapshot) return;
    if (status.error?.code === "SESSION_EXPIRED") await this.loadRow(sectionRef, filter);
    else { row.error = status.error ?? { code: "UPSTREAM_FAILED", message: "Image failed to load", retryable: true }; }
  }
  loadCatalog() { return this.loadHome(null, 1, null, true); }
  async initialize() {
    await this.restoreSettings();
    await this.refreshRuntime();
    await this.loadSources();
  }
  async restoreSettings() {
    if (this.settings.paused) await this.settings.recover();
    this.state.settings = await this.settings.initialize();
    this.state.playback.volume = this.state.settings.volume;
    this.state.playback.muted = this.state.settings.muted;
    return this.state.settings;
  }
  async refreshRuntime() {
    const sequence = ++this.runtimeSequence;
    const runtime = await this.api.call("runtime_status", {});
    if (sequence !== this.runtimeSequence) return;
    this.applyRuntime(runtime);
  }
  applyRuntime(runtime: RuntimeStatus) {
    requireBinding(runtime.phase !== "ready" || (typeof runtime.engine_epoch === "string"
      && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(runtime.engine_epoch)
      && runtime.health?.status === "ready" && runtime.health.media_gateway_ready), "Ready runtime requires healthy host/gateway");
    this.runtimeSequence++;
    const previous = this.state.runtime;
    const hostChanged = previous.engine_epoch !== runtime.engine_epoch;
    const guestChanged = previous.cloudstream.engine_epoch !== runtime.cloudstream.engine_epoch;
    this.state.runtime = clone(runtime);
    if (hostChanged) {
      this.cancelPages();
      this.sourceSequence++;
      this.cancel("sources");
      this.invalidatePlayback();
      this.state.sources = { ...page(), request_epoch: this.state.sources.request_epoch + 1 };
      this.state.home.data = this.state.details.data = null;
      this.state.home.phase = this.state.details.phase = "idle";
      this.state.home.error = this.state.details.error = null;
      this.state.selected_metadata_source = this.state.selected_playback_source = null;
      this.state.route = { name: "home" };
    } else if (guestChanged) {
      this.cancel("sources"); this.sourceSequence++;
      this.state.sources.request_epoch++; this.state.sources.request_id = null;
      for (const [scope, ticket] of this.requests) if (ticket.guest !== undefined) this.cancel(scope);
      const selected = this.state.selected_playback_source;
      if (this.state.sources.data?.providers.some(source => source.id === selected?.source_id && source.kind === "cloudstream")) this.invalidatePlayback();
      if (this.state.sources.data) this.state.sources.data.providers = this.state.sources.data.providers.filter(source => source.kind !== "cloudstream");
      this.state.sources.phase = this.state.sources.data ? this.state.sources.data.providers.length ? "ready" : "empty" : "idle";
      this.state.sources.error = null;
      if (selected && !this.state.sources.data?.providers.some(source => source.id === selected.source_id)) this.state.selected_playback_source = null;
    }
  }
  async loadSources() {
    const target = this.state.sources;
    const sequence = ++this.sourceSequence;
    const ticket = this.ticket("sources", null, ++target.request_epoch);
    target.phase = "loading"; target.request_id = ticket.id; target.error = null;
    try {
      let response = await this.api.call("get_sources", {}, ticket.id, ticket.host ?? undefined);
      // Configurations are persisted, while published catalogs belong to this host session.
      const pending = response.providers.filter(source => source.runtime_status === "configured" && source.kind !== "cloudstream");
      for (const source of pending) {
        if (sequence !== this.sourceSequence || !this.current("sources", ticket)) return;
        if (source.kind === "cloudstream") continue;
        try { await this.syncConfig(source.kind, source.origin.config_id); }
        catch (cause) { this.diagnostics.push(failure(cause)); }
      }
      if (pending.length) response = await this.api.call("get_sources", {}, crypto.randomUUID(), ticket.host ?? undefined);
      if (sequence !== this.sourceSequence || !this.current("sources", ticket)) return;
      const oldMetadata = this.state.selected_metadata_source, oldPlayback = this.state.selected_playback_source;
      target.data = clone(response);
      target.phase = response.providers.length ? "ready" : "empty";
      const ready = response.providers.filter(source => source.runtime_status === "ready");
      const metadataReady = ready.filter(source => source.kind === "stremio" && source.capabilities.home && source.capabilities.details);
      const playbackReady = ready.filter(source => source.kind !== "stremio" && source.capabilities.resolve);
      const metadata = metadataReady.find(source => source.kind === "stremio" && source.origin.config_id === this.state.settings.last_metadata_config_id)
        ?? metadataReady.find(source => source.id === oldMetadata?.source_id) ?? metadataReady[0];
      const playback = playbackReady.find(source => source.id === (this.state.settings.last_playback_source_key ?? this.state.settings.last_source_key))
        ?? playbackReady.find(source => source.id === oldPlayback?.source_id) ?? playbackReady[0];
      this.state.selected_metadata_source = metadata ? { source_id: metadata.id, source_revision: metadata.source_revision } : null;
      this.state.selected_playback_source = playback ? { source_id: playback.id, source_revision: playback.source_revision } : null;
      if (oldMetadata && !sameSource(oldMetadata, this.state.selected_metadata_source)) {
        this.cancelPages(); this.invalidatePlayback(); this.state.details.data = this.state.home.data = null;
        this.state.home.phase = this.state.details.phase = "idle";
        this.state.home.error = this.state.details.error = null;
        this.state.route = { name: "home" };
      } else if (oldPlayback && !sameSource(oldPlayback, this.state.selected_playback_source)) this.invalidatePlayback();
    } catch (error) { if (this.current("sources", ticket)) { target.phase = "failed"; target.error = failure(error); } }
    finally { if (this.requests.get("sources") === ticket) { this.requests.delete("sources"); target.request_id = null; } }
  }
  async updateSettings(change: (value: Settings) => Settings) {
    const previous = this.state.settings;
    const confirmed = await this.settings.update(change);
    this.state.settings = confirmed;
    if ((previous.ui_locale ?? "en") !== (confirmed.ui_locale ?? "en")) {
      this.cancelPages();
      // Active playback retains the same confirmed identity and session; only display DTOs reload.
      if (this.state.route.name === "home" && this.state.selected_metadata_source) void (this.catalogMode ? this.loadCatalog() : this.loadHome());
      else if (this.state.details.data && this.state.route.name === "details") void this.loadDetails(this.state.details.data.details.ref);
    }
    return confirmed;
  }
  async selectSource(sourceId: string, role: "metadata" | "playback") {
    const source = this.state.sources.data?.providers.find(source => source.id === sourceId);
    requireBinding(source?.runtime_status === "ready" && (role === "metadata" ? source.kind === "stremio" && source.capabilities.home && source.capabilities.details : source.kind !== "stremio" && source.capabilities.resolve), "Source is unavailable for this role");
    const configId = source.kind === "stremio" ? source.origin.config_id : null;
    const unchanged = sameSource(role === "metadata" ? this.state.selected_metadata_source : this.state.selected_playback_source,
      { source_id: source.id, source_revision: source.source_revision });
    // A fallback selection can differ from the saved choice; confirming it only saves the choice.
    if (unchanged && (role === "metadata" ? this.state.settings.last_metadata_config_id === configId : this.state.settings.last_playback_source_key === source.id)) return;
    const host = this.state.runtime.engine_epoch;
    await this.updateSettings(settings => role === "metadata"
      ? { ...settings, last_metadata_config_id: configId }
      : { ...settings, last_playback_source_key: source.id });
    requireBinding(host === this.state.runtime.engine_epoch && this.state.sources.data?.providers.some(current =>
      current.id === source.id && current.source_revision === source.source_revision && current.runtime_status === "ready"), "Source changed while saving its preference");
    if (unchanged) return;
    const context = this.state.playback.context && clone(this.state.playback.context);
    this.invalidatePlayback();
    const key = { source_id: source.id, source_revision: source.source_revision };
    if (role === "metadata") {
      this.cancelPages(); this.state.home.data = this.state.details.data = null;
      this.state.home.phase = this.state.details.phase = "idle";
      this.state.home.error = this.state.details.error = null;
      this.state.selected_metadata_source = key;
      this.state.route = { name: "home" };
    } else {
      this.state.selected_playback_source = key;
      if (context) await this.match(context.metadata_video_id);
    }
  }
  navigate(route: Route) {
    if (route.name === "details") requireBinding(sameSource(route.ref, this.state.selected_metadata_source), "Route requires the selected metadata source");
    if (route.name === "playback") { assertContext(this.state, route.context); requireBinding(this.state.playback.context && sameContext(this.state.playback.context, route.context), "Route requires the current confirmed context"); }
    else this.invalidatePlayback();
    this.cancelPages();
    this.state.route = clone(route);
  }
  async loadHome(sectionRef: string | null = null, pageNumber = 1, filter?: Bridge_HomeRequest["filter"], catalogOnly = false) {
    const source = this.state.selected_metadata_source;
    requireBinding(source && this.state.runtime.engine_epoch, "Home requires a metadata source and host epoch");
    const full = sectionRef === null && pageNumber === 1;
    requireBinding(sectionRef !== null || full, "Pagination requires a section reference");
    const selectedFilter = filter === undefined ? this.homeFilters.get(sectionRef ?? "") ?? null : filter;
    const filterKey = (value: Bridge_HomeRequest["filter"]) => JSON.stringify(Object.entries(value ?? {}).sort(([a], [b]) => a.localeCompare(b)));
    if (pageNumber >= 2) requireBinding(filterKey(selectedFilter) === filterKey(this.homeFilters.get(sectionRef!)), "Pagination requires the current section filter");
    if (full) {
      this.catalogMode = catalogOnly;
      this.catalogGeneration++;
      for (const key of Object.keys(this.rows)) delete this.rows[key];
      this.imageAttempts.home.refreshed = false; this.homeImage.error = "";
      for (const scope of [...this.requests.keys()]) if (scope.startsWith("section:")) this.cancel(scope);
      for (const key of Object.keys(this.sections)) delete this.sections[key];
      this.state.home.request_epoch++;
    }
    const scope = full ? "home" : `section:${sectionRef}`;
    const ticket = this.ticket(scope, source, this.state.home.request_epoch);
    const input = { ...clone(source), section_ref: sectionRef, page: pageNumber, ...(selectedFilter ? { filter: clone(selectedFilter) } : {}), ...(catalogOnly ? { catalog_only: true } : {}) };
    if (full) { this.state.home.phase = "loading"; this.state.home.request_id = ticket.id; this.state.home.error = null; }
    else this.sections[sectionRef!] = { request_id: ticket.id, phase: "loading", error: null };
    const valid = () => this.current(scope, ticket) && ticket.epoch === this.state.home.request_epoch;
    try {
      const response = await this.api.call("get_home", input, ticket.id, ticket.host ?? undefined);
      if (!valid()) return;
      const next = mergeHome(this.state.home.data, response, input);
      this.state.home.data = next;
      if (full) this.homeFilters.clear();
      for (const section of response.page.sections) this.homeFilters.set(section.section_ref, clone(selectedFilter));
      if (full) {
        this.state.home.phase = next.page.sections.some(section => catalogOnly || section.items.length) ? "ready" : next.page.failed_sections.length ? "failed" : "empty";
        this.state.home.error = this.state.home.phase === "failed" ? next.page.failed_sections[0]!.error : null;
      }
      else {
        const error = response.page.failed_sections[0]?.error ?? null;
        this.sections[sectionRef!] = { request_id: null, phase: error ? "failed" : "ready", error };
      }
    } catch (error) {
      if (valid()) {
        if (full) { this.state.home.phase = "failed"; this.state.home.error = failure(error); }
        else this.sections[sectionRef!] = { request_id: null, phase: "failed", error: failure(error) };
      }
    } finally { if (this.requests.get(scope) === ticket) { this.requests.delete(scope); if (full) this.state.home.request_id = null; } }
  }
  homeImageError(resourceId: string) { return this.imageError("home", resourceId); }
  detailsImageError(resourceId: string) { return this.imageError("details", resourceId); }
  // SPEC 11.3: only confirmed expiry refreshes the owning DTO, once; home replaces it with page 1.
  private async imageError(owner: "home" | "details", resourceId: string) {
    const page = owner === "home" ? this.state.home : this.state.details, attempt = this.imageAttempts[owner];
    const output = owner === "home" ? this.homeImage : this.detailsImage;
    const data = page.data as HomeViewData | DetailsViewData | null, host = this.state.runtime.engine_epoch, epoch = page.request_epoch;
    const active = () => page.data === data && page.request_epoch === epoch && this.state.runtime.engine_epoch === host;
    // A page already loading replaces this DTO; its old images must not reload the old page over it.
    if (!data?.images[resourceId] || !host || page.phase === "loading") return;
    if (attempt.data !== data) { attempt.data = data; attempt.queried.clear(); }
    if (attempt.queried.has(resourceId)) return;
    attempt.queried.add(resourceId);
    try {
      const status = await this.api.call("get_resource_status", { kind: "image", target_id: resourceId }, crypto.randomUUID(), host);
      if (!active()) return;
      requireBinding(status.kind === "image" && status.target_id === resourceId, "Image status target differs");
      if (status.error?.code === "SESSION_EXPIRED" && !attempt.refreshed) {
        const refresh = owner === "home" ? this.loadHome() : this.loadDetails((data as DetailsViewData).details.ref);
        attempt.refreshed = true;
        await refresh;
      } else output.error = status.error?.message ?? "Image failed to load";
    } catch (cause) { if (active()) output.error = failure(cause).message; }
  }
  // The metadata video after this one in the current details order; null at the last episode.
  nextVideo(videoId: string) {
    const videos = (this.state.details.data?.details.groups ?? []).flatMap(group => group.episodes).map(episode => metadataVideoId(episode.ref));
    const index = videos.indexOf(videoId);
    return index >= 0 ? videos[index + 1] ?? null : null;
  }
  lastSourceResult(sourceId: string) { return this.api.sourceResults[sourceId] ?? null; }
  private async syncConfig(kind: "maccms" | "stremio", configId: string) {
    await this.api.call("sync_catalog", { kind, config_id: configId });
  }
  async saveSourceConfig(draft: SourceDraft & { config_id: string | null; enabled: boolean }) {
    const urls = draftUrls(draft), id = draft.config_id ?? crypto.randomUUID(), name = draft.name.trim();
    const config: HttpSourceConfig = draft.kind === "maccms" ? { kind: "maccms", config_id: id, name, enabled: draft.enabled, api_url: urls[0]! }
      : { kind: "stremio", config_id: id, name, enabled: draft.enabled, manifest_urls: urls };
    await this.updateSettings(settings => ({ ...settings, source_configs: draft.config_id
      ? settings.source_configs.map(row => row.config_id === id ? config : row) : [...settings.source_configs, config] }));
    try { if (config.enabled) await this.syncConfig(config.kind, id); }
    finally { await this.loadSources(); }
  }
  async setSourceEnabled(id: string, enabled: boolean) {
    const config = this.state.settings.source_configs.find(row => row.config_id === id);
    requireBinding(config, "Source configuration is missing");
    await this.updateSettings(settings => ({ ...settings, source_configs: settings.source_configs.map(row => row.config_id === id ? { ...row, enabled } : row) }));
    try { if (enabled) await this.syncConfig(config.kind, id); }
    finally { await this.loadSources(); }
  }
  async removeSourceConfig(id: string) {
    await this.updateSettings(settings => ({ ...settings, source_configs: settings.source_configs.filter(row => row.config_id !== id),
      last_metadata_config_id: settings.last_metadata_config_id === id ? null : settings.last_metadata_config_id }));
    await this.loadSources();
  }
  diagnosticsReport() {
    const { runtime, sources } = this.state;
    const lines = [`platform: ${runtime.platform}`, `host phase: ${runtime.phase}`, `contract: ${runtime.health?.contract_version ?? "unavailable"}`,
      ...(sources.data?.providers ?? []).map(source => `source ${source.kind} ${source.name} registration=${source.runtime_status}${source.error ? ` error=${source.error.code}` : ""}`),
      ...this.api.failures.map(row => `failure ${row.command} ${row.code} correlation=${row.request_id}: ${row.message}`)];
    return redactDiagnostics(lines.join("\n"));
  }
  async loadDetails(ref: Bridge_MediaRef) {
    requireBinding(sameSource(ref, this.state.selected_metadata_source), "Details must belong to selected metadata source");
    const target = this.state.details;
    this.imageAttempts.details.refreshed = false; this.detailsImage.error = "";
    const ticket = this.ticket("details", ref, ++target.request_epoch);
    target.phase = "loading"; target.request_id = ticket.id; target.error = null;
    try {
      const response = await this.api.call("get_details", { ref: clone(ref) }, ticket.id, ticket.host ?? undefined);
      if (!this.current("details", ticket)) return;
      requireBinding(sameRef(response.details.ref, ref) && response.metadata_ref && sameRef(response.metadata_ref.content_ref, ref), "Details response identity differs");
      requireBinding(response.details.groups.every(group => group.episodes.every(episode => sameSource(episode.ref, ref))), "Metadata episode source/revision differs");
      const provider = this.state.sources.data?.providers.find(source => source.id === ref.source_id);
      requireBinding(provider?.kind === "stremio" && provider.origin.config_id === response.metadata_ref.identity.config_id, "Details metadata configuration differs");
      target.data = clone(response); target.phase = "ready";
    } catch (error) { if (this.current("details", ticket)) { target.phase = "failed"; target.error = failure(error); } }
    finally { if (this.requests.get("details") === ticket) { this.requests.delete("details"); target.request_id = null; } }
  }
  invalidatePlayback() {
    this.confirmation = null;
    this.matching.result = null; this.matching.details = null; this.matching.error = null; this.matching.loading = false; this.matching.pending = null;
    this.cancel("match"); this.cancel("candidate");
    // The existing media controller owns Session/Resolution retirement and cancellation.
    // Keep the reactive playback object stable for mounted views and event listeners.
    if (this.stopMedia) {
      const stop = this.stopMedia;
      this.retryRetirement = stop;
      this.cleanup = stop().then(() => { if (this.retryRetirement === stop) this.retryRetirement = null; });
    }
    else {
      const state = this.state.playback;
      const session = state.session, resolution = state.resolution;
      Object.assign(state, createAppState().playback, { playback_epoch: state.playback_epoch + 1,
        volume: state.volume, muted: state.muted, speed: state.speed });
      this.cleanup = this.cleanup.then(async () => {
        if (session) await this.api.call("close_playback", { session_id: session.session_id });
        if (resolution) await this.api.call("release_resolution", { resolution_id: resolution.id });
      });
    }
    void this.cleanup.catch(error => this.diagnostics.push(failure(error)));
    if (this.state.route.name === "playback") this.state.route = this.state.details.data
      ? { name: "details", ref: clone(this.state.details.data.details.ref) } : { name: "home" };
  }
  async match(videoId: string, selection: PlaybackMatchSelection | null = null): Promise<PlaybackMatchResult | null> {
    const metadata = this.state.details.data?.metadata_ref, source = this.state.selected_playback_source;
    requireBinding(metadata && source && this.state.details.phase === "ready", "Match requires ready metadata details and playback source");
    requireBinding(this.state.details.data!.details.groups.some(group => group.episodes.some(episode => {
      return sameSource(episode.ref, metadata.content_ref) && metadataVideoId(episode.ref) === videoId;
    })), "Video is absent from current metadata details");
    requireBinding(!selection || sameSource(selection.content_ref, source), "Candidate belongs to another playback source");
    if (selection) requireBinding(this.confirmation?.status === "needs_confirmation"
      && sameMetadata(this.confirmation.metadata_ref, metadata) && this.confirmation.video_id === videoId
      && this.confirmation.candidates.some(candidate => sameRef(candidate.content_ref, selection.content_ref))
      && selection.group_id && selection.episode_id, "Candidate is absent from the current confirmation result");
    const selectedEpisode = selection && this.matching.details?.details.groups.find(group => group.id === selection.group_id)?.episodes.find(episode => episode.id === selection.episode_id);
    if (selection) requireBinding(this.matching.details && sameRef(this.matching.details.details.ref, selection.content_ref)
      && selectedEpisode && sameSource(selectedEpisode.ref, source), "Candidate episode is absent from the current details");
    this.invalidatePlayback();
    const target = this.state.playback;
    const ticket = this.ticket("match", source, target.playback_epoch);
    this.matching.pending = videoId;
    try {
      await this.cleanup;
      if (!this.current("match", ticket)) return null;
      const response = await this.api.call("match_playback", { metadata_ref: clone(metadata), video_id: videoId, playback_source: clone(source), selection }, ticket.id, ticket.host ?? undefined);
      if (!this.current("match", ticket) || target.playback_epoch !== this.state.playback.playback_epoch) return null;
      const result = response;
      requireBinding(["matched", "needs_confirmation", "not_found"].includes(result.status) && result.candidates.length <= 20
        && (result.status === "not_found" ? result.candidates.length === 0 : result.candidates.length > 0), "Match status/candidate count differs");
      requireBinding(sameMetadata(result.metadata_ref, metadata) && result.video_id === videoId
        && result.candidates.every(candidate => sameSource(candidate.content_ref, source)), "Match response metadata/video/provider differs");
      requireBinding((result.status === "matched") === (result.context !== null), "Match status/context differs");
      if (result.context) {
        const context = result.context;
        assertContext(this.state, context);
        requireBinding(context.metadata_video_id === videoId && sameMetadata(context.metadata_ref, metadata)
          && context.selected_stream_id === null && result.candidates.length === 1
          && sameRef(result.candidates[0]!.content_ref, context.content_ref), "Confirmed match/candidate binding differs");
        if (selection) requireBinding(sameRef(context.content_ref, selection.content_ref) && context.group_id === selection.group_id
          && context.episode_id === selection.episode_id, "Confirmed context differs from the selected candidate episode");
        if (selectedEpisode) requireBinding(sameRef(context.episode_ref, selectedEpisode.ref), "Confirmed episode reference differs");
        target.context = clone(context); target.engine_epoch = ticket.host;
        target.phase = "resolving";
        this.state.route = { name: "playback", context: target.context };
      } else { target.phase = "idle"; if (result.status === "needs_confirmation") this.confirmation = clone(result); }
      this.matching.result = clone(result);
      return result;
    } catch (error) { if (this.current("match", ticket)) { target.phase = "failed"; target.error = failure(error); } return null; }
    finally { if (this.requests.get("match") === ticket) { this.requests.delete("match"); this.matching.pending = null; } }
  }
  async loadCandidate(ref: Bridge_MediaRef) {
    requireBinding(this.confirmation?.status === "needs_confirmation" && this.confirmation.candidates.some(candidate => sameRef(candidate.content_ref, ref)), "Candidate is absent from the current confirmation result");
    const ticket = this.ticket("candidate", ref, this.state.playback.playback_epoch);
    this.matching.loading = true; this.matching.error = null; this.matching.details = null;
    try {
      const data = await this.api.call("get_details", { ref: clone(ref) }, ticket.id, ticket.host ?? undefined);
      if (!this.current("candidate", ticket)) return;
      requireBinding(sameRef(data.details.ref, ref) && data.metadata_ref === null
        && data.details.groups.every(group => group.episodes.every(episode => sameSource(episode.ref, ref))), "Candidate details/episode source binding differs");
      this.matching.details = data;
    } catch (cause) { if (this.current("candidate", ticket)) this.matching.error = failure(cause); }
    finally { if (this.requests.get("candidate") === ticket) { this.requests.delete("candidate"); this.matching.loading = false; } }
  }
}
export const appController = new AppController();
