<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { NAlert, NButton } from "naive-ui";
import { appState, appController, createPlaybackController, metadataVideoId } from "../state.ts";
import { settingsQueue } from "../api.ts";
import type { PlaybackContext, ResumeSnapshot } from "../models.ts";
import { locale } from "../i18n";
import Icon from "../components/Icon.vue";

const props = defineProps<{ context: PlaybackContext | null; resume?: ResumeSnapshot | null; autoNext?: boolean }>();
const emit = defineEmits<{ ended: [context: PlaybackContext, epoch: number]; play: [video: string]; "update:autoNext": [value: boolean]; seek: [] }>();
const playback = createPlaybackController(appState, undefined, (context, epoch) => emit("ended", context, epoch), runtime => {
  if (appState.runtime.engine_epoch === null) appState.runtime = runtime;
  else appController.applyRuntime(runtime);
});
const unbind = appController.bindPlayback(playback);
const state = playback.state;
const ui = playback.ui;
const video = ref<HTMLVideoElement>();
const panel = ref<HTMLElement>();

const zh = computed(() => locale.value === "zh-CN");
const native = Boolean(window.__TAURI__);
const word = (cn: string, en: string) => zh.value ? cn : en;
const priorFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;

let mounted = false;
let audioInitialized = false;
const imageLoadEpoch = ref(0);
watch(() => appState.details.data, () => imageLoadEpoch.value++);
const poster = computed(() => {
  const data = appState.details.data;
  const id = data?.details.poster?.resource_id;
  const epoch = appState.details.request_epoch;
  return data && id && data.images[id] ? { url: data.images[id]!.url, key: `${epoch}:${imageLoadEpoch.value}:${id}`, title: data.details.title,
    error: () => playback.imageError(data, id, epoch) } : null;
});
const groups = computed(() => appState.details.data?.details.groups ?? []);
const currentVideo = computed(() => props.context?.metadata_video_id ?? "");
const currentGroup = computed(() => groups.value.find(group => group.episodes.some(episode => metadataVideoId(episode.ref) === currentVideo.value)) ?? null);
const currentEpisode = computed(() => currentGroup.value?.episodes.find(episode => metadataVideoId(episode.ref) === currentVideo.value) ?? null);
const nowPlaying = computed(() => currentEpisode.value ? [appState.details.data!.details.title, currentGroup.value!.title, currentEpisode.value.label].filter(Boolean).join(" · ") : "");
const nextVideo = computed(() => currentVideo.value ? appController.nextVideo(currentVideo.value) : null);
const busy = computed(() => Boolean(appController.matching.pending));
const panelOpen = ref(false);
const interaction = new AbortController();
const officialPlayer = ref<HTMLElement>();
// Dragging the slider only saves its settled value.
let volumeTimer: ReturnType<typeof setTimeout> | undefined;
function commitPlaybackAudio() {
  const volume = state.volume, muted = state.muted;
  if (appState.settings.volume === volume && appState.settings.muted === muted) return;
  appController.updateSettings(settings => ({ ...settings, volume, muted })).catch(cause => {
    // After leaving, this view can no longer show the failure itself.
    if (mounted) ui.diagnostic = String(cause);
    else appController.diagnostics.push({ code: "INTERNAL_ERROR", message: String(cause), retryable: true });
  });
}
function savePlaybackAudio() { clearTimeout(volumeTimer); volumeTimer = setTimeout(commitPlaybackAudio, 300); }
watch(() => [state.duration_ms, state.session?.session_id], () => {
  const seek = officialPlayer.value?.querySelector('video-compat-skin')?.shadowRoot?.querySelector<HTMLElement & { disabled: boolean }>('media-time-slider');
  if (seek) seek.disabled = state.duration_ms === null || !state.session;
}, { flush: 'post' });
watch(() => props.context, context => {
  if (!mounted) return;
  if (context) void playback.load(context, { resume: props.resume });
  // Already stopped by the controller: another stop would bump the epoch and discard its pending rematch.
  else if (state.context || state.session || state.resolution || state.phase !== "idle") void playback.stop();
});

