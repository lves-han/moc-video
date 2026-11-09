<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from 'vue';
import { api } from '../api';
import { appController as app, appState as state } from '../state';
import { historyPoster, metadataReference } from '../data/history';
import { locale, t } from '../i18n';
import type { Bridge_MediaRef, WatchlistEntry } from '../models';

const emit = defineEmits<{ open: [ref: Bridge_MediaRef] }>();
const entries = ref<WatchlistEntry[]>([]), posters = ref<Record<string, string>>({}), error = ref('');
let generation = 0;
async function load() {
  const current = ++generation; error.value = '';
  try {
    const response = await api.call('list_watchlist', {});
    if (current !== generation) return;
    entries.value = response.entries; posters.value = {};
    for (const entry of response.entries) {
      try {
        const poster = await historyPoster(entry, state.sources.data?.providers ?? [], api.call.bind(api));
        if (current !== generation) return;
        if (poster) posters.value[entry.content_id] = poster;
      } catch (cause) { if (current === generation) error.value = String(cause); }
    }
  } catch (cause) { if (current === generation) error.value = String(cause); }
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
  <section v-if="entries.length || error" class="watchlist-row home-section">
    <h2>{{ locale === 'zh-CN' ? '待播清单' : 'Watchlist' }}</h2>
    <p v-if="error" role="alert">{{ error }} <button class="glass-pill" @click="load">{{ t('Retry') }}</button></p>
    <div class="watchlist-track" data-focus-row>
      <article v-for="entry in entries" :key="entry.content_id" class="watchlist-card home-card" :data-content-id="entry.content_id">
        <button class="poster-link watchlist-poster" :aria-label="entry.title" @click="open(entry)"><img v-if="posters[entry.content_id]" :src="posters[entry.content_id]" alt="" loading="lazy" @error="delete posters[entry.content_id]" /><span v-else>{{ entry.title }}</span></button>
        <p class="card-title">{{ entry.title }}</p>
      </article>
    </div>
  </section>
</template>
