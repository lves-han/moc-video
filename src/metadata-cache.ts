import type { Bridge_MediaRef, DetailsViewData } from './models.ts';
import { cacheRead, cacheRows, cacheWrite, hasIndexedCache } from './data/indexed-cache.ts';

export const METADATA_TTL = 7 * 24 * 60 * 60 * 1000;
const copy = <T>(value: T): T => JSON.parse(JSON.stringify(value));

const KEY = 'moc-video.metadata.v1';
const MAX_ENTRIES = 100;
type Store = Pick<Storage, 'getItem' | 'setItem'>;
type Entry = { key: string; savedAt: number; value: DetailsViewData };
const sameRef = (a: Bridge_MediaRef | undefined, b: Bridge_MediaRef) => Boolean(a && a.source_id === b.source_id
  && a.source_revision === b.source_revision && a.data === b.data);

export function metadataCache(store: () => Store = () => localStorage) {
  const memory = new Map<string, { host: string; value: DetailsViewData; savedAt: number }>();
  const key = (ref: Bridge_MediaRef, locale: string) => JSON.stringify([ref.source_id, ref.source_revision, ref.data, locale]);
  const read = (): Entry[] => {
    try {
      const value: unknown = JSON.parse(store().getItem(KEY) ?? '[]');
      return Array.isArray(value) ? value.filter(row => row && typeof row.key === 'string'
        && typeof row.savedAt === 'number' && Number.isFinite(row.savedAt) && row.value?.details?.ref
        && typeof row.value.details.ref.source_id === 'string' && typeof row.value.details.ref.source_revision === 'string'
        && typeof row.value.details.ref.data === 'string' && row.value?.metadata_ref?.content_ref) : [];
    } catch { return []; }
  };
  const dueRows = (rows: Entry[], locale: string, now: number) => rows.filter(row => {
        try {
          const fields: unknown = JSON.parse(row.key);
          return Array.isArray(fields) && fields.length === 4 && fields[3] === locale
            && (now < row.savedAt || now - row.savedAt >= METADATA_TTL)
            && row.value.details.ref?.source_id === fields[0]
            && row.value.details.ref?.source_revision === fields[1]
            && row.value.details.ref?.data === fields[2];
        } catch { return false; }
      }).sort((a, b) => a.savedAt - b.savedAt).map(row => row.value.details.ref);
  const restore = (item: Entry | undefined | null, ref: Bridge_MediaRef, now: number) => {
    if (!item || !sameRef(item.value.details.ref, ref) || !sameRef(item.value.metadata_ref?.content_ref, ref)
        || !item.value.metadata_ref?.identity || !['config_id', 'addon_key', 'namespace', 'type', 'meta_id', 'canonical_id']
          .every(field => typeof (item.value.metadata_ref!.identity as unknown as Record<string, unknown>)[field] === 'string')
        || typeof item.value.details.title !== 'string' || !Array.isArray(item.value.details.groups)
        || !Array.isArray(item.value.details.related) || !Array.isArray(item.value.details.tags)
        || item.value.details.groups.some(group => !Array.isArray(group.episodes)
          || group.episodes.some(episode => episode.ref?.source_id !== ref.source_id || episode.ref?.source_revision !== ref.source_revision))) return null;
    return { value: { ...copy(item.value), images: {} }, stale: now < item.savedAt || now - item.savedAt >= METADATA_TTL, imagesReady: false };
  };
  return {
    due(locale: string, now = Date.now()) { return dueRows(read(), locale, now); },
    async duePersistent(locale: string, now = Date.now()) {
      const rows = hasIndexedCache() ? await cacheRows<Entry>('details') : read();
      return dueRows(rows, locale, now);
    },
    get(ref: Bridge_MediaRef, locale: string, host: string, now = Date.now()) {
      const id = key(ref, locale), active = memory.get(id);
      if (active && active.host === host) return { value: copy(active.value), stale: now < active.savedAt || now - active.savedAt >= METADATA_TTL, imagesReady: true };
      const item = read().find(row => row.key === id);
      return restore(item, ref, now);
    },
    async getPersistent(ref: Bridge_MediaRef, locale: string, host: string, now = Date.now()) {
      const id = key(ref, locale), active = memory.get(id);
      if (active && active.host === host) {
        memory.delete(id); memory.set(id, active);
        if (hasIndexedCache()) await cacheRead<Entry>('details', id);
        return { value: copy(active.value), stale: now < active.savedAt || now - active.savedAt >= METADATA_TTL, imagesReady: true };
      }
      const item = (hasIndexedCache() ? await cacheRead<Entry>('details', id) : null) ?? read().find(row => row.key === id);
      return restore(item, ref, now);
    },
    put(ref: Bridge_MediaRef, locale: string, host: string, value: DetailsViewData, now = Date.now()) {
      const id = key(ref, locale);
      memory.delete(id);
      memory.set(id, { host, value: copy(value), savedAt: now });
      if (memory.size > MAX_ENTRIES) memory.delete(memory.keys().next().value!);
      // Gateway URLs and media URLs contain short-lived credentials. Keep only
      // display DTOs and opaque references; image access is reacquired later.
      const safe = { ...copy(value), images: {} };
      const encoded = JSON.stringify(safe);
      if (/https?:\/\//i.test(encoded)) return;
      const rows = read().filter(row => row.key !== id);
      rows.push({ key: id, savedAt: now, value: safe });
      while (rows.length > MAX_ENTRIES) rows.shift();
      if (!hasIndexedCache()) { try { store().setItem(KEY, JSON.stringify(rows)); } catch { /* The live response remains usable. */ } }
    },
    async putPersistent(ref: Bridge_MediaRef, locale: string, host: string, value: DetailsViewData, now = Date.now()) {
      this.put(ref, locale, host, value, now);
      const safe = { ...copy(value), images: {} };
      if (hasIndexedCache() && !/https?:\/\//i.test(JSON.stringify(safe))) await cacheWrite('details', { key: key(ref, locale), savedAt: now, value: safe });
    },
  };
}
