import test, { after } from "node:test";
import assert from "node:assert/strict";
import { writeFileSync } from "node:fs";
import { Api, ApiFailure, SettingsQueue } from "../src/api.ts";
import { AppController, assertContext, assertPlayback, createAppState, createPlaybackController, defaultSettings, mergeHome } from "../src/state.ts";
import type { AppSource, AppState, Bridge_HomeSection, Bridge_MediaCard, Bridge_MediaRef, DetailsViewData, HomeViewData,
  MetadataRef, PlaybackContext, PlaybackMatchResult, RuntimeStatus, SourceKey } from "../src/models.ts";

const HOST = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa", NEXT = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
const GUEST = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";
const CONFIG = "dddddddd-dddd-4ddd-8ddd-dddddddddddd";
const META = { source_id: "metadata", source_revision: "metadata-revision" };
const PLAY = { source_id: "playback", source_revision: "playback-revision" };
const CS = { source_id: "guest-source", source_revision: "guest-revision" };
const expires = "2099-01-01T00:00:00Z";
const copy = <T>(value: T): T => JSON.parse(JSON.stringify(value));
const flush = async () => { for (let i = 0; i < 40; i++) await Promise.resolve(); };
function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>(yes => { resolve = yes; }); return { promise, resolve }; }
const error = { code: "UPSTREAM_FAILED" as const, message: "fixture upstream failed", retryable: true };
const ref = (id: string, source: SourceKey = META): Bridge_MediaRef => ({ ...source, data: JSON.stringify({ id }) });
const episodeRef = (id: string, video: string, source: SourceKey = META): Bridge_MediaRef => ({ ...source, data: JSON.stringify(["stremio_video", "addon", "series", id, video]) });
function metadata(id = "show"): MetadataRef {
  return { content_ref: ref(id), identity: { config_id: CONFIG, addon_key: "a".repeat(64), namespace: "addon:" + "a".repeat(64),
    type: "series", meta_id: id, canonical_id: id } };
}
function details(id = "show", source: SourceKey = META): DetailsViewData {
  return { metadata_ref: source.source_id === META.source_id ? metadata(id) : null, images: {},
    details: { id, ref: ref(id, source), title: id, kind: "series", tags: [], related: [], groups: [{ id: "group", title: "Season 1", kind: "season", dub_status: "unspecified",
      episodes: [1, 2, 3].map((n, index) => ({ id: `${id}:${n}`, label: String(n), index, ref: episodeRef(id, `${id}:${n}`, source) })) }] } };
}
function context(video = "show:1", source: SourceKey = PLAY): PlaybackContext {
  const id = video.split(":")[0]!;
  return { metadata_ref: metadata(id), metadata_video_id: video, match_id: `match:${video}`, content_ref: ref(id, source),
    group_id: "group", episode_id: video, episode_ref: episodeRef(id, video, source), selected_stream_id: null };
}
function providers(): AppSource[] {
  const caps = { home: true, search: true, details: true, resolve: true, sequential_home: false };
  return [
    { id: META.source_id, source_revision: META.source_revision, kind: "stremio", name: "Metadata", language: "en", content_types: ["series"], runtime_status: "ready", capabilities: { ...caps, resolve: false }, origin: { config_id: CONFIG, manifest_urls: ["https://metadata.example/manifest.json"] } },
    { id: PLAY.source_id, source_revision: PLAY.source_revision, kind: "maccms", name: "Playback", language: "zh", content_types: ["series"], runtime_status: "ready", capabilities: caps, origin: { config_id: NEXT, api_url: "https://playback.example/api.php" } },
    { id: CS.source_id, source_revision: CS.source_revision, kind: "cloudstream", name: "Guest", language: "en", content_types: ["series"], runtime_status: "ready", capabilities: { ...caps, sequential_home: true }, origin: { plugin_internal_name: "fixture", plugin_sha256: "b".repeat(64), registration_key: "c".repeat(64) } },
  ];
}
function runtime(host = HOST, guest = GUEST): RuntimeStatus {
  return { platform: "macos", phase: "ready", engine_epoch: host, health: { status: "ready", contract_version: "1.5.0", media_gateway_ready: true }, error: null,
    cloudstream: { phase: "stopped", engine_epoch: guest, display_mode: "none", health: null, error: null } };
}
function card(id: string, image = id): Bridge_MediaCard {
  return { id, ref: ref(id), title: id, kind: "series", poster: { resource_id: image, path: `/images/${image}`, capability: "X".repeat(43), expires_at: expires } };
}
function section(index: number, ids = [`card-${index}`], page = 1): Bridge_HomeSection {
  return { id: `section-${index}`, section_ref: `opaque:${9 - index}`, index, title: index ? "AAA" : "ZZZ",
    page, has_next: page === 1, next_page: page === 1 ? 2 : null, items: ids.map(id => card(id)) };
}
function home(sections = [section(2), section(0)], failed: number[] = []): HomeViewData {
  return { page: { ...META, sections, failed_sections: failed.map(index => ({ index, title: "Failed", section_ref: `opaque:${9 - index}`, error })) },
    images: Object.fromEntries(sections.flatMap(section => section.items.map(item => [item.poster!.resource_id, {
      resource_id: item.poster!.resource_id, url: `http://127.0.0.1:18080/images/${item.poster!.resource_id}?cap=${"X".repeat(43)}`, expires_at: expires,
    }]))) };
}
type Call = { command: string; input: any; request_id: string };
type Intercept = (call: Call, response: any) => unknown | Promise<unknown>;
async function fixture() {
  const root = createAppState(), calls: Call[] = [], resolutions = new Map<string, any>(), sessions = new Map<string, any>();
  let saved = defaultSettings(), intercept: Intercept | undefined;
  const client = new Api(async (command, args) => {
    const input = args.input as any, call = { command, input, request_id: args.request_id }; calls.push(copy(call));
    let data: any;
    if (command === "get_settings") data = saved;
    else if (command === "save_settings") data = saved = copy(input);
    else if (command === "get_sources") data = { providers: providers(), plugins: [] };
    else if (command === "runtime_status" || command === "runtime_prepare") data = runtime();
    else if (command === "get_home") data = input.section_ref == null ? home() : home([section(9 - Number(input.section_ref.split(":")[1]), ["more"], input.page)]);
    else if (command === "get_details") data = details(JSON.parse(input.ref.data).id, input.ref);
    else if (command === "match_playback") {
      const ctx = context(input.video_id, input.playback_source);
      data = { status: "matched", metadata_ref: input.metadata_ref, video_id: input.video_id, context: ctx,
        candidates: [{ content_ref: ctx.content_ref, title: "show", year: null }] };
    } else if (command === "resolve_episode") {
      data = { id: crypto.randomUUID(), source_id: input.episode_ref.source_id, source_revision: input.episode_ref.source_revision, episode_ref: input.episode_ref,
        expires_at: expires, streams: [{ id: "stream", label: "Stream", kind: "progressive", playable: true, has_interceptor: false }], subtitles: [] };
      resolutions.set(data.id, copy(data));
    } else if (command === "open_playback") {
      const resolution = resolutions.get(input.resolution_id);
      data = { session_id: crypto.randomUUID(), resolution_id: resolution.id, stream_id: input.stream_id, source_id: resolution.source_id, source_revision: resolution.source_revision,
        episode_ref: resolution.episode_ref, stream_kind: "progressive", mime_type: "video/mp4", subtitles: [], expires_at: expires,
        media_url: `http://127.0.0.1:18080/media/fixture.mp4?cap=${"X".repeat(43)}` };
      sessions.set(data.session_id, copy(data));
    } else if (command === "close_playback") { sessions.delete(input.session_id); data = { ok: true }; }
    else if (command === "release_resolution") { resolutions.delete(input.resolution_id); data = { ok: true }; }
    else if (command === "cancel_request") data = { cancelled: false, target_request_id: input.target_request_id };
    else if (command === "get_resource_status") data = { ...input, state: "ready", error: null };
    else throw new Error(`Unexpected command ${command}`);
    const response = { request_id: args.request_id, engine_epoch: HOST, data: copy(data) };
    return intercept ? await intercept(call, response) : response;
  });
  const queue = new SettingsQueue(client), app = new AppController(client, queue, root);
  await app.initialize();
  return { root, calls, client, queue, app, resolutions, sessions, intercept(fn?: Intercept) { intercept = fn; },
    async show(id = "show") { app.navigate({ name: "details", ref: ref(id) }); await app.loadDetails(ref(id)); } };
}
class Video extends EventTarget {
  currentTime = 0; duration = NaN; readyState = 0; networkState = 0; paused = true; ended = false; src = "";
  volume = 1; muted = false; playbackRate = 1; error = null; textTracks: unknown[] = [];
  buffered = { length: 0, start: () => 0, end: () => 30 };
  canPlayType() { return "maybe"; }
  removeAttribute() { this.src = ""; }
  querySelectorAll() { return []; }
  load() { this.currentTime = 0; this.readyState = 0; this.duration = NaN; }
  async play() { this.paused = false; this.dispatchEvent(new Event("playing")); }
  pause() { if (!this.paused) { this.paused = true; this.dispatchEvent(new Event("pause")); } }
  ready() { this.duration = 30; this.readyState = 4; this.dispatchEvent(new Event("loadedmetadata")); }
}
const trace: { name: string; value: AppState }[] = [];
function capture(name: string, value: AppState) { assertPlayback(value); trace.push({ name, value: copy(value) }); }
after(() => { if (process.env.T13_STATE_TRACE) writeFileSync(process.env.T13_STATE_TRACE, JSON.stringify(trace, null, 2)); });

