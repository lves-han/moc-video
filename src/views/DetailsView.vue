<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, reactive, ref, watch } from "vue";
import { NAlert, NButton, NEmpty, NModal } from "naive-ui";
import { appState as state, appController as app, metadataVideoId as videoId } from "../state";
import { api } from "../api";
import Icon from '../components/Icon.vue';
import MediaRow from '../components/MediaRow.vue';
import { locale, t } from "../i18n";
import { correspondingEpisode, prioritizePlaybackGroups } from '../line-preference';
import type { Bridge_Episode, Bridge_EpisodeGroup, Bridge_ImageRef, PlaybackMatchSelection, WatchHistoryRecord } from "../models";

const emit = defineEmits<{ open: [ref: import('../models').Bridge_MediaRef]; play: [video: string, explicitLine?: boolean, resumePosition?: number]; resume: [record: WatchHistoryRecord]; cancelPlayback: []; back: [] }>();
const word = (zh: string, en: string) => locale.value === "zh-CN" ? zh : en;
const native = Boolean(window.__TAURI__);
const data = computed(() => state.details.data);
const details = computed(() => data.value?.details ?? null);
const kindLabel = computed(() => ({ movie: word("电影", "Movie"), series: word("剧集", "Series"), anime: word("动画", "Anime"), other: "" })[details.value?.kind ?? "other"]);
const expanded = ref(false), chosenGroup = ref<string | null>(null);
const plotElement = ref<HTMLElement>(), longPlot = ref(false);
let observer: ResizeObserver | undefined;
watch(plotElement, element => {
  observer?.disconnect();
  if (element) { observer = new ResizeObserver(() => { longPlot.value = element.scrollHeight > element.clientHeight + 1; }); observer.observe(element); }
});
const groups = computed(() => prioritizePlaybackGroups(details.value?.groups ?? []));
const group = computed(() => groups.value.find(item => item.id === chosenGroup.value) ?? groups.value[0] ?? null);
const episodes = computed(() => groups.value.flatMap(item => item.episodes));
const single = computed(() => episodes.value.length === 1 ? episodes.value[0]! : null);
const canPlay = (episode: Bridge_Episode) => Boolean(videoId(episode.ref) && (state.settings.playback_service?.enabled || !native && state.selected_playback_source) && !app.matching.pending && state.details.phase === "ready");
const label = (episode: Bridge_Episode) => `${episode.number ?? episode.index + 1}. ${episode.title ?? episode.label}`;
const minutes = (ms: number) => Math.max(0, Math.ceil(ms / 60000));
const history = ref<WatchHistoryRecord | null>(null), listed = ref(false), watchlistPending = ref(false), savedError = ref('');
let savedGeneration = 0;
async function loadSaved() {
  const current = ++savedGeneration, title = details.value; history.value = null; listed.value = false; savedError.value = '';
  if (!title) return;
  try {
    const [past, watchlist] = await Promise.all([api.call('list_history', {}), api.call('list_watchlist', {})]);
    if (current !== savedGeneration) return;
    history.value = past.entries.find(entry => entry.content_id === title.id && JSON.stringify(entry.metadata_identity) === JSON.stringify(data.value?.metadata_ref?.identity))
      ?? past.entries.find(entry => entry.content_id === title.id) ?? null;
    listed.value = watchlist.entries.some(entry => entry.content_id === title.id);
    await nextTick(); if (current === savedGeneration && plotElement.value) longPlot.value = plotElement.value.scrollHeight > plotElement.value.clientHeight + 1;
  } catch (cause) { if (current === savedGeneration) savedError.value = String(cause); }
}
watch([() => details.value?.id, () => state.details.request_epoch, () => state.runtime.engine_epoch], () => {
  expanded.value = false; chosenGroup.value = null; void loadSaved();
}, { immediate: true });
const resumeEpisode = computed(() => history.value && !history.value.completed && history.value.position_ms > 0
  ? episodes.value.find(episode => videoId(episode.ref) === history.value!.video_id) : undefined);
