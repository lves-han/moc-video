type StoreName = 'details' | 'catalog';
type CacheRow = { key: string; savedAt: number; touchedAt?: number; [name: string]: unknown };
let opening: Promise<IDBDatabase> | undefined;
let lastTouch = 0;
const touch = () => lastTouch = Math.max(Date.now(), lastTouch + 1);
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
export const hasIndexedCache = () => typeof globalThis.indexedDB !== 'undefined';
export async function cacheRead<T>(store: StoreName, key: string): Promise<T | null> {
  const db = await database();
  return await new Promise((resolve, reject) => {
    const tx = db.transaction(store, store === 'details' ? 'readwrite' : 'readonly');
    const bucket = tx.objectStore(store), request = bucket.get(key);
    let value: T | null = null;
    request.onsuccess = () => {
      const row = request.result as CacheRow | undefined;
      if (row) { value = row as T; if (store === 'details') bucket.put({ ...row, touchedAt: touch() }); }
    };
    tx.oncomplete = () => resolve(value);
    tx.onerror = tx.onabort = () => reject(tx.error ?? new Error('Cache read failed'));
  });
}
export async function cacheRows<T>(store: StoreName): Promise<T[]> {
  const db = await database();
  return await new Promise((resolve, reject) => {
    const tx = db.transaction(store, 'readonly'), request = tx.objectStore(store).getAll();
    request.onsuccess = () => resolve(request.result as T[]);
    request.onerror = () => reject(request.error ?? new Error('Cache read failed'));
  });
}
export async function pruneCatalogCache(cutoff: number): Promise<void> {
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
  const db = await database();
  await new Promise<void>((resolve, reject) => {
    const tx = db.transaction(store, 'readwrite'), bucket = tx.objectStore(store);
    bucket.put(store === 'details' ? { ...row, touchedAt: touch() } : row);
    if (store === 'details') {
      let count = 0;
      const cursor = bucket.index('touched').openCursor(null, 'prev');
      cursor.onsuccess = () => { const value = cursor.result; if (!value) return; if (++count > 200) value.delete(); value.continue(); };
    }
    tx.oncomplete = () => resolve();
    tx.onerror = tx.onabort = () => reject(tx.error ?? new Error('Cache write failed'));
  });
}