test("U07/R01/R31/REQ-T13: one AppState restores independent source preferences and schema transitions", async () => {
  const f = await fixture(); capture("initialized", f.root);
  assert.deepEqual(f.root.selected_metadata_source, META); assert.deepEqual(f.root.selected_playback_source, PLAY);
  await f.app.loadHome(); capture("home", f.root);
  assert.deepEqual(f.root.home.data!.page.sections.map(section => section.index), [0, 2]);
  await f.show(); capture("details", f.root);
  const result = await f.app.match("show:1"); assert.equal(result?.status, "matched"); capture("confirmed", f.root);
  const video = new Video(), media = createPlaybackController(f.root, f.client);
  const unbind = f.app.bindPlayback(media); media.attach(video as unknown as HTMLVideoElement);
  await media.load(f.root.playback.context!); capture("opened", f.root);
  video.ready(); await flush(); media.pause(); media.seek(4); capture("paused", f.root);
  assert.equal(f.root.playback.position_ms, 4000); assert.equal(f.sessions.size, 1);
  f.app.navigate({ name: "home" }); await media.settled(); capture("returned", f.root); unbind();
  assert.equal(f.sessions.size, 0); assert.equal(f.resolutions.size, 0); assert.ok(f.root.home.data);
});

test("U07/R01/R36/REQ-T13: fast A/B details and cancelled return discard late results", async () => {
  const f = await fixture(), late = deferred<any>(); let old: any;
  f.intercept((call, response) => { if (call.command === "get_details" && JSON.parse(call.input.ref.data).id === "A") { old = response; return late.promise; } return response; });
  const a = f.app.loadDetails(ref("A")); await flush(); const id = f.calls.at(-1)!.request_id;
  const b = f.app.loadDetails(ref("B")); await b; late.resolve(old); await a;
  assert.equal(f.root.details.data!.details.id, "B"); assert.ok(f.calls.some(call => call.command === "cancel_request" && call.input.target_request_id === id));
  const next = deferred<any>(); f.intercept((call, response) => call.command === "get_details" ? next.promise : response);
  const pending = f.app.loadDetails(ref("C")); await flush(); f.app.navigate({ name: "home" }); const before = copy(f.root);
  next.resolve({ ...old, request_id: f.calls.findLast(call => call.command === "get_details")!.request_id }); await pending;
  assert.deepEqual(copy(f.root), before);
});