onMounted(async () => {
  mounted = true;
  playback.attach(video.value!);
  video.value!.addEventListener('volumechange', () => {
    state.volume = video.value!.volume; state.muted = video.value!.muted; savePlaybackAudio();
  }, { signal: interaction.signal });
  video.value!.addEventListener('ratechange', () => { state.speed = video.value!.playbackRate; }, { signal: interaction.signal });
  video.value!.addEventListener('seeked', () => {
    const media = video.value!;
    if (state.session?.media_url !== media.currentSrc || !Number.isFinite(media.currentTime)) return;
    // Official controls seek the media directly; snapshots/history need the
    // settled position before the next coalesced position event.
    state.position_ms = Math.max(0, Math.round(media.currentTime * 1000));
    emit('seek');
  }, { signal: interaction.signal });
  window.addEventListener('yingxiang-player-action-error', event => { ui.diagnostic = String((event as CustomEvent).detail); }, { signal: interaction.signal });
  try { appState.settings = await settingsQueue.initialize(); }
  catch (cause) { ui.diagnostic = String(cause); }
  await nextTick();
  if (!mounted) return;
  state.volume = appState.settings.volume;
  state.muted = appState.settings.muted;
  playback.setVolume(state.volume); playback.setMuted(state.muted); audioInitialized = true;
  if (props.context) void playback.load(props.context, { resume: props.resume });
  const container = officialPlayer.value?.querySelector('video-compat-skin')?.shadowRoot?.querySelector<HTMLElement>('media-container');
  (container ?? panel.value)?.focus({ preventScroll: true });
});
onBeforeUnmount(() => {
  if (audioInitialized && video.value) { state.volume = video.value.volume; state.muted = video.value.muted; }
  interaction.abort();
  unbind();
  mounted = false;

  clearTimeout(volumeTimer);
  commitPlaybackAudio();
  if (state.context || state.session || state.resolution || state.phase !== "idle") void playback.stop();
  if (priorFocus?.isConnected) priorFocus.focus({ preventScroll: true });
});
defineExpose({ playback });
</script>

