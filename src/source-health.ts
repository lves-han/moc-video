import { cacheRead, cacheWrite, nativeCacheEnabled } from './data/indexed-cache.ts';

// The timestamp contains no provider URLs or credentials and survives restarts.
const KEY = 'moc-video.source-checks.v1';
const NATIVE_KEY = 'source-health';
export const SOURCE_CHECK_INTERVAL = 4 * 60 * 60 * 1000;
type Store = Pick<Storage, 'getItem' | 'setItem'>;

export function sourceChecks(store: () => Store = () => localStorage) {
  let memory: Record<string, number> = {};
  const failures = new Set<string>();
  const dirty = new Set<string>();
  const native = nativeCacheEnabled();
  const load = native ? cacheRead<{ value?: Record<string, unknown> }>('details', NATIVE_KEY).then(row => {
    const value = row?.value;
    if (!value || typeof value !== 'object' || Array.isArray(value)) return;
    for (const [id, time] of Object.entries(value)) {
      if (!dirty.has(id) && typeof time === 'number' && Number.isFinite(time) && time >= 0) memory[id] = time;
    }
    if (Array.isArray(value.failures)) for (const id of value.failures) if (typeof id === 'string' && !dirty.has(id)) failures.add(id);
  }).catch(() => {}) : Promise.resolve();
  let writes = Promise.resolve();
  try {
    const value: unknown = native ? {} : JSON.parse(store().getItem(KEY) ?? '{}');
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
      dirty.add(id);
      memory[id] = now;
      if (failed) failures.add(id); else failures.delete(id);
      if (native) {
        writes = writes.then(() => load).then(() => cacheWrite('details', { key: NATIVE_KEY, savedAt: Date.now(), value: { ...memory, failures: [...failures] } })).catch(() => {});
      } else {
        try { store().setItem(KEY, JSON.stringify({ ...memory, failures: [...failures] })); } catch { /* Retry with an in-memory schedule. */ }
      }
    },
  };
}