test("U07/R31/R36/REQ-T13: late match A cannot publish after B, return or source revision", async () => {
  for (const action of ["B", "back", "revision"]) {
    const f = await fixture(); await f.show(); const late = deferred<any>(); let response: any;
    f.intercept((call, value) => { if (call.command === "match_playback" && call.input.video_id === "show:1") { response = value; return late.promise; } return value; });
    const pending = f.app.match("show:1"); await flush();
    if (action === "B") await f.app.match("show:2");
    else if (action === "back") f.app.navigate({ name: "home" });
    else f.root.sources.data!.providers.find(source => source.id === PLAY.source_id)!.source_revision = "new-revision";
    const before = copy(f.root); late.resolve(response); await pending; assert.deepEqual(copy(f.root), before);
  }
});

test("U07/R31/R36/REQ-T13: host restart invalidates every old page, source and playback handle", async () => {
  const f = await fixture(); await f.app.loadHome(); await f.show(); await f.app.match("show:1");
  const epoch = f.root.playback.playback_epoch, homeEpoch = f.root.home.request_epoch;
  f.app.applyRuntime(runtime(NEXT));
  assert.equal(f.root.runtime.engine_epoch, NEXT); assert.equal(f.root.home.data, null); assert.equal(f.root.details.data, null); assert.equal(f.root.sources.data, null);
  assert.equal(f.root.selected_metadata_source, null); assert.equal(f.root.selected_playback_source, null); assert.equal(f.root.playback.context, null);
  assert.ok(f.root.home.request_epoch > homeEpoch && f.root.playback.playback_epoch > epoch); assert.equal(f.root.route.name, "home");
});

