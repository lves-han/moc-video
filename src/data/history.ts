import { api, ApiFailure, UnknownCommit, type Api } from '../api.ts';
import type { AppSource, WatchHistoryRecord } from '../models.ts';
import { parseHistoryEntry, type HistoryEntry } from './history-model.ts';

// About 5s between periodic snapshots; pause and end write immediately, as does an explicit seek (SPEC 11.4).
export const HISTORY_INTERVAL_MS = 5000;
export function historyWriteDue(phase: string, lastWrite: number, now: number) {
  return ["paused", "ended"].includes(phase) || now - lastWrite >= HISTORY_INTERVAL_MS;
}

// Browser interface preview only; the native app persists through Rust/SQLite below.
const entries = new Map<string, HistoryEntry>();
async function save(entry: HistoryEntry): Promise<void> {
  const valid = parseHistoryEntry(entry);
  const previous = entries.get(valid.content_id);
  if (!previous || valid.updated_at >= previous.updated_at) entries.set(valid.content_id, valid);
}
const preview = {
  async list(): Promise<HistoryEntry[]> {
    return [...entries.values()]
      .sort((a,b) => b.updated_at-a.updated_at || a.content_id.localeCompare(b.content_id))
      .map(entry => ({ ...entry }));
  },
  async get(id: string): Promise<HistoryEntry | null> {
    const entry = entries.get(id);
    return entry ? { ...entry } : null;
  },
  save,
  saveOnExit: save,
  async remove(id: string): Promise<void> { entries.delete(id); },
  async clear(): Promise<void> { entries.clear(); },
};

export function entryFromRecord(record: WatchHistoryRecord): HistoryEntry {
  const episode = record.episode_number ?? 1;
  return { content_id: record.content_id, title: record.title, poster: "/mark.svg", kind: record.kind === "other" ? "series" : record.kind,
    episode, episode_total: episode, source_id: record.source_id, source_name: record.source_name, position_ms: record.position_ms,
    duration_ms: record.duration_ms, completed: record.completed, updated_at: record.updated_at, record };
}

// Rebuild the metadata reference from durable identity; image access stays in memory.
export async function historyPoster(record: WatchHistoryRecord, sources: AppSource[], call: Api["call"]): Promise<string | null> {
  const identity = record.metadata_identity;
  const source = sources.find(item => item.kind === "stremio" && item.runtime_status === "ready" && item.origin.config_id === identity.config_id);
  if (!source) return null;
  const data = await call("get_details", { ref: { source_id: source.id, source_revision: source.source_revision,
    data: JSON.stringify(["stremio", identity.addon_key, identity.type, identity.meta_id]) } });
  if (data.details.id !== record.content_id || !data.details.poster) return null;
  return data.images[data.details.poster.resource_id]?.url ?? null;
}

// The Host refused for good: the Session ended or a newer change won, so retrying cannot help.
export function historyRefused(error: unknown) {
  return error instanceof ApiFailure && ["SESSION_EXPIRED", "CANCELLED"].includes(error.error.code);
}

// One queue gives snapshots, deletes and clears their increasing write sequence (C16).
export function createNativeHistory(call: Api["call"]) {
  // The upper bound of every sequence handed out: reads never lower it, so a new snapshot cannot reuse a failed one's number.
  let sequence = 0, known = false, tail: Promise<unknown> = Promise.resolve();
  let failed: { entry: HistoryEntry; sequence: number } | null = null;
  const read = async () => { const data = await call("list_history", {}); sequence = Math.max(sequence, data.write_sequence); known = true; return data.entries; };
  function enqueue<T>(work: (next: () => Promise<number>) => Promise<T>): Promise<T> {
    const run = tail.then(() => work(async () => { if (!known) await read(); return ++sequence; }))
      .catch(error => { if (error instanceof UnknownCommit) known = false; throw error; });
    tail = run.catch(() => {});
    return run;
  }
  return {
    list: () => enqueue(async () => (await read()).map(entryFromRecord)),
    get: (id: string) => enqueue(async () => { const row = (await read()).find(item => item.content_id === id); return row ? entryFromRecord(row) : null; }),
    // A retry of the same snapshot object reuses its sequence so the Host can match the payload.
    save: (entry: HistoryEntry) => enqueue(async next => {
      const capture = entry.capture;
      if (!capture) throw new TypeError("A native history snapshot needs its playback capture");
      const write_sequence = failed?.entry === entry ? failed.sequence : await next();
      try {
        await call("save_history", { record: entry.record!, ...capture, write_sequence });
        failed = null;
      } catch (error) {
        // A refused snapshot must not take the place of one the Host still holds for retry.
        if (!historyRefused(error)) failed = { entry, sequence: write_sequence };
        else if (failed?.entry === entry) failed = null;
        throw error;
      }
    }),
    remove: (id: string) => enqueue(async next => { await call("remove_history", { content_id: id, write_sequence: await next() }); failed = null; }),
    clear: () => enqueue(async next => { await call("clear_history", { write_sequence: await next() }); failed = null; }),
    // Settles once everything already queued has finished.
    drain: () => tail,
  };
}
const native = createNativeHistory((command, input, ...rest) => api.call(command, input, ...rest));
const isNative = Boolean((globalThis as { __TAURI__?: unknown }).__TAURI__);

export const historyRepository = {
  list: () => isNative ? native.list() : preview.list(),
  get: (id: string) => isNative ? native.get(id) : preview.get(id),
  save: (entry: HistoryEntry) => isNative ? native.save(entry) : preview.save(entry),
  saveOnExit: (entry: HistoryEntry) => isNative ? native.save(entry) : preview.saveOnExit(entry),
  remove: (id: string) => isNative ? native.remove(id) : preview.remove(id),
  clear: () => isNative ? native.clear() : preview.clear(),
  drain: () => isNative ? native.drain() : Promise.resolve(),
};
