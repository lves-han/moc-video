import assert from "node:assert/strict";
import test from "node:test";
import { Api } from "../src/api.ts";
import { readFileSync } from "node:fs";
import { describeError } from "../src/api.ts";
import { SOURCE_PRESETS } from "../src/data/sourcePresets.ts";
import { locale } from "../src/i18n.ts";
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

test('cancelled source requests do not appear as failures or let older failures replace their state', async () => {
  const pending: ((value: unknown) => void)[] = [], host = '11111111-1111-4111-8111-111111111111';
  const api = new Api(((_: string, args: { request_id: string }) => new Promise(resolve => pending.push(code => resolve({
    request_id: args.request_id, engine_epoch: host,
    ...(code === 'ok' ? { data: { page: {}, images: {} } } : { error: { code, message: String(code), retryable: false } }),
  })))) as never);
  const call = () => api.call('get_home', { source_id: 's', source_revision: '1', section_ref: null, page: 1 } as never).catch(() => null);
  const success = call(); pending.shift()!('ok'); await success;
  const older = call(), cancelled = call();
  pending[1]!('CANCELLED'); await cancelled;
  pending[0]!('UPSTREAM_FAILED'); await older;
  assert.equal(api.sourceResults.s!.ok, true);
  assert.ok(!api.failures.some(row => row.code === 'CANCELLED'));
});

test("U02/R03/REQ-N02: presets are exactly the supported sources of the compatibility report", () => {
  const report = JSON.parse(readFileSync("docs/compatibility-report.json", "utf8"));
  assert.deepEqual(SOURCE_PRESETS.map(preset => preset.name), report.supported_sources);
  for (const preset of SOURCE_PRESETS) {
    assert.equal(report.sources.find((source: { name: string }) => source.name === preset.name).api_url, preset.api_url);
    assert.deepEqual(validateSourceDraft({ kind: "maccms", name: preset.name, urls: preset.api_url }, 0), []);
  }
  assert.ok(validateSourceDraft({ kind: "maccms", name: "x", urls: SOURCE_PRESETS[0].api_url }, 32).length, "the limit also stops presets");
});

test("R13/R58/REQ-N02: validation and access failures read in Chinese and name their cause", () => {
  locale.value = "zh-CN";
  try {
    for (const message of [...validateSourceDraft({ kind: "maccms", name: " ", urls: "" }, 32), ...validateSourceDraft({ kind: "stremio", name: "x", urls: "not a url\nhttps://u:p@a.test/x\nhttps://a.test/x" }, 0)]) {
      assert.match(message, /[\u4e00-\u9fff]/, message);
      assert.ok(!message.includes("u:p@"), message);
    }
    const shown = (details: Record<string, unknown>) => describeError({ code: "UPSTREAM_FAILED", message: "raw", retryable: false, details } as never);
    assert.match(shown({ reason: "access_control", signal: "status", http_status: 403 }), /拒绝访问.*403.*已停止请求/);
    assert.match(shown({ reason: "access_control", signal: "challenge", http_status: 200 }), /人机验证.*已停止请求/);
    assert.match(shown({ reason: "tls_certificate" }), /证书校验失败/);
    assert.equal(shown({ http_status: 500 }), "raw");
  } finally { locale.value = "en"; }
});

test("R24/REQ-T18: a sync result reaches the source it published once sources reload", async () => {
  let reply: (value: unknown) => void = () => {};
  const api = new Api((_command, args) => new Promise(resolve => { reply = data => resolve({ request_id: args.request_id, engine_epoch: "11111111-1111-4111-8111-111111111111", ...(data as object) }); }));
  const sync = api.call("sync_catalog", { kind: "maccms", config_id: "c" } as never);
  await new Promise(resolve => setTimeout(resolve));
  reply({ data: { changed: true } });
  await sync;
  assert.equal(api.sourceResults.c?.ok, true);
  const load = api.call("get_sources", {} as never);
  await new Promise(resolve => setTimeout(resolve));
  reply({ data: { providers: [{ id: "s", kind: "maccms", origin: { config_id: "c" } }], plugins: [] } });
  await load;
  assert.equal(api.sourceResults.c, undefined);
  assert.equal(api.sourceResults.s?.ok, true);
});