test("U07/R31/R36/REQ-T13: guest restart retains HTTP playback but invalidates Cloudstream only", async () => {
  for (const guest of [false, true]) {
    const f = await fixture(); await f.show(); if (guest) await f.app.selectSource(CS.source_id, "playback"); await f.app.match("show:1");
    const before = copy(f.root.playback), dto = f.root.details.data;
    f.app.applyRuntime(runtime(HOST, NEXT));
    assert.equal(f.root.details.data, dto); assert.equal(f.root.selected_metadata_source!.source_id, META.source_id);
    assert.ok(!f.root.sources.data!.providers.some(source => source.kind === "cloudstream"));
    if (guest) { assert.equal(f.root.playback.context, null); assert.equal(f.root.selected_playback_source, null); }
    else assert.deepEqual(copy(f.root.playback), before);
  }
});

test("U07/R31/REQ-T13: full home replacement sorts original index and replaces image ownership", () => {
  const before = home([section(0, ["old"])]), incoming = home([section(2), section(0)], [1]);
  incoming.images.orphan = { resource_id: "orphan", url: "unused", expires_at: expires };
  const merged = mergeHome(before, incoming, { ...META, page: 1 });
  assert.deepEqual(merged.page.sections.map(section => section.index), [0, 2]); assert.deepEqual(merged.page.failed_sections.map(section => section.index), [1]);
  assert.deepEqual(Object.keys(merged.images).sort(), ["card-0", "card-2"]); assert.deepEqual(before.page.sections[0]!.items.map(item => item.id), ["old"]);
});

test("U07/R31/REQ-T13: single-section page one preserves peers and clears only its failure", () => {
  const before = home([section(2), section(0)], [1]);
  const merged = mergeHome(before, home([section(1, ["retry"])]), { ...META, section_ref: "opaque:8", page: 1 });
  assert.deepEqual(merged.page.sections.map(section => section.index), [0, 1, 2]); assert.equal(merged.page.failed_sections.length, 0);
  assert.deepEqual(merged.page.sections[0], before.page.sections.find(section => section.index === 0));
  assert.deepEqual(Object.keys(merged.images).sort(), ["card-0", "card-2", "retry"]);
  const refreshed = mergeHome(merged, home([section(1, ["replacement"])]), { ...META, section_ref: "opaque:8", page: 1 });
  assert.ok(!refreshed.images.retry); assert.ok(refreshed.images["card-2"]);
});

test("U07/R31/REQ-T13: pagination deduplicates cards, updates page fields and merges shared images", () => {
  const before = home([section(0, ["same", "old"]), section(2, ["peer"])]);
  const incoming = home([section(0, ["same", "new"], 2)]); incoming.page.sections[0]!.items[0]!.title = "updated";
  incoming.images.same!.url += "&fresh=1";
  const merged = mergeHome(before, incoming, { ...META, section_ref: "opaque:9", page: 2 });
  const target = merged.page.sections[0]!;
  assert.deepEqual(target.items.map(item => item.id), ["same", "old", "new"]); assert.equal(target.items[0]!.title, "updated");
  assert.equal(target.page, 2); assert.equal(target.has_next, false); assert.equal(target.next_page, null);
  assert.ok(merged.images.same!.url.endsWith("fresh=1")); assert.ok(merged.images.peer && merged.images.old && merged.images.new);
});

