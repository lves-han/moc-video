import { reactive, toRaw } from "vue";
import { ApiFailure, type AppApi, type ApiResponse, SettingsQueue } from "./api.ts";
import type {
  AppState, AppSource, Bridge_ApiError, Bridge_HomeRequest, Bridge_MediaRef,
  Bridge_Resolution, DetailsViewData, HomeViewData, MetadataRef, PlaybackContext,
  PlaybackMatchResult, PlaybackMatchSelection, PlaybackViewSession, PlayerEvent,
  Route, RuntimeStatus, Settings, SourceKey,
} from "./models.ts";

export const defaultSettings = (): Settings => ({ schema_version: 2, last_source_key: null,
  last_metadata_config_id: null, last_playback_source_key: null, ui_locale: "en", color_palette: "lime",
  volume: 0.65, muted: false, auto_next: true, window_bounds: null, source_configs: [] });
const page = () => ({ phase: "idle" as const, request_epoch: 0, request_id: null, data: null, error: null });
export function createAppState(settings = defaultSettings()): AppState {
  return { route: { name: "home" }, selected_metadata_source: null, selected_playback_source: null,
    sources: page(), home: page(), details: page(), settings,
    runtime: { platform: "macos", phase: "unavailable", engine_epoch: null, health: null, error: null,
      cloudstream: { phase: "stopped", engine_epoch: null, display_mode: "none", health: null, error: null } },
    playback: { phase: "idle", playback_epoch: 0, engine_epoch: null, context: null, resolution: null,
      session: null, position_ms: 0, duration_ms: null, buffered: [], volume: settings.volume,
      muted: settings.muted, speed: 1, subtitle_id: null, error: null } };
}
const clone = <T>(value: T): T => structuredClone(toRaw(value));
export const sameSource = (a: SourceKey | Bridge_MediaRef | null, b: SourceKey | Bridge_MediaRef | null) =>
  a !== null && b !== null && a.source_id === b.source_id && a.source_revision === b.source_revision;
const sameRef = (a: Bridge_MediaRef, b: Bridge_MediaRef) => sameSource(a, b) && a.data === b.data;
const sameMetadata = (a: MetadataRef, b: MetadataRef) => sameRef(a.content_ref, b.content_ref)
  && (Object.keys(a.identity) as (keyof MetadataRef["identity"])[]).every(key => a.identity[key] === b.identity[key]);
const sameContext = (a: PlaybackContext, b: PlaybackContext) => sameMetadata(a.metadata_ref, b.metadata_ref)
  && sameRef(a.content_ref, b.content_ref) && sameRef(a.episode_ref, b.episode_ref)
  && (["metadata_video_id", "match_id", "group_id", "episode_id", "selected_stream_id"] as const).every(key => a[key] === b[key]);
