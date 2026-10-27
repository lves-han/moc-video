import test, { after } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, writeFileSync } from "node:fs";
import { toRaw } from "vue";
import { Api, SettingsQueue } from "../src/api.ts";
import { AppController, assertPlayback, createAppState, mergeHome } from "../src/state.ts";
import type { AppState, Bridge_MediaRef, DetailsViewData, HomeViewData, PlaybackContext, PlaybackViewSession, Settings } from "../src/models.ts";

const examples = JSON.parse(readFileSync(new URL("../protocol/examples/contract-cases.json", import.meta.url), "utf8"));
const rows: any[] = Array.isArray(examples) ? examples : examples.cases;
const golden = (): AppState => structuredClone(rows.find(row => row.name === "valid_AppState").value);
const copy = <T>(value: T): T => structuredClone(toRaw(value));
const traces: { name: string; value: AppState }[] = [];
after(() => { if (process.env.T13_STATE_TRACE) writeFileSync(process.env.T13_STATE_TRACE, JSON.stringify(traces)); });
const trace = (name: string, value: AppState) => traces.push({ name, value: copy(value) });
const error = { code: "UPSTREAM_FAILED" as const, message: "fixture upstream failure", retryable: true };
const tick = () => new Promise<void>(resolve => setImmediate(resolve));
type Pending = { command: string; args: { request_id: string; input: any }; resolve: (value: unknown) => void };
function harness(initial = golden()) {
  let stored = copy(initial.settings), rejectSave = false;
  const pending: Pending[] = [], calls: { command: string; input: any }[] = [];
  const host = initial.runtime.engine_epoch;
  const api = new Api(async (command, args) => {
    calls.push({ command, input: copy(args.input) });
    const response = (data: unknown) => ({ request_id: args.request_id, engine_epoch: host, data });
    if (command === "get_settings") return response(copy(stored));
    if (command === "save_settings") {
      if (rejectSave) return { request_id: args.request_id, engine_epoch: host, error: { ...error, message: "disk full" } };
      stored = copy(args.input as Settings); return response(stored);
    }
    if (command === "cancel_request") return response({ target_request_id: (args.input as any).target_request_id, cancelled: true });
    if (command === "close_playback" || command === "release_resolution") return response({ ok: true });
    return new Promise(resolve => pending.push({ command, args, resolve }));
  });
  let stops = 0;
  const controller = new AppController(api, new SettingsQueue(api), () => { stops++; }, initial);
  function take(command: string) {
    const index = pending.findIndex(row => row.command === command);
    assert.ok(index >= 0, `Expected pending ${command}`);
    return pending.splice(index, 1)[0]!;
  }
  function reply(command: string, data: unknown, engine_epoch = host) {
    const request = take(command);
    request.resolve({ request_id: request.args.request_id, engine_epoch, data: copy(data) });
  }
  return { controller, api, calls, pending, take, reply, get stored() { return stored; }, get stops() { return stops; }, failSave(value: boolean) { rejectSave = value; } };
}
function idle() {
  const initial = golden();
  initial.route = { name: "home" };
  initial.playback = { ...createAppState(initial.settings).playback, playback_epoch: 10 };
  initial.home.data = null; initial.home.phase = "idle";
  return initial;
}
function details(id: string): DetailsViewData {
  const data = copy(golden().details.data!);
  const ref = { ...data.details.ref, data: JSON.stringify({ id }) };
  data.details.ref = ref; data.details.id = id; data.details.title = id;
  data.metadata_ref!.content_ref = copy(ref);
  data.metadata_ref!.identity.meta_id = id; data.metadata_ref!.identity.canonical_id = id;
  return data;
}
const ref = (id: string) => details(id).details.ref;
function home(indices = [2, 0], page = 1): HomeViewData {
  const source = golden().selected_metadata_source!;
  return { images: Object.fromEntries(indices.map(index => [`image-${index}-${page}`, { resource_id: `image-${index}-${page}`, url: `http://127.0.0.1:1234/images/image-${index}-${page}?cap=${"A".repeat(43)}`, expires_at: "2099-01-01T00:00:00Z" }])),
    page: { ...source, failed_sections: [], sections: indices.map(index => ({ id: `s${index}`, title: index ? "AAA" : "ZZZ", section_ref: `opaque:${index}`, index, page,
      items: [{ id: `card-${index}-${page}`, title: `card-${index}-${page}`, kind: "movie", ref: { ...source, data: String(index) },
        poster: { resource_id: `image-${index}-${page}`, path: `/images/image-${index}-${page}`, capability: "A".repeat(43), expires_at: "2099-01-01T00:00:00Z" } }],
      has_next: page < 3, next_page: page < 3 ? page + 1 : null })) } };
}
function matchResult(context = golden().playback.context!) {
  const value = copy(context); value.selected_stream_id = null;
  return { status: "matched" as const, metadata_ref: value.metadata_ref, video_id: value.metadata_video_id,
    candidates: [{ content_ref: copy(value.content_ref), title: "Candidate", year: null }], context: value };
}

