import assert from "node:assert/strict";
import test from "node:test";
import { Api } from "../src/api.ts";
import { AppController, createAppState, metadataLocale, TMDB_CONFIG_ID } from "../src/state.ts";
import { SOURCE_PRESETS, unaddedPresets } from "../src/data/sourcePresets.ts";
import type { HttpSourceConfig, Settings } from "../src/models.ts";

const host = "11111111-1111-4111-8111-111111111111";
const tmdb = (language: "follow" | "en" | "zh-CN") => ({ enabled: true, instance_url: "https://tmdb.elfhosted.com", language });

test("REQ-X05/tmdb-language: metadata follows the app unless a TMDB language is chosen", () => {
  const base = createAppState().settings;
  assert.equal(metadataLocale({ ...base, ui_locale: "zh-CN" }), "zh-CN");
  assert.equal(metadataLocale({ ...base, ui_locale: "zh-CN", tmdb: tmdb("follow") }), "zh-CN");
  assert.equal(metadataLocale({ ...base, ui_locale: "zh-CN", tmdb: tmdb("en") }), "en");
  assert.equal(metadataLocale({ ...base, ui_locale: "en", tmdb: tmdb("zh-CN") }), "zh-CN");
});

test("REQ-X06/stremio-mode: saving TMDB persists it, syncs the fixed TMDB source and reloads sources", async () => {
  const state = createAppState(); state.runtime.engine_epoch = host;
  const calls: { command: string; input: any }[] = [];
  const api = new Api(((command: string, args: { request_id: string; input: any }) => {
    calls.push({ command, input: args.input });
    const data = command === "get_sources" ? { providers: [], plugins: [] } : { kind: "stremio", config_id: TMDB_CONFIG_ID, changed: true, sources: { providers: [], plugins: [] }, cloudstream_result: null };
    return Promise.resolve({ request_id: args.request_id, engine_epoch: host, data });
  }) as never);
  const saved: Settings[] = [];
  const queue = { update: async (change: (value: Settings) => Settings) => { const next = change(state.settings); saved.push(next); return next; } };
  const app = new AppController(api, queue as never, state);
  await app.saveTmdb({ enabled: true, instance_url: "https://tmdb.example.test", language: "en" });
  assert.deepEqual(saved.at(-1)!.tmdb, { enabled: true, instance_url: "https://tmdb.example.test", language: "en" });
  assert.deepEqual(calls.map(call => call.command), ["sync_catalog", "get_sources"]);
  assert.deepEqual(calls[0]!.input, { kind: "stremio", config_id: TMDB_CONFIG_ID });
  calls.length = 0;
  await app.saveTmdb({ enabled: false, instance_url: "https://tmdb.example.test", language: "en" });
  assert.deepEqual(calls.map(call => call.command), ["get_sources"], "a disabled TMDB source is not synced");
});

test("REQ-X04/merged-list: presets appear once, and an added preset leaves the unadded list", () => {
  assert.equal(SOURCE_PRESETS.length, 8);
  assert.equal(unaddedPresets([]).length, 8);
  const preset = SOURCE_PRESETS[0]!;
  const added: HttpSourceConfig[] = [{ kind: "maccms", config_id: "33333333-3333-4333-8333-333333333333", name: "Renamed", enabled: false, api_url: preset.api_url }];
  const rest = unaddedPresets(added);
  assert.equal(rest.length, 7);
  assert.ok(!rest.some(row => row.api_url === preset.api_url));
});

test("playback source addresses cannot be duplicated by adding or editing", async () => {
  const state = createAppState();
  state.settings.source_configs = [
    { kind: "maccms", config_id: "33333333-3333-4333-8333-333333333333", name: "A", enabled: false, api_url: "https://example.test/api" },
    { kind: "maccms", config_id: "44444444-4444-4444-8444-444444444444", name: "B", enabled: false, api_url: "https://other.test/api" },
  ];
  const queue = { update: async (change: (value: Settings) => Settings) => change(state.settings) };
  const app = new AppController({} as never, queue as never, state);
  const draft = { kind: "maccms" as const, name: "Copy", urls: "https://EXAMPLE.test/api", enabled: false };
  await assert.rejects(app.saveSourceConfig({ ...draft, config_id: null }), /already added/);
  await assert.rejects(app.saveSourceConfig({ ...draft, config_id: "44444444-4444-4444-8444-444444444444" }), /already added/);
});
