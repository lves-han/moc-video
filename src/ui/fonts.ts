import { reactive } from 'vue';
import { fontSpec, openFontStore, loadFontBytes, type FontSpec, type FontStore } from './fontCache.ts';
import { fontDownloader } from './fontDownload.ts';
export { fontOptions, fontSpec } from './fontCache.ts';
export const fontStatus = reactive({ busy: false, received: 0, total: 0, error: '', source: '', active: '' });
const loaded = new Map<string, FontFace>();
let store: FontStore | undefined;
let pending: Promise<void> | undefined;
export function setFontScale(value: number = 100) {
  const scale = [90, 100, 110, 120].includes(value) ? value : 100;
  document.documentElement.style.zoom = String(scale / 100);
}
export async function prepareFont(id: unknown): Promise<void> {
  const spec = fontSpec(id);
  const operation = (pending ?? Promise.resolve()).catch(() => {}).then(() => activate(spec));
  pending = operation;
  try { await operation; } finally { if (pending === operation) pending = undefined; }
}
async function activate(spec: FontSpec) {
  fontStatus.busy = true; fontStatus.error = ''; fontStatus.received = 0; fontStatus.total = spec.bytes;
  try {
    let face = loaded.get(spec.id);
    if (!face) {
      store ??= await openFontStore();
      const core = window.__TAURI__?.core;
      const download = fontDownloader(core ? id => core.invoke<string>('app_font_url', { font_id: id }) : undefined);
      const result = await loadFontBytes(spec, store, download, n => { fontStatus.received = n; });
      face = await new FontFace(spec.family, result.bytes, { weight: spec.weights, style: 'normal' }).load();
      document.fonts.add(face); loaded.set(spec.id, face); fontStatus.source = result.source;
      // Persistence is best effort; the durable copy is always written before use.
      void navigator.storage?.persist?.().catch(() => {});
    } else { fontStatus.source = 'local'; }
    document.documentElement.style.setProperty('--app-font', `"${spec.family}", "Moc Bootstrap"`);
    fontStatus.active = spec.id;
  } catch (error) {
    fontStatus.error = error instanceof Error ? error.message : String(error);
    throw error;
  } finally { fontStatus.busy = false; }
}
