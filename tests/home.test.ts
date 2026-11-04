import assert from "node:assert/strict";
import test from "node:test";
import { AppController, createAppState, defaultSettings } from "../src/state.ts";

const HOST = "11111111-1111-4111-8111-111111111111", SOURCE = { source_id: "meta", source_revision: "1" };
const image = { resource_id: "a", url: "http://x/a", capability: "c", expires_at: 1 };
function fixture(status: (target: string) => unknown) {
  const state = createAppState(), calls: { command: string; input: any }[] = [];
  state.runtime.engine_epoch = HOST; state.selected_metadata_source = SOURCE; state.settings = defaultSettings();
  state.sources.data = { providers: [{ id: "meta", source_revision: "1", runtime_status: "ready", kind: "stremio" }], plugins: [] } as never;
  const home = () => ({ page: { ...SOURCE, sections: [{ id: "s", title: "S", section_ref: "r", index: 0, page: 1, has_next: false, next_page: null, items: ["a", "b"].map(id => ({ id, title: id, kind: "series", ref: { ...SOURCE, data: id }, poster: { resource_id: id } })) }], failed_sections: [] }, images: { a: image, b: { ...image, resource_id: "b" } } });
  const client = { cancel: async () => {}, call: async (command: string, input: any) => {
    calls.push({ command, input });
    if (command === "get_home") return { page: { ...home().page, source_id: SOURCE.source_id, source_revision: SOURCE.source_revision }, images: home().images };
    return status(input.target_id);
  } };
  return { app: new AppController(client as never, {} as never, state), state, calls, count: (name: string) => calls.filter(call => call.command === name).length };
}
const expired = (id: string) => ({ kind: "image", target_id: id, error: { code: "SESSION_EXPIRED", message: "expired" } });

test("R34/R37/REQ-T14: confirmed expiry refreshes page one once and repeated errors do not re-query", async () => {
  const f = fixture(expired); await f.app.loadHome(); const homes = f.count("get_home");
  await Promise.all([f.app.homeImageError("a"), f.app.homeImageError("a")]);
  assert.equal(f.count("get_resource_status"), 1); assert.equal(f.count("get_home"), homes + 1);
  assert.deepEqual([f.calls.at(-1)!.input.page, f.calls.at(-1)!.input.section_ref], [1, null]);
  await f.app.homeImageError("b");
  assert.equal(f.count("get_home"), homes + 1); assert.equal(f.app.homeImage.error, "expired");
});

test("R34/R37/REQ-T14: other image failures show retry state without refreshing; explicit retry clears it", async () => {
  const f = fixture(id => ({ kind: "image", target_id: id, error: { code: "UPSTREAM_FAILED", message: "bad gateway" } }));
  await f.app.loadHome(); const homes = f.count("get_home");
  await f.app.homeImageError("a");
  assert.equal(f.app.homeImage.error, "bad gateway"); assert.equal(f.count("get_home"), homes);
  await f.app.loadHome(); assert.equal(f.app.homeImage.error, "");
});

test("R34/REQ-T14: a late image status and wrong target have no effect on the current page", async () => {
  let release!: (value: unknown) => void;
  const pending = new Promise(resolve => { release = resolve; });
  const f = fixture(() => pending); await f.app.loadHome();
  const late = f.app.homeImageError("a"); await f.app.loadHome(); const homes = f.count("get_home");
  release(expired("a")); await late; assert.equal(f.count("get_home"), homes);
  const g = fixture(() => expired("other")); await g.app.loadHome(); const before = g.count("get_home");
  await g.app.homeImageError("a"); assert.equal(g.count("get_home"), before); assert.match(g.app.homeImage.error, /target differs/);
});

test("R37/REQ-T14: an image outside the current snapshot is not queried", async () => {
  const f = fixture(expired); await f.app.loadHome();
  await f.app.homeImageError("missing"); assert.equal(f.count("get_resource_status"), 0);
});