test("U07/R01/R08/R31/REQ-T13: default state and route-context-resolution-session cross negatives", () => {
  const state = createAppState();
  assert.equal(state.settings.ui_locale, "en"); assert.equal(state.settings.color_palette, "lime");
  assert.equal(state.home.phase, "idle"); assert.equal(state.playback.resolution, null);
  assertPlayback(state); assertPlayback(golden());
  trace("default", state); trace("golden", golden());
  const mutations: ((state: AppState) => void)[] = [
    s => { (s.route as { name: "playback"; context: PlaybackContext }).context.match_id = "other"; },
    s => { s.playback.context!.episode_ref.data = "another episode"; },
    s => { s.playback.context!.content_ref.source_id = "other"; },
    s => { s.playback.context!.metadata_video_id = "absent-video"; },
    s => { s.playback.context!.metadata_ref.identity.config_id = "other"; },
    s => { s.playback.engine_epoch = crypto.randomUUID(); },
    s => { s.selected_metadata_source = s.selected_playback_source; },
    s => { s.selected_playback_source = s.selected_metadata_source; },
    s => { s.details.data!.metadata_ref!.identity.addon_key = "f".repeat(64); },
    s => { s.details.data!.details.groups = []; },
    s => { s.playback.resolution!.source_revision = "other"; },
    s => { s.playback.resolution!.episode_ref.data = "other"; },
    s => { s.playback.context!.selected_stream_id = "absent"; },
    s => { s.playback.resolution!.streams[0]!.playable = false; },
    s => { s.playback.session!.resolution_id = "other"; },
    s => { s.playback.session!.stream_id = "other"; },
    s => { s.playback.session!.stream_kind = "dash"; },
    s => { s.playback.session!.source_id = "other"; },
    s => { s.playback.session!.source_revision = "other"; },
    s => { s.playback.session!.episode_ref.data = "other"; },
    s => { s.playback.resolution = null; },
    s => { s.playback.context = null; },
  ];
  for (const mutate of mutations) { const next = golden(); mutate(next); assert.throws(() => assertPlayback(next)); }
});

test("R31/R36/REQ-T13: API preserves the verified response host epoch", async () => {
  const h = harness(idle());
  const request = h.api.request("get_sources", {});
  h.reply("get_sources", golden().sources.data);
  const result = await request;
  assert.equal(result.engine_epoch, golden().runtime.engine_epoch);
  assert.ok(result.request_id); assert.ok(result.data.providers.length);
});

test("R31/R36/REQ-T13: stale source responses cannot change current request budgets", async () => {
  const h = harness(idle());
  const first = h.api.request("get_sources", {}), old = h.take("get_sources");
  const latest = h.api.request("get_sources", {});
  const providers = { providers: [{ id: "budget-source", kind: "stremio" }], plugins: [] };
  h.reply("get_sources", providers); await latest;
  old.resolve({ request_id: old.args.request_id, engine_epoch: golden().runtime.engine_epoch,
    data: { providers: [{ id: "budget-source", kind: "cloudstream" }], plugins: [] } }); await first;
  assert.equal(h.api.budget("get_home", { source_id: "budget-source", source_revision: "revision", page: 1 }), 45000);
});