<template>
  <main id="main-content" ref="panel" class="playback-view" tabindex="-1">
    <header><a :href="context ? native ? `#/details/${encodeURIComponent(JSON.stringify(context.metadata_ref.content_ref))}` : `#/film/${context.metadata_ref.identity.meta_id}` : '#/home'" class="playback-icon-link" :aria-label="word('返回', 'Back')" :title="word('返回', 'Back')"><Icon name="back" /></a><h1>{{ word('正在播放', 'Now playing') }}</h1></header>
    <div v-if="poster" class="playback-title"><img :key="poster.key" :src="poster.url" :alt="poster.title" @error="poster.error" /><span>{{ poster.title }}</span></div>
    <p v-if="!context" role="status">{{ word('请从影片详情选择已确认的剧集。', 'Choose a confirmed episode from film details.') }}</p>
    <p v-if="nowPlaying" class="playback-now">{{ nowPlaying }}</p>
    <media-i18n :lang="locale">
    <video-player class="official-player" :lang="locale" default-playback-rates="0.5 1 1.25 1.5 2" ref="officialPlayer">
      <video-compat-skin class="official-skin">
        <video ref="video" crossorigin="anonymous" playsinline :aria-label="word('视频', 'Video')" />
      </video-compat-skin>
    </video-player>
    </media-i18n>
    <p v-if="state.session && state.duration_ms === null" role="status">{{ word('时长未知', 'Unknown duration') }}</p>
    <div class="playback-business-controls">
      <NButton v-if="ui.autoplayBlocked" data-action="continue" :title="word('继续播放', 'Continue playback')" @click="playback.play()"><Icon name="play" /></NButton>
      <NButton data-action="stop" :aria-label="word('停止', 'Stop')" :title="word('停止', 'Stop')" @click="playback.stop()"><Icon name="stop" /></NButton>
      <NButton v-if="groups.length" data-action="next" :aria-label="word('下一集', 'Next episode')" :title="word('下一集', 'Next episode')" :disabled="!nextVideo || busy" @click="emit('play', nextVideo!)"><Icon name="next" /></NButton>
      <NButton v-if="groups.length" data-action="episodes" :aria-label="word('选集', 'Episodes')" :title="word('选集', 'Episodes')" :aria-expanded="panelOpen" @click="panelOpen = !panelOpen"><Icon name="film" /></NButton>
      <label v-if="groups.length"><input data-action="auto-next" type="checkbox" :aria-label="word('自动下一集', 'Auto-play next')" :checked="autoNext" @change="emit('update:autoNext', ($event.target as HTMLInputElement).checked)" />{{ word('自动下一集', 'Auto-play next') }}</label>
      <label>{{ word('线路', 'Stream') }}<select :aria-label="word('线路', 'Stream')" :disabled="!state.session" :value="state.session?.stream_id ?? ''" @change="playback.selectStream(($event.target as HTMLSelectElement).value)"><option v-for="stream in state.resolution?.streams ?? []" :key="stream.id" :value="stream.id" :disabled="!stream.playable">{{ stream.label }}</option></select></label>
    </div>
    <section v-if="panelOpen" class="playback-episodes" :aria-label="word('选集', 'Episodes')">
      <template v-for="group in groups" :key="group.id"><h2>{{ group.title }}</h2><div class="playback-episode-grid"><NButton v-for="episode in group.episodes" :key="episode.id" class="playback-episode" :data-video-id="metadataVideoId(episode.ref)" :type="metadataVideoId(episode.ref) === currentVideo ? 'primary' : 'default'" :aria-current="metadataVideoId(episode.ref) === currentVideo ? 'true' : undefined" :disabled="busy || !metadataVideoId(episode.ref)" @click="emit('play', metadataVideoId(episode.ref))">{{ episode.label }}</NButton></div></template>
    </section>
    <p role="status" data-playback-phase>{{ state.phase }}</p>
    <NAlert v-if="state.error" class="playback-error" type="error" role="alert">{{ state.error.code }} · {{ state.error.message }}</NAlert>
    <p v-if="ui.diagnostic" role="alert">{{ ui.diagnostic }}</p>
    <p v-if="ui.imageError" role="alert">{{ ui.imageError }}</p>
    <NButton v-if="ui.imageError" data-action="retry-images" @click="playback.retryImages()" class="playback-icon-button" :aria-label="word('重试图片', 'Retry image')" :title="word('重试图片', 'Retry image')"><Icon name="repeat" /></NButton>
    <NButton v-if="ui.action === 'retry'" data-action="retry" @click="playback.retry()" class="playback-icon-button" :aria-label="word('重试', 'Retry')" :title="word('重试', 'Retry')"><Icon name="repeat" /></NButton>
    <NButton v-if="ui.action === 'reconnect'" data-action="reconnect" @click="playback.reconnect()" class="playback-icon-button" :aria-label="word('重新连接', 'Reconnect')" :title="word('重新连接', 'Reconnect')"><Icon name="link" /></NButton>
  </main>
</template>

<style scoped>
.playback-view { max-width: 1200px; margin: auto; padding: 24px clamp(12px, 3vw, 36px); }
header { display: flex; align-items: center; gap: 24px; margin-bottom: 20px; } h1 { font-size: clamp(24px,4vw,32px); }
.playback-title { display: flex; align-items: center; gap: 16px; margin-bottom: 16px; } .playback-title img { width: 48px; height: 72px; object-fit: cover; }
.playback-now { overflow-wrap: anywhere; margin-bottom: 12px; }
.playback-icon-link { display: inline-flex; align-items: center; justify-content: center; width: 44px; height: 44px; color: var(--text); }
label { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; max-width: 100%; }
select { min-height: 44px; max-width: 100%; color: var(--text); background: var(--surface); border: 1px solid var(--border); }
.playback-episodes { margin-top: 16px; } .playback-episodes h2 { font-size: 16px; margin: 12px 0 8px; }
.playback-episode-grid { display: flex; flex-wrap: wrap; gap: 8px; }
.playback-icon-button { width: 44px; min-height: 44px; padding: 0; }
[role=alert] { overflow-wrap: anywhere; margin: 12px 0; }
a:focus-visible, input:focus-visible, select:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
</style>

<style scoped>
.official-player { display: block; width: 100%; }
video { display: block; width: 100%; height: 100%; min-width: 0; object-fit: contain; }
.official-skin { display: block; width: 100%; aspect-ratio: 16 / 9; --media-accent-color: var(--accent); --media-accent-text-color: var(--accent-ink); --media-scale-unit: 20px; }
.playback-business-controls { display: flex; flex-wrap: wrap; align-items: center; gap: 10px; margin: 12px 0; }
.playback-business-controls :deep(button) { min-width: 44px; min-height: 44px; }
</style>
