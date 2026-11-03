// Test-mode only. Runs the production App/PlaybackView/state/Api against a real Host.
import { createApp, h, nextTick, ref, type App as VueApp } from "vue";
import App from "../../src/App.vue";
import UiProvider from "../../src/ui/UiProvider.vue";
import { appState, navigatePlayback, type PlaybackController } from "../../src/state.ts";
import { nativeInvoke, type Player } from "../../src/player.ts";
import { locale } from "../../src/i18n";
import { api } from "../../src/api.ts";
import type { PlaybackContext, PlaybackMatchInput, PlaybackViewSession, DetailsViewData, AppSourceList } from "../../src/models.ts";

type Command = { action?: string; payload?: unknown };
type Bootstrap = { initial_context: PlaybackContext; match_inputs: PlaybackMatchInput[]; details: DetailsViewData; sources: AppSourceList };
type Harness = {
  app: VueApp; container: HTMLElement; video: HTMLVideoElement; controller: PlaybackController; matches: PlaybackMatchInput[];
  rpc: (value: unknown) => Promise<any>; audio: AudioContext; source: MediaElementAudioSourceNode; analyser: AnalyserNode;
  sampling: ReturnType<typeof createMediaSampler>; retiredTracks: WeakRef<HTMLTrackElement>[];
  frames: number; callback: number; originalCore: NonNullable<Window["__TAURI__"]>["core"];
  errors: string[]; stopErrors: () => void;
  initialLocale: typeof locale.value; details: DetailsViewData;
};
const diagnostic = (value: unknown) => String(value).replace(/([?&](?:cap|token|capability)=)[^\s"'<>]*/gi, "$1<redacted>");
let harness: Harness | undefined;
const delay = (ms: number) => new Promise<void>(resolve => setTimeout(resolve, ms));
function check(value: unknown, message: string): asserts value { if (!value) throw new Error(message); }
async function until(predicate: () => boolean, message: string, timeout = 20000) {
  const end = performance.now() + timeout;
  while (!predicate()) { check(performance.now() < end, message); await delay(20); }
}
// Keep observer scratch storage bounded independently of readiness polling.
// The previous 50-switch report contains 437 reads, each allocating 24 KiB
// plus a canvas. Raw numeric observations still leave the WebView in full.
export function createMediaSampler(video: HTMLVideoElement, analyser: AnalyserNode, sampleRate: number) {
  const bins = new Float32Array(analyser.fftSize);
  const spectrum = new Float32Array(analyser.frequencyBinCount);
  const canvas = document.createElement("canvas"); canvas.width = canvas.height = 1;
  const drawing = canvas.getContext("2d");
  check(drawing, "Media observation requires a canvas 2D context");
  let reads = 0;
  return {
    read() {
      reads++;
      analyser.getFloatTimeDomainData(bins);
      analyser.getFloatFrequencyData(spectrum);
      let peak = 0;
      for (let i = 1; i < spectrum.length; i++) if (spectrum[i] > spectrum[peak]) peak = i;
      drawing.clearRect(0, 0, 1, 1);
      if (video.readyState >= 2) drawing.drawImage(video, 0, 0, 1, 1);
      return { audio_rms: Math.sqrt(bins.reduce((sum, v) => sum + v * v, 0) / bins.length),
        audio_peak_hz: peak * sampleRate / analyser.fftSize, pixel: [...drawing.getImageData(0, 0, 1, 1).data] };
    },
    diagnostics: () => ({ reads, scratch_buffers: 2, scratch_bytes: bins.byteLength + spectrum.byteLength, scratch_canvases: 1 }),
    dispose() { canvas.width = canvas.height = 0; },
  };
}
function sample(current: Harness) {
  const video = current.video;
  const media = current.sampling.read();
  const retained = current.retiredTracks.flatMap(reference => { const track = reference.deref(); return track ? [track] : []; });
  return { position: video.currentTime, paused: video.paused, ready_state: video.readyState, network_state: video.networkState,
    sampled_at: performance.now(), audio_state: current.audio.state, audio_time: current.audio.currentTime,
    session_bound: Boolean(appState.playback.session && video.currentSrc === appState.playback.session.media_url),
    source_present: video.hasAttribute("src"), tracks: video.querySelectorAll("track").length,
    video_count: document.querySelectorAll("video").length, frames: current.frames, width: video.videoWidth, height: video.videoHeight,
    ...media, media_error: video.error?.code ?? null,
    resources: { dom_nodes: document.querySelectorAll("*").length, text_tracks: video.textTracks.length,
      buffered_ranges: video.buffered.length, resource_timing_entries: performance.getEntriesByType("resource").length,
      retired_tracks_observed: current.retiredTracks.length, retired_tracks_alive: retained.length,
      retired_tracks_connected: retained.filter(track => track.isConnected).length,
      retired_tracks_enabled: retained.filter(track => track.track.mode !== "disabled").length,
      observer: current.sampling.diagnostics() },
    phase: appState.playback.phase, epoch: appState.playback.playback_epoch, diagnostics: current.controller.diagnostics() };
}

// Video presentation and WebAudio render quanta become ready independently after
// src changes. A silent PCM block with a smoothed old FFT peak proves neither.
export async function waitForEpisodeSample(read: () => ReturnType<typeof sample>, index: number, deadline: number) {
  const observations = [];
  while (true) {
    const value = read();
    observations.push(value);
    const [red, green, blue, alpha] = value.pixel;
    const isRed = index % 2 === 0 || index === 51;
    const frame = alpha === 255 && green < 50 && (isRed ? red > 180 && blue < 50 : blue > 180 && red < 50);
    const audio = value.audio_state === "running" && value.audio_rms > .001 && Math.abs(value.audio_peak_hz - (isRed ? 440 : 880)) < 40;
    if (performance.now() >= deadline) throw new Error(`Current episode media did not become ready: ${JSON.stringify({ index, observations })}`);
    if (value.session_bound && frame && (index === 51 || audio)) return { sample: value, observations };
    await delay(20);
  }
}
async function gesture(current: Harness) {
  const button = current.container.querySelector<HTMLButtonElement>('[data-action="continue"]') ?? current.container.querySelector<HTMLButtonElement>('[data-action="play"]');
  check(button && !button.disabled, "Actual playback button is missing/disabled");
  button.click();
  await current.audio.resume();
}
async function playing(current: Harness) {
  await until(() => current.video.readyState >= 2 || Boolean(current.video.error), "Formal media metadata missing");
  check(!current.video.error, "Formal video failed before playback");
  if (current.video.paused) await gesture(current);
  await until(() => !current.video.paused && current.video.currentTime > .1, "Formal playback did not advance");
  await current.audio.resume();
}

export function requireCodecMedia(session: PlaybackViewSession | null, currentSrc: string) {
  check(session?.stream_kind === "progressive", "Codec stimulus requires a progressive MP4 session; an invalid HLS manifest fails at the gateway");
  check(currentSrc === session.media_url, "Codec stimulus must use the current session media URL");
}

export async function formalPlayback(command: Command, prototype: { video: HTMLVideoElement; player: Player; base: string; cap: string }) {
  const started = performance.now();
  if (command.action === "start") {
    check(!harness, "Formal App already mounted");
    const payload = command.payload as Bootstrap;
    check(payload.match_inputs.length === 52 && payload.initial_context.match_id && payload.details.metadata_ref, "Actual Host context/DTO inventory missing");
    prototype.player.stop();
    const initialLocale = locale.value;
    locale.value = "en";
    const container = document.getElementById("app") as HTMLElement & { __vue_app__?: VueApp };
    check(container.__vue_app__, "Original native fixture root is missing");
    container.__vue_app__.unmount();
    const rpc = async (value: unknown) => {
      const response = await fetch(`${prototype.base}control/invoke?cap=${prototype.cap}`, {
        method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(value),
      });
      check(response.ok, "Real Host test transport failed");
      return response.json();
    };
    check(window.__TAURI__, "Native Tauri invoke is required");
    const originalCore = window.__TAURI__.core;
    // The fixture changes only transport; every envelope and DTO is generated by Host.
    window.__TAURI__.core = { ...originalCore, invoke: <T>(name: string, args?: Record<string, unknown>): Promise<T> =>
      name === "t01_control" || name === "player_fullscreen" ? originalCore.invoke<T>(name, args) : rpc({ command: name, args }) };
    const errors: string[] = [];
    const onError = (event: ErrorEvent) => errors.push(diagnostic(event.message));
    const onRejection = (event: PromiseRejectionEvent) => errors.push(diagnostic(event.reason));
    const oldError = console.error, oldWarn = console.warn;
    console.error = (...args) => { errors.push(`console.error: ${args.map(diagnostic).join(" ")}`); oldError(...args); };
    console.warn = (...args) => { errors.push(`console.warn: ${args.map(diagnostic).join(" ")}`); oldWarn(...args); };
    window.addEventListener("error", onError); window.addEventListener("unhandledrejection", onRejection);
    // Publish the untouched Host DTO only after LRU eviction in image-expire.
    // Otherwise the already decoded poster can satisfy a same-src assignment
    // from its legitimate 60s cache without any native network error.
    appState.details.data = null; appState.details.request_epoch++;
    appState.sources.data = payload.sources;
    navigatePlayback(payload.initial_context);
    const component = ref<InstanceType<typeof App>>();
    const app = createApp({ render: () => h(UiProvider, null, { default: () => h(App, { ref: component }) }) });
    app.mount(container); await nextTick();
    const controller = component.value?.playbackController();
    check(controller, "Actual App/PlaybackView public controller missing");
    const video = container.querySelector("video")!;
    check(video && document.querySelectorAll("video").length === 1, "Formal App must own exactly one video");
    const audio = new AudioContext(); const source = audio.createMediaElementSource(video); const analyser = audio.createAnalyser();
    analyser.fftSize = 4096; source.connect(analyser); analyser.connect(audio.destination);
    const current: Harness = { app, container, video, controller, matches: payload.match_inputs, rpc, audio, source, analyser,
      sampling: createMediaSampler(video, analyser, audio.sampleRate), retiredTracks: [],
      frames: 0, callback: 0, originalCore, initialLocale, details: payload.details, errors, stopErrors: () => {
        window.removeEventListener("error", onError); window.removeEventListener("unhandledrejection", onRejection);
        console.error = oldError; console.warn = oldWarn;
      } };
    harness = current;
    const onFrame: VideoFrameRequestCallback = () => { current.frames++; current.callback = video.requestVideoFrameCallback(onFrame); };
    current.callback = video.requestVideoFrameCallback(onFrame);
    await until(() => Boolean(appState.playback.session) || appState.playback.phase === "failed", "Formal session was not opened");
    await playing(current);
    current.controller.selectSubtitle(appState.playback.session!.subtitles[0]!.id);
    await until(() => video.querySelector("track")!.readyState === 2, "Formal native subtitle did not load");
  }
  const current = harness;
  check(current, "Formal App must be started first");
  const video = current.video;
  let result: Record<string, unknown> = {};
  switch (command.action) {
    case "start": result = { title: document.title, origin: location.origin, nonblank: Boolean(current.container.textContent?.includes("Now playing")),
      framework_overlay: Boolean(document.querySelector("vite-error-overlay")) }; break;
    case "switch": {
      const index = Number(command.payload);
      check(index >= 1 && index <= 51, "Episode index outside fixture");
      const oldTracks = [...video.querySelectorAll("track")]; const oldSession = appState.playback.session!.session_id;
      current.retiredTracks.push(...oldTracks.map(track => new WeakRef(track)));
      const before = current.frames;
      // The production state action and App route feed the same mounted PlaybackView.
      const matched = await api.call("match_playback", current.matches[index]!);
      check(matched.status === "matched" && matched.context, "Next episode must be matched again before playing");
      navigatePlayback(matched.context); await nextTick();
      await until(() => appState.playback.session?.session_id !== oldSession && Boolean(appState.playback.session), "New episode session not opened");
      await playing(current);
      current.controller.selectSubtitle(appState.playback.session!.subtitles[0]!.id);
      await until(() => video.querySelector("track")?.readyState === 2, "New episode subtitle failed");
      await until(() => current.frames > before + 1, "New episode did not present frames");
      const decoded = await waitForEpisodeSample(() => sample(current), index, started + 30000);
      result = { index, old_tracks_disabled: oldTracks.every(t => !t.isConnected && t.track.mode === "disabled"),
        session_changed: appState.playback.session!.session_id !== oldSession, new_frames: current.frames - before,
        actual_gateway: video.currentSrc === appState.playback.session!.media_url,
        sample: decoded.sample, media_observations: decoded.observations,
        host: await current.rpc({ operation: "observe" }) };
      break;
    }
    case "controls": {
      current.container.querySelector<HTMLButtonElement>('[data-action="play"]')!.click();
      await until(() => video.paused, "Formal pause control failed");
      const position = video.currentTime; await delay(300); check(Math.abs(video.currentTime - position) < .05, "Paused clock advanced");
      const seek = current.container.querySelector<HTMLInputElement>('input[type="range"]')!;
      seek.value = ".5"; seek.dispatchEvent(new Event("input", { bubbles: true }));
      await until(() => !video.seeking && Math.abs(video.currentTime - .5) < .05, "Formal seek control failed");
      const res = appState.playback.resolution!.id; const session = appState.playback.session!.session_id;
      await current.controller.selectStream(appState.playback.resolution!.streams[1]!.id);
      await until(() => video.readyState >= 2, "Alternate line metadata missing");
      check(video.paused && Math.abs(video.currentTime - .5) < .05, "Line switch lost paused resume snapshot");
      result = { paused: video.paused, seek: video.currentTime, resolution_retained: res === appState.playback.resolution!.id,
        session_changed: session !== appState.playback.session!.session_id };
      break;
    }
    case "expire": {
      const old = appState.playback.session!.session_id;
      const position = video.currentTime;
      await current.rpc({ operation: "expire-session", session_id: old });
      video.load();
      await until(() => appState.playback.session?.session_id !== old && Boolean(appState.playback.session), "Expired session did not recover");
      await until(() => video.readyState >= 2 && !video.seeking, "Recovered metadata/seek missing");
      result = { session_changed: old !== appState.playback.session!.session_id, restored: Math.abs(video.currentTime - position) < .1, paused: video.paused };
      // Automatic recovery is bounded for the same user action.
      const recovered = appState.playback.session!.session_id;
      await current.rpc({ operation: "expire-session", session_id: recovered }); video.load();
      await until(() => appState.playback.error?.code === "SESSION_EXPIRED", "Second expiry must expose retry");
      result.recovered_once = appState.playback.session?.session_id === recovered;
      await nextTick();
      const retry = current.container.querySelector<HTMLButtonElement>('[data-action="retry"]');
      check(retry && !retry.disabled, "Expired session must expose its actual Retry button");
      retry.click();
      await until(() => Boolean(appState.playback.session) && appState.playback.session!.session_id !== recovered, "Manual expiry retry did not open a new session");
      await playing(current);
      break;
    }
    case "image-expire": {
      const deadline = started + 30000;
      const id = current.details.details.poster?.resource_id;
      check(id && current.details.images[id], "Actual Host poster access is missing");
      const access = current.details.images[id]!;
      const host = appState.runtime.engine_epoch;
      const pageEpoch = appState.details.request_epoch;
      const events: Record<string, unknown>[] = [], calls: Record<string, unknown>[] = [], stages: string[] = [];
      const evidence = { resource_id: id, url: diagnostic(access.url), page_epoch: pageEpoch, expected_host_epoch: host, events, calls, stages,
        expired_http_status: null as number | null, before: null as any, after: null as any };
      let dto: DetailsViewData | null = null;
      const core = window.__TAURI__!.core;
      const observe = (event: Event) => {
        const image = event.target;
        if (!(image instanceof HTMLImageElement) || !image.matches(".playback-title img")) return;
        const owner = appState.details.data;
        const ownerId = owner?.details.poster?.resource_id;
        events.push({ type: event.type, trusted: event.isTrusted, resource_id: new URL(image.currentSrc || image.src).pathname.split("/").at(-1),
          original_url: image.src === access.url, owning_url: Boolean(ownerId && image.src === owner?.images[ownerId]?.url), original_dto: owner === dto,
          page_epoch: appState.details.request_epoch, natural_width: image.naturalWidth, at_ms: performance.now() - started });
      };
      const abort = new AbortController();
      const timer = setTimeout(() => abort.abort(), Math.max(0, deadline - performance.now()));
      current.container.addEventListener("load", observe, true);
      current.container.addEventListener("error", observe, true);
      // Observe the real transport; every request still reaches the real Host.
      window.__TAURI__!.core = { ...core, invoke: async <T>(name: string, args?: Record<string, unknown>): Promise<T> => {
        const input = args?.input as any;
        const row = name === "get_resource_status" || name === "get_details" ? {
          command: name, request_id: args?.request_id, kind: input?.kind, target_id: input?.target_id,
          ref: input?.ref, response_request_id: null as unknown, engine_epoch: null as unknown,
          state: null as unknown, code: null as unknown, response_ref: null as unknown,
          response_kind: null as unknown, response_target_id: null as unknown,
          started_ms: performance.now() - started, finished_ms: null as number | null,
        } : null;
        if (row) calls.push(row);
        const response = await core.invoke<any>(name, args);
        if (row) Object.assign(row, { response_request_id: response.request_id, engine_epoch: response.engine_epoch,
          state: response.data?.state ?? null, code: response.data?.error?.code ?? response.error?.code ?? null,
          response_kind: response.data?.kind ?? null, response_target_id: response.data?.target_id ?? null,
          response_ref: response.data?.details?.ref ?? null, finished_ms: performance.now() - started });
        return response;
      } };
      try {
        check(appState.details.data === null && !current.container.querySelector(".playback-title img"), "Poster was already published/cached by the formal view");
        evidence.before = await current.rpc({ operation: "observe" }); stages.push("before-eviction");
        await current.rpc({ operation: "evict-images" }); stages.push("4096-lru-evicted");
        const response = await fetch(access.url, { cache: "no-store", signal: abort.signal });
        evidence.expired_http_status = response.status;
        await response.arrayBuffer(); stages.push("expired-http-read");
        check(response.status === 410, "Actual expired poster HTTP must be 410");
        appState.details.data = current.details; dto = appState.details.data;
        stages.push("host-dto-published"); await nextTick();
        await until(() => appState.details.data !== dto || Boolean(current.controller.ui.imageError), "Image error did not query/refresh its DTO", Math.min(20000, deadline - performance.now()));
        stages.push("dto-recovery-finished"); await nextTick();
        await until(() => events.some(event => event.type === "load" && event.trusted === true && event.original_dto === false), "Refreshed image did not emit native load", Math.min(20000, deadline - performance.now()));
        const image = current.container.querySelector<HTMLImageElement>(".playback-title img");
        check(image, "Refreshed image element is missing"); await image.decode();
        stages.push("refreshed-image-decoded");
        evidence.after = await current.rpc({ operation: "observe" });
        check(performance.now() < deadline, "Image recovery exceeded its original 30s deadline");
        result = { ...evidence, dto_refreshed: appState.details.data !== dto, decoded: image.naturalWidth > 0,
          owning_ref: dto!.details.ref, refreshed_ref: appState.details.data!.details.ref,
          refreshed_resource_id: appState.details.data!.details.poster?.resource_id,
          page_epoch_after: appState.details.request_epoch, host_epoch_after: appState.runtime.engine_epoch,
          image_error: current.controller.ui.imageError };
      } catch (cause) {
        // Keep all completed stages and native events even when a later gate fails.
        throw new Error(`Image expiry chain failed: ${diagnostic(cause)}; evidence=${JSON.stringify(evidence)}`);
      } finally {
        clearTimeout(timer); abort.abort(); window.__TAURI__!.core = core;
        current.container.removeEventListener("load", observe, true);
        current.container.removeEventListener("error", observe, true);
      }
      break;
    }
    case "errors": {
      const outcomes: Record<string, any>[] = [], calls: Record<string, any>[] = [], events: Record<string, any>[] = [];
      let mode = 0;
      const snapshot = () => {
        const state = appState.playback, session = state.session;
        return { epoch: state.playback_epoch, host_epoch: appState.runtime.engine_epoch, session_id: session?.session_id ?? null,
          stream_kind: session?.stream_kind ?? null, source_id: session?.source_id ?? null, source_revision: session?.source_revision ?? null,
          episode_ref: session?.episode_ref ?? null, session_path: session ? new URL(session.media_url).pathname : null,
          current_path: video.currentSrc ? new URL(video.currentSrc).pathname : null,
          session_bound: Boolean(session && session.media_url === video.currentSrc), assigned_bound: Boolean(session && session.media_url === video.src), phase: state.phase,
          code: state.error?.code ?? null, error: state.error ? JSON.parse(diagnostic(JSON.stringify(state.error))) : null,
          media_error: video.error ? { code: video.error.code, message: diagnostic(video.error.message) } : null,
          pending_requests: current.controller.diagnostics().pending_requests };
      };
      const observe = (event: Event) => {
        if (event.target !== video && !(event.target instanceof HTMLTrackElement)) return;
        events.push({ mode, trusted: event.isTrusted, target: event.target === video ? "video" : "track",
          at_ms: performance.now() - started, ...snapshot() });
      };
      const core = window.__TAURI__!.core;
      window.__TAURI__!.core = { ...core, invoke: async <T>(name: string, args?: Record<string, unknown>): Promise<T> => {
        const input = args?.input as any;
        const row = name === "get_resource_status" ? { mode, command: name, request_id: args?.request_id,
          kind: input?.kind, target_id: input?.target_id, before: snapshot(), started_ms: performance.now() - started,
          response: null as any, finished_ms: null as number | null } : null;
        if (row) calls.push(row);
        try {
          const response = await core.invoke<any>(name, args);
          if (row) Object.assign(row, { response, after: snapshot(), finished_ms: performance.now() - started });
          return response;
        } catch (cause) {
          if (row) Object.assign(row, { transport_error: diagnostic(cause), finished_ms: performance.now() - started });
          throw cause;
        }
      } };
      current.container.addEventListener("error", observe, true);
      try {
        // The preceding native-HLS case remains intact. Corrupt MP4 bytes must
        // reach a progressive decoder, rather than HLS syntax validation.
        await current.rpc({ operation: "mode", value: 0 });
        const matched = await api.call("match_playback", current.matches[50]!);
        check(matched.status === "matched" && matched.context, "Codec episode match is missing");
        navigatePlayback(matched.context); await nextTick(); await current.controller.settled(); await playing(current);
        requireCodecMedia(appState.playback.session, video.currentSrc);
        for (mode of [1, 2, 3]) {
          const row: Record<string, any> = { mode, before: snapshot(), stages: [], call_start: calls.length, event_start: events.length };
          outcomes.push(row); // Preserve the failing mode before any assertion.
          const stimulus = await current.rpc({ operation: "mode", value: mode });
          row.stages.push("stimulus-set");
          await current.controller.retry(); row.stages.push("retry-opened"); row.opened = snapshot();
          await until(() => appState.playback.phase === "failed" && Boolean(appState.playback.error), "Native resource failure not mapped");
          await until(() => current.controller.diagnostics().pending_requests === 0, "Native error query still pending");
          row.final = snapshot(); row.code = row.final.code; row.media_error = row.final.media_error?.code ?? null;
          row.host = await current.rpc({ operation: "observe" });
          row.requests = row.host.error_requests.slice(stimulus.request_start);
          row.stages.push("diagnosis-finished");
          check(row.code === (mode === 3 ? "MEDIA_UNSUPPORTED" : "UPSTREAM_FAILED"),
            `Native classification differs: mode=${mode}, actual=${row.code}, media=${row.media_error}`);
        }
        mode = 0;
        await current.rpc({ operation: "mode", value: 0 }); await current.controller.retry(); await playing(current);
        mode = 4;
        const track: Record<string, any> = { mode, before: snapshot(), call_start: calls.length, event_start: events.length };
        outcomes.push(track);
        const stimulus = await current.rpc({ operation: "mode", value: 4 });
        current.controller.selectSubtitle(appState.playback.session!.subtitles[0]!.id);
        await until(() => current.container.querySelector("track")?.readyState === 3, "Native track error not raised");
        await until(() => appState.playback.phase === "failed" && current.controller.diagnostics().pending_requests === 0, "Native track status query did not finish");
        track.final = snapshot(); track.host = await current.rpc({ operation: "observe" });
        track.requests = track.host.error_requests.slice(stimulus.request_start);
        check(appState.playback.error?.code === "UPSTREAM_FAILED", "Native track error lost gateway diagnosis");
        const trackError = appState.playback.error.code;
        mode = 0;
        await current.rpc({ operation: "mode", value: 0 }); await current.controller.retry(); await playing(current);
        const beforePrepare = (await current.rpc({ operation: "observe" })).command_counts.runtime_prepare ?? 0;
        const beforeEpoch = appState.playback.playback_epoch;
        const reconnect: Record<string, any> = { mode: "unauthorized-query", before: snapshot(), call_start: calls.length, event_start: events.length };
        outcomes.push(reconnect);
        await current.rpc({ operation: "unauthorized-query" }); mode = 2;
        await current.rpc({ operation: "mode", value: 2 }); await current.controller.retry();
        await until(() => appState.playback.playback_epoch >= beforeEpoch + 2 && appState.playback.phase === "failed" && current.controller.diagnostics().pending_requests === 0, "Unauthorized query did not reconnect and finish its bounded attempt");
        reconnect.final = snapshot(); reconnect.host = await current.rpc({ operation: "observe" });
        const reconnects = (reconnect.host.command_counts.runtime_prepare ?? 0) - beforePrepare;
        check(reconnects === 1, "Unauthorized must reconnect exactly once");
        mode = 0;
        await current.rpc({ operation: "mode", value: 0 }); await current.controller.retry(); await playing(current);
        result = { outcomes, calls, events, track_error: trackError, reconnects };
      } catch (cause) {
        throw new Error(`Native errors chain failed: ${diagnostic(cause)}; evidence=${diagnostic(JSON.stringify({ mode, outcomes, calls, events, current: snapshot() }))}`);
      } finally {
        window.__TAURI__!.core = core;
        current.container.removeEventListener("error", observe, true);
      }
      break;
    }
    case "eos": {
      const events: Event[] = [];
      const onEnded = (event: Event) => events.push(event);
      video.addEventListener("ended", onEnded, { once: true });
      current.controller.seek(video.duration - .25); await gesture(current);
      await until(() => appState.playback.phase === "ended", "Formal EOS event missing");
      video.removeEventListener("ended", onEnded);
      result = { ended: true, dom_events: events.length };
      break;
    }
    case "autoplay": {
      current.controller.pause();
      const original = video.play;
      try {
        video.play = () => Promise.reject(new DOMException("Injected system autoplay denial", "NotAllowedError"));
        await current.controller.play(); await nextTick();
        check(Boolean(current.container.querySelector('[data-action="continue"]')), "Continue button did not render for autoplay denial");
      } finally { video.play = original; }
      await gesture(current); await playing(current); await nextTick();
      result = { continue_button: true, resumed: !current.container.querySelector('[data-action="continue"]'), stimulus: "injected NotAllowedError; actual component/button and native playback" };
      break;
    }
    case "unknown-duration": {
      const descriptor = Object.getOwnPropertyDescriptor(video, "duration");
      try {
        Object.defineProperty(video, "duration", { configurable: true, get: () => Infinity });
        video.dispatchEvent(new Event("durationchange")); await nextTick();
        const seek = current.container.querySelector<HTMLInputElement>('input[type="range"]')!;
        result = { disabled: seek.disabled, unknown_text: current.container.textContent!.includes("Unknown duration"), stimulus: "injected nonfinite DOM duration; actual rendered state" };
      } finally {
        if (descriptor) Object.defineProperty(video, "duration", descriptor); else Reflect.deleteProperty(video, "duration");
        video.dispatchEvent(new Event("durationchange")); await nextTick();
      }
      break;
    }
    case "layout": {
      const width = Number(command.payload);
      await nativeInvoke("t01_control", { action: "width", value: width }); await delay(200);
      const controls = [...current.container.querySelectorAll<HTMLElement>("button,input,select")];
      const rects = controls.filter(el => el.getClientRects().length).map(el => el.getBoundingClientRect());
      check(document.documentElement.scrollWidth <= innerWidth + 1 && rects.every(r => r.width > 0 && r.left >= 0 && r.right <= innerWidth + 1), "Formal controls overflow or clip");
      result = { width: innerWidth, controls: rects.length, no_overflow: true };
      break;
    }
    case "stop": {
      await current.controller.stop(); await delay(300); const before = current.frames; await delay(500);
      const value = sample(current);
      check(video.paused && !video.hasAttribute("src") && video.readyState === 0 && video.networkState === 0 && video.buffered.length === 0 && video.videoWidth === 0 && video.querySelectorAll("track").length === 0, "Formal media not released");
      check(current.frames <= before + 1 && value.audio_rms < .001, "Old frames/audio remain after stop");
      check(!value.diagnostics.listening && value.diagnostics.hls_instances === 0 && value.diagnostics.retiring === 0, "Formal controller retains transport/listeners/handles");
      result = { source_removed: true, frames_after_stop: current.frames - before, host: await current.rpc({ operation: "observe" }) };
      break;
    }
    case "dispose": {
      await current.controller.stop(); current.app.unmount(); await current.controller.settled();
      video.cancelVideoFrameCallback(current.callback); current.source.disconnect(); current.analyser.disconnect(); await current.audio.close();
      current.sampling.dispose();
      current.container.remove(); current.stopErrors();
      window.__TAURI__!.core = current.originalCore;
      locale.value = current.initialLocale;
      result = { audio_closed: current.audio.state === "closed", errors: current.errors, videos: document.querySelectorAll("video").length };
      harness = undefined;
      return { ...result, native: await nativeInvoke("t01_control", { action: "identity" }) };
    }
    default: throw new Error(`Unknown formal action ${command.action}`);
  }
  return { ...result, sample: result.sample ?? sample(current), errors: [...current.errors], ui_ms: performance.now() - started,
    native: await nativeInvoke("t01_control", { action: "identity" }) };
}