test("R01/R31/R36/REQ-T13: A/B details responses cannot replace the latest page", async () => {
  const h = harness(idle()), c = h.controller;
  const a = c.loadDetails(ref("A")), b = c.loadDetails(ref("B"));
  const old = h.take("get_details"), latest = h.take("get_details");
  latest.resolve({ request_id: latest.args.request_id, engine_epoch: c.state.runtime.engine_epoch, data: details("B") }); await b;
  old.resolve({ request_id: old.args.request_id, engine_epoch: c.state.runtime.engine_epoch, data: details("A") }); await a;
  assert.equal(c.state.details.data!.details.id, "B");
  trace("details-B", c.state);
  assert.ok(h.calls.some(call => call.command === "cancel_request" && call.input.target_request_id === old.args.request_id));
  const late = c.loadDetails(ref("A")); c.navigate({ name: "home" }); h.reply("get_details", details("A")); await late;
  assert.equal(c.state.details.data!.details.id, "B"); assert.equal(c.state.route.name, "home");
});

test("R01/R31/R36/REQ-T13: foreign host/revision/details identities fail without replacing confirmed data", async () => {
  for (const mode of ["host", "revision", "content", "metadata"]) {
    const h = harness(idle()), c = h.controller, previous = copy(c.state.details.data);
    const task = c.loadDetails(ref("A")), data = details("A");
    if (mode === "revision") data.details.ref.source_revision = "old";
    if (mode === "content") data.details.ref.data = "other";
    if (mode === "metadata") data.metadata_ref!.content_ref.data = "other";
    h.reply("get_details", data, mode === "host" ? crypto.randomUUID() : c.state.runtime.engine_epoch);
    await task;
    assert.deepEqual(copy(c.state.details.data), previous);
    if (mode !== "host") assert.equal(c.state.details.phase, "failed");
  }
});

test("U07/R24/R35/R36/REQ-T13: home full/retry/pagination merge by index and prune image mappings", () => {
  const source = golden().selected_metadata_source!;
  const first = home(); first.images.unused = { resource_id: "unused", url: "http://127.0.0.1:1234/unused", expires_at: "2099-01-01T00:00:00Z" };
  first.page.failed_sections = [{ title: "Same title", section_ref: "opaque:1", index: 1, error }];
  let merged = mergeHome(null, first, { ...source, page: 1 });
  assert.deepEqual(merged.page.sections.map(section => section.index), [0, 2]);
  assert.equal(merged.images.unused, undefined);
  merged = mergeHome(merged, home([1]), { ...source, section_ref: "opaque:1", page: 1 });
  assert.deepEqual(merged.page.sections.map(section => section.index), [0, 1, 2]);
  assert.equal(merged.page.failed_sections.length, 0);
  merged = mergeHome(merged, home([0], 2), { ...source, section_ref: "opaque:0", page: 2 });
  assert.equal(merged.page.sections[0]!.items.length, 2);
  assert.deepEqual(Object.keys(merged.images).sort(), ["image-0-1", "image-0-2", "image-1-1", "image-2-1"]);
  merged = mergeHome(merged, home([0]), { ...source, section_ref: "opaque:0", page: 1 });
  assert.equal(merged.page.sections[0]!.items.length, 1); assert.equal(merged.images["image-0-2"], undefined);
  const snapshot = copy(merged);
  for (const mutate of [(data: HomeViewData) => { data.page.sections[0]!.index = 9; },
    (data: HomeViewData) => { data.page.sections[0]!.section_ref = "alien"; },
    (data: HomeViewData) => { data.page.sections[0]!.id = "alien"; },
    (data: HomeViewData) => { data.page.sections[0]!.items[0]!.ref.source_revision = "old"; }]) {
    const data = home([0], 2); mutate(data);
    assert.throws(() => mergeHome(merged, data, { ...source, section_ref: "opaque:0", page: 2 }));
    assert.deepEqual(merged, snapshot);
  }
  const duplicate = home([0, 0]); assert.throws(() => mergeHome(null, duplicate, { ...source, page: 1 }));
  const empty = home([]); assert.equal(mergeHome(merged, empty, { ...source, page: 1 }).page.sections.length, 0);
});

