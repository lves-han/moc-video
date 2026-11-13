import test from "node:test";
import assert from "node:assert/strict";
import { AppController, createAppState, defaultSettings, TMDB_CONFIG_ID } from "../src/state.ts";

test("connecting the service syncs the service catalog without importing old add-ons", async t => {
  const configId = "dddddddd-dddd-4ddd-8ddd-dddddddddddd";
  const sourceId = "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee";
  const storage = new Map<string, string>();
  storage.set("moc-video.source-checks.v1", JSON.stringify({ [configId]: Date.now(), [TMDB_CONFIG_ID]: Date.now(), failures: [configId, TMDB_CONFIG_ID] }));
  const oldStorage = Object.getOwnPropertyDescriptor(globalThis, "localStorage");
  const oldWindow = Object.getOwnPropertyDescriptor(globalThis, "window");
  Object.defineProperty(globalThis, "localStorage", { configurable: true, value: { getItem: (key: string) => storage.get(key) ?? null, setItem: (key: string, value: string) => storage.set(key, value) } });
  const groups: unknown[] = [];
  Object.defineProperty(globalThis, "window", { configurable: true, value: { __TAURI__: { core: { invoke: async () => ({ groups }) } } } });
  t.after(() => { if (oldStorage) Object.defineProperty(globalThis, "localStorage", oldStorage); else Reflect.deleteProperty(globalThis, "localStorage"); if (oldWindow) Object.defineProperty(globalThis, "window", oldWindow); else Reflect.deleteProperty(globalThis, "window"); });
  const state = createAppState();
  state.runtime.engine_epoch = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
  state.settings = { ...defaultSettings(), source_configs: [{ kind: "stremio", config_id: configId, name: "Addon", enabled: true, manifest_urls: ["https://addon.test/manifest.json"] }] };
  let saved = JSON.parse(JSON.stringify(state.settings)) as typeof state.settings;
  const settings = { update: async (change: (value: typeof saved) => typeof saved) => saved = change(saved) };
  const calls: string[] = [];
  const ready = new Set<string>();
  const providers = () => [configId, TMDB_CONFIG_ID].map(id => ({ id, source_revision: "revision", kind: "stremio", name: id, language: "en", content_types: ["movie"], runtime_status: ready.has(id) ? "ready" : "configured", capabilities: { home: true, search: true, details: true, resolve: false, sequential_home: false }, origin: { config_id: id, manifest_urls: ["https://addon.test/manifest.json"] } }));
  const api = { cancel: async () => {}, call: async (command: string, input: { config_id?: string }) => {
    if (command === "get_sources") return { providers: providers(), plugins: [] };
    if (command === "sync_catalog") { calls.push(input.config_id!); ready.add(input.config_id!); return {}; }
    throw new Error(`Unexpected ${command}`);
  } };
  const app = new AppController(api as never, settings as never, state, null);
  await app.connectPlaybackService("http://127.0.0.1:8787", "fixture-token");
  assert.deepEqual(calls, [TMDB_CONFIG_ID]);
  assert.equal(state.settings.playback_service?.groups.length, 0);
  assert.equal(state.settings.source_configs[0]?.config_id, configId);
});
