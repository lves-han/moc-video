import test from "node:test";
import assert from "node:assert/strict";
import { Api, ApiFailure, SettingsQueue, UnknownCommit } from "../src/api.ts";
import type { Invoke } from "../src/api.ts";
import type { Settings } from "../src/models.ts";

const epoch = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const initial = (): Settings => ({ schema_version: 2, last_source_key: null, volume: 0.5, muted: false, auto_next: true, window_bounds: null, source_configs: [] });
const success = (request_id: string, data: unknown) => ({ request_id, engine_epoch: epoch, data });
const failure = (request_id: string) => ({ request_id, engine_epoch: epoch, error: { code: "INTERNAL_ERROR", message: "disk error", retryable: false } });
const deferred = <T>() => {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
};

test("T05 settings actions serialize against the latest confirmed object and drain at exit", async () => {
  let stored = initial();
  const first = deferred<void>();
  const entered = deferred<void>();
  const submitted: Settings[] = [];
  let active = 0;
  const api = new Api(async (command, args) => {
    if (command === "get_settings") return success(args.request_id, structuredClone(stored));
    assert.equal(command, "save_settings");
    active++; assert.equal(active, 1);
    submitted.push(structuredClone(args.input as Settings));
    if (submitted.length === 1) { entered.resolve(); await first.promise; }
    stored = structuredClone(args.input as Settings);
    active--;
    return success(args.request_id, structuredClone(stored));
  });
  const queue = new SettingsQueue(api);
  const volume = queue.update(s => ({ ...s, volume: 0.2 }));
  const mute = queue.update(s => ({ ...s, muted: true }));
  await entered.promise;
  assert.equal(submitted.length, 1);
  const closing = queue.close();
  await assert.rejects(queue.update(s => s), /closing/);
  first.resolve();
  await Promise.all([volume, mute, closing]);
  assert.equal(submitted[1].volume, 0.2);
  assert.equal(submitted[1].muted, true);
  assert.deepEqual(queue.value, stored);
  const copy = queue.value!; copy.volume = 1;
  assert.equal(queue.value!.volume, 0.2);
});

test("T05 failed settings retain confirmation and the next action uses it", async () => {
  let stored = initial(); let saves = 0; const errors: unknown[] = [];
  const queue = new SettingsQueue(new Api(async (command, args) => {
    if (command === "get_settings") return success(args.request_id, stored);
    saves++;
    if (saves === 1) return failure(args.request_id);
    stored = args.input as Settings;
    return success(args.request_id, stored);
  }), error => errors.push(error));
  const rejected = queue.update(s => ({ ...s, volume: 0.9 }));
  const next = queue.update(s => ({ ...s, auto_next: false }));
  await assert.rejects(rejected, ApiFailure);
  const confirmed = await next;
  assert.equal(confirmed.volume, 0.5);
  assert.equal(confirmed.auto_next, false);
  assert.equal(errors.length, 1);
});

test("T05 unknown commit cancels then rereads before processing another edit", async () => {
  let stored = initial(); const order: string[] = []; let saves = 0;
  const queue = new SettingsQueue(new Api(async (command, args) => {
    order.push(command);
    if (command === "get_settings") return success(args.request_id, structuredClone(stored));
    if (command === "cancel_request") return success(args.request_id, { target_request_id: (args.input as {target_request_id: string}).target_request_id, cancelled: false });
    saves++; stored = structuredClone(args.input as Settings);
    if (saves === 1) throw new Error("connection closed after rename");
    return success(args.request_id, stored);
  }));
  const unknown = queue.update(s => ({ ...s, volume: 0.8 }));
  const next = queue.update(s => ({ ...s, muted: true }));
  await assert.rejects(unknown, UnknownCommit);
  assert.deepEqual(await next, { ...initial(), volume: 0.8, muted: true });
  assert.deepEqual(order, ["get_settings", "save_settings", "cancel_request", "get_settings", "save_settings"]);
  assert.equal(queue.paused, false);
});

test("T05 failed reread holds the queue until explicit successful recovery", async () => {
  let stored = initial(); let unknown = true; let readFailure = true;
  const recoveryFailed = deferred<void>(); let saves = 0;
  const queue = new SettingsQueue(new Api(async (command, args) => {
    if (command === "get_settings") {
      if (saves && readFailure) { recoveryFailed.resolve(); return failure(args.request_id); }
      return success(args.request_id, structuredClone(stored));
    }
    if (command === "cancel_request") return success(args.request_id, { target_request_id: (args.input as {target_request_id: string}).target_request_id, cancelled: false });
    saves++;
    stored = structuredClone(args.input as Settings);
    if (unknown) { unknown = false; throw new Error("transport"); }
    return success(args.request_id, stored);
  }));
  const first = queue.update(s => ({ ...s, volume: 0.7 }));
  const pending = queue.update(s => ({ ...s, muted: true }));
  await assert.rejects(first, UnknownCommit);
  await recoveryFailed.promise;
  await assert.rejects(queue.close(), /unresolved/);
  assert.equal(saves, 1); assert.equal(queue.paused, true);
  assert.equal(queue.value!.volume, 0.5);
  readFailure = false;
  await queue.recover();
  assert.equal((await pending).volume, 0.7);
  await queue.close();
  assert.equal(stored.muted, true);
});

