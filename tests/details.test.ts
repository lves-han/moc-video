import assert from "node:assert/strict";
import test from "node:test";
import { AppController, createAppState, defaultSettings } from "../src/state.ts";

const HOST = "11111111-1111-4111-8111-111111111111", SOURCE = { source_id: "meta", source_revision: "1" };
const REF = { ...SOURCE, data: "show" };
const metadata = { content_ref: REF, identity: { config_id: "c", addon_key: "a", namespace: "n", type: "series", meta_id: "m", canonical_id: "m" } };
function fixture(status: (target: string) => unknown) {
  const state = createAppState(), calls: { command: string; input: any }[] = [];
  state.runtime.engine_epoch = HOST; state.selected_metadata_source = SOURCE; state.settings = defaultSettings();
  state.sources.data = { providers: [{ id: "meta", source_revision: "1", runtime_status: "ready", kind: "stremio", origin: { config_id: "c" } }], plugins: [] } as never;
  const client = { cancel: async () => {}, call: async (command: string, input: any) => {
    calls.push({ command, input });
    if (command === "get_details") return { metadata_ref: metadata, images: { a: { resource_id: "a", url: "http://x/a" }, b: { resource_id: "b", url: "http://x/b" } },
      details: { ref: REF, title: "Show", groups: [], related: [], tags: [], kind: "series" } };
    return status(input.target_id);
  } };
  return { app: new AppController(client as never, {} as never, state), state, count: (name: string) => calls.filter(call => call.command === name).length };
}
const expired = (id: string) => ({ kind: "image", target_id: id, error: { code: "SESSION_EXPIRED", message: "expired" } });

test("R22/REQ-T15: confirmed poster expiry refreshes the owning details DTO once", async () => {
  const f = fixture(expired); await f.app.loadDetails(REF); const before = f.count("get_details");
  await Promise.all([f.app.detailsImageError("a"), f.app.detailsImageError("a")]);
  assert.equal(f.count("get_resource_status"), 1); assert.equal(f.count("get_details"), before + 1);
  await f.app.detailsImageError("b");
  assert.equal(f.count("get_details"), before + 1); assert.equal(f.app.detailsImage.error, "expired");
});

test("R22/REQ-T15: other poster failures show their error and reloading details clears it", async () => {
  const f = fixture(id => ({ kind: "image", target_id: id, error: { code: "UPSTREAM_FAILED", message: "bad gateway" } }));
  await f.app.loadDetails(REF); const before = f.count("get_details");
  await f.app.detailsImageError("a");
  assert.equal(f.app.detailsImage.error, "bad gateway"); assert.equal(f.count("get_details"), before);
  await f.app.loadDetails(REF); assert.equal(f.app.detailsImage.error, "");
});
