import { reactive } from "vue";
import type { Bridge_ApiError, Command, CommandData, CommandInput, Settings, WindowBounds } from "./models.ts";
import { REQUEST_BUDGETS } from "./models.ts";
import { t } from "./i18n.ts";

export type Invoke = (command: string, args: { request_id: string; input: unknown }) => Promise<unknown>;
type Timer = { set: (callback: () => void, ms: number) => unknown; clear: (handle: unknown) => void };
const timers: Timer = {
  set: (callback, ms) => setTimeout(callback, ms),
  clear: handle => clearTimeout(handle as ReturnType<typeof setTimeout>),
};

// Access control and certificate failures name their cause and that requests stopped.
export function describeError(error: Bridge_ApiError): string {
  const details = error.details as Record<string, unknown> | undefined;
  if (details?.reason === "access_control") return details.signal === "challenge"
    ? t("This source asked for human verification, so requests to it stopped. Retry later or switch source.")
    : t("This source refused access (HTTP {status}), so requests to it stopped. Retry later or switch source.", { status: String(details.http_status) });
  if (details?.reason === "media_cache_io") return t("Media cache files could not be updated. Check disk space and permissions, then retry.");
  if (details?.reason === "tls_certificate") return t("This source's certificate could not be verified, so the connection was refused. Retry later or switch source.");
  return error.message;
}

export class ApiFailure extends Error {
  readonly error: Bridge_ApiError;
  constructor(error: Bridge_ApiError) { super(error.message); this.error = error; }
}
export class UnknownCommit extends Error {
  readonly request_id: string;
  constructor(request_id: string, message = "Transport result is unknown") { super(message); this.request_id = request_id; }
}

const commands: readonly Command[] = ["runtime_status", "runtime_prepare", "get_sources", "sync_catalog", "get_home",
  "get_details", "resolve_episode", "open_playback", "close_playback", "cancel_request", "start_interaction",
  "finish_interaction", "get_settings", "save_settings", "get_resource_status", "release_resolution", "match_playback",
  "list_history", "save_history", "remove_history", "clear_history", "get_media_cache", "clear_media_cache", "list_watchlist", "add_watchlist", "remove_watchlist", "search_catalog", "get_live_channels", "open_live", "list_local_videos", "open_local"];
const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const record = (value: unknown): value is Record<string, unknown> => typeof value === "object" && value !== null && !Array.isArray(value);

// No caller can select a process, path or arbitrary invoke command.
const nativeInvoke: Invoke = async (command, args) => {
  const tauri = (globalThis as typeof globalThis & { __TAURI__?: { core: { invoke: Invoke } } }).__TAURI__;
  if (!tauri) {
    if (typeof window === "undefined") throw new Error("Tauri host is unavailable");
    // The browser interface preview is explicit; native failures never fall back.
    return (await import("./data/preview.ts")).previewInvoke(command, args);
  }
  return tauri.core.invoke(command, args);
};

export type ApiFailureRecord = { command: Command; request_id: string; source_id: string | null; code: string; message: string };
export type SourceResult = { sequence: number; ok: boolean; message: string };
export class Api {
  // Recent failures with their correlation request IDs, for the diagnostics page.
  readonly failures = reactive<ApiFailureRecord[]>([]);
  // The latest started request's result per source; a slower older request cannot overwrite it.
  readonly sourceResults = reactive<Record<string, SourceResult>>({});
  private requestSequence = 0;
  private recordSourceResult(source: string | null, sequence: number, ok: boolean, message: string) {
    const known = source ? this.sourceResults[source] : undefined;
    if (source && (!known || known.sequence < sequence)) this.sourceResults[source] = { sequence, ok, message };
  }
  private sourceKinds = new Map<string, string>();
  private readonly invoke: Invoke;
  private readonly timer: Timer;
  constructor(invoke: Invoke = nativeInvoke, timer: Timer = timers) { this.invoke = invoke; this.timer = timer; }

  budget<C extends Command>(command: C, input: CommandInput<C>): number {
    const operations = REQUEST_BUDGETS.operations;
    if (command === "runtime_prepare") return operations.runtime_prepare.total_ms;
    if (command === "runtime_status" || command === "cancel_request") return operations.health.total_ms;
    if (command === "get_resource_status") return operations.get_resource_status.total_ms;
    if (command === "match_playback") return operations.match_playback.total_ms;
    if (command === "resolve_episode") return operations.resolve.total_ms;
    const data = input as Record<string, unknown>;
    if (command === "sync_catalog" && data.kind !== "cloudstream") return operations.http_source_sync.total_ms;
    if (command === "get_home") {
      const kind = this.sourceKinds.get(String(data.source_id));
      if (kind === "cloudstream") return data.page === 1 && data.section_ref == null ? operations.cloudstream_home.total_ms : operations.cloudstream_home_page.total_ms;
      if (data.page === 1 && data.section_ref == null && (kind === "maccms" || kind === "stremio")) return operations[`${kind}_home`].total_ms;
    }
    return operations.default_metadata.total_ms;
  }

