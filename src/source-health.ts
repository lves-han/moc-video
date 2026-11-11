import { readProductStorage } from './storage-compat.ts';
// The timestamp contains no provider URLs or credentials and survives restarts.
const KEY = 'moc-video.source-checks.v1';
export const SOURCE_CHECK_INTERVAL = 4 * 60 * 60 * 1000;
type Store = Pick<Storage, 'getItem' | 'setItem'>;

export function sourceChecks(store: () => Store = () => localStorage) {
  let memory: Record<string, number> = {};
  const failures = new Set<string>();
  try {
    const value: unknown = JSON.parse(readProductStorage(store(), KEY) ?? '{}');
    if (value && typeof value === 'object' && !Array.isArray(value)) {
      memory = Object.fromEntries(Object.entries(value).filter(([, time]) => typeof time === 'number' && Number.isFinite(time) && time >= 0));
      const saved = (value as Record<string, unknown>).failures;
      if (Array.isArray(saved)) for (const id of saved) if (typeof id === 'string') failures.add(id);
    }
  } catch { /* Storage is optional; keep the in-memory schedule. */ }
  return {
    due(id: string, now = Date.now()) { return memory[id] !== undefined && (now < memory[id]! || now - memory[id]! >= SOURCE_CHECK_INTERVAL); },
    has(id: string) { return memory[id] !== undefined; },
    failed(id: string) { return failures.has(id); },
    mark(id: string, now = Date.now(), failed = false) {
      memory[id] = now;
      if (failed) failures.add(id); else failures.delete(id);
      try { store().setItem(KEY, JSON.stringify({ ...memory, failures: [...failures] })); } catch { /* Retry with an in-memory schedule. */ }
    },
  };
}
