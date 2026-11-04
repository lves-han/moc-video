import assert from "node:assert/strict";
import test from "node:test";
import { HISTORY_INTERVAL_MS, historyWriteDue } from "../src/data/history.ts";
import { AppController, createAppState, metadataVideoId } from "../src/state.ts";

test("R42/REQ-T16: history writes about every 5s and immediately on pause and end", () => {
  assert.equal(HISTORY_INTERVAL_MS, 5000);
  assert.equal(historyWriteDue("playing", 1000, 5999), false);
  assert.equal(historyWriteDue("playing", 1000, 6000), true);
  for (const phase of ["paused", "ended"]) assert.equal(historyWriteDue(phase, 1000, 1001), true);
});

test("R23/REQ-T16: next video follows metadata order across groups and stops at the last", () => {
  const state = createAppState(), ref = (video: string) => ({ source_id: "s", source_revision: "1", data: JSON.stringify(["stremio_video", "a", "series", "m", video]) });
  state.details.data = { images: {}, metadata_ref: null, details: { groups: [{ episodes: [{ ref: ref("v1") }, { ref: ref("v2") }] }, { episodes: [{ ref: ref("v3") }] }] } } as never;
  const app = new AppController({} as never, {} as never, state);
  assert.equal(metadataVideoId(ref("v1")), "v1");
  assert.equal(app.nextVideo("v1"), "v2"); assert.equal(app.nextVideo("v2"), "v3");
  assert.equal(app.nextVideo("v3"), null); assert.equal(app.nextVideo("missing"), null);
});