test("U07/R24/R35/R36/REQ-T13: stale full/local responses and partial retries preserve page state", async () => {
  const h = harness(idle()), c = h.controller;
  const a = c.loadHome(), b = c.loadHome();
  const old = h.take("get_home"); h.reply("get_home", home()); await b;
  old.resolve({ request_id: old.args.request_id, engine_epoch: c.state.runtime.engine_epoch, data: home([8]) }); await a;
  assert.deepEqual(c.state.home.data!.page.sections.map(section => section.index), [0, 2]);
  const paging = c.loadHome("opaque:0", 2), pending = h.take("get_home");
  assert.equal(c.state.home.phase, "ready");
  const full = c.loadHome(); h.reply("get_home", home([0])); await full;
  pending.resolve({ request_id: pending.args.request_id, engine_epoch: c.state.runtime.engine_epoch, data: home([0], 2) }); await paging;
  assert.equal(c.state.home.data!.page.sections[0]!.items.length, 1);
  const retry = c.loadHome("opaque:0", 2);
  const failed = home([]); failed.page.failed_sections = [{ title: "Error", section_ref: "opaque:0", index: 0, error }];
  h.reply("get_home", failed); await retry;
  assert.equal(c.state.home.phase, "ready"); assert.equal(c.sections["opaque:0"]!.phase, "failed");
  assert.equal(c.state.home.data!.page.sections[0]!.items.length, 1);
  const again = c.loadHome("opaque:0", 2); h.reply("get_home", home([0], 2)); await again;
  assert.equal(c.sections["opaque:0"]!.phase, "ready"); assert.equal(c.state.home.data!.page.sections[0]!.items.length, 2);
  trace("home-merged", c.state);
});

test("U07/R01/R31/R36/REQ-T13: confirmed match rejects cross-source/video/configuration and illegal candidates", async () => {
  const mutations = [
    (value: ReturnType<typeof matchResult>) => { value.video_id = "foreign-video"; },
    (value: ReturnType<typeof matchResult>) => { value.context.metadata_video_id = "foreign-video"; },
    (value: ReturnType<typeof matchResult>) => { value.context.metadata_ref.identity.config_id = "foreign-config"; },
    (value: ReturnType<typeof matchResult>) => { value.context.episode_ref.source_id = value.context.metadata_ref.content_ref.source_id; },
    (value: ReturnType<typeof matchResult>) => { value.context.episode_ref.source_revision = "old"; },
    (value: ReturnType<typeof matchResult>) => { value.candidates.push(copy(value.candidates[0]!)); },
    (value: ReturnType<typeof matchResult>) => { value.candidates[0]!.content_ref.data = "foreign-candidate"; },
    (value: ReturnType<typeof matchResult>) => { value.context.selected_stream_id = "unresolved-stream"; },
  ];
  for (const mutate of mutations) {
    const h = harness(idle()), c = h.controller, result = matchResult();
    const task = c.match(result.video_id); mutate(result); h.reply("match_playback", result); await task;
    assert.equal(c.state.playback.phase, "failed"); assert.equal(c.state.playback.context, null); assert.equal(c.state.route.name, "home");
  }
  const h = harness(idle());
  await assert.rejects(h.controller.match("video", { content_ref: ref("wrong-provider"), group_id: "g", episode_id: "e" }));
  assert.equal(h.pending.length, 0);
});

