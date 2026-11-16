import { api } from './api.ts';
import { watch } from 'vue';
import { appState, metadataLocale, metadataVideoId, TMDB_CONFIG_ID } from './state.ts';
import type { Bridge_MediaCard } from './models.ts';

type Status = 'available' | 'unavailable' | 'unknown' | 'upcoming';
type Entry = { generation: number; key: string; listeners: Set<(status: Status) => void>; status?: Status; checkedAt?: number; running: boolean; card: Bridge_MediaCard; epoch: string; serviceUrl: string };
const entries = new Map<string, Entry>();
const queue: Entry[] = [];
let running = 0, generation = 0;
watch(() => [appState.settings.playback_service?.url, appState.settings.playback_service?.token, appState.settings.playback_service?.enabled, JSON.stringify(appState.settings.playback_service?.groups ?? [])], () => { generation++; entries.clear(); });
const age = 5 * 60 * 1000;

async function check(entry: Entry): Promise<Status> {
  const service = appState.settings.playback_service;
  if (!service?.enabled || service.url !== entry.serviceUrl || appState.runtime.engine_epoch !== entry.epoch) return 'unknown';
  const data = await api.call('get_details', { ref: entry.card.ref }, crypto.randomUUID(), entry.epoch);
  if (appState.runtime.engine_epoch !== entry.epoch || appState.settings.playback_service?.url !== entry.serviceUrl) return 'unknown';
  const identity = data.metadata_ref?.identity;
  if (!identity || data.details.id !== entry.card.id || data.metadata_ref?.content_ref.data !== entry.card.ref.data || !['movie', 'series'].includes(identity.type)
      || data.metadata_ref?.content_ref.source_id !== entry.card.ref.source_id
      || data.metadata_ref?.content_ref.source_revision !== entry.card.ref.source_revision) return 'unknown';
  if (data.details.status === 'upcoming') return 'upcoming';
  const metadataSource = identity.config_id === TMDB_CONFIG_ID ? 'tmdb' : service.groups.find(group => group.config_id === identity.config_id && group.status === 'ready')?.addons.find(addon => addon.addon_key === identity.addon_key)?.source_id;
  if (!metadataSource) return 'unknown';
  const videos = [...new Set(data.details.groups.flatMap(group => group.episodes).map(episode => metadataVideoId(episode.ref)).filter(Boolean))];
  if (!videos.length) return 'unknown';
  const tauri = (globalThis as typeof globalThis & { __TAURI__?: { core?: { invoke: (command: string, args: Record<string, unknown>) => Promise<string> } } }).__TAURI__;
  if (!tauri?.core) return 'unknown';
  let unknown = false;
  // An episodic title is unavailable only after all its episodes have been checked.
  // Limit background work; long series remain unlabelled when sampled episodes fail.
  for (const video of videos.slice(0, 3)) {
    if (!entry.listeners.size || appState.runtime.engine_epoch !== entry.epoch || appState.settings.playback_service?.url !== entry.serviceUrl) return 'unknown';
    const result = await tauri.core.invoke('app_probe_availability', { metadata_source_id: metadataSource, kind: identity.type, meta_id: identity.meta_id, video_id: video, lang: metadataLocale(appState.settings) });
    if (result === 'available') return 'available';
    if (result === 'unavailable_title') return 'unavailable';
    if (result !== 'unavailable') unknown = true;
  }
  return unknown || videos.length > 3 ? 'unknown' : 'unavailable';
}
function pump() {
  while (running < 2 && queue.length) {
    const entry = queue.shift()!;
    if (!entry.listeners.size || entry.running || entry.generation !== generation) continue;
    entry.running = true; running++;
    void check(entry).catch(() => 'unknown' as Status).then(status => {
      if (entry.generation !== generation) return;
      entry.status = status; entry.checkedAt = Date.now();
      for (const listener of entry.listeners) listener(status);
    }).finally(() => { entry.running = false; running--; if (!entry.listeners.size && entries.get(entry.key) === entry) entries.delete(entry.key); pump(); });
  }
}
export function observeAvailability(card: Bridge_MediaCard, listener: (status: Status) => void): () => void {
  const service = appState.settings.playback_service, epoch = appState.runtime.engine_epoch;
  if (!service?.enabled || !epoch || card.status === 'upcoming' || !(globalThis as typeof globalThis & { __TAURI__?: unknown }).__TAURI__) return () => {};
  const key = JSON.stringify([generation, epoch, service.url, metadataLocale(appState.settings), card.ref]);
  let entry = entries.get(key);
  if (!entry) { entry = { generation, key, listeners: new Set(), running: false, card, epoch, serviceUrl: service.url }; entries.set(key, entry); }
  entry.listeners.add(listener);
  if (entry.status && entry.checkedAt && Date.now() - entry.checkedAt < age) listener(entry.status);
  else if (!entry.running && !queue.includes(entry)) { queue.push(entry); pump(); }
  return () => {
    entry!.listeners.delete(listener);
    if (!entry!.listeners.size && !entry!.running) entries.delete(key);
  };
}
