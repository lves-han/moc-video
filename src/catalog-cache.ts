import type { Bridge_HomeSection, Bridge_MediaCard, ImageAccessMap, SourceKey } from './models.ts';
import { cacheDelete, cacheInvalidate, cacheRead, cacheWrite, hasIndexedCache, pruneCatalogCache } from './data/indexed-cache.ts';

export const CATALOG_TTL = 24 * 60 * 60 * 1000;

const copy = <T>(value: T): T => JSON.parse(JSON.stringify(value));

const KEY = 'moc-video.catalog.v1';
type Store = Pick<Storage, 'getItem' | 'setItem'>;
type Row = { key: string; savedAt: number; items: Bridge_MediaCard[]; invalidated?: boolean; page?: number; hasNext?: boolean; nextPage?: number | null };

export function catalogCache(store: () => Store = () => localStorage) {
  const memory = new Map<string, { host: string; images: ImageAccessMap; section: Bridge_HomeSection; savedAt: number }>();
  let prunedAt = 0;
  const key = (source: SourceKey, section: string, filter: unknown, locale: string) => JSON.stringify([source.source_id, source.source_revision, section, filter, locale]);
  const read = (): Row[] => {
    try {
      const rows: unknown = JSON.parse(store().getItem(KEY) ?? '[]');
      return Array.isArray(rows) ? rows.filter(row => row && typeof row.key === 'string'
        && typeof row.savedAt === 'number' && Number.isFinite(row.savedAt) && Array.isArray(row.items)) : [];
    } catch { return []; }
  };
  const invalidateLegacy = (id: string) => {
    const storage = store(), raw = storage.getItem(KEY);
    if (raw === null) return;
    const rows: unknown = JSON.parse(raw);
    if (!Array.isArray(rows)) throw new Error('Catalog cache invalidation failed: invalid stored data');
    const remaining = rows.filter(row => !row || row.key !== id);
    if (remaining.length !== rows.length) storage.setItem(KEY, JSON.stringify(remaining));
  };
  return {
    get(source: SourceKey, section: string, filter: unknown, locale: string, host: string, now = Date.now()) {
      const id = key(source, section, filter, locale), live = memory.get(id);
      if (live && live.host === host) return { items: copy(live.section.items), images: copy(live.images), stale: now < live.savedAt || now - live.savedAt >= CATALOG_TTL, imagesReady: true, sectionRef: live.section.section_ref, page: live.section.page, hasNext: live.section.has_next, nextPage: live.section.next_page };
      if (hasIndexedCache()) return null;
      const row = read().find(item => item.key === id);
      if (!row || row.items.some(item => !item || typeof item.id !== 'string' || typeof item.title !== 'string'
        || item.ref?.source_id !== source.source_id || item.ref?.source_revision !== source.source_revision || typeof item.ref.data !== 'string')) return null;
      return { items: copy(row.items), images: {}, stale: now < row.savedAt || now - row.savedAt >= CATALOG_TTL, imagesReady: false, sectionRef: null, hasNext: false, nextPage: null };
    },
    async getPersistent(source: SourceKey, section: string, filter: unknown, locale: string, host: string, now = Date.now()) {
      const id = key(source, section, filter, locale), live = memory.get(id);
      if (live && live.host === host && now >= live.savedAt && now - live.savedAt < CATALOG_TTL) return this.get(source, section, filter, locale, host, now);
      if (live && (now < live.savedAt || now - live.savedAt >= CATALOG_TTL)) memory.delete(id);
      if (hasIndexedCache() && now - prunedAt >= 60 * 60 * 1000) {
        prunedAt = now;
        void pruneCatalogCache(now - CATALOG_TTL).catch(() => { prunedAt = 0; });
      }
      const saved = hasIndexedCache() ? await cacheRead<Row>('catalog', id) : null;
      if (saved?.invalidated) return null;
      const row = saved ?? read().find(item => item.key === id);
      if (!row || now < row.savedAt || now - row.savedAt >= CATALOG_TTL || row.items.some(item => !item || typeof item.id !== 'string' || typeof item.title !== 'string'
        || item.ref?.source_id !== source.source_id || item.ref?.source_revision !== source.source_revision || typeof item.ref.data !== 'string')) return null;
      const validPage = Number.isSafeInteger(row.page) && (row.page ?? 0) >= 1;
      const validNext = row.hasNext === true && Number.isSafeInteger(row.nextPage) && row.nextPage === (row.page ?? 0) + 1;
      return { items: copy(row.items), images: {}, stale: false, imagesReady: false, sectionRef: null,
        page: validPage ? row.page! : 1, hasNext: validNext, nextPage: validNext ? row.nextPage! : null };
    },
    put(source: SourceKey, section: string, filter: unknown, locale: string, host: string, data: Bridge_HomeSection, images: ImageAccessMap, now = Date.now()) {
      const id = key(source, section, filter, locale), values = copy(data.items);
      memory.delete(id);
      memory.set(id, { host, section: copy(data), images: copy(images), savedAt: now });
      if (/https?:\/\//i.test(JSON.stringify(values))) {
        invalidateLegacy(id);
        return;
      }
      const rows = read().filter(item => item.key !== id);
      rows.push({ key: id, savedAt: now, items: values, page: data.page, hasNext: data.has_next, nextPage: data.next_page });
      if (!hasIndexedCache()) { try { store().setItem(KEY, JSON.stringify(rows)); } catch { /* Keep the live row usable. */ } }
    },
    async putPersistent(source: SourceKey, section: string, filter: unknown, locale: string, host: string, data: Bridge_HomeSection, images: ImageAccessMap, now = Date.now()) {
      const values = copy(data.items);
      const unsafe = /https?:\/\//i.test(JSON.stringify(values)), id = key(source, section, filter, locale);
      let legacyFailed = false, legacyError: unknown;
      try { this.put(source, section, filter, locale, host, data, images, now); } catch (error) { legacyFailed = true; legacyError = error; }
      try {
        if (hasIndexedCache()) {
          if (unsafe && legacyFailed) await cacheInvalidate('catalog', id);
          else if (unsafe) await cacheDelete('catalog', id);
          else {
            if ((await cacheRead<Row>('catalog', id))?.invalidated) invalidateLegacy(id);
            await cacheWrite('catalog', { key: id, savedAt: now, items: values, page: data.page, hasNext: data.has_next, nextPage: data.next_page });
          }
        }
      } catch (error) {
        if (legacyFailed) throw new AggregateError([legacyError, error], 'Catalog cache invalidation failed in both stores');
        throw error;
      }
      if (legacyFailed) throw legacyError;
    },
  };
}
