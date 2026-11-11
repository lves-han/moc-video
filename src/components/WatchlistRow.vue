<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from 'vue';
import { api } from '../api';
import { appController as app, appState as state } from '../state';
import { metadataReference } from '../data/history';
import { locale, t } from '../i18n';
import type { Bridge_MediaRef, WatchlistEntry } from '../models';

const emit = defineEmits<{ open: [ref: Bridge_MediaRef] }>();
defineProps<{ wall?:boolean }>();
const entries = ref<WatchlistEntry[]>([]), posters = ref<Record<string, { id: string; url: string }>>({}), error = ref('');
const refreshed = new Set<string>();
let generation = 0;
function detailsHref(entry: WatchlistEntry) {
  const ref = metadataReference(entry, state.sources.data?.providers ?? []);
  return ref ? `#/details/${encodeURIComponent(JSON.stringify(ref))}` : '#/home';
}
async function loadPoster(entry: WatchlistEntry, current: number, host: string | null) {
  const ref = metadataReference(entry, state.sources.data?.providers ?? []);
  if (!ref || !host) return;
  const data = await api.call('get_details', { ref }, crypto.randomUUID(), host);
  if (current !== generation || host !== state.runtime.engine_epoch) return;
  if (data.details.id !== entry.content_id || data.details.ref.source_id !== ref.source_id || data.details.ref.source_revision !== ref.source_revision || data.details.ref.data !== ref.data) throw new Error('Watchlist details identity differs');
  const id = data.details.poster?.resource_id, access = id ? data.images[id] : undefined;
  if (id && access) posters.value[entry.content_id] = { id, url: access.url };
}
async function load() {
  const current = ++generation, host = state.runtime.engine_epoch; error.value = '';
  refreshed.clear();
  try {
    const response = await api.call('list_watchlist', {}, crypto.randomUUID(), host ?? undefined);
    if (current !== generation || host !== state.runtime.engine_epoch) return;
    entries.value = response.entries; posters.value = {};
    for (const entry of response.entries) {
      try {
        await loadPoster(entry, current, host);
        if (current !== generation) return;
      } catch (cause) { if (current === generation) error.value = String(cause); }
    }
  } catch (cause) { if (current === generation) error.value = String(cause); }
}
async function imageFailed(entry: WatchlistEntry, event: Event) {
  const poster = posters.value[entry.content_id], current = generation, host = state.runtime.engine_epoch;
  if (!poster || !host || !(event.target as HTMLImageElement).isConnected) return;
  // Removing the failed access also prevents duplicate events from querying it again.
  delete posters.value[entry.content_id];
  try {
    const status = await api.call('get_resource_status', { kind: 'image', target_id: poster.id }, crypto.randomUUID(), host);
    if (current !== generation || host !== state.runtime.engine_epoch) return;
    if (status.kind !== 'image' || status.target_id !== poster.id) throw new Error('Image status target differs');
    // Only confirmed expiry refreshes this title's DTO, once per load attempt.
    if (status.error?.code === 'SESSION_EXPIRED' && !refreshed.has(entry.content_id)) {
      refreshed.add(entry.content_id); await loadPoster(entry, current, host);
    } else error.value = status.error?.message ?? 'Image failed to load';
  } catch (cause) { if (current === generation && host === state.runtime.engine_epoch) error.value = String(cause); }
}
async function open(entry: WatchlistEntry) {
  const ref = metadataReference(entry, state.sources.data?.providers ?? []);
  if (!ref) { error.value = t('This title is unavailable in the current catalog.'); return; }
  try {
    await app.selectSource(ref.source_id, entry.metadata_identity.namespace === 'maccms' ? 'playback' : 'metadata');
    emit('open', ref);
  } catch (cause) { error.value = String(cause); }
}
watch([() => state.runtime.engine_epoch, () => state.sources.data, () => locale.value], load, { immediate: true });
onBeforeUnmount(() => generation++);
</script>
<template>
  <section v-if="wall || entries.length || error" class="watchlist-row home-section" :class="{ 'watchlist-wall':wall }">
    <h2>{{ locale === 'zh-CN' ? '待播清单' : 'Watchlist' }}</h2>
    <p v-if="error" role="alert">{{ error }} <button class="glass-pill" @click="load">{{ t('Retry') }}</button></p>
    <p v-if="wall && !entries.length && !error" class="library-empty" role="status">{{ locale === 'zh-CN' ? '添加影片到待播清单，稍后观看。' : 'Add titles to your watchlist to watch later.' }}</p>
    <div class="watchlist-track" data-focus-row>
      <article v-for="entry in entries" :key="entry.content_id" class="watchlist-card home-card" :data-content-id="entry.content_id">
        <a class="poster-link watchlist-poster" :href="detailsHref(entry)" :aria-label="entry.title" @click.prevent="open(entry)"><img v-if="posters[entry.content_id]" :src="posters[entry.content_id]!.url" alt="" loading="lazy" @error="imageFailed(entry, $event)" /><span v-else>{{ entry.title }}</span></a>
        <p class="card-title">{{ entry.title }}</p>
      </article>
    </div>
  </section>
</template>
