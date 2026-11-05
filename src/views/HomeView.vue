<script setup lang="ts">
import { computed, reactive } from "vue";
import { NAlert, NButton, NEmpty } from "naive-ui";
import { appState as state, appController as app } from "../state";
import { locale, t } from "../i18n";
import { describeError } from "../api";
import type { Bridge_MediaCard, Bridge_MediaRef } from "../models";

const emit = defineEmits<{ open: [ref: Bridge_MediaRef]; select: [id: string, role: "metadata" | "playback"] }>();
const word = (zh: string, en: string) => locale.value === "zh-CN" ? zh : en;
const providers = computed(() => (state.sources.data?.providers ?? []).filter(source => source.runtime_status === "ready"));
const metadataSources = computed(() => providers.value.filter(source => source.kind === "stremio" && source.capabilities.home && source.capabilities.details));
const playbackSources = computed(() => providers.value.filter(source => source.kind !== "stremio" && source.capabilities.resolve));
const rows = computed(() => [
  ...(state.home.data?.page.sections ?? []).map(section => ({ index: section.index, title: section.title, ref: section.section_ref, section, failed: null })),
  ...(state.home.data?.page.failed_sections ?? []).map(failed => ({ index: failed.index, title: failed.title, ref: failed.section_ref, section: null, failed })),
].sort((a, b) => a.index - b.index));
const firstLoad = computed(() => state.home.phase === "loading" && !state.home.data);
const broken = reactive(new Set<string>());
// The epoch recreates <img> after a refreshed snapshot, even when its URL is unchanged.
const imageKey = (id: string) => `${state.home.request_epoch}:${id}`;
const poster = (card: Bridge_MediaCard) => {
  const id = card.poster?.resource_id, image = id ? state.home.data?.images[id] : undefined;
  return id && image && !broken.has(imageKey(id)) ? { id, url: image.url, key: imageKey(id) } : null;
};
function imageFailed(id: string) { broken.add(imageKey(id)); void app.homeImageError(id); }
const detail = (card: Bridge_MediaCard) => [card.year, card.update_label ?? card.subtitle].filter(Boolean).join(" · ");
async function refresh() {
  try { await app.refreshRuntime(); await app.loadSources(); if (state.selected_metadata_source) await app.loadHome(); }
  catch (cause) { app.diagnostics.push({ code: "ENGINE_NOT_READY", message: String(cause), retryable: true }); }
}
</script>