const primary = computed(() => resumeEpisode.value ?? single.value ?? group.value?.episodes[0]);
const primaryLabel = computed(() => {
  const saved = history.value;
  if (!resumeEpisode.value || !saved) return t('Play');
  const episode = saved.episode_number ?? resumeEpisode.value.number ?? resumeEpisode.value.index + 1;
  const left = saved.duration_ms ? Math.max(0, saved.duration_ms - saved.position_ms) : null;
  return word(`继续播放 · 第 ${episode} 集${left === null ? '' : ` · 剩 ${minutes(left)} 分钟`}`,
    `Continue playing · Episode ${episode}${left === null ? '' : ` · ${minutes(left)} min left`}`);
});
const progress = (episode: Bridge_Episode | undefined) => episode && history.value?.video_id === videoId(episode.ref) && history.value.duration_ms
  ? Math.min(100, history.value.position_ms / history.value.duration_ms * 100) : 0;
function playPrimary() { if (resumeEpisode.value && history.value) emit('resume', history.value); else if (primary.value) emit('play', videoId(primary.value.ref), chosenGroup.value !== null); }
async function toggleWatchlist() {
  const snapshot = data.value, current = savedGeneration;
  if (!snapshot?.metadata_ref || watchlistPending.value) return;
  watchlistPending.value = true; savedError.value = '';
  try {
    if (listed.value) await api.call('remove_watchlist', { content_id: snapshot.details.id });
    else await api.call('add_watchlist', { content_id: snapshot.details.id, metadata_identity: snapshot.metadata_ref.identity,
      title: snapshot.details.title, kind: snapshot.details.kind, poster_cache_key: null });
    if (current === savedGeneration) listed.value = !listed.value;
  } catch (cause) { if (current === savedGeneration) savedError.value = String(cause); }
  finally { watchlistPending.value = false; }
}
const broken = reactive(new Set<string>());
const image = (reference: Bridge_ImageRef | null | undefined) => {
  const id = reference?.resource_id, access = id ? data.value?.images[id] : undefined, key = `${state.details.request_epoch}:${id}`;
  return id && access && !broken.has(key) ? { id, url: access.url, key } : null;
};
function imageFailed(id: string) { broken.add(`${state.details.request_epoch}:${id}`); void app.detailsImageError(id); }
// Current providers publish the title; a provider-supplied logo takes precedence when present.
const logo = computed(() => image((details.value as typeof details.value & { logo?: Bridge_ImageRef })?.logo));
const backdrop = computed(() => image(details.value?.backdrop) ?? image(details.value?.poster));
const candidate = computed(() => app.matching.details?.details ?? null);
const candidateGroups = computed(() => prioritizePlaybackGroups(candidate.value?.groups ?? []));
function candidateEpisodes(group: Bridge_EpisodeGroup) {
  const target = episodes.value.find(episode => videoId(episode.ref) === app.matching.result?.video_id);
  const match = details.value && target ? correspondingEpisode(details.value, target, group) : null;
  return match ? [match] : group.episodes;
}
async function confirm(selection: PlaybackMatchSelection) {
  const result = app.matching.result;
  if (!result) return;
  try { if ((await app.match(result.video_id, selection))?.context) location.hash = "#/playback"; }
  catch (cause) { app.matching.error = { code: "BAD_REQUEST", message: String(cause), retryable: false }; }
}
function updateMatchingModal(show: boolean) { if (!show) app.invalidatePlayback(); }
function updateServiceModal(show: boolean) {
  if (show) return;
  app.invalidatePlayback();
  emit('cancelPlayback');
  if (state.route.name === 'details') location.hash = `#/details/${encodeURIComponent(JSON.stringify(state.route.ref))}`;
}
function retryServiceRound() { void app.startServiceRound(app.serviceRound.videoId); }
const sourceGroups = computed(() => (app.serviceRound.snapshot?.sources ?? []).map(source => ({
  ...source,
  lines: (app.serviceRound.snapshot?.lines ?? []).filter(line => line.source_id === source.source_id),
  candidates: (app.serviceRound.snapshot?.candidates ?? []).filter(candidate => candidate.source_id === source.source_id),
})));
function sourceStatus(source: (typeof sourceGroups.value)[number]) {
  if (source.lines.some(line => line.probe_status === 'ready')) return word('可播放', 'Available');
  if (source.lines.some(line => line.probe_status === 'testing') || ['pending', 'searching'].includes(source.status)) return word('检测中', 'Checking');
  if (source.candidates.length || source.status === 'needs_confirmation') return word('需要确认影片', 'Confirm title');
  if (source.status === 'disabled') return word('已停用', 'Disabled');
  if (source.status === 'no_episode') return word('无此集', 'Episode unavailable');
  if (source.status === 'not_found') return word('未找到影片', 'Not found');
  return word('暂不可用', 'Unavailable');
}
function lineStatus(status: string) {
  return ({ ready: word('可播放', 'Available'), testing: word('检测中', 'Checking'), timeout: word('超时', 'Timed out'), failed: word('检测失败', 'Check failed'), unsupported: word('暂不支持', 'Unsupported'), unknown: word('未检测，可刷新', 'Unchecked; refresh') } as Record<string, string>)[status] ?? status;
}
function lineReason(reason: string | null) {
  return ({ needs_sniffing: word('需要解析接口', 'Parser required'), unsupported_media_format: word('格式不支持', 'Unsupported format'), external_url_unsupported: word('外部网页', 'External page') } as Record<string, string>)[reason ?? ''] ?? '';
}
function formatSize(bytes: number | null | undefined) {
  if (!Number.isSafeInteger(bytes) || !bytes || bytes < 0) return '';
  const units = ['B', 'KB', 'MB', 'GB', 'TB']; let value = bytes, unit = 0;
  while (value >= 1024 && unit < units.length - 1) { value /= 1024; unit++; }
  return `${unit ? value.toFixed(value < 10 ? 1 : 0) : value} ${units[unit]}`;
}
function retry() { if (state.route.name === "details") void app.loadDetails(state.route.ref); }
onBeforeUnmount(() => { observer?.disconnect(); savedGeneration++; });
</script>

