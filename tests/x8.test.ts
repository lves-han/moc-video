import assert from "node:assert/strict";
import test from "node:test";
import { approximateResults, normalizeTitle } from "../src/data/search-match.ts";

test("REQ-X08/approximate-hint: results are approximate only when no title contains the typed text", () => {
  assert.equal(normalizeTitle("复仇者联盟　４：终局之战"), "复仇者联盟4终局之战");
  assert.equal(approximateResults("复仇者联盟 4", ["复仇者联盟4：终局之战"]), false, "spacing differences are still an exact match");
  assert.equal(approximateResults("庆余年 第二季", ["庆余年"]), true);
  assert.equal(approximateResults("Inception", ["盗梦空间"]), true);
  assert.equal(approximateResults("流浪地球", ["流浪地球2", "流浪地球"]), false);
  assert.equal(approximateResults("anything", []), false, "no results shows the empty state instead");
});
