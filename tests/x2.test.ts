import assert from "node:assert/strict";
import test from "node:test";
import { createAppState, liveAvailable } from "../src/state.ts";

const live = (enabled: boolean) => ({ config_id: "6a1d2c3b-4e5f-4a6b-8c7d-9e0f1a2b3c4d", name: "Live", url: "https://live.test/list.m3u", enabled });

test("REQ-X02/live-conditional: the Live menu needs at least one enabled live list", () => {
  const base = createAppState().settings;
  assert.equal(liveAvailable(base), false, "no live_configs field");
  assert.equal(liveAvailable({ ...base, live_configs: [] }), false);
  assert.equal(liveAvailable({ ...base, live_configs: [live(false)] }), false);
  assert.equal(liveAvailable({ ...base, live_configs: [live(false), { ...live(true), config_id: "7a1d2c3b-4e5f-4a6b-8c7d-9e0f1a2b3c4d" }] }), true);
});