test("U07/R31/REQ-T13: wrong section index/ref/source/page and duplicate indexes are rejected atomically", () => {
  const before = home(), frozen = copy(before);
  for (const mutate of [
    (value: HomeViewData) => { value.page.source_revision = "other"; },
    (value: HomeViewData) => { value.page.sections[0]!.index = 9; },
    (value: HomeViewData) => { value.page.sections[0]!.section_ref = "other"; },
    (value: HomeViewData) => { value.page.sections[0]!.items[0]!.ref = ref("bad", PLAY); },
    (value: HomeViewData) => { value.page.sections[0]!.page = 3; },
  ]) {
    const incoming = home([section(0)]); mutate(incoming);
    assert.throws(() => mergeHome(before, incoming, { ...META, section_ref: "opaque:9", page: 1 })); assert.deepEqual(before, frozen);
  }
  assert.throws(() => mergeHome(before, home([section(0), section(0)]), { ...META, page: 1 }));
  assert.throws(() => mergeHome(before, home([section(0, ["skipped"], 3)]), { ...META, section_ref: "opaque:9", page: 3 }));
});

test("U07/R31/R36/REQ-T13: local loading/failure never replaces full phase or successful peer cards", async () => {
  const f = await fixture(); await f.app.loadHome(); const before = copy(f.root.home), late = deferred<any>(); let response: any;
  f.intercept((call, value) => { if (call.command === "get_home") { response = value; return late.promise; } return value; });
  const request = f.app.loadHome("opaque:9", 2); await flush();
  assert.equal(f.root.home.phase, "ready"); assert.equal(f.app.sections["opaque:9"]!.phase, "loading");
  late.resolve({ ...response, data: home([], [0]) }); await request;
  assert.deepEqual(copy(f.root.home), before); assert.equal(f.app.sections["opaque:9"]!.phase, "failed"); assert.equal(f.app.sections["opaque:9"]!.error!.code, "UPSTREAM_FAILED");
});

test("U07/R31/R36/REQ-T13: a new full snapshot cancels local requests and drops their late images", async () => {
  const f = await fixture(); await f.app.loadHome(); const late = deferred<any>(); let response: any;
  f.intercept((call, value) => { if (call.command === "get_home" && call.input.section_ref) { response = value; return late.promise; } return value; });
  const local = f.app.loadHome("opaque:9", 2); await flush(); await f.app.loadHome(); const before = copy(f.root.home);
  late.resolve(response); await local; assert.deepEqual(copy(f.root.home), before); assert.deepEqual(Object.keys(f.app.sections), []);
});

test("U07/R31/R36/REQ-T13: per-section sequencing rejects stale retry after the latest page", async () => {
  const f = await fixture(); await f.app.loadHome(); const late = deferred<any>(); let response: any; let first = true;
  f.intercept((call, value) => { if (call.command === "get_home" && call.input.section_ref && first) { first = false; response = value; return late.promise; } return value; });
  const old = f.app.loadHome("opaque:9", 1); await flush(); await f.app.loadHome("opaque:9", 2); const before = copy(f.root.home);
  late.resolve(response); await old; assert.deepEqual(copy(f.root.home), before);
});

test("R01/R31/R36/R62/R67/REQ-T13: queued locale/palette commits preserve independent settings and playback", async () => {
  const f = await fixture(); await f.show(); await f.app.match("show:1"); const before = copy(f.root.playback);
  await Promise.all([f.app.updateSettings(value => ({ ...value, ui_locale: "zh-CN" })), f.app.updateSettings(value => ({ ...value, color_palette: "ocean" }))]);
  assert.equal(f.root.settings.ui_locale, "zh-CN"); assert.equal(f.root.settings.color_palette, "ocean"); assert.deepEqual(copy(f.root.playback), before);
  f.intercept((call, response) => call.command === "save_settings" ? { request_id: call.request_id, engine_epoch: HOST, error } : response);
  const confirmed = copy(f.root.settings); await assert.rejects(f.app.updateSettings(value => ({ ...value, ui_locale: "en" })), ApiFailure);
  assert.deepEqual(copy(f.root.settings), confirmed); assert.deepEqual(f.queue.value, confirmed);
});