function requireBinding(condition: unknown, message: string): asserts condition {
  if (!condition) throw new ApiFailure({ code: "BAD_REQUEST", message, retryable: false });
}
function failure(error: unknown): Bridge_ApiError {
  return error instanceof ApiFailure ? error.error : { code: "INTERNAL_ERROR", message: error instanceof Error ? error.message : String(error), retryable: true };
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
  requireBinding(details.details.groups.some(group => group.episodes.some(episode => {
    try { return sameSource(episode.ref, context.metadata_ref.content_ref) && JSON.parse(episode.ref.data).video_id === context.metadata_video_id; }
    catch { return false; }
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
      if (request.page >= 2) replacement.items = [...previous!.items, ...replacement.items];
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
  const images = new Set(result.page.sections.flatMap(section => section.items.flatMap(item => item.poster ? [item.poster.resource_id] : [])));
  result.images = Object.fromEntries(Object.entries(result.images).filter(([key]) => images.has(key)));
  return result;
}

type Ticket = { id: string; source: SourceKey | null; host: string | null; guest: string | null | undefined; locale: string; epoch: number };
type LocalRequest = { request_id: string | null; phase: "loading" | "ready" | "failed"; error: Bridge_ApiError | null };
export class AppController {
  readonly state: AppState;
  readonly sections = reactive<Record<string, LocalRequest>>({});
  readonly diagnostics = reactive<Bridge_ApiError[]>([]);
  private requests = new Map<string, Ticket>();
  private runtimeSequence = 0;
  private sourceSequence = 0;
  private confirmation: PlaybackMatchResult | null = null;
  private settings: SettingsQueue;
  private stopMedia: () => void;
  private api: AppApi;
  constructor(api: AppApi, settings = new SettingsQueue(api), stopMedia: () => void = () => {}, initial = createAppState()) {
    this.api = api;
    this.state = reactive(initial);
    this.settings = settings;
    this.stopMedia = stopMedia;
  }
  private ticket(scope: string, source: SourceKey | null, epoch: number): Ticket {
    this.cancel(scope);
    const provider = this.state.sources.data?.providers.find(item => item.id === source?.source_id);
    const value = { id: crypto.randomUUID(), source: source && clone(source), host: this.state.runtime.engine_epoch,
      guest: provider?.kind === "cloudstream" ? this.state.runtime.cloudstream.engine_epoch : undefined,
      locale: this.state.settings.ui_locale ?? "en", epoch };
    this.requests.set(scope, value);
    return value;
  }
  private current(scope: string, ticket: Ticket, response?: { engine_epoch: string | null }) {
    const provider = this.state.sources.data?.providers.find(item => item.id === ticket.source?.source_id);
    return this.requests.get(scope) === ticket && ticket.host === this.state.runtime.engine_epoch
      && (!response || response.engine_epoch === ticket.host)
      && (!["home", "details"].includes(scope) && !scope.startsWith("section:") || ticket.locale === (this.state.settings.ui_locale ?? "en"))
      && (ticket.guest === undefined || ticket.guest === this.state.runtime.cloudstream.engine_epoch)
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
  }
  async initialize() {
    this.state.settings = await this.settings.initialize();
    this.state.playback.volume = this.state.settings.volume;
    this.state.playback.muted = this.state.settings.muted;
    await this.refreshRuntime();
    await this.loadSources();
  }
  async refreshRuntime() {
    const sequence = ++this.runtimeSequence;
    const response = await this.api.request("runtime_status", {});
    if (sequence !== this.runtimeSequence) return;
    requireBinding(response.data.engine_epoch === response.engine_epoch, "Runtime envelope host epoch differs");
    this.applyRuntime(response.data);
  }
  applyRuntime(runtime: RuntimeStatus) {
    requireBinding(runtime.phase !== "ready" || (typeof runtime.engine_epoch === "string"
      && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(runtime.engine_epoch)
      && runtime.health?.status === "ready" && runtime.health.media_gateway_ready), "Ready runtime requires healthy host/gateway");
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
      this.state.selected_metadata_source = this.state.selected_playback_source = null;
    } else if (guestChanged) {
      for (const [scope, ticket] of this.requests) if (ticket.guest !== undefined) this.cancel(scope);
      const selected = this.state.selected_playback_source;
      if (this.state.sources.data?.providers.some(source => source.id === selected?.source_id && source.kind === "cloudstream")) this.invalidatePlayback();
      if (this.state.sources.data) this.state.sources.data.providers = this.state.sources.data.providers.filter(source => source.kind !== "cloudstream");
      if (selected && !this.state.sources.data?.providers.some(source => source.id === selected.source_id)) this.state.selected_playback_source = null;
    }
  }
  async loadSources() {
    const target = this.state.sources;
    const sequence = ++this.sourceSequence;
    const ticket = this.ticket("sources", null, ++target.request_epoch);
    target.phase = "loading"; target.request_id = ticket.id; target.error = null;
    try {
      const response = await this.api.request("get_sources", {}, ticket.id);
      if (sequence !== this.sourceSequence || !this.current("sources", ticket, response)) return;
      const oldMetadata = this.state.selected_metadata_source, oldPlayback = this.state.selected_playback_source;
      target.data = clone(response.data);
      target.phase = response.data.providers.length ? "ready" : "empty";
      const ready = response.data.providers.filter(source => source.runtime_status === "ready");
      const metadata = ready.find(source => source.kind === "stremio" && source.origin.config_id === this.state.settings.last_metadata_config_id)
        ?? ready.find(source => source.kind === "stremio" && source.id === oldMetadata?.source_id)
        ?? ready.find(source => source.kind === "stremio");
      const playback = ready.find(source => source.kind !== "stremio" && source.id === this.state.settings.last_playback_source_key)
        ?? ready.find(source => source.kind !== "stremio" && source.capabilities.resolve && source.id === oldPlayback?.source_id)
        ?? ready.find(source => source.kind !== "stremio" && source.capabilities.resolve);
      this.state.selected_metadata_source = metadata ? { source_id: metadata.id, source_revision: metadata.source_revision } : null;
      this.state.selected_playback_source = playback ? { source_id: playback.id, source_revision: playback.source_revision } : null;
      if (oldMetadata && !sameSource(oldMetadata, this.state.selected_metadata_source)) {
        this.cancelPages(); this.invalidatePlayback(); this.state.details.data = this.state.home.data = null;
        this.state.route = { name: "home" };
      } else if (oldPlayback && !sameSource(oldPlayback, this.state.selected_playback_source)) this.invalidatePlayback();
    } catch (error) { if (this.current("sources", ticket)) { target.phase = "failed"; target.error = failure(error); } }
    finally { if (this.requests.get("sources") === ticket) this.requests.delete("sources"); }
  }
  async updateSettings(change: (value: Settings) => Settings) {
    const previous = this.state.settings;
    const confirmed = await this.settings.update(change);
    this.state.settings = confirmed;
    if ((previous.ui_locale ?? "en") !== (confirmed.ui_locale ?? "en")) {
      this.cancelPages();
      // Active playback retains the same confirmed identity and session; only display DTOs reload.
      if (this.state.route.name === "home" && this.state.selected_metadata_source) void this.loadHome();
      else if (this.state.details.data && this.state.route.name === "details") void this.loadDetails(this.state.details.data.details.ref);
    }
    return confirmed;
  }
  async selectSource(sourceId: string, role: "metadata" | "playback") {
    const source = this.state.sources.data?.providers.find(source => source.id === sourceId);
    requireBinding(source?.runtime_status === "ready" && (role === "metadata" ? source.kind === "stremio" : source.kind !== "stremio" && source.capabilities.resolve), "Source is unavailable for this role");
    if (sameSource(role === "metadata" ? this.state.selected_metadata_source : this.state.selected_playback_source,
      { source_id: source.id, source_revision: source.source_revision })) return;
    const configId = source.kind === "stremio" ? source.origin.config_id : null;
    const host = this.state.runtime.engine_epoch;
    await this.updateSettings(settings => role === "metadata"
      ? { ...settings, last_metadata_config_id: configId }
      : { ...settings, last_playback_source_key: source.id });
    requireBinding(host === this.state.runtime.engine_epoch && this.state.sources.data?.providers.some(current =>
      current.id === source.id && current.source_revision === source.source_revision && current.runtime_status === "ready"), "Source changed while saving its preference");
    const context = this.state.playback.context && clone(this.state.playback.context);
    this.invalidatePlayback();
    const key = { source_id: source.id, source_revision: source.source_revision };
    if (role === "metadata") {
      this.cancelPages(); this.state.home.data = this.state.details.data = null;
      this.state.home.phase = this.state.details.phase = "idle";
      this.state.selected_metadata_source = key;
      this.state.route = { name: "home" };
    } else {
      this.state.selected_playback_source = key;
      if (context) await this.match(context.metadata_video_id);
    }
  }
  navigate(route: Route) {
    if (route.name === "playback") { assertContext(this.state, route.context); requireBinding(this.state.playback.context && sameContext(this.state.playback.context, route.context), "Route requires the current confirmed context"); }
    else this.invalidatePlayback();
    this.cancelPages();
    this.state.route = clone(route);
  }
  async loadHome(sectionRef: string | null = null, pageNumber = 1) {
    const source = this.state.selected_metadata_source;
    requireBinding(source && this.state.runtime.engine_epoch, "Home requires a metadata source and host epoch");
    const full = sectionRef === null && pageNumber === 1;
    requireBinding(sectionRef !== null || full, "Pagination requires a section reference");
    if (full) {
      for (const scope of [...this.requests.keys()]) if (scope.startsWith("section:")) this.cancel(scope);
      for (const key of Object.keys(this.sections)) delete this.sections[key];
      this.state.home.request_epoch++;
    }
    const scope = full ? "home" : `section:${sectionRef}`;
    const ticket = this.ticket(scope, source, this.state.home.request_epoch);
    const input = { ...clone(source), section_ref: sectionRef, page: pageNumber };
    if (full) { this.state.home.phase = "loading"; this.state.home.request_id = ticket.id; this.state.home.error = null; }
    else this.sections[sectionRef!] = { request_id: ticket.id, phase: "loading", error: null };
    const valid = (response?: ApiResponse<"get_home">) => this.current(scope, ticket, response) && ticket.epoch === this.state.home.request_epoch;
    try {
      const response = await this.api.request("get_home", input, ticket.id);
      if (!valid(response)) return;
      const next = mergeHome(this.state.home.data, response.data, input);
      this.state.home.data = next;
      if (full) this.state.home.phase = next.page.sections.some(section => section.items.length) ? "ready" : next.page.failed_sections.length ? "failed" : "empty";
      else {
        const error = response.data.page.failed_sections[0]?.error ?? null;
        this.sections[sectionRef!] = { request_id: null, phase: error ? "failed" : "ready", error };
      }
    } catch (error) {
      if (valid()) {
        if (full) { this.state.home.phase = "failed"; this.state.home.error = failure(error); }
        else this.sections[sectionRef!] = { request_id: null, phase: "failed", error: failure(error) };
      }
    } finally { if (this.requests.get(scope) === ticket) this.requests.delete(scope); }
  }
  async loadDetails(ref: Bridge_MediaRef) {
    requireBinding(sameSource(ref, this.state.selected_metadata_source), "Details must belong to selected metadata source");
    const target = this.state.details;
    const ticket = this.ticket("details", ref, ++target.request_epoch);
    target.phase = "loading"; target.request_id = ticket.id; target.error = null;
    try {
      const response = await this.api.request("get_details", { ref: clone(ref) }, ticket.id);
      if (!this.current("details", ticket, response)) return;
      requireBinding(sameRef(response.data.details.ref, ref) && response.data.metadata_ref && sameRef(response.data.metadata_ref.content_ref, ref), "Details response identity differs");
      const provider = this.state.sources.data?.providers.find(source => source.id === ref.source_id);
      requireBinding(provider?.kind === "stremio" && provider.origin.config_id === response.data.metadata_ref.identity.config_id, "Details metadata configuration differs");
      target.data = clone(response.data); target.phase = "ready";
    } catch (error) { if (this.current("details", ticket)) { target.phase = "failed"; target.error = failure(error); } }
    finally { if (this.requests.get("details") === ticket) this.requests.delete("details"); }
  }
  private async dispose(session: PlaybackViewSession | null, resolution: Bridge_Resolution | null) {
    // Captured handles keep delayed cleanup independent from a newly opened playback.
    if (session) try { await this.api.call("close_playback", { session_id: session.session_id }); } catch (error) { this.diagnostics.push(failure(error)); }
    if (resolution) try { await this.api.call("release_resolution", { resolution_id: resolution.id }); } catch (error) { this.diagnostics.push(failure(error)); }
  }
  invalidatePlayback() {
    this.confirmation = null;
    this.cancel("match"); this.cancel("resolve"); this.cancel("open");
    const { session, resolution } = this.state.playback;
    this.stopMedia();
    this.state.playback = { ...createAppState(this.state.settings).playback, playback_epoch: this.state.playback.playback_epoch + 1,
      speed: this.state.playback.speed };
    if (this.state.route.name === "playback") this.state.route = this.state.details.data ? { name: "details", ref: clone(this.state.details.data.details.ref) } : { name: "home" };
    void this.dispose(session && clone(session), resolution && clone(resolution));
  }
  async match(videoId: string, selection: PlaybackMatchSelection | null = null): Promise<PlaybackMatchResult | null> {
    const metadata = this.state.details.data?.metadata_ref, source = this.state.selected_playback_source;
    requireBinding(metadata && source, "Match requires metadata details and playback source");
    requireBinding(!selection || sameSource(selection.content_ref, source), "Candidate belongs to another playback source");
    if (selection) requireBinding(this.confirmation?.status === "needs_confirmation"
      && sameMetadata(this.confirmation.metadata_ref, metadata) && this.confirmation.video_id === videoId
      && this.confirmation.candidates.some(candidate => sameRef(candidate.content_ref, selection.content_ref))
      && selection.group_id && selection.episode_id, "Candidate is absent from the current confirmation result");
    this.invalidatePlayback();
    const target = this.state.playback;
    const ticket = this.ticket("match", source, target.playback_epoch);
    try {
      const response = await this.api.request("match_playback", { metadata_ref: clone(metadata), video_id: videoId, playback_source: clone(source), selection }, ticket.id);
      if (!this.current("match", ticket, response) || target.playback_epoch !== this.state.playback.playback_epoch) return null;
      const result = response.data;
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
        target.context = clone(context); target.engine_epoch = ticket.host;
        target.phase = "resolving";
        this.state.route = { name: "playback", context: clone(context) };
      } else { target.phase = "idle"; if (result.status === "needs_confirmation") this.confirmation = clone(result); }
      return result;
    } catch (error) { if (this.current("match", ticket)) { target.phase = "failed"; target.error = failure(error); } return null; }
    finally { if (this.requests.get("match") === ticket) this.requests.delete("match"); }
  }
  async resolve() {
    const target = this.state.playback, context = target.context;
    requireBinding(context, "Resolve requires a confirmed playback context");
    assertContext(this.state, context);
    this.cancel("open");
    this.stopMedia();
    const previousSession = target.session;
    const previousResolution = target.resolution;
    target.playback_epoch++;
    target.session = null;
    target.resolution = null;
    context.selected_stream_id = null;
    this.state.route = { name: "playback", context: clone(context) };
    const ticket = this.ticket("resolve", this.state.selected_playback_source, target.playback_epoch);
    target.phase = "resolving"; target.error = null;
    try {
      if (previousSession || previousResolution) await this.dispose(previousSession && clone(previousSession), previousResolution && clone(previousResolution));
      if (!this.current("resolve", ticket)) return;
      const response = await this.api.request("resolve_episode", { episode_ref: clone(context.episode_ref), match_id: context.match_id }, ticket.id);
      if (!this.current("resolve", ticket, response) || target !== this.state.playback) {
        if (response.engine_epoch === this.state.runtime.engine_epoch) void this.dispose(null, response.data);
        return;
      }
      requireBinding(sameRef(response.data.episode_ref, context.episode_ref) && sameSource(response.data, context.episode_ref), "Resolution response belongs to another episode");
      requireBinding(new Set(response.data.streams.map(stream => stream.id)).size === response.data.streams.length, "Duplicate stream identity");
      target.resolution = clone(response.data); context.selected_stream_id = null; target.session = null;
      this.state.route = { name: "playback", context: clone(context) }; target.phase = "preparing";
    } catch (error) { if (this.current("resolve", ticket)) { target.phase = "failed"; target.error = failure(error); } }
    finally { if (this.requests.get("resolve") === ticket) this.requests.delete("resolve"); }
  }
  async openStream(streamId: string) {
    const target = this.state.playback, context = target.context, resolution = target.resolution;
    requireBinding(context && resolution, "Open requires a confirmed context and Resolution");
    const stream = resolution.streams.find(stream => stream.id === streamId);
    requireBinding(stream?.playable && ["progressive", "hls", "dash"].includes(stream.kind), "Stream is absent or unplayable");
    this.cancel("open");
    this.stopMedia();
    const old = target.session;
    target.playback_epoch++;
    target.session = null; context.selected_stream_id = null;
    this.state.route = { name: "playback", context: clone(context) };
    const ticket = this.ticket("open", this.state.selected_playback_source, target.playback_epoch);
    target.phase = "preparing"; target.error = null;
    try {
      if (old) await this.dispose(clone(old), null);
      if (!this.current("open", ticket)) return;
      const response = await this.api.request("open_playback", { resolution_id: resolution.id, stream_id: streamId }, ticket.id);
      if (!this.current("open", ticket, response) || target !== this.state.playback) {
        if (response.engine_epoch === this.state.runtime.engine_epoch) void this.dispose(response.data, null);
        return;
      }
      const candidate: AppState = { ...clone(this.state), playback: { ...clone(target), context: { ...clone(context), selected_stream_id: streamId }, session: response.data } };
      candidate.route = { name: "playback", context: candidate.playback.context! };
      try { assertPlayback(candidate); } catch (error) { void this.dispose(response.data, null); throw error; }
      context.selected_stream_id = streamId; target.session = clone(response.data); target.phase = "ready";
      this.state.route = { name: "playback", context: clone(context) };
    } catch (error) { if (this.current("open", ticket)) { target.phase = "failed"; target.error = failure(error); } }
    finally { if (this.requests.get("open") === ticket) this.requests.delete("open"); }
  }
  playerEvent(event: PlayerEvent): boolean {
    const target = this.state.playback;
    if (event.playback_epoch !== target.playback_epoch || event.engine_epoch !== target.engine_epoch
      || event.session_id !== target.session?.session_id || !target.session) return false;
    if (event.type === "position") {
      if (!Number.isSafeInteger(event.position_ms) || event.position_ms < 0) return false;
      target.position_ms = target.duration_ms === null ? event.position_ms : Math.min(target.duration_ms, event.position_ms);
    } else if (event.type === "ready") {
      if (event.duration_ms !== null && (!Number.isSafeInteger(event.duration_ms) || event.duration_ms < 0)) return false;
      target.phase = "ready"; target.duration_ms = event.duration_ms;
    }
    else if (event.type === "failed") { target.phase = "failed"; target.error = event.error; }
    else if (event.type === "stopped") this.invalidatePlayback();
    else { target.phase = event.type; target.error = null; }
    return true;
  }
}
