import { parseHistoryEntry, type HistoryEntry } from './history-model.ts';

// About 5s between periodic snapshots; pause and end write immediately, as does an explicit seek (SPEC 11.4).
export const HISTORY_INTERVAL_MS = 5000;
export function historyWriteDue(phase: string, lastWrite: number, now: number) {
  return ["paused", "ended"].includes(phase) || now - lastWrite >= HISTORY_INTERVAL_MS;
}

// Session-only preview. The native implementation will persist through Rust/SQLite.
const entries = new Map<string, HistoryEntry>();
async function save(entry: HistoryEntry): Promise<void> {
  const valid = parseHistoryEntry(entry);
  const previous = entries.get(valid.content_id);
  if (!previous || valid.updated_at >= previous.updated_at) entries.set(valid.content_id, valid);
}
export const historyRepository = {
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
