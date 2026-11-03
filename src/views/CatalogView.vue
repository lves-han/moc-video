<script setup lang="ts">
import { computed } from "vue";
import { NAlert, NButton, NEmpty, NModal } from "naive-ui";
import { appState as state, appController as app } from "../state";
import { t } from "../i18n";
import type { Bridge_MediaRef, PlaybackMatchSelection } from "../models";

const emit = defineEmits<{ open: [ref: Bridge_MediaRef]; play: [video: string]; select: [id: string, role: "metadata" | "playback"] }>();
const sections = computed(() => [
  ...(state.home.data?.page.sections ?? []).map(section => ({ index: section.index, section, failed: null })),
  ...(state.home.data?.page.failed_sections ?? []).map(failed => ({ index: failed.index, section: null, failed })),
].sort((a, b) => a.index - b.index));
const page = computed(() => state.route.name === "details" ? state.details : state.home);
function videoId(ref: Bridge_MediaRef) {
  try { const value = JSON.parse(ref.data).video_id; return typeof value === "string" ? value : ""; }
  catch { return ""; }
}
async function refresh() {
  try { await app.refreshRuntime(); await app.loadSources(); if (state.selected_metadata_source) await app.loadHome(); }
  catch (cause) { app.diagnostics.push({ code: "ENGINE_NOT_READY", message: String(cause), retryable: true }); }
}
async function confirm(selection: PlaybackMatchSelection) {
  const result = app.matching.result;
  if (!result) return;
  try { if ((await app.match(result.video_id, selection))?.context) location.hash = "#/playback"; }
  catch (cause) { app.matching.error = { code: "BAD_REQUEST", message: String(cause), retryable: false }; }
}
</script>

