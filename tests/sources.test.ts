import assert from "node:assert/strict";
import test from "node:test";
import { Api } from "../src/api.ts";
import { redactDiagnostics, validateSourceDraft } from "../src/state.ts";

const draft = (kind: "maccms" | "stremio", urls: string, name = "Name") => ({ kind, name, urls });

test("R58/REQ-T17: drafts enforce anonymous http URLs and kind ownership", () => {
  assert.deepEqual(validateSourceDraft(draft("maccms", "https://a.test/api.php"), 0), []);
  assert.deepEqual(validateSourceDraft(draft("stremio", "https://a.test/x/manifest.json\nhttps://b.test/manifest.json?lang=en"), 0), []);
  for (const bad of ["https://user:pass@a.test/api", "https://a.test/api#frag", "ftp://a.test/api", "not a url", ""]) assert.ok(validateSourceDraft(draft("maccms", bad), 0).length, bad);
  assert.ok(validateSourceDraft(draft("stremio", "https://a.test/catalog.json"), 0).length);
  assert.ok(validateSourceDraft(draft("maccms", "https://a.test/a\nhttps://a.test/b"), 0).length);
  assert.ok(validateSourceDraft(draft("maccms", "https://a.test/a", "  "), 0).length);
});

test("R58/REQ-T17: drafts enforce 32 configurations and 16 manifests", () => {
  assert.deepEqual(validateSourceDraft(draft("maccms", "https://a.test/a"), 31), []);
  assert.ok(validateSourceDraft(draft("maccms", "https://a.test/a"), 32).length);
  const urls = (count: number) => Array.from({ length: count }, (_, index) => `https://a.test/${index}/manifest.json`).join("\n");
  assert.deepEqual(validateSourceDraft(draft("stremio", urls(16)), 0), []);
  assert.ok(validateSourceDraft(draft("stremio", urls(17)), 0).length);
});

test("R24/REQ-T17: diagnostics and validation messages drop URL secrets", () => {
  const text = redactDiagnostics("get https://user:pw@a.test/api.php?token=SECRET#frag failed cap=SECRET2 Bearer: SECRET3 authorization=SECRET4");
  assert.ok(text.includes("https://a.test/api.php"));
  for (const secret of ["SECRET", "user:pw", "#frag"]) assert.ok(!text.includes(secret), text);
  const error = validateSourceDraft(draft("maccms", "https://user:hidden@a.test/api?token=HIDDEN"), 0).join();
  assert.ok(!error.includes("hidden") && !error.includes("HIDDEN"), error);
});

test("R24/REQ-T17: the last request per source follows the newest request, even when it finishes first", async () => {
  const pending: ((value: unknown) => void)[] = [], host = "11111111-1111-4111-8111-111111111111";
  const api = new Api(((_: string, args: { request_id: string }) => new Promise(resolve => pending.push(outcome => resolve(outcome === "ok"
    ? { request_id: args.request_id, engine_epoch: host, data: { page: {}, images: {} } }
    : { request_id: args.request_id, engine_epoch: host, error: { code: "UPSTREAM_FAILED", message: "boom", retryable: true } })))) as never);
  const call = () => api.call("get_home", { source_id: "s", source_revision: "1", section_ref: null, page: 1 } as never).catch(() => null);
  let first = call(); pending.shift()!("fail"); await first;
  assert.deepEqual([api.sourceResults.s!.ok, api.sourceResults.s!.message], [false, "boom"]);
  first = call(); pending.shift()!("ok"); await first;
  assert.equal(api.sourceResults.s!.ok, true);
  const older = call(), newer = call();
  pending[1]!("ok"); await newer; pending[0]!("fail"); await older;
  assert.equal(api.sourceResults.s!.ok, true);
});
