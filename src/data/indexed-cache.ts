type StoreName = 'details' | 'catalog';
type CacheRow = { key: string; savedAt: number; touchedAt?: number; invalidated?: boolean; [name: string]: unknown };
let opening: Promise<IDBDatabase> | undefined;
let lastTouch = 0;
const touch = () => lastTouch = Math.max(Date.now(), lastTouch + 1);
type NativeCore = { invoke(command: string, args: Record<string, unknown>): Promise<unknown> };
const nativeCore = () => (globalThis as typeof globalThis & { __TAURI__?: { core?: NativeCore } }).__TAURI__?.core;
export const nativeCacheEnabled = () => Boolean(nativeCore());
const nativeCall = (action: string, store: StoreName, key = '', row: CacheRow | null = null) =>
  nativeCore()!.invoke('app_cache', { action, store, key, row });
let migration: Promise<void> | undefined;

// New native writes remain disabled until both legacy WebView caches are removed.
async function cleanupLegacy(): Promise<void> {
  return migration ??= (async () => {
    localStorage.removeItem('moc-video.catalog.v1');
    localStorage.removeItem('moc-video.metadata.v1');
    localStorage.removeItem('moc-video.source-checks.v1');
    if (typeof globalThis.indexedDB !== 'undefined') {
      if (opening) { const db = await opening; db.close(); opening = undefined; }
      await new Promise<void>((resolve, reject) => {
        const request = indexedDB.deleteDatabase('moc-video-cache');
        request.onsuccess = () => resolve();
        request.onerror = () => reject(request.error ?? new Error('Legacy cache cleanup failed'));
        request.onblocked = () => reject(new Error('Legacy cache cleanup blocked'));
      });
    }
    await nativeCall('ready', 'catalog');
  })().catch(error => { migration = undefined; throw error; });
}
export async function prepareNativeCache(): Promise<void> {
  if (nativeCacheEnabled()) await cleanupLegacy();
}
export async function nativeCacheControl<T>(action: 'video_status' | 'video_clear' | 'metadata_clear'): Promise<T> {
  if (!nativeCacheEnabled()) throw new Error('Desktop cache is unavailable');
  await prepareNativeCache();
  return await nativeCall(action, 'catalog') as T;
}
function database(): Promise<IDBDatabase> {
  return opening ??= new Promise<IDBDatabase>((resolve, reject) => {
    const request = indexedDB.open('moc-video-cache', 2);
    request.onupgradeneeded = () => {
      const db = request.result;
      const details = db.objectStoreNames.contains('details') ? request.transaction!.objectStore('details') : db.createObjectStore('details', { keyPath: 'key' });
      if (!details.indexNames.contains('touched')) details.createIndex('touched', 'touchedAt');
      const catalog = db.objectStoreNames.contains('catalog') ? request.transaction!.objectStore('catalog') : db.createObjectStore('catalog', { keyPath: 'key' });
      if (!catalog.indexNames.contains('saved')) catalog.createIndex('saved', 'savedAt');
    };
    request.onsuccess = () => { request.result.onversionchange = () => { request.result.close(); opening = undefined; }; resolve(request.result); };
    request.onerror = () => reject(request.error ?? new Error('Cache database unavailable'));
    request.onblocked = () => reject(new Error('Cache database upgrade blocked'));
  }).catch(error => { opening = undefined; throw error; });
}
export const hasIndexedCache = () => nativeCacheEnabled() || typeof globalThis.indexedDB !== 'undefined';
export async function cacheRead<T>(store: StoreName, key: string): Promise<T | null> {
  if (nativeCacheEnabled()) { await cleanupLegacy(); return await nativeCall('read', store, key) as T | null; }
  const db = await database();
  return await new Promise((resolve, reject) => {
    const tx = db.transaction(store, store === 'details' ? 'readwrite' : 'readonly');
    const bucket = tx.objectStore(store), request = bucket.get(key);
    let value: T | null = null;
    request.onsuccess = () => {
      const row = request.result as CacheRow | undefined;
      if (row) { value = row as T; if (store === 'details' && !row.invalidated) bucket.put({ ...row, touchedAt: touch() }); }
    };
    tx.oncomplete = () => resolve(value);
    tx.onerror = tx.onabort = () => reject(tx.error ?? new Error('Cache read failed'));
  });
}
export async function cacheRows<T>(store: StoreName): Promise<T[]> {
  if (nativeCacheEnabled()) { await cleanupLegacy(); return await nativeCall('list', store) as T[]; }
  const db = await database();
  return await new Promise((resolve, reject) => {
    const tx = db.transaction(store, 'readonly'), request = tx.objectStore(store).getAll();
    request.onsuccess = () => resolve(request.result as T[]);
    request.onerror = () => reject(request.error ?? new Error('Cache read failed'));
  });
}
export async function pruneCatalogCache(cutoff: number): Promise<void> {
  if (nativeCacheEnabled()) { await cleanupLegacy(); return; }
  const db = await database();
  await new Promise<void>((resolve, reject) => {
    const tx = db.transaction('catalog', 'readwrite');
    const cursor = tx.objectStore('catalog').index('saved').openCursor(IDBKeyRange.upperBound(cutoff));
    cursor.onsuccess = () => { const row = cursor.result; if (!row) return; row.delete(); row.continue(); };
    tx.oncomplete = () => resolve();
    tx.onerror = tx.onabort = () => reject(tx.error ?? new Error('Catalog cache cleanup failed'));
  });
}
export async function cacheWrite(store: StoreName, row: CacheRow): Promise<void> {
  if (nativeCacheEnabled()) { await cleanupLegacy(); await nativeCall('write', store, row.key, row); return; }
  const db = await database();
  await new Promise<void>((resolve, reject) => {
    const tx = db.transaction(store, 'readwrite'), bucket = tx.objectStore(store);
    bucket.put(store === 'details' ? { ...row, touchedAt: touch() } : row);
    tx.oncomplete = () => resolve();
    tx.onerror = tx.onabort = () => reject(tx.error ?? new Error('Cache write failed'));
  });
}
export async function cacheDelete(store: StoreName, key: string): Promise<void> {
  if (nativeCacheEnabled()) { await cleanupLegacy(); await nativeCall('delete', store, key); return; }
  const db = await database();
  await new Promise<void>((resolve, reject) => {
    const tx = db.transaction(store, 'readwrite');
    tx.objectStore(store).delete(key);
    tx.oncomplete = () => resolve();
    tx.onerror = tx.onabort = () => reject(tx.error ?? new Error('Cache deletion failed'));
  });
}
// A tombstone prevents legacy fallback after an unsuccessful legacy cleanup.
export async function cacheInvalidate(store: StoreName, key: string): Promise<void> {
  if (nativeCacheEnabled()) { await cleanupLegacy(); await nativeCall('write', store, key, { key, savedAt: Date.now(), invalidated: true }); return; }
  const db = await database();
  await new Promise<void>((resolve, reject) => {
    const tx = db.transaction(store, 'readwrite');
    tx.objectStore(store).put({ key, savedAt: Number.MAX_SAFE_INTEGER, invalidated: true });
    tx.oncomplete = () => resolve();
    tx.onerror = tx.onabort = () => reject(tx.error ?? new Error('Cache invalidation failed'));
  });
}