  async call<C extends Command>(command: C, input: CommandInput<C>, requestId: string = crypto.randomUUID(), expectedHostEpoch?: string): Promise<CommandData<C>> {
    if (!commands.includes(command)) throw new Error("Unknown business command");
    if (!uuid.test(requestId)) throw new Error("request_id must be UUID");
    const target = input as { source_id?: unknown; ref?: { source_id?: unknown }; playback_source?: { source_id?: unknown }; config_id?: unknown };
    // A sync names its configuration; get_sources moves that result to the source it became.
    const owner = target.source_id ?? target.ref?.source_id ?? target.playback_source?.source_id ?? (command === "sync_catalog" ? target.config_id : undefined);
    const source = typeof owner === "string" ? owner : null, sequence = ++this.requestSequence;
    let handle: unknown;
    const timeout = new Promise<never>((_, reject) => {
      handle = this.timer.set(() => reject(new UnknownCommit(requestId, "Transport deadline exhausted")), this.budget(command, input) + REQUEST_BUDGETS.transport_grace_ms);
    });
    try {
      const response = await Promise.race([this.invoke(command, { request_id: requestId, input }), timeout]);
      if (!record(response) || response.request_id !== requestId || !(response.engine_epoch === null || (typeof response.engine_epoch === "string" && uuid.test(response.engine_epoch)))) {
        throw new UnknownCommit(requestId, "Invalid response identity");
      }
      if (expectedHostEpoch !== undefined && response.engine_epoch !== expectedHostEpoch) throw new UnknownCommit(requestId, "Host epoch changed; reconnect required");
      const success = Object.hasOwn(response, "data");
      if (success === Object.hasOwn(response, "error") || Object.keys(response).sort().join() !== (success ? "data,engine_epoch,request_id" : "engine_epoch,error,request_id")) {
        throw new UnknownCommit(requestId, "Invalid response envelope");
      }
      if (!success) {
        const error = response.error;
        if (!record(error) || typeof error.code !== "string" || typeof error.message !== "string" || typeof error.retryable !== "boolean") throw new UnknownCommit(requestId, "Invalid API error");
        const shown = { ...error, message: describeError(error as Bridge_ApiError) } as Bridge_ApiError;
        this.recordSourceResult(source, sequence, false, shown.message);
        this.failures.push({ command, request_id: requestId, source_id: source, code: shown.code, message: shown.message });
        if (this.failures.length > 20) this.failures.shift();
        throw new ApiFailure(shown);
      }
      if ((command === "runtime_status" || command === "runtime_prepare") && (!record(response.data) || typeof response.data.phase !== "string")) {
        throw new UnknownCommit(requestId, "Missing runtime status");
      }
      if ((command === "runtime_status" || command === "runtime_prepare") && record(response.data) && response.data.phase === "ready") {
        const health = response.data.health;
        if (!record(health) || health.status !== "ready" || health.media_gateway_ready !== true || typeof response.data.engine_epoch !== "string" || !uuid.test(response.data.engine_epoch) || response.data.engine_epoch !== response.engine_epoch) {
          throw new UnknownCommit(requestId, "Invalid ready status");
        }
      }
      if (!["get_settings", "save_settings", "cancel_request", "runtime_status", "runtime_prepare"].includes(command) && response.engine_epoch === null) throw new UnknownCommit(requestId, "Missing host epoch");
      if (command === "get_sources" && record(response.data) && Array.isArray(response.data.providers)) {
        this.sourceKinds.clear();
        for (const source of response.data.providers) if (record(source) && record(source.origin) && typeof source.id === "string" && typeof source.origin.config_id === "string") {
          const synced = this.sourceResults[source.origin.config_id];
          if (synced) { delete this.sourceResults[source.origin.config_id]; this.recordSourceResult(source.id, synced.sequence, synced.ok, synced.message); }
        }
        for (const source of response.data.providers) if (record(source) && typeof source.id === "string" && typeof source.kind === "string") this.sourceKinds.set(source.id, source.kind);
      }
      this.recordSourceResult(source, sequence, true, "");
      return response.data as CommandData<C>;
    } catch (error) {
      if (error instanceof ApiFailure) throw error;
      if (command !== "cancel_request" && command !== "save_settings") {
        // Best effort for reads; the settings queue requires a confirmed barrier.
        void this.cancel(requestId).catch(() => {});
      }
      throw error instanceof UnknownCommit ? error : new UnknownCommit(requestId);
    } finally { this.timer.clear(handle); }
  }

