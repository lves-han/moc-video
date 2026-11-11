import { readProductStorage } from './storage-compat.ts';
import type { Bridge_HomeSection, Bridge_MediaCard, ImageAccessMap, SourceKey } from './models.ts';
import { METADATA_TTL } from './metadata-cache.ts';

const copy = <T>(value: T): T => JSON.parse(JSON.stringify(value));

const KEY = 'moc-video.catalog.v1';
const LIMIT = 2 * 1024 * 1024;
type Store = Pick<Storage, 'getItem' | 'setItem'>;
type Row = { key: string; savedAt: number; items: Bridge_MediaCard[] };

export function catalogCache(store: () => Store = () => localStorage) {
  const memory = new Map<string, { host: string; images: ImageAccessMap; section: Bridge_HomeSection; savedAt: number }>();
  const key = (source: SourceKey, section: string, filter: unknown, locale: string) => JSON.stringify([source.source_id, source.source_revision, section, filter, locale]);
  const read = (): Row[] => {
    try {
      const rows: unknown = JSON.parse(readProductStorage(store(), KEY) ?? '[]');
      return Array.isArray(rows) ? rows.filter(row => row && typeof row.key === 'string'
        && typeof row.savedAt === 'number' && Number.isFinite(row.savedAt) && Array.isArray(row.items)) : [];
    } catch { return []; }
  };
  return {
    get(source: SourceKey, section: string, filter: unknown, locale: string, host: string, now = Date.now()) {
      const id = key(source, section, filter, locale), live = memory.get(id);
      if (live && live.host === host) return { items: copy(live.section.items), images: copy(live.images), stale: now < live.savedAt || now - live.savedAt >= METADATA_TTL, imagesReady: true, sectionRef: live.section.section_ref, hasNext: live.section.has_next, nextPage: live.section.next_page };
      const row = read().find(item => item.key === id);
      if (!row || row.items.some(item => !item || typeof item.id !== 'string' || typeof item.title !== 'string'
        || item.ref?.source_id !== source.source_id || item.ref?.source_revision !== source.source_revision || typeof item.ref.data !== 'string')) return null;
      return { items: copy(row.items), images: {}, stale: now < row.savedAt || now - row.savedAt >= METADATA_TTL, imagesReady: false, sectionRef: null, hasNext: false, nextPage: null };
    },
    put(source: SourceKey, section: string, filter: unknown, locale: string, host: string, data: Bridge_HomeSection, images: ImageAccessMap, now = Date.now()) {
      const id = key(source, section, filter, locale), values = copy(data.items);
      memory.delete(id);
      memory.set(id, { host, section: copy(data), images: copy(images), savedAt: now });
      if (memory.size > 40) memory.delete(memory.keys().next().value!);
      if (/https?:\/\//i.test(JSON.stringify(values))) return;
      const rows = read().filter(item => item.key !== id);
      rows.push({ key: id, savedAt: now, items: values });
      while (rows.length > 40 || JSON.stringify(rows).length > LIMIT) rows.shift();
      try { store().setItem(KEY, JSON.stringify(rows)); } catch { /* Keep the live row usable. */ }
    },
  };
}
