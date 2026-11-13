import assert from "node:assert/strict";
import test from "node:test";
import { AppController, createAppState, defaultSettings, metadataVideoId } from "../src/state.ts";

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

test("R22/REQ-T15: the metadata video id comes from Host's exact video reference", () => {
  const read = (data: string) => metadataVideoId({ ...SOURCE, data });
  assert.equal(read(JSON.stringify(["stremio_video", "addon", "series", "meta", "video:1"])), "video:1");
  for (const data of [JSON.stringify(["maccms", "addon", "series", "meta", "video:1"]), JSON.stringify(["stremio_video", "video:1"]), "not json"]) assert.equal(read(data), "");
});

test("R22/REQ-T15: browser preview episode references keep a usable metadata video id", () => {
  // The shape src/data/preview.ts publishes; the T13 WebKit UI test exercises it end to end.
  assert.equal(metadataVideoId({ ...SOURCE, data: JSON.stringify({ id: "film", video_id: "film:2" }) }), "film:2");
});

test("R22/REQ-T15: an old poster failing while newer details load cannot take over the page", async () => {
  const f = fixture(expired), OTHER = { ...SOURCE, data: "other" }; await f.app.loadDetails(REF);
  const client = (f.app as unknown as { api: { call: (command: string, input: any) => Promise<any> } }).api, call = client.call;
  let release!: () => void; const gate = new Promise<void>(resolve => { release = resolve; });
  client.call = async (command, input) => {
    const response = await call(command, input);
    if (command !== "get_details" || input.ref.data !== "other") return response;
    await gate;
    return { ...response, metadata_ref: { ...metadata, content_ref: OTHER }, details: { ...response.details, ref: OTHER, title: "Other" } };
  };
  const newer = f.app.loadDetails(OTHER);
  await new Promise(resolve => setTimeout(resolve, 0));
  const before = f.count("get_details");
  assert.equal(f.state.details.phase, "loading");
  assert.equal(f.state.details.data, null, "previous title must not appear during the new request");
  await f.app.detailsImageError("a");
  assert.equal(f.count("get_details"), before); assert.equal(f.count("get_resource_status"), 0);
  release(); await newer;
  assert.equal(f.state.details.data!.details.title, "Other");
});

test('TV5: related titles reuse loaded same-source/revision/kind/genre cards and transient poster access', async () => {
  const f = fixture(() => ({}));
  const ref = (source_id = SOURCE.source_id, source_revision = SOURCE.source_revision) => ({ source_id, source_revision, data: 'related' });
  const card = (id: string, overrides: Record<string, unknown> = {}) => ({ id, ref: ref(), title: id, kind: 'series', genres: ['Drama'], poster: { resource_id: id }, ...overrides });
  const cards = [card('accepted'), card('wrong-source', { ref: ref('other') }), card('old-revision', { ref: ref('meta', 'old') }), card('movie', { kind: 'movie' }), card('duplicate')];
  f.state.home.data = { page: { ...SOURCE, sections: [{ items: cards }], failed_sections: [] },
    images: Object.fromEntries(cards.map(card => [card.id, { resource_id: card.id, url: `http://127.0.0.1/${card.id}`, expires_at: 'transient' }])) } as never;
  await f.app.loadDetails(REF);
  assert.equal(f.state.details.phase, 'ready', f.state.details.error?.message);
  assert.deepEqual(f.state.details.data!.details.related.map(card => card.id), ['accepted', 'duplicate']);
  assert.equal(f.state.details.data!.images.accepted!.url, 'http://127.0.0.1/accepted');
  assert.equal(f.state.details.data!.images['wrong-source'], undefined);
  assert.equal(f.count('get_details'), 1, 'reuse must not request another catalog');
});

test('TV5: a different catalog revision supplies no related cards or image access', async () => {
  const f = fixture(() => ({}));
  f.state.home.data = { page: { ...SOURCE, source_revision: 'old', sections: [{ items: [{ id: 'old', ref: REF, kind: 'series' }] }], failed_sections: [] }, images: { old: { url: 'http://127.0.0.1/old' } } } as never;
  await f.app.loadDetails(REF);
  assert.deepEqual(f.state.details.data!.details.related, []);
  assert.equal(f.state.details.data!.images.old, undefined);
});