<template>
  <div class="tv-details">
  <a class="details-back" href="#/home" @click.prevent="emit('back')">{{ word('返回', 'Back') }}</a>
  <section v-if="state.details.phase === 'loading' && !details" class="details-loading" role="status" aria-busy="true">
    <div class="details-loading-art" aria-hidden="true"><div class="details-loading-copy"><span /><span /><span /></div></div>
    <p>{{ word('正在加载影片详情…', 'Loading details…') }}</p>
  </section>
  <NAlert v-if="state.details.phase === 'failed'" class="details-failed" type="error">{{ state.details.error?.message }}<NButton class="details-retry" size="small" @click="retry">{{ t('Retry') }}</NButton></NAlert>
  <NAlert v-if="app.detailsImage.error" class="details-image-error" type="error">{{ app.detailsImage.error }}<NButton class="details-retry" size="small" @click="retry">{{ t('Retry') }}</NButton></NAlert>
  <template v-if="details">
    <section class="details-head" :class="{ 'details-backdrop-fallback': !details.backdrop }">
      <img v-if="backdrop" :key="backdrop.key" class="details-backdrop" :src="backdrop.url" alt="" @error="imageFailed(backdrop.id)" />
      <div class="details-bottom-mask" /><div class="details-left-mask" />
      <div class="details-copy">
        <img v-if="logo" class="details-logo" :src="logo.url" :alt="details.title" @error="imageFailed(logo.id)" />
        <h1 v-else>{{ details.title }}</h1>
        <p class="details-meta"><span v-if="details.rating?.value" class="details-rating">{{ details.rating.value }} ★</span><span v-if="details.year">{{ details.year }}</span><span v-if="details.tags.length || kindLabel">{{ details.tags.join(' / ') || kindLabel }}</span><span v-if="details.duration_ms">{{ minutes(details.duration_ms) }} {{ word('分钟', 'min') }}</span><span v-if="details.regions?.length">{{ details.regions.join(' / ') }}</span></p>
        <div v-if="details.plot" class="details-synopsis"><p ref="plotElement" class="details-plot">{{ details.plot }}</p><button v-if="longPlot" class="details-plot-toggle" :aria-expanded="expanded" @click="expanded = true">{{ word('更多', 'More') }}</button></div>
        <div class="details-actions">
          <NButton v-if="primary" class="details-play" :disabled="!canPlay(primary)" :loading="app.matching.pending === videoId(primary.ref)" @click="playPrimary"><Icon name="play" :size="20" />{{ primaryLabel }}<span v-if="progress(primary)" class="details-play-progress" :style="{ width: `${progress(primary)}%` }" /></NButton>
          <NButton v-if="primary && native && state.settings.playback_service?.enabled" :disabled="!canPlay(primary)" @click="emit('play', videoId(primary.ref), true, resumeEpisode && history ? history.position_ms : undefined)">{{ word('选择线路', 'Choose line') }}</NButton>
          <button class="details-watchlist glass-pill" :disabled="watchlistPending || !data?.metadata_ref" :aria-pressed="listed" :aria-label="listed ? word('移出待播清单', 'Remove from watchlist') : word('加入待播清单', 'Add to watchlist')" @click="toggleWatchlist"><span :key="String(listed)" class="watchlist-symbol">{{ listed ? '✓' : '+' }}</span></button>
        </div>
        <p v-if="native && !state.settings.playback_service?.enabled" role="status">{{ word('请先连接播放服务', 'Connect the playback service first') }}</p>
        <p v-else-if="!native && !state.selected_playback_source" role="status">{{ word('请先选择播放源', 'Choose a playback source first') }}</p>
      </div>
    </section>
    <p v-if="savedError" class="details-saved-error" role="alert">{{ savedError }} <button class="glass-pill" @click="loadSaved">{{ t('Retry') }}</button></p>
    <p v-if="app.lineMemory.error" class="details-saved-error" role="status">{{ app.lineMemory.error }}</p>
    <NAlert v-if="state.playback.error" class="details-play-error" type="error">{{ state.playback.error.message }}</NAlert>
    <p v-if="app.matching.result?.status === 'not_found'" class="details-not-found" role="status">{{ t('No matching playback found') }}</p>
    <NEmpty v-if="!episodes.length" class="details-no-episodes" :description="word('暂无可选剧集', 'No episodes available')" />
    <section v-else-if="!single" class="details-episodes details-content">
      <h2>{{ t('Episodes') }}</h2>
      <div v-if="groups.length > 1" class="details-groups" role="group" :aria-label="t('Episodes')" data-focus-row><button v-for="item in groups" :key="item.id" class="details-group filter-pill" :class="{ selected: item.id === group?.id }" :data-group-id="item.id" :aria-pressed="item.id === group?.id" @click="chosenGroup = item.id">{{ item.title }}</button></div>
      <div class="details-episode-grid" data-focus-row><button v-for="episode in group?.episodes ?? []" :key="episode.id" class="details-episode" :data-episode-id="episode.id" :data-video-id="videoId(episode.ref)" :disabled="!canPlay(episode)" :aria-busy="app.matching.pending === videoId(episode.ref)" @click="emit('play', videoId(episode.ref), chosenGroup !== null)">
        <span class="details-episode-art"><img v-if="image(episode.poster)" :key="image(episode.poster)!.key" :src="image(episode.poster)!.url" alt="" loading="lazy" @error="imageFailed(image(episode.poster)!.id)" /><template v-else><img v-if="backdrop" class="details-episode-fallback" :src="backdrop.url" alt="" /><span class="details-episode-number">{{ episode.number ?? episode.index + 1 }}</span></template><span v-if="resumeEpisode?.id === episode.id" class="details-watching">{{ word('正在观看', 'Watching') }}</span><span v-if="progress(episode)" class="details-episode-progress"><span :style="{ width: `${progress(episode)}%` }" /></span></span>
        <strong>{{ label(episode) }}</strong><span v-if="episode.plot" class="details-episode-plot">{{ episode.plot }}</span><span v-if="episode.duration_ms" class="details-episode-duration">{{ minutes(episode.duration_ms) }} {{ word('分钟', 'min') }}</span>
      </button></div>
    </section>
    <section v-if="details.actors?.length" class="details-cast details-content"><h2>{{ word('演员与角色', 'Cast & crew') }}</h2><div class="details-cast-track"><article v-for="(actor, index) in details.actors" :key="index" class="details-actor"><img v-if="image(actor.image)" :src="image(actor.image)!.url" alt="" loading="lazy" @error="imageFailed(image(actor.image)!.id)" /><span v-else class="details-actor-initial">{{ actor.name.slice(0, 1) }}</span><p>{{ actor.name }}</p><span v-if="actor.role" class="details-actor-role">{{ actor.role }}</span></article></div></section>
    <section v-if="details.related.length" class="details-related details-content"><MediaRow :title="word('相关内容', 'Related')" :items="details.related" :images="data?.images" @open="emit('open', $event)" @image-error="imageFailed" /></section>
    <section class="details-information details-content"><h2>{{ word('关于', 'About') }}</h2><dl>
      <div><dt>{{ word('类型', 'Genre') }}</dt><dd>{{ details.tags.join(' / ') || kindLabel || '—' }}</dd></div><div><dt>{{ word('年份', 'Year') }}</dt><dd>{{ details.year ?? '—' }}</dd></div><div><dt>{{ word('地区', 'Region') }}</dt><dd>{{ details.regions?.join(' / ') || '—' }}</dd></div><div><dt>{{ word('语言', 'Language') }}</dt><dd>{{ details.languages?.join(' / ') || '—' }}</dd></div><div><dt>{{ word('导演', 'Director') }}</dt><dd>{{ details.directors?.map(person => person.name).join(' / ') || '—' }}</dd></div><div><dt>{{ word('时长', 'Duration') }}</dt><dd>{{ details.duration_ms ? `${minutes(details.duration_ms)} ${word('分钟', 'min')}` : '—' }}</dd></div>
    </dl></section>
    <NModal v-model:show="expanded" preset="card" role="dialog" class="tv-dialog details-plot-modal" :title="details.title" :style="{ width: 'min(640px, calc(100vw - 32px))' }"><p>{{ details.plot }}</p></NModal>
  </template>
  <NModal :show="app.matching.result?.status === 'needs_confirmation'" preset="card" role="dialog" class="tv-dialog details-confirm" :title="word('选择播放线路', 'Choose a playback line')" :aria-label="word('选择播放线路', 'Choose a playback line')" :style="{ width: 'min(640px, calc(100vw - 32px))' }" @update:show="updateMatchingModal">
    <p v-if="app.matching.loading" role="status">{{ word('正在加载可用线路…', 'Loading available lines…') }}</p>
    <div v-if="(app.matching.result?.candidates.length ?? 0) > 1 || (!candidate && !app.matching.loading)" class="details-candidates"><NButton v-for="item in app.matching.result?.candidates ?? []" :key="item.content_ref.data" class="details-candidate" :class="{ selected: candidate?.ref.data === item.content_ref.data }" :loading="app.matching.loading" @click="app.loadCandidate(item.content_ref)">{{ item.title }}<template v-if="item.year"> ({{ item.year }})</template></NButton></div>
    <NAlert v-if="app.matching.error" type="error">{{ app.matching.error.message }}</NAlert>
    <section v-for="item in candidateGroups" :key="item.id" class="details-candidate-group" :data-group-id="item.id"><h3>{{ item.title }}</h3><div class="details-episode-grid"><NButton v-for="episode in candidateEpisodes(item)" :key="episode.id" class="details-candidate-episode" :data-episode-id="episode.id" :disabled="Boolean(app.matching.pending)" @click="confirm({ content_ref: candidate!.ref, group_id: item.id, episode_id: episode.id })">{{ episode.label }}</NButton></div></section>
    <NButton class="details-confirm-cancel" @click="app.invalidatePlayback()">{{ t('Cancel') }}</NButton>
  </NModal>
  <NModal :show="app.serviceRound.open" preset="card" role="dialog" class="tv-dialog details-confirm" :title="app.serviceRound.autoSelecting ? word('正在准备下一集', 'Preparing next episode') : word('选择播放线路', 'Choose a playback line')" :style="{ width: 'min(640px, calc(100vw - 32px))' }" @update:show="updateServiceModal">
    <p v-if="app.serviceRound.autoSelecting" role="status">{{ word('正在查找上一集的播放线路…', 'Finding the previous episode’s playback line…') }}</p>
    <p v-else-if="!app.serviceRound.snapshot?.query_done || app.serviceRound.snapshot.pending || app.serviceRound.snapshot.pending_confirm" role="status">{{ word('正在查找和测试线路…', 'Finding and testing lines…') }}</p>
    <NAlert v-if="app.serviceRound.error" type="error">{{ app.serviceRound.error }} <NButton size="small" @click="retryServiceRound">{{ t('Retry') }}</NButton></NAlert>
    <template v-if="!app.serviceRound.autoSelecting">
    <section v-for="source in sourceGroups" :key="source.source_id" class="line-source">
      <div class="line-source-heading"><strong>{{ source.name || source.source_id }}</strong><span>{{ sourceStatus(source) }}</span></div>
      <div v-for="line in source.lines" :key="line.line_id" class="line-source-row">
        <button class="line-choice" :disabled="line.probe_status !== 'ready' || app.serviceRound.busy" @click="app.chooseServiceLine(line.line_id)">
          <span class="line-choice-name">{{ line.line_name }}</span>
          <span v-if="line.description" class="line-choice-description">{{ line.description }}</span>
          <span class="line-choice-meta"><span v-if="line.quality">{{ line.quality }}</span><span v-if="formatSize(line.size_bytes)">{{ formatSize(line.size_bytes) }}</span><span>{{ lineReason(line.reason) || lineStatus(line.probe_status) }}</span><span v-if="line.latency_ms !== null">{{ word('服务端', 'Server') }} {{ line.latency_ms }} ms</span><span v-if="line.availability_expires_at">{{ word('缓存至', 'Cached until') }} {{ new Date(line.availability_expires_at * 1000).toLocaleString(locale) }}</span></span>
        </button>
        <NButton v-if="line.kind !== 'unsupported'" size="small" :disabled="app.serviceRound.busy" :aria-label="word('刷新线路', 'Refresh line') + ' ' + line.line_name" @click="app.refreshServiceLine(line.line_id)">{{ word('刷新', 'Refresh') }}</NButton>
      </div>
      <NButton v-for="candidate in source.candidates" :key="candidate.candidate_id" class="details-candidate" :disabled="app.serviceRound.busy" @click="app.confirmServiceCandidate(candidate)">{{ word('确认', 'Confirm') }} · {{ candidate.title }}<template v-if="candidate.year"> ({{ candidate.year }})</template></NButton>
    </section>
    <p v-if="app.serviceRound.snapshot?.query_done && !app.serviceRound.snapshot.pending && !app.serviceRound.snapshot.lines.length && !app.serviceRound.snapshot.candidates.length">{{ word('没有找到可用线路', 'No lines found') }}</p>
    </template>
    <NButton class="details-confirm-cancel" @click="updateServiceModal(false)">{{ t('Cancel') }}</NButton>
  </NModal>
  </div>
</template>