<template>
  <div v-if="state.route.name === 'sources' || state.route.name === 'home'" class="catalog-sources">
    <label>{{ t('Metadata provider') }}<select :value="state.selected_metadata_source?.source_id ?? ''" @change="emit('select', ($event.target as HTMLSelectElement).value, 'metadata')"><option value="" disabled>{{ t('Choose a metadata source') }}</option><option v-for="source in state.sources.data?.providers.filter(source => source.kind === 'stremio' && source.runtime_status === 'ready' && source.capabilities.home && source.capabilities.details)" :key="source.id" :value="source.id">{{ source.name }}</option></select></label>
    <NButton :loading="state.sources.phase === 'loading'" @click="refresh">{{ t('Refresh catalog') }}</NButton>
  </div>
  <NAlert v-if="state.sources.error || state.runtime.error" type="error">{{ state.sources.error?.message ?? state.runtime.error?.message }}</NAlert>
  <template v-if="state.route.name === 'sources'">
    <h1>{{ t('Source manager') }}</h1>
    <article v-for="source in state.sources.data?.providers ?? []" :key="source.id" class="settings-panel"><h2>{{ source.name }}</h2><p>{{ source.kind }} · {{ source.runtime_status }}</p><NButton :disabled="source.runtime_status !== 'ready'" @click="emit('select', source.id, source.kind === 'stremio' ? 'metadata' : 'playback')">{{ t('Select source') }}</NButton></article>
  </template>
  <template v-else>
    <p v-if="page.phase === 'loading'" role="status" aria-busy="true">{{ t('Loading catalog') }}</p>
    <NAlert v-if="page.error" type="error">{{ page.error.message }}</NAlert>
    <NEmpty v-if="page.phase === 'empty'" :description="t('No titles found')" />
    <template v-if="state.route.name === 'home'">
      <section v-for="row in sections" :key="row.index" class="catalog-section">
        <div class="section-heading"><h2>{{ row.section?.title ?? row.failed?.title }}</h2><NButton :loading="app.sections[(row.section?.section_ref ?? row.failed?.section_ref) ?? '']?.phase === 'loading'" :disabled="!row.section?.section_ref && !row.failed?.section_ref" @click="app.loadHome(row.section?.section_ref ?? row.failed!.section_ref)">{{ t('Retry') }}</NButton></div>
        <NAlert v-if="app.sections[row.section?.section_ref ?? row.failed?.section_ref ?? '']?.error || row.failed" type="error">{{ app.sections[row.section?.section_ref ?? row.failed?.section_ref ?? '']?.error?.message ?? row.failed?.error.message }}</NAlert>
        <div v-if="row.section" class="film-grid">
          <article v-for="card in row.section.items" :key="card.id" class="film-card"><a :href="`#/details/${encodeURIComponent(JSON.stringify(card.ref))}`" class="poster-link" :aria-label="t('View details for {title}', { title: card.title })" @click.prevent="emit('open', card.ref)"><img v-if="card.poster && state.home.data?.images[card.poster.resource_id]" :src="state.home.data.images[card.poster.resource_id]!.url" alt="" width="400" height="600" loading="lazy" /><span class="poster-lettering"><strong>{{ card.title }}</strong></span></a><a :href="`#/details/${encodeURIComponent(JSON.stringify(card.ref))}`" class="card-title" @click.prevent="emit('open', card.ref)">{{ card.title }}</a></article>
        </div>
        <NButton v-if="row.section?.has_next && row.section.next_page" :loading="app.sections[row.section!.section_ref]?.phase === 'loading'" @click="app.loadHome(row.section!.section_ref, row.section!.next_page!)">{{ t('Load more') }}</NButton>
      </section>
    </template>
    <template v-else-if="state.route.name === 'details' && state.details.phase === 'ready' && state.details.data">
      <h1>{{ state.details.data.details.title }}</h1><p>{{ state.details.data.details.plot }}</p>
      <section v-for="group in state.details.data.details.groups" :key="group.id" class="episodes-section"><h2>{{ group.title }}</h2><div class="episode-grid"><NButton v-for="episode in group.episodes" :key="episode.id" :disabled="!videoId(episode.ref) || !state.selected_playback_source" @click="emit('play', videoId(episode.ref))">{{ episode.label }}</NButton></div></section>
      <NAlert v-if="state.playback.error" type="error">{{ state.playback.error.message }}</NAlert>
      <p v-if="app.matching.result?.status === 'not_found'" role="status">{{ t('No matching playback found') }}</p>
    </template>
  </template>
  <NModal :show="app.matching.result?.status === 'needs_confirmation'" preset="card" role="dialog" :title="t('Confirm episode')" :aria-label="t('Confirm episode')" :style="{ width: 'min(640px, calc(100vw - 32px))' }" @update:show="value => { if (!value) app.invalidatePlayback(); }">
    <NButton v-for="candidate in app.matching.result?.candidates ?? []" :key="candidate.content_ref.data" @click="app.loadCandidate(candidate.content_ref)">{{ candidate.title }}</NButton>
    <p v-if="app.matching.loading" role="status">{{ t('Loading catalog') }}</p>
    <NAlert v-if="app.matching.error" type="error">{{ app.matching.error.message }}</NAlert>
    <section v-for="group in app.matching.details?.details.groups ?? []" :key="group.id"><h3>{{ group.title }}</h3><div class="episode-grid"><NButton v-for="episode in group.episodes" :key="episode.id" @click="confirm({ content_ref: app.matching.details!.details.ref, group_id: group.id, episode_id: episode.id })">{{ episode.label }}</NButton></div></section>
    <NButton @click="app.invalidatePlayback()">{{ t('Cancel') }}</NButton>
  </NModal>
</template>

<style scoped>
.catalog-sources { display: flex; flex-wrap: wrap; gap: 16px; align-items: center; margin-bottom: 24px; }
label { display: flex; flex-wrap: wrap; gap: 12px; align-items: center; }
select { min-height: 44px; max-width: min(100%, 340px); padding: 8px; color: var(--text); background: var(--surface); border: 1px solid var(--border); }
.poster-link { background: var(--surface); aspect-ratio: 2/3; }
h1, h2, p { overflow-wrap: anywhere; }
</style>
