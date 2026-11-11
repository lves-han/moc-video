type Reader = Pick<Storage, 'getItem'>;

// Keep the previous product's records readable; writes use the new namespace.
export function readProductStorage(storage: Reader, key: string): string | null {
  const current = storage.getItem(key);
  if (current !== null || !key.startsWith('moc-video.')) return current;
  return storage.getItem(key.replace(/^moc-video\./, 'yingxiang-video.'));
}
