<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue';
import { appController as app, appState as state } from '../state';
import { describeError } from '../api';
import { locale, t } from '../i18n';
import MediaRow from '../components/MediaRow.vue';
import Icon from '../components/Icon.vue';
import type { Bridge_MediaCard, Bridge_MediaRef } from '../models';
import { approximateResults } from '../data/search-match';

const emit = defineEmits<{ open:[ref:Bridge_MediaRef] }>();
const word = (zh:string, en:string) => locale.value === 'zh-CN' ? zh : en;
const native = Boolean(window.__TAURI__);
const input = ref<HTMLInputElement>(), query = ref(app.search.query), sourceId = ref(app.search.source?.source_id ?? '');
const providers = computed(() => (state.sources.data?.providers ?? []).filter(p => p.runtime_status === 'ready' && p.capabilities.search && (!native || p.kind === 'stremio')));
watch(providers, sources => {
  if (!sources.some(p => p.id === sourceId.value)) sourceId.value = sources.find(p => p.id === state.selected_metadata_source?.source_id)?.id ?? sources[0]?.id ?? '';
}, { immediate:true });
const groups = computed(() => [
  { key:'movie', title:t('Movies'), kinds:['movie'] },
  { key:'series', title:t('Series'), kinds:['series','anime'] },
  { key:'other', title:word('其他','Other'), kinds:['other'] },
].map(group => ({ ...group, items:app.search.items.filter(card => group.kinds.includes(card.kind)) })).filter(group => group.items.length));
const genres = computed(() => [
  { kind:'movie', title:t('Movies'), icon:'film' }, { kind:'series', title:t('Series'), icon:'tv' },
  { kind:'anime', title:t('Animation'), icon:'sparkles' }, { kind:'documentary', title:t('Documentary'), icon:'documentary' },
  ...(native && state.home.data?.page.sections.some(section => section.kind === 'series' && /^(variety|综艺)$/i.test(section.title)) ? [{ kind:'variety', title:t('Variety'), icon:'variety' }] : []),
  ...(!native ? [{ kind:'variety', title:t('Variety'), icon:'variety' }, { kind:'short', title:t('Short dramas'), icon:'short' }] : []),
]);
const approximate = computed(() => app.search.phase === 'ready' && approximateResults(app.search.query, app.search.items.map(item => item.title)));
let timer: ReturnType<typeof setTimeout> | undefined, observer:IntersectionObserver | undefined;
let posterQueue: Bridge_MediaCard[] = [], posterRunning = 0, posterGeneration = 0;
let pumpPosters = () => {};
const opening = ref(false);
let mounted = true;
function search() {
  const provider = providers.value.find(p => p.id === sourceId.value), value = query.value.trim();
  if (provider && value) void app.searchCatalog(value, { source_id:provider.id, source_revision:provider.source_revision });
}
watch([query, sourceId, () => state.settings.ui_locale, () => providers.value.find(p => p.id === sourceId.value)?.source_revision], () => {
  clearTimeout(timer); app.cancelSearch();
  if (!query.value.trim()) { app.cancelSearch(true); app.search.query = ''; return; }
  timer = setTimeout(search, 300);
}, { immediate:true });
watch(() => app.search.items, async items => {
  observer?.disconnect(); posterQueue = []; const generation = ++posterGeneration; await nextTick();
  if (!mounted || generation !== posterGeneration) return;
  pumpPosters = () => {
    while (mounted && generation === posterGeneration && posterRunning < 4 && posterQueue.length) {
      const card = posterQueue.shift()!; posterRunning++;
      void (query.value.trim() === app.search.query ? app.searchPoster(card) : Promise.resolve())
        .finally(() => { posterRunning--; pumpPosters(); });
    }
  };
  observer = new IntersectionObserver(entries => {
    for (const entry of entries) if (entry.isIntersecting) {
      observer?.unobserve(entry.target);
      const card = items.find(card => card.id === (entry.target as HTMLElement).dataset.cardId);
      if (card) { posterQueue.push(card); pumpPosters(); }
    }
  });
  document.querySelectorAll('.search-results .media-card').forEach(card => observer!.observe(card));
});
async function open(ref:Bridge_MediaRef) {
  if (opening.value) return;
  opening.value = true;
  try {
    const provider = providers.value.find(p => p.id === ref.source_id);
    await app.selectSource(ref.source_id, provider?.kind === 'maccms' ? 'playback' : 'metadata');
    if (mounted) emit('open', ref);
  } catch (cause) { app.search.error = { code:'UPSTREAM_FAILED', message:String(cause), retryable:true }; }
  finally { opening.value = false; }
}
function focus() { input.value?.focus(); input.value?.select(); }
defineExpose({ focus });
onBeforeUnmount(() => { mounted = false; posterGeneration++; posterQueue = []; pumpPosters = () => {}; clearTimeout(timer); observer?.disconnect(); app.cancelSearch(); });
</script>

<template>
  <section class="search-page">
    <div class="search-bar"><Icon name="search" :size="24" /><input ref="input" v-model="query" class="search-input" type="search" maxlength="256" :aria-label="word('搜索影片','Search titles')" :placeholder="word('电影、电视剧，还有更多','Movies, series and more')" @keydown.esc.prevent="query = ''" /><button v-if="query" class="search-clear" :aria-label="word('清除搜索','Clear search')" @click="query = ''; focus()"><Icon name="close" /></button></div>
    <label class="search-provider">{{ word('搜索来源','Search source') }}<select v-model="sourceId" :aria-label="word('搜索来源','Search source')"><option v-for="provider in providers" :key="provider.id" :value="provider.id">{{ provider.name }}</option></select></label>
    <p v-if="!providers.length" class="catalog-status" role="status">{{ word('请添加支持搜索的来源。','Add a source with search support.') }} <a href="#/sources">{{ t('Source manager') }}</a></p>
    <div v-if="!query.trim()" class="search-genres" data-focus-row><a v-for="genre in genres" :key="genre.kind" class="search-genre" :href="`#/browse/${genre.kind}`"><Icon :name="genre.icon" :size="28" /><span>{{ genre.title }}</span></a></div>
    <p v-if="opening" role="status">{{ word('正在打开影片…', 'Opening title…') }}</p>
    <p v-if="app.search.error" class="search-error" role="alert">{{ describeError(app.search.error) }} <button class="glass-pill" @click="search">{{ t('Retry') }}</button></p>
    <div v-if="query.trim()" class="search-results" :class="{ pending:app.search.phase === 'loading' }" :aria-busy="app.search.phase === 'loading'">
      <p v-if="app.search.phase === 'loading'" class="search-loading" role="status">{{ t('Loading…') }}</p>
      <p v-if="approximate" class="search-approximate" role="status">{{ word('没有完全匹配的片名，已显示相近结果。', 'No exact title match; showing close results.') }}</p>
      <MediaRow v-for="group in groups" :key="group.key" :data-kind="group.key" :title="group.title" :items="group.items" :images="app.search.images" @open="open" @image-error="app.searchImageError" />
      <div v-if="app.search.phase === 'empty'" class="catalog-status search-empty" role="status"><Icon name="search" :size="48" /><h2>{{ word('未找到结果','No results found') }}</h2><p>{{ word('试试其他片名或搜索来源。','Try another title or search source.') }}</p></div>
    </div>
  </section>
</template>

<style scoped>
.search-approximate { margin: 0 0 12px; color: var(--muted); }
</style>