<template>
  <div class="catalog-sources">
    <label>{{ t('Metadata provider') }}<select class="home-metadata-source" :value="state.selected_metadata_source?.source_id ?? ''" @change="emit('select', ($event.target as HTMLSelectElement).value, 'metadata')"><option value="" disabled>{{ t('Choose a metadata source') }}</option><option v-for="source in metadataSources" :key="source.id" :value="source.id">{{ source.name }}</option></select></label>
    <label>{{ word('播放源', 'Playback source') }}<select class="home-playback-source" :value="state.selected_playback_source?.source_id ?? ''" @change="emit('select', ($event.target as HTMLSelectElement).value, 'playback')"><option value="" disabled>{{ t('Choose a video source') }}</option><option v-for="source in playbackSources" :key="source.id" :value="source.id">{{ source.name }}</option></select></label>
    <NButton class="home-refresh" :loading="state.sources.phase === 'loading'" @click="refresh">{{ t('Refresh catalog') }}</NButton>
  </div>
  <p v-if="state.runtime.phase !== 'ready'" role="status">{{ word('宿主准备中', 'Preparing the app engine') }}</p>
  <p v-if="state.sources.phase !== 'loading' && !metadataSources.length" class="home-setup" role="status">{{ word('还没有可用的元数据来源。', 'No metadata source is available yet.') }} <a href="#/setup">{{ word('前往设置来源', 'Set up sources') }}</a></p>
  <NAlert v-if="state.sources.error || state.runtime.error" type="error">{{ state.sources.error?.message ?? state.runtime.error?.message }}</NAlert>
  <NAlert v-if="state.home.phase === 'failed' && state.home.error" class="home-failed" type="error">{{ state.home.error.message }}<NButton class="home-retry" size="small" @click="app.loadHome()">{{ t('Retry') }}</NButton></NAlert>
  <NAlert v-else-if="app.homeImage.error" class="home-image-error" type="error">{{ app.homeImage.error }}<NButton size="small" @click="app.loadHome()">{{ t('Retry') }}</NButton></NAlert>
  <div v-if="firstLoad" class="film-grid home-grid" role="status" aria-busy="true" :aria-label="t('Loading catalog')"><div v-for="n in 12" :key="n" class="skeleton-card"><div /><span /></div></div>
  <div v-else-if="state.home.phase === 'empty'" class="home-empty"><NEmpty :description="word('该源暂无内容', 'This source has no content')" /><NButton class="home-retry" @click="app.loadHome()">{{ t('Retry') }}</NButton></div>
  <section v-for="row in rows" :key="row.index" class="catalog-section home-section" :data-section-index="row.index">
    <div class="section-heading"><h2>{{ row.title }}</h2></div>
    <NAlert v-if="row.failed || app.sections[row.ref ?? '']?.error" class="home-section-error" type="error" role="alert">{{ app.sections[row.ref ?? '']?.error?.message ?? (row.failed && describeError(row.failed.error)) }}<NButton class="home-section-retry" size="small" :loading="app.sections[row.ref ?? '']?.phase === 'loading'" :disabled="!row.ref" @click="app.loadHome(row.ref)">{{ t('Retry') }}</NButton></NAlert>
    <div v-if="row.section" class="film-grid home-grid">
      <article v-for="card in row.section.items" :key="card.id" class="film-card home-card" :data-card-id="card.id">
        <a :href="`#/details/${encodeURIComponent(JSON.stringify(card.ref))}`" class="poster-link" :aria-label="t('View details for {title}', { title: card.title })" @click.prevent="emit('open', card.ref)">
          <img v-if="poster(card)" :key="poster(card)!.key" :src="poster(card)!.url" alt="" width="400" height="600" loading="lazy" @error="imageFailed(poster(card)!.id)" />
          <span v-else class="home-poster-missing poster-lettering"><strong>{{ card.title }}</strong></span>
        </a>
        <a :href="`#/details/${encodeURIComponent(JSON.stringify(card.ref))}`" class="card-title home-card-title" :title="card.title" @click.prevent="emit('open', card.ref)">{{ card.title }}</a>
        <p v-if="detail(card)">{{ detail(card) }}</p>
      </article>
    </div>
    <NButton v-if="row.section?.has_next && row.section.next_page" class="home-load-more" :loading="app.sections[row.ref ?? '']?.phase === 'loading'" @click="app.loadHome(row.ref, row.section!.next_page!)">{{ t('Load more') }}</NButton>
  </section>
</template>

<style scoped>
.catalog-sources { display: flex; flex-wrap: wrap; gap: 16px; align-items: center; margin-bottom: 24px; }
label { display: flex; flex-wrap: wrap; gap: 12px; align-items: center; }
select { min-height: 44px; max-width: min(100%, 340px); padding: 8px; color: var(--text); background: var(--surface); border: 1px solid var(--border); }
/* 4..7 cards per row: columns are counted at the 150px minimum (a fixed 180px maximum would leave three
   at 764px), cards stop at 180px, and 1325px is the widest grid before an eighth column fits. */
.film-grid.home-grid { grid-template-columns: repeat(auto-fill, minmax(150px, 1fr)); max-width: 1325px; gap: 25px 18px; }
.home-card { max-width: 180px; }
.home-card-title { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.home-poster-missing { position: absolute; inset: 0; display: flex; align-items: center; justify-content: center; padding: 12px; text-align: center; overflow-wrap: anywhere; overflow: hidden; }
.home-load-more { margin-top: 20px; }
.home-section-retry, .home-retry { margin-inline-start: 12px; }
.home-empty { display: grid; justify-items: center; gap: 12px; padding: 48px 0; }
@media (max-width: 480px) { .film-grid.home-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 20px 12px; } }
</style>