test("R01/R31/R36/REQ-T13: match A/B and late resolutions cannot revive a departed playback", async () => {
  const h = harness(idle()), c = h.controller, result = matchResult();
  const a = c.match(result.video_id), old = h.take("match_playback");
  const b = c.match(result.video_id); h.reply("match_playback", result); await b;
  old.resolve({ request_id: old.args.request_id, engine_epoch: c.state.runtime.engine_epoch, data: { ...result, context: { ...result.context, match_id: "obsolete" } } }); await a;
  assert.equal(c.state.playback.context!.match_id, result.context.match_id);
  const resolving = c.resolve(); c.navigate({ name: "home" }); h.reply("resolve_episode", golden().playback.resolution); await resolving; await tick();
  assert.equal(c.state.playback.resolution, null); assert.equal(c.state.route.name, "home");
  assert.ok(h.calls.some(call => call.command === "release_resolution"));
  const saved = copy(c.state.home.data); c.navigate({ name: "settings" }); c.navigate({ name: "home" });
  assert.deepEqual(copy(c.state.home.data), saved);
});

test("R01/R31/R36/REQ-T13: confirmed match and Resolution prepare a Session through valid AppState phases", async () => {
  const h = harness(idle()), c = h.controller, result = matchResult();
  const matching = c.match(result.video_id);
  trace("matching-awaits-confirmation", c.state);
  h.reply("match_playback", result); await matching;
  assert.equal(c.state.playback.phase, "resolving"); trace("matched-context", c.state);
  const resolving = c.resolve(); trace("resolving", c.state);
  h.reply("resolve_episode", golden().playback.resolution); await resolving;
  assert.equal(c.state.playback.phase, "preparing"); assert.equal(c.state.playback.session, null);
  trace("resolved-candidates", c.state);
  const opening = c.openStream(golden().playback.session!.stream_id); await tick();
  trace("preparing-session", c.state);
  h.reply("open_playback", golden().playback.session); await opening;
  assert.equal(c.state.playback.phase, "ready"); assertPlayback(c.state); trace("ready-session", c.state);
});

test("R01/R31/R36/REQ-T13: playback source rematches current video and metadata source invalidates its page", async () => {
  const initial = golden();
  const provider = copy(initial.sources.data!.providers.find(source => source.kind !== "stremio")!);
  provider.id = "b".repeat(64);
  (provider as any).kind = "maccms";
  (provider as any).origin = { config_id: crypto.randomUUID(), base_url: "https://example.com/api" };
  const metadata = copy(initial.sources.data!.providers.find(source => source.kind === "stremio")!);
  metadata.id = "f".repeat(64); (metadata.origin as any).config_id = crypto.randomUUID();
  initial.sources.data!.providers.push(provider, metadata);
  const h = harness(copy(initial)), c = h.controller, detailsBefore = copy(c.state.details.data);
  const switching = c.selectSource(provider.id, "playback"); await tick();
  assert.deepEqual(copy(c.state.details.data), detailsBefore);
  const input = h.calls.find(call => call.command === "match_playback")!.input;
  assert.equal(input.video_id, golden().playback.context!.metadata_video_id);
  assert.equal(input.playback_source.source_id, provider.id);
  const result = matchResult();
  for (const ref of [result.context.content_ref, result.context.episode_ref, result.candidates[0]!.content_ref]) {
    ref.source_id = provider.id; ref.source_revision = provider.source_revision;
  }
  h.reply("match_playback", result); await switching;
  assert.equal(c.state.playback.context!.metadata_video_id, input.video_id);
  assert.equal(c.state.settings.last_playback_source_key, provider.id);
  const changingMetadata = c.selectSource(metadata.id, "metadata"); await changingMetadata;
  assert.equal(c.state.route.name, "home"); assert.equal(c.state.playback.context, null);
  assert.equal(c.state.details.data, null); assert.equal(c.state.home.data, null);
  assert.equal(c.state.settings.last_metadata_config_id, (metadata.origin as any).config_id);
  assert.equal(c.state.settings.last_playback_source_key, provider.id);
  const failed = harness(copy(initial)); failed.failSave(true);
  await assert.rejects(failed.controller.selectSource(provider.id, "playback"), /disk full/);
  assert.deepEqual(copy(failed.controller.state.playback.session), initial.playback.session);
  assert.deepEqual(copy(failed.controller.state.selected_playback_source), initial.selected_playback_source);
});