test("R01/R31/R36/R62/REQ-T13: English/Chinese/English late home and detail responses stay in their request generation", async () => {
  const f = await fixture(), late = deferred<any>(); let response: any; let initial = true;
  f.intercept((call, value) => { if (call.command === "get_home" && initial) { initial = false; response = value; return late.promise; } return value; });
  const old = f.app.loadHome(); await flush(); await f.app.updateSettings(value => ({ ...value, ui_locale: "zh-CN" })); await flush();
  await f.app.updateSettings(value => ({ ...value, ui_locale: "en" })); await flush(); const before = copy(f.root.home);
  late.resolve(response); await old; assert.deepEqual(copy(f.root.home), before);
  await f.show(); const delayed = deferred<any>(); let detailResponse: any;
  f.intercept((call, value) => { if (call.command === "get_details" && !detailResponse) { detailResponse = value; return delayed.promise; } return value; });
  const oldDetails = f.app.loadDetails(ref("show")); await flush(); await f.app.updateSettings(value => ({ ...value, ui_locale: "zh-CN" })); await flush();
  const detailsBefore = copy(f.root.details); delayed.resolve(detailResponse); await oldDetails; assert.deepEqual(copy(f.root.details), detailsBefore);
});

test("R01/R31/R36/REQ-T13: changing playback source rematches the same metadata video and persists the role", async () => {
  const f = await fixture(); await f.show(); await f.app.match("show:2"); const metadataBefore = copy(f.root.details.data!.metadata_ref);
  await f.app.selectSource(CS.source_id, "playback");
  assert.equal(f.root.playback.context!.metadata_video_id, "show:2"); assert.equal(f.root.playback.context!.episode_ref.source_id, CS.source_id);
  assert.deepEqual(copy(f.root.details.data!.metadata_ref), metadataBefore); assert.equal(f.root.settings.last_playback_source_key, CS.source_id);
  await assert.rejects(f.app.selectSource(META.source_id, "playback")); await assert.rejects(f.app.selectSource(PLAY.source_id, "metadata"));
});

test("R01/R31/R36/REQ-T13: source revision refresh invalidates old page and confirmed match", async () => {
  const f = await fixture(); await f.app.loadHome(); await f.show(); await f.app.match("show:1");
  f.intercept((call, response) => call.command === "get_sources" ? { ...response, data: { ...response.data, providers: response.data.providers.map((source: AppSource) => source.kind === "stremio" ? { ...source, source_revision: "next" } : source) } } : response);
  await f.app.loadSources(); assert.equal(f.root.details.data, null); assert.equal(f.root.home.data, null); assert.equal(f.root.playback.context, null); assert.equal(f.root.route.name, "home");
});

test("U07/R31/REQ-T13: Route/context/detail video, host, Resolution and Session cross bindings reject substitutions", async () => {
  const f = await fixture(); await f.show(); await f.app.match("show:1");
  const video = new Video(), media = createPlaybackController(f.root, f.client); media.attach(video as unknown as HTMLVideoElement);
  await media.load(f.root.playback.context!); const valid = copy(f.root); assertPlayback(valid);
  const mutations: ((value: AppState) => void)[] = [
    value => { if (value.route.name === "playback") value.route.context.metadata_video_id = "show:2"; },
    value => { value.playback.context!.metadata_ref.content_ref.source_revision = "other"; },
    value => { value.playback.context!.metadata_ref.identity.config_id = NEXT; },
    value => { value.playback.context!.metadata_video_id = "show:999"; },
    value => { value.playback.context!.episode_ref.source_id = META.source_id; },
    value => { value.playback.engine_epoch = NEXT; },
    value => { value.playback.resolution!.episode_ref = episodeRef("show", "show:2", PLAY); },
    value => { value.playback.resolution!.source_revision = "other"; },
    value => { value.playback.context!.selected_stream_id = "missing"; },
    value => { value.playback.resolution!.streams[0]!.playable = false; },
    value => { value.playback.session!.resolution_id = "other"; },
    value => { value.playback.session!.source_id = META.source_id; },
    value => { value.playback.session!.episode_ref = episodeRef("show", "show:2", PLAY); },
    value => { value.playback.session!.stream_kind = "hls"; },
  ];
  for (const mutate of mutations) { const invalid = copy(valid); mutate(invalid); assert.throws(() => assertPlayback(invalid)); }
  const bad = context("show:999"); assert.throws(() => assertContext(f.root, bad)); assert.throws(() => f.app.navigate({ name: "playback", context: bad }));
  await assert.rejects(media.load(context(), { resume: { context: context("show:2"), position_ms: 10, was_playing: false } }));
  await media.stop();
});

