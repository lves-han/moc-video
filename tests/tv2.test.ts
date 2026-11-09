import assert from "node:assert/strict";
import test from "node:test";
import { AppController, createAppState, mergeHome } from "../src/state.ts";
import type { Bridge_HomeRequest, HomeViewData } from "../src/models.ts";

const SOURCE = { source_id: "metadata", source_revision: "r1" };
function page(filter: string, number = 1): HomeViewData {
  return { page: { ...SOURCE, failed_sections: [], sections: [{ id: "s", section_ref: "ref", index: 0, title: "S",
    page: number, has_next: true, next_page: number + 1, scanned_count: number * 20, matched_count: number,
    items: [{ id: `${filter}-${number}`, title: filter, kind: "movie", ref: { ...SOURCE, data: filter },
      poster: null, backdrop: { resource_id: "backdrop" } as never }] }] },
    images: { backdrop: { resource_id: "backdrop", url: "http://127.0.0.1/image", expires_at: "2030-01-01T00:00:00Z" } as never,
      unused: {} as never } };
}

test("TV-CATALOG-DTO: home merge retains backdrop access and drops unused images", () => {
  const result = mergeHome(null, page("initial"), { ...SOURCE, page: 1 });
  assert.deepEqual(Object.keys(result.images), ["backdrop"]);
});

test("TV-FILTER: controller sends filters, inherits them for paging and rejects mixed cursors", async () => {
  const state = createAppState();
  state.runtime.engine_epoch = "11111111-1111-4111-8111-111111111111";
  state.selected_metadata_source = SOURCE;
  state.sources.data = { providers: [{ id: SOURCE.source_id, source_revision: SOURCE.source_revision, runtime_status: "ready", kind: "stremio" }], plugins: [] } as never;
  const calls: Bridge_HomeRequest[] = [];
  const api = { cancel: async () => {}, call: async (command: string, input: Bridge_HomeRequest) => {
    assert.equal(command, "get_home"); calls.push(input);
    return page(input.filter?.genre ?? "initial", input.page);
  } };
  const app = new AppController(api as never, {} as never, state);
  await app.loadHome();
  await app.loadHome("ref", 1, { genre: "Drama", year: "2024" });
  assert.deepEqual(calls.at(-1)!.filter, { genre: "Drama", year: "2024" });
  await app.loadHome("ref", 2);
  assert.deepEqual(calls.at(-1)!.filter, { genre: "Drama", year: "2024" });
  assert.equal(state.home.data!.page.sections[0]!.items.length, 2);
  await assert.rejects(app.loadHome("ref", 3, { genre: "Comedy" }), /current section filter/);
  await app.loadHome("ref", 1, null);
  assert.equal(calls.at(-1)!.filter, undefined);
  assert.equal(state.home.data!.page.sections[0]!.items.length, 1);
});

test("TV-FILTER: slower previous filter response is discarded", async () => {
  const state = createAppState();
  state.runtime.engine_epoch = "11111111-1111-4111-8111-111111111111";
  state.selected_metadata_source = SOURCE;
  state.sources.data = { providers: [{ id: SOURCE.source_id, source_revision: SOURCE.source_revision, runtime_status: "ready", kind: "stremio" }], plugins: [] } as never;
  let release!: (data: HomeViewData) => void;
  const old = new Promise<HomeViewData>(resolve => { release = resolve; });
  const api = { cancel: async () => {}, call: async (_command: string, input: Bridge_HomeRequest) =>
    input.filter?.genre === "Drama" ? old : page(input.filter?.genre ?? "initial", input.page) };
  const app = new AppController(api as never, {} as never, state);
  await app.loadHome();
  const pending = app.loadHome("ref", 1, { genre: "Drama" });
  await app.loadHome("ref", 1, { genre: "Comedy" });
  release(page("Drama")); await pending;
  assert.equal(state.home.data!.page.sections[0]!.items[0]!.title, "Comedy");
  await app.loadHome("ref", 2);
  assert.equal(state.home.data!.page.sections[0]!.items[1]!.title, "Comedy");
});