  cancel(target: string) { return this.call("cancel_request", { target_request_id: target }); }
}

type Change = (confirmed: Settings) => Settings;
type Pending = { change: Change; resolve: (value: Settings) => void; reject: (error: unknown) => void };

// A single writer owns every full-object settings commit. Actions receive a copy
// only when they reach the head, so queued edits retain previous confirmations.
export class SettingsQueue {
  private confirmed: Settings | null = null;
  private pending: Pending[] = [];
  private running: Promise<void> | null = null;
  private unknown: string | null = null;
  private accepting = true;
  private recovery: Promise<void> | null = null;
  private initialization: Promise<Settings> | null = null;
  private readonly api: Api;
  private readonly onError: (error: unknown) => void;
  constructor(api: Api, onError: (error: unknown) => void = () => {}) { this.api = api; this.onError = onError; }

  get value(): Settings | null { return this.confirmed && structuredClone(this.confirmed); }
  get paused(): boolean { return this.unknown !== null; }

  async initialize(): Promise<Settings> {
    if (this.confirmed) return this.value!;
    if (!this.initialization) this.initialization = this.api.call("get_settings", {}).then(value => {
      this.confirmed = value;
      return this.value!;
    }).finally(() => { this.initialization = null; });
    return this.initialization;
  }

  update(change: Change): Promise<Settings> {
    if (!this.accepting) return Promise.reject(new Error("Settings queue is closing"));
    const result = new Promise<Settings>((resolve, reject) => this.pending.push({ change, resolve, reject }));
    this.pump();
    return result;
  }

  private pump() {
    if (this.running || this.paused || !this.pending.length) return;
    this.running = this.drain().finally(() => { this.running = null; this.pump(); });
  }

  private async drain() {
    while (this.pending.length && !this.paused) {
      const item = this.pending.shift()!;
      try {
        await this.initialize();
        const next = item.change(this.value!);
        this.confirmed = await this.api.call("save_settings", next);
        item.resolve(this.value!);
      } catch (error) {
        item.reject(error);
        this.onError(error);
        if (error instanceof UnknownCommit) {
          this.unknown = error.request_id;
          try { await this.recover(); } catch { /* Keep paused until explicit retry. */ }
        }
      }
    }
  }

  async recover(): Promise<void> {
    if (this.recovery) return this.recovery;
    if (!this.unknown) return;
    const target = this.unknown;
    this.recovery = (async () => {
      try {
        await this.api.cancel(target);
        // get_settings shares the Rust commit lock: an in-progress replacement
        // finishes before this read; cancelled queued writers cannot commit later.
        this.confirmed = await this.api.call("get_settings", {});
        this.unknown = null;
      } catch (error) { this.onError(error); throw error; }
    })().finally(() => { this.recovery = null; this.pump(); });
    return this.recovery;
  }

  // A close that could not finish leaves the app open, so it can take changes again.
  reopen() { this.accepting = true; }

  async close(): Promise<void> {
    this.accepting = false;
    while (this.running) await this.running;
    if (this.paused) throw new Error("Settings commit is unresolved; retry recovery before exit");
  }
}

// Measured natively, the same way the Host restores the window; null when unsupported or outside the app.
export async function nativeWindowBounds(): Promise<WindowBounds | null> {
  const core = (globalThis as typeof globalThis & { __TAURI__?: { core: { invoke: Invoke } } }).__TAURI__?.core;
  return core ? await core.invoke("window_bounds_now", {} as never) as WindowBounds | null : null;
}

// History problems keep their own notice and never hold the window. Settings must land first;
// if they cannot, the window stays open with the error, and closing it again exits.
export async function finishBeforeClose(steps: { saveHistory: () => Promise<unknown>; saveSettings: () => Promise<unknown>;
  settings: Pick<SettingsQueue, "close" | "reopen">; release: () => Promise<unknown>; abort: () => Promise<unknown> }) {
  await steps.saveHistory().catch(() => {});
  try { await steps.saveSettings(); await steps.settings.close(); }
  catch (error) { steps.settings.reopen(); await steps.abort().catch(() => {}); throw error; }
  await steps.release();
}

export const api = new Api();
export const settingsQueue = new SettingsQueue(api);