test("R31/R36/REQ-T13: not-found/confirmation results stay out of playback and late open closes only captured Session", async () => {
  for (const status of ["not_found", "needs_confirmation"] as const) {
    const h = harness(idle()), result = matchResult();
    const task = h.controller.match(result.video_id);
    h.reply("match_playback", { ...result, status, context: null, candidates: status === "not_found" ? [] : result.candidates });
    assert.equal((await task)!.status, status);
    assert.equal(h.controller.state.route.name, "home"); assert.equal(h.controller.state.playback.context, null);
  }
  const h = harness(), c = h.controller, original = copy(c.state.playback.session!);
  const opening = c.openStream(original.stream_id); await tick();
  c.navigate({ name: "home" });
  h.reply("open_playback", { ...original, session_id: "late-only" }); await opening; await tick();
  assert.equal(c.state.playback.session, null); assert.equal(c.state.route.name, "home");
  assert.ok(h.calls.some(call => call.command === "close_playback" && call.input.session_id === "late-only"));
});

test("U07/R01/R31/R36/R40/REQ-T13: open Session binding, captured cleanup, line switching and stale EOS", async () => {
  const h = harness(), c = h.controller, current = copy(c.state.playback.session!);
  const opening = c.openStream(current.stream_id); await tick();
  h.reply("open_playback", current); await opening; assertPlayback(c.state);
  trace("opened-session", c.state);
  assert.ok(h.calls.findIndex(call => call.command === "close_playback") < h.calls.findIndex(call => call.command === "open_playback"));
  assert.equal(h.calls.filter(call => call.command === "release_resolution").length, 0);
  const epoch = c.state.playback.playback_epoch;
  const event = { type: "ended" as const, playback_epoch: epoch - 1, engine_epoch: c.state.playback.engine_epoch, session_id: current.session_id };
  assert.equal(c.playerEvent(event), false); assert.notEqual(c.state.playback.phase, "ended");
  assert.equal(c.playerEvent({ ...event, playback_epoch: epoch }), true); assert.equal(c.state.playback.phase, "ended");
  assert.equal(c.playerEvent({ type: "position", playback_epoch: epoch, engine_epoch: c.state.playback.engine_epoch, session_id: "foreign", position_ms: 123 }), false);
  c.navigate({ name: "history" }); await tick();
  const closed = h.calls.filter(call => call.command === "close_playback");
  assert.equal(closed.at(-1)!.input.session_id, current.session_id);
  assert.equal(h.calls.filter(call => call.command === "release_resolution").at(-1)!.input.resolution_id, current.resolution_id);
  assert.equal(c.state.playback.context, null); assert.ok(h.stops > 0);
});

test("R31/R36/REQ-T13: wrong Resolution and Session responses fail before publication", async () => {
  for (const field of ["resolution_id", "source_revision", "episode_ref", "stream_kind"] as const) {
    const h = harness(), c = h.controller;
    const task = c.openStream(c.state.playback.session!.stream_id); await tick();
    const session = copy(golden().playback.session!);
    if (field === "episode_ref") session.episode_ref.data = "wrong";
    else if (field === "stream_kind") session.stream_kind = "dash";
    else session[field] = "wrong";
    h.reply("open_playback", session); await task; await tick();
    assert.equal(c.state.playback.phase, "failed"); assert.equal(c.state.playback.session, null);
  }
  const h = harness();
  const task = h.controller.resolve(), resolution = copy(golden().playback.resolution!); await tick();
  resolution.episode_ref.data = "wrong"; h.reply("resolve_episode", resolution); await task;
  assert.equal(h.controller.state.playback.phase, "failed");
});

