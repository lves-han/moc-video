<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import { NAlert, NButton, NEmpty, NModal } from "naive-ui";
import { appState as state, appController as app, metadataVideoId as videoId } from "../state";
import { locale, t } from "../i18n";
import type { Bridge_Episode, Bridge_ImageRef, Bridge_MediaCard, Bridge_MediaRef, PlaybackMatchSelection } from "../models";

const emit = defineEmits<{ open: [ref: Bridge_MediaRef]; play: [video: string] }>();
const word = (zh: string, en: string) => locale.value === "zh-CN" ? zh : en;
const data = computed(() => state.details.data);
const details = computed(() => data.value?.details ?? null);
const kindLabel = computed(() => ({ movie: word("电影", "Movie"), series: word("剧集", "Series"), anime: word("动画", "Anime"), other: "" })[details.value?.kind ?? "other"]);

const PLOT_LIMIT = 280;
const expanded = ref(false);
const longPlot = computed(() => (details.value?.plot?.length ?? 0) > PLOT_LIMIT);
const plot = computed(() => !longPlot.value || expanded.value ? details.value?.plot ?? "" : `${details.value!.plot!.slice(0, PLOT_LIMIT)}…`);

const chosenGroup = ref<string | null>(null);
// App reuses this view between titles; refreshing the same title keeps its state.
watch(() => details.value?.ref.data, () => { expanded.value = false; chosenGroup.value = null; });
const groups = computed(() => details.value?.groups ?? []);
const group = computed(() => groups.value.find(item => item.id === chosenGroup.value) ?? groups.value[0] ?? null);
const episodes = computed(() => groups.value.flatMap(item => item.episodes));
const single = computed(() => episodes.value.length === 1 ? episodes.value[0]! : null);
const canPlay = (episode: Bridge_Episode) => Boolean(videoId(episode.ref) && state.selected_playback_source && !app.matching.pending && state.details.phase === "ready");
const label = (episode: Bridge_Episode) => episode.number != null && !episode.label.includes(String(episode.number)) ? `${episode.number}. ${episode.label}` : episode.label;

const broken = reactive(new Set<string>());
// The epoch recreates <img> after a refreshed DTO, even when its URL is unchanged.
const image = (reference: Bridge_ImageRef | null | undefined) => {
  const id = reference?.resource_id, access = id ? data.value?.images[id] : undefined, key = `${state.details.request_epoch}:${id}`;
  return id && access && !broken.has(key) ? { id, url: access.url, key } : null;
};
function imageFailed(id: string) { broken.add(`${state.details.request_epoch}:${id}`); void app.detailsImageError(id); }
const relatedPoster = (card: Bridge_MediaCard) => image(card.poster);

const candidate = computed(() => app.matching.details?.details ?? null);
async function confirm(selection: PlaybackMatchSelection) {
  const result = app.matching.result;
  if (!result) return;
  try { if ((await app.match(result.video_id, selection))?.context) location.hash = "#/playback"; }
  catch (cause) { app.matching.error = { code: "BAD_REQUEST", message: String(cause), retryable: false }; }
}
function updateMatchingModal(show: boolean) { if (!show) app.invalidatePlayback(); }
function retry() { if (state.route.name === "details") void app.loadDetails(state.route.ref); }
</script>