test("R01/R31/REQ-T13: candidates and confirmed episodes must belong to the current confirmation", async () => {
  const f = await fixture(); await f.show();
  f.intercept((call, response) => call.command === "match_playback" && !call.input.selection ? { ...response, data: { ...response.data, status: "needs_confirmation", context: null } } : response);
  const result = await f.app.match("show:1"); assert.equal(result?.status, "needs_confirmation");
  const candidate = result!.candidates[0]!.content_ref;
  await assert.rejects(f.app.loadCandidate(ref("unoffered", PLAY)));
  await f.app.loadCandidate(candidate);
  await assert.rejects(f.app.match("show:1", { content_ref: candidate, group_id: "group", episode_id: "unoffered" }));
  await assert.rejects(f.app.match("show:1", { content_ref: ref("show", CS), group_id: "group", episode_id: "show:1" }));
  const matched = await f.app.match("show:1", { content_ref: candidate, group_id: "group", episode_id: "show:1" }); assert.equal(matched?.status, "matched");
  f.app.navigate({ name: "home" }); await assert.rejects(f.app.loadCandidate(candidate));
});

test("R01/R31/REQ-T13: match statuses, candidate count, metadata and chosen video reject illegal envelopes", async () => {
  const mutations: ((value: PlaybackMatchResult) => void)[] = [
    value => { value.video_id = "show:2"; }, value => { value.metadata_ref.identity.addon_key = "other"; },
    value => { value.candidates[0]!.content_ref.source_revision = "other"; }, value => { value.candidates = []; },
    value => { value.status = "not_found"; }, value => { value.context!.selected_stream_id = "premature"; },
    value => { value.context!.content_ref = ref("other", PLAY); },
  ];
  for (const mutate of mutations) {
    const f = await fixture(); await f.show(); f.intercept((call, response) => { if (call.command === "match_playback") mutate(response.data); return response; });
    assert.equal(await f.app.match("show:1"), null); assert.equal(f.root.playback.phase, "failed"); assert.equal(f.root.playback.context, null);
  }
});

test("R01/R31/REQ-T13: unhealthy ready runtime, foreign detail/config/episode and direct metadata playback fail", async () => {
  const f = await fixture(); const bad = runtime(); bad.health!.status = "failed"; assert.throws(() => f.app.applyRuntime(bad));
  for (const fault of ["ref", "config", "episode"]) {
    f.intercept((call, response) => {
      if (call.command === "get_details") {
        if (fault === "ref") response.data.details.ref = ref("other");
        else if (fault === "config") response.data.metadata_ref.identity.config_id = NEXT;
        else response.data.details.groups[0].episodes[0].ref.source_id = PLAY.source_id;
      }
      return response;
    });
    await f.app.loadDetails(ref("show")); assert.equal(f.root.details.phase, "failed"); assert.ok(f.root.details.error);
  }
  await assert.rejects(f.app.loadDetails(ref("show", PLAY))); assert.throws(() => f.app.navigate({ name: "details", ref: ref("show", PLAY) }));
});

test("configured sources synchronize on load and ready sources are reused", async () => {
  const root = createAppState();
  let published = false;
  const calls: string[] = [];
  const client = new Api(async (command, args) => {
    calls.push(command);
    let data: unknown;
    if (command === "get_sources") {
      const rows = providers();
      rows[0]!.runtime_status = published ? "ready" : "configured";
      data = { providers: rows, plugins: [] };
    } else if (command === "sync_catalog") {
      published = true;
      data = { ok: true };
    } else throw new Error(`Unexpected command ${command}`);
    return { request_id: args.request_id, engine_epoch: HOST, data };
  });
  const app = new AppController(client, new SettingsQueue(client), root);
  await app.loadSources();
  assert.equal(root.sources.phase, "ready");
  assert.equal(root.selected_metadata_source?.source_id, META.source_id);
  assert.deepEqual(calls, ["get_sources", "sync_catalog", "get_sources"]);
  await app.loadSources();
  assert.equal(calls.filter(command => command === "sync_catalog").length, 1);
});