test("R01/R31/R36/REQ-T13: host restart invalidates all resources while guest restart preserves HTTP", async () => {
  const h = harness(), c = h.controller;
  const runtime = copy(c.state.runtime); runtime.engine_epoch = crypto.randomUUID();
  c.applyRuntime(runtime); await tick();
  assert.equal(c.state.playback.session, null); assert.equal(c.state.home.data, null); assert.equal(c.state.details.data, null);
  assert.equal(c.state.sources.data, null); assert.equal(c.state.selected_metadata_source, null);
  const run = c.refreshRuntime(), next = copy(runtime); next.health!.status = "failed";
  h.reply("runtime_status", next, next.engine_epoch); await assert.rejects(run, /healthy/);
  for (const kind of ["maccms", "cloudstream"] as const) {
    const initial = golden();
    const source = initial.sources.data!.providers.find(source => source.id === initial.selected_playback_source!.source_id)!;
    (source as any).kind = kind;
    if (kind === "maccms") (source as any).origin = { config_id: crypto.randomUUID(), base_url: "https://example.com/api" };
    initial.runtime.cloudstream.engine_epoch = crypto.randomUUID();
    const other = harness(initial), before = copy(other.controller.state.playback.session);
    const changed = copy(initial.runtime); changed.cloudstream.engine_epoch = crypto.randomUUID();
    other.controller.applyRuntime(changed);
    if (kind === "maccms") { assert.deepEqual(copy(other.controller.state.playback.session), before); assertPlayback(other.controller.state); }
    else assert.equal(other.controller.state.playback.session, null);
    assert.equal(other.controller.state.runtime.engine_epoch, initial.runtime.engine_epoch);
    assert.ok(other.controller.state.sources.data!.providers.some(source => source.kind === "stremio"));
  }
});

test("R01/R31/R36/R62/R67/REQ-T13: Settings queue preserves independent source and display preferences on failure", async () => {
  const h = harness(), c = h.controller, session = copy(c.state.playback.session);
  const oldKey = c.state.settings.last_playback_source_key;
  await Promise.all([c.updateSettings(settings => ({ ...settings, ui_locale: "zh-CN" })),
    c.updateSettings(settings => ({ ...settings, color_palette: "ocean" }))]);
  assert.equal(c.state.settings.ui_locale, "zh-CN"); assert.equal(c.state.settings.color_palette, "ocean");
  assert.equal(c.state.settings.last_playback_source_key, oldKey); assert.deepEqual(copy(c.state.playback.session), session);
  trace("locale-palette-preserved-session", c.state);
  h.failSave(true);
  await assert.rejects(c.updateSettings(settings => ({ ...settings, ui_locale: "en" })), /disk full/);
  assert.equal(c.state.settings.ui_locale, "zh-CN");
  const metadata = c.state.selected_metadata_source, playback = c.state.selected_playback_source;
  await assert.rejects(c.selectSource(metadata!.source_id, "playback"));
  await assert.rejects(c.selectSource(playback!.source_id, "metadata"));
  assert.deepEqual(c.state.selected_metadata_source, metadata); assert.deepEqual(c.state.selected_playback_source, playback);
});

test("R01/R31/R36/REQ-T13: source publication restores both preferences and rejects old registry revisions", async () => {
  const h = harness(idle()), c = h.controller;
  const initial = golden(), providers = copy(initial.sources.data!);
  const metadata = providers.providers.find(source => source.kind === "stremio")!;
  const playback = providers.providers.find(source => source.kind !== "stremio")!;
  c.state.settings.last_metadata_config_id = (metadata.origin as any).config_id;
  c.state.settings.last_playback_source_key = playback.id;
  const task = c.loadSources(); h.reply("get_sources", providers); await task;
  assert.equal(c.state.selected_metadata_source!.source_id, metadata.id);
  assert.equal(c.state.selected_playback_source!.source_id, playback.id);
  const loading = c.loadDetails(ref("old"));
  const changed = copy(providers); changed.providers.find(source => source.id === metadata.id)!.source_revision = "new-revision";
  const refresh = c.loadSources(); h.reply("get_sources", changed); await refresh;
  h.reply("get_details", details("old")); await loading;
  assert.equal(c.state.details.data, null); assert.equal(c.state.selected_metadata_source!.source_revision, "new-revision");
  assert.equal(c.state.settings.last_playback_source_key, playback.id);
});