<template>
  <a class="details-back" href="#/home">{{ word('返回首页', 'Back to home') }}</a>
  <p v-if="state.details.phase === 'loading' && !details" class="details-loading" role="status" aria-busy="true">{{ t('Loading catalog') }}</p>
  <NAlert v-if="state.details.phase === 'failed'" class="details-failed" type="error">{{ state.details.error?.message }}<NButton class="details-retry" size="small" @click="retry">{{ t('Retry') }}</NButton></NAlert>
  <NAlert v-if="app.detailsImage.error" class="details-image-error" type="error">{{ app.detailsImage.error }}<NButton class="details-retry" size="small" @click="retry">{{ t('Retry') }}</NButton></NAlert>
  <template v-if="details">
    <section class="details-head">
      <div class="details-poster">
        <img v-if="image(details.poster)" :key="image(details.poster)!.key" :src="image(details.poster)!.url" alt="" @error="imageFailed(image(details.poster)!.id)" />
        <span v-else class="details-poster-missing">{{ details.title }}</span>
      </div>
      <div class="details-copy">
        <h1>{{ details.title }}</h1>
        <p v-if="details.year || kindLabel || details.tags.length" class="details-meta">{{ [details.year, kindLabel, ...details.tags].filter(Boolean).join(' · ') }}</p>
        <p v-if="plot" class="details-plot">{{ plot }}</p>
        <NButton v-if="longPlot" class="details-plot-toggle" text :aria-expanded="expanded" @click="expanded = !expanded">{{ expanded ? word('收起', 'Show less') : word('展开', 'Show more') }}</NButton>
        <div v-if="single" class="details-actions"><NButton class="details-play" type="primary" :disabled="!canPlay(single)" :loading="app.matching.pending === videoId(single.ref)" @click="emit('play', videoId(single.ref))">{{ t('Play') }}</NButton></div>
        <p v-if="!state.selected_playback_source" role="status">{{ word('请先选择播放源', 'Choose a playback source first') }}</p>
      </div>
    </section>
    <NAlert v-if="state.playback.error" class="details-play-error" type="error">{{ state.playback.error.message }}</NAlert>
    <p v-if="app.matching.result?.status === 'not_found'" class="details-not-found" role="status">{{ t('No matching playback found') }}</p>
    <NEmpty v-if="!episodes.length" class="details-no-episodes" :description="word('暂无可选剧集', 'No episodes available')" />
    <section v-else-if="!single" class="details-episodes">
      <h2>{{ t('Episodes') }}</h2>
      <div v-if="groups.length > 1" class="details-groups" role="group" :aria-label="t('Episodes')"><NButton v-for="item in groups" :key="item.id" class="details-group" :data-group-id="item.id" :aria-pressed="item.id === group?.id" :type="item.id === group?.id ? 'primary' : 'default'" @click="chosenGroup = item.id">{{ item.title }}</NButton></div>
      <div class="details-episode-grid"><NButton v-for="episode in group?.episodes ?? []" :key="episode.id" class="details-episode" :data-episode-id="episode.id" :data-video-id="videoId(episode.ref)" :disabled="!canPlay(episode)" :loading="app.matching.pending === videoId(episode.ref)" :title="episode.title ?? episode.label" @click="emit('play', videoId(episode.ref))">{{ label(episode) }}</NButton></div>
    </section>
    <section v-if="details.related.length" class="details-related">
      <h2>{{ word('相关内容', 'Related') }}</h2>
      <div class="details-related-grid"><a v-for="card in details.related" :key="card.id" class="details-related-card" :href="`#/details/${encodeURIComponent(JSON.stringify(card.ref))}`" @click.prevent="emit('open', card.ref)"><img v-if="relatedPoster(card)" :key="relatedPoster(card)!.key" :src="relatedPoster(card)!.url" alt="" loading="lazy" @error="imageFailed(relatedPoster(card)!.id)" /><span>{{ card.title }}</span></a></div>
    </section>
  </template>
  <NModal :show="app.matching.result?.status === 'needs_confirmation'" preset="card" role="dialog" class="details-confirm" :title="t('Confirm episode')" :aria-label="t('Confirm episode')" :style="{ width: 'min(640px, calc(100vw - 32px))' }" @update:show="updateMatchingModal">
    <div class="details-candidates"><NButton v-for="item in app.matching.result?.candidates ?? []" :key="item.content_ref.data" class="details-candidate" :loading="app.matching.loading" @click="app.loadCandidate(item.content_ref)">{{ item.title }}<template v-if="item.year"> ({{ item.year }})</template></NButton></div>
    <NAlert v-if="app.matching.error" type="error">{{ app.matching.error.message }}</NAlert>
    <section v-for="item in candidate?.groups ?? []" :key="item.id" class="details-candidate-group" :data-group-id="item.id"><h3>{{ item.title }}</h3><div class="details-episode-grid"><NButton v-for="episode in item.episodes" :key="episode.id" class="details-candidate-episode" :data-episode-id="episode.id" :disabled="Boolean(app.matching.pending)" @click="confirm({ content_ref: candidate!.ref, group_id: item.id, episode_id: episode.id })">{{ episode.label }}</NButton></div></section>
    <NButton class="details-confirm-cancel" @click="app.invalidatePlayback()">{{ t('Cancel') }}</NButton>
  </NModal>
</template>

<style scoped>
.details-back { display: inline-block; margin-bottom: 20px; }
.details-head { display: grid; grid-template-columns: minmax(150px, 240px) minmax(0, 1fr); gap: 28px; margin-bottom: 32px; }
.details-poster { aspect-ratio: 2/3; border-radius: 8px; overflow: hidden; background: var(--control-surface); }
.details-poster img { width: 100%; height: 100%; object-fit: cover; display: block; }
.details-poster-missing { display: flex; height: 100%; align-items: center; justify-content: center; padding: 16px; text-align: center; overflow-wrap: anywhere; }
.details-copy h1, .details-plot { overflow-wrap: anywhere; }
.details-plot { white-space: pre-line; }
.details-actions { margin-top: 20px; }
.details-groups, .details-episode-grid { display: flex; flex-wrap: wrap; gap: 10px; margin-bottom: 18px; }
.details-episode-grid :deep(.n-button) { min-width: 96px; }
.details-candidates { display: flex; flex-wrap: wrap; gap: 10px; margin-bottom: 16px; }
.details-candidate-group h3 { margin: 12px 0 8px; }
.details-confirm-cancel { margin-top: 12px; }
.details-retry { margin-inline-start: 12px; }
.details-related-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(150px, 1fr)); gap: 18px; max-width: 1325px; }
.details-related-card { display: block; max-width: 180px; overflow-wrap: anywhere; }
.details-related-card img { width: 100%; aspect-ratio: 2/3; object-fit: cover; border-radius: 8px; display: block; }
@media (max-width: 600px) { .details-head { grid-template-columns: minmax(0, 1fr); } .details-poster { max-width: 200px; } }
</style>