test("T05 one initialization read serves concurrent initialization and queued edits", async () => {
  const read = deferred<unknown>(); let reads = 0;
  let request = "";
  const queue = new SettingsQueue(new Api(async (command, args) => {
    if (command === "get_settings") { request = args.request_id; reads++; return read.promise; }
    return success(args.request_id, args.input);
  }));
  const ready = queue.initialize();
  const edit = queue.update(s => ({ ...s, volume: 0.1 }));
  read.resolve(success(request, initial()));
  await ready; await edit;
  assert.equal(reads, 1);
  assert.equal(queue.value!.volume, 0.1);
});

test("T05 malformed envelope and empty ready responses reject", async () => {
  const invalid = [
    (id: string) => ({ request_id: id, engine_epoch: epoch, data: { phase: "ready", engine_epoch: epoch, health: null } }),
    (id: string) => ({ request_id: id, engine_epoch: epoch, data: { phase: "ready", engine_epoch: epoch, health: { status: "ready", media_gateway_ready: false } } }),
    (id: string) => ({ request_id: id, engine_epoch: epoch, data: {}, error: { code: "BAD_REQUEST", message: "both", retryable: false } }),
    (id: string) => ({ request_id: id, engine_epoch: "guest-epoch", data: {} }),
    (_: string) => ({ request_id: crypto.randomUUID(), engine_epoch: epoch, data: {} }),
    (id: string) => ({ request_id: id, engine_epoch: epoch, data: {}, unexpected: true }),
  ];
  for (const response of invalid) {
    const api = new Api(async (command, args) => command === "cancel_request"
      ? success(args.request_id, { cancelled: false, target_request_id: (args.input as {target_request_id: string}).target_request_id })
      : response(args.request_id));
    await assert.rejects(api.call("runtime_prepare", {}), UnknownCommit);
  }
});

test("T05 fixed command deadlines include transport grace and cancel with a fresh UUID", async () => {
  const scheduled: Array<{ callback: () => void; ms: number }> = [];
  const calls: Array<{ command: string; request_id: string; input: unknown }> = [];
  const timer = { set: (callback: () => void, ms: number) => { scheduled.push({ callback, ms }); return scheduled.length; }, clear: (_: unknown) => {} };
  const transport: Invoke = async (command, args) => {
    calls.push({ command, ...args });
    if (command === "cancel_request") return success(args.request_id, { cancelled: true, target_request_id: (args.input as {target_request_id: string}).target_request_id });
    return new Promise(() => {});
  };
  const api = new Api(transport, timer);
  const pending = api.call("runtime_prepare", {});
  assert.equal(scheduled[0].ms, 122_000);
  scheduled[0].callback();
  await assert.rejects(pending, UnknownCommit);
  assert.equal(calls[1].command, "cancel_request");
  assert.notEqual(calls[0].request_id, calls[1].request_id);
  assert.equal((calls[1].input as {target_request_id: string}).target_request_id, calls[0].request_id);
  assert.equal(api.budget("get_resource_status", {kind: "image", target_id: "image"}), 3_000);
  assert.equal(api.budget("sync_catalog", {kind: "maccms", config_id: epoch}), 45_000);
});

test("T05 a late timed-out settings response has no queue side effects", async () => {
  let stored = initial(); const old = deferred<unknown>(); let oldId = ""; let saves = 0;
  const saveStarted = deferred<void>();
  const scheduled: Array<() => void> = [];
  const timer = { set: (callback: () => void, _ms: number) => { scheduled.push(callback); return scheduled.length; }, clear: (_: unknown) => {} };
  const api = new Api(async (command, args) => {
    if (command === "get_settings") return success(args.request_id, structuredClone(stored));
    if (command === "cancel_request") return success(args.request_id, {cancelled: true, target_request_id: oldId});
    saves++;
    if (saves === 1) { oldId = args.request_id; saveStarted.resolve(); return old.promise; }
    stored = structuredClone(args.input as Settings); return success(args.request_id, stored);
  }, timer);
  const queue = new SettingsQueue(api);
  const first = queue.update(s => ({ ...s, volume: 0.9 }));
  const next = queue.update(s => ({ ...s, muted: true }));
  await saveStarted.promise;
  scheduled[1]();
  await assert.rejects(first, UnknownCommit);
  await next;
  old.resolve(success(oldId, { ...initial(), volume: 0.9 }));
  await Promise.resolve();
  assert.equal(queue.value!.volume, 0.5);
  assert.equal(queue.value!.muted, true);
});
