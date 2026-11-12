import catalog from "../data/fontCatalog.json" with { type: "json" };
export interface FontSpec { id: string; label: string; family: string; url: string; sha256: string; bytes: number; weights: string }
export const fontOptions: readonly FontSpec[] = catalog;
export function fontSpec(id: unknown): FontSpec { return fontOptions.find(font => font.id === id) ?? fontOptions[0]!; }
export interface FontStore { get(key: string): Promise<ArrayBuffer | undefined>; put(key: string, bytes: ArrayBuffer): Promise<void> }
export async function validFont(font: FontSpec, bytes: ArrayBuffer): Promise<boolean> {
  if (bytes.byteLength !== font.bytes) return false;
  const hash = await crypto.subtle.digest('SHA-256', bytes);
  return Array.from(new Uint8Array(hash), n => n.toString(16).padStart(2, '0')).join('') === font.sha256;
}
export async function loadFontBytes(font: FontSpec, store: FontStore, download: typeof fetch = fetch,
  progress: (received: number) => void = () => {}): Promise<{ bytes: ArrayBuffer; source: 'local' | 'download' }> {
  const key = `${font.id}:${font.sha256}`;
  const saved = await store.get(key);
  if (saved && await validFont(font, saved)) return { bytes: saved, source: 'local' };
  const controller = new AbortController(), timeout = setTimeout(() => controller.abort(), 180_000);
  try {
    const response = await download(font.url, { signal: controller.signal, credentials: 'omit', cache: 'no-store', redirect: 'error' });
    if (!response.ok || !response.body) throw new Error(`Font download failed (HTTP ${response.status})`);
    const reader = response.body.getReader(), chunks: Uint8Array[] = [];
    let length = 0;
    try {
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        length += value.byteLength;
        if (length > font.bytes) throw new Error('Font download exceeds the expected size');
        chunks.push(value); progress(length);
      }
    } finally { await reader.cancel().catch(() => {}); }
    const bytes = new Uint8Array(length);
    let offset = 0;
    for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.byteLength; }
    if (!await validFont(font, bytes.buffer)) throw new Error('Font integrity verification failed');
    // Resolve only after IndexedDB commits: a successful download must be reusable offline.
    await store.put(key, bytes.buffer);
    return { bytes: bytes.buffer, source: 'download' };
  } finally { clearTimeout(timeout); }
}

export async function openFontStore(): Promise<FontStore> {
  const db = await new Promise<IDBDatabase>((resolve, reject) => {
    const request = indexedDB.open('moc-video-fonts', 1);
    request.onupgradeneeded = () => request.result.createObjectStore('files');
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error('Font cache could not be opened'));
    request.onblocked = () => reject(new Error('Close other app windows to open the font cache'));
  });
  db.onversionchange = () => db.close();
  return {
    get: key => new Promise((resolve, reject) => {
      const transaction = db.transaction('files', 'readonly');
      const request = transaction.objectStore('files').get(key);
      transaction.oncomplete = () => resolve(request.result instanceof ArrayBuffer ? request.result : undefined);
      transaction.onabort = () => reject(transaction.error ?? new Error('Font cache read failed'));
      transaction.onerror = () => reject(transaction.error ?? new Error('Font cache read failed'));
    }),
    put: (key, bytes) => new Promise((resolve, reject) => {
      const transaction = db.transaction('files', 'readwrite');
      transaction.objectStore('files').put(bytes, key);
      transaction.oncomplete = () => resolve();
      transaction.onabort = () => reject(transaction.error ?? new Error('Font cache write failed'));
      transaction.onerror = () => reject(transaction.error ?? new Error('Font cache write failed'));
    }),
  };
}
