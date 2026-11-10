<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { VideoPlayerStore } from "@videojs/core/dom";
import { appState, appController, createPlaybackController, metadataVideoId } from "../state.ts";
import { settingsQueue } from "../api.ts";
import type { PlaybackContext, ResumeSnapshot } from "../models.ts";
import { locale } from "../i18n";
import Icon from "../components/Icon.vue";

const props = defineProps<{ context: PlaybackContext | null; resume?: ResumeSnapshot | null; autoNext?: boolean }>();
const emit = defineEmits<{ ended: [context: PlaybackContext, epoch: number]; play: [video: string, explicitLine?: boolean]; "update:autoNext": [value: boolean]; seek: [] }>();
const playback = createPlaybackController(appState, undefined, onEnded, runtime => {
  if (appState.runtime.engine_epoch === null) appState.runtime = runtime;
  else appController.applyRuntime(runtime);
}, (context, epoch, session) => appController.rememberLine(context, epoch, session));
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
  const id = (data?.details.backdrop ?? data?.details.poster)?.resource_id;
  const epoch = appState.details.request_epoch;
  return data && id && data.images[id] ? { url: data.images[id]!.url, key: `${epoch}:${imageLoadEpoch.value}:${id}`, title: data.details.title,
    error: () => playback.imageError(data, id, epoch) } : null;
});
const title = computed(() => appState.details.data?.details.title ?? word("正在播放", "Now playing"));
const groups = computed(() => appState.details.data?.details.groups ?? []);
const currentVideo = computed(() => appController.matching.pending ?? props.context?.metadata_video_id ?? "");
const currentGroup = computed(() => groups.value.find(group => group.episodes.some(episode => metadataVideoId(episode.ref) === currentVideo.value)) ?? null);
const currentEpisode = computed(() => currentGroup.value?.episodes.find(episode => metadataVideoId(episode.ref) === currentVideo.value) ?? null);
const nowPlaying = computed(() => currentEpisode.value ? [appState.details.data!.details.title, currentGroup.value!.title, currentEpisode.value.label].filter(Boolean).join(" · ") : "");
const nextVideo = computed(() => currentVideo.value ? appController.nextVideo(currentVideo.value) : null);
const nextEpisode = computed(() => groups.value.flatMap(group => group.episodes).find(episode => metadataVideoId(episode.ref) === nextVideo.value));
function episodeImage(episode: typeof nextEpisode.value) {
  const data = appState.details.data;
  const id = episode?.poster?.resource_id;
  return id && data?.images[id] ? data.images[id]!.url : poster.value?.url;
}
const busy = computed(() => Boolean(appController.matching.pending));
const tray = ref<"episodes" | "streams" | "tracks" | null>(null);
const trayTrigger = ref<HTMLElement>();
const interaction = new AbortController();
const officialPlayer = ref<HTMLElement & { store: VideoPlayerStore }>();
const nativeFullscreen = ref(false);
const controlsVisible = ref(true);
const caption = ref("");
let captionTrack: TextTrack | null = null;
function updateCaption() {
  const media = video.value;
  caption.value = media && state.session?.media_url === media.currentSrc && captionTrack?.mode === "showing"
    ? Array.from(captionTrack.activeCues ?? []).map(cue => cue instanceof VTTCue ? cue.getCueAsHTML().textContent ?? "" : "").join("\n") : "";
}
function syncCaptionTrack() {
  const selected = Array.from(video.value?.textTracks ?? []).find(track => track.mode === "showing") ?? null;
  if (selected !== captionTrack) {
    captionTrack?.removeEventListener("cuechange", updateCaption);
    captionTrack = selected;
    captionTrack?.addEventListener("cuechange", updateCaption);
  }
  updateCaption();
}
watch(() => state.session?.session_id, syncCaptionTrack, { flush: "post" });
const mediaReady = ref(false);
const loading = computed(() => !state.error && (busy.value || state.phase === "resolving" || state.phase === "preparing" ||
  Boolean(props.context && !mediaReady.value && state.phase !== "ended" && state.phase !== "failed")));
const longLoading = ref(false);
let loadingTimer: ReturnType<typeof setTimeout> | undefined;
watch(loading, active => {
  clearTimeout(loadingTimer); longLoading.value = false;
  if (active) loadingTimer = setTimeout(() => { longLoading.value = true; }, 10000);
}, { immediate: true });
const nextCancelled = ref(false);
const countdown = ref<number | null>(null);
let nextTimer: ReturnType<typeof setInterval> | undefined;
const nextVisible = computed(() => Boolean(nextVideo.value && !nextCancelled.value && !state.error &&
  (countdown.value !== null || state.phase === "ended" || state.duration_ms !== null && state.duration_ms > 0 &&
    state.position_ms >= state.duration_ms - 20000 && ["playing", "paused", "buffering"].includes(state.phase))));
function cancelNext() {
  clearInterval(nextTimer); nextTimer = undefined; countdown.value = null; nextCancelled.value = true;
}
function playNext() {
  const next = nextVideo.value;
  cancelNext(); tray.value = null;
  if (next && !busy.value) emit("play", next);
}
function onEnded(context: PlaybackContext, epoch: number) {
  emit("ended", context, epoch);
  if (!props.autoNext || !nextVideo.value || nextCancelled.value) return;
  clearInterval(nextTimer); countdown.value = 5;
  nextTimer = setInterval(() => {
    if (!mounted || epoch !== state.playback_epoch || state.phase !== "ended" || !props.autoNext) { cancelNext(); return; }
    if (countdown.value !== null && countdown.value > 1) countdown.value--;
    else playNext();
  }, 1000);
}
watch(() => state.playback_epoch, () => { cancelNext(); nextCancelled.value = false; tray.value = null; });
watch(() => props.autoNext, value => { if (!value && countdown.value !== null) cancelNext(); });
watch(() => state.phase, phase => { if (phase !== "ended" && countdown.value !== null) cancelNext(); });
let releaseTray: (() => void) | undefined;
async function openTray(tab: NonNullable<typeof tray.value>, trigger?: EventTarget | null) {
  if (trigger instanceof HTMLElement) trayTrigger.value = trigger;
  tray.value = tab;
  await nextTick();
  panel.value?.querySelector<HTMLElement>(`[data-tray-tab="${tab}"]`)?.focus();
}
function closeTray() {
  tray.value = null;
  void nextTick(() => {
    (trayTrigger.value ?? panel.value?.querySelector<HTMLElement>('[data-player-tray]'))?.focus({ preventScroll: true });
  });
}
watch(tray, value => {
  releaseTray?.(); releaseTray = undefined;
  if (value) releaseTray = officialPlayer.value?.store.requestControlsLock();
});
const backHref = computed(() => {
  const reference = props.context?.metadata_ref.content_ref ?? appState.details.data?.details.ref;
  const identity = props.context?.metadata_ref.identity ?? appState.details.data?.metadata_ref?.identity;
  return native && reference ? `#/details/${encodeURIComponent(JSON.stringify(reference))}`
    : identity ? `#/film/${identity.meta_id}` : "#/home";
});
async function back() {
  if (tray.value) { closeTray(); return; }
  const store = officialPlayer.value?.store;
  if (store?.isFullscreen) {
    try {
      await store.exitFullscreen();
    } catch (cause) { ui.diagnostic = String(cause); }
    return;
  }
  location.hash = backHref.value;
}
function onEscape(event: KeyboardEvent) {
  if (event.key !== "Escape" || event.defaultPrevented) return;
  // Official popups consume Escape first; the window then unwinds one level.
  event.preventDefault(); event.stopPropagation(); void back();
}
let unsubscribeOfficial: (() => void) | undefined;
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
  const seek = officialPlayer.value?.querySelector<HTMLElement & { disabled: boolean }>('media-time-slider');
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
  const official = officialPlayer.value!;
  const syncOfficial = () => {
    nativeFullscreen.value = native && official.store.isFullscreen;
    controlsVisible.value = official.store.controlsVisible;
  };
  unsubscribeOfficial = official.store.subscribe(syncOfficial);
  syncOfficial();
  playback.attach(video.value!);
  // Native fullscreen transitions can leave focus on body. Handle Escape after
  // the official popup handlers while this playback view owns the window.
  window.addEventListener('keydown', onEscape, { signal: interaction.signal });
  document.addEventListener('visibilitychange', () => {
    if (!native || document.visibilityState !== 'visible') return;
    // AppKit's Space transition can leave a visible key window with an
    // unfocused WebView. Restore it after the WebView becomes visible again.
    window.focus();
    if (document.activeElement instanceof HTMLElement) document.activeElement.blur();
    const target = tray.value ? panel.value?.querySelector<HTMLElement>(`[data-tray-tab="${tray.value}"]`)
      : official.querySelector<HTMLElement>('media-container');
    target?.focus({ preventScroll: true });
  }, { signal: interaction.signal });
  video.value!.addEventListener('loadstart', () => { mediaReady.value = false; }, { signal: interaction.signal });
  video.value!.addEventListener('loadeddata', () => { mediaReady.value = true; }, { signal: interaction.signal });
  video.value!.addEventListener('volumechange', () => {
    state.volume = video.value!.volume; state.muted = video.value!.muted; savePlaybackAudio();
  }, { signal: interaction.signal });
  video.value!.addEventListener('ratechange', () => { state.speed = video.value!.playbackRate; }, { signal: interaction.signal });
  video.value!.textTracks.addEventListener('change', () => {
    syncCaptionTrack();
    const media = video.value!, session = state.session;
    if (!session || session.media_url !== media.currentSrc) return;
    const selected = [...media.querySelectorAll('track')].find(track => track.track.mode === 'showing');
    state.subtitle_id = session.subtitles.some(track => track.id === selected?.id) ? selected!.id : null;
  }, { signal: interaction.signal });
  video.value!.textTracks.addEventListener('addtrack', syncCaptionTrack, { signal: interaction.signal });
  video.value!.textTracks.addEventListener('removetrack', syncCaptionTrack, { signal: interaction.signal });
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
  const container = officialPlayer.value?.querySelector<HTMLElement>('media-container');
  (container ?? panel.value)?.focus({ preventScroll: true });
});
onBeforeUnmount(() => {
  unsubscribeOfficial?.();
  captionTrack?.removeEventListener("cuechange", updateCaption); captionTrack = null;
  releaseTray?.();
  clearTimeout(loadingTimer); clearInterval(nextTimer);
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
  <main id="main-content" ref="panel" class="playback-view" :class="{ 'controls-hidden': !controlsVisible, 'native-fullscreen': nativeFullscreen, 'tray-open': tray }" tabindex="-1">
    <media-i18n :lang="locale">
    <video-player class="official-player" :lang="locale" default-playback-rates="0.5 1 1.25 1.5 2" ref="officialPlayer">
      <!-- Dynamic native tags use Vue's standard runtime path, keeping the frozen compiler configuration. -->
      <component :is="'media-container'" class="playback-skin">
        <video ref="video" crossorigin="anonymous" playsinline :aria-label="word('视频', 'Video')" />
        <div v-if="caption" class="playback-captions" aria-hidden="true">{{ caption }}</div>
        <div v-show="loading" class="playback-loading" role="status">
          <img v-if="poster" :key="poster.key" class="playback-backdrop" :src="poster.url" alt="" @error="poster.error" />
          <div class="playback-loading-copy"><h1>{{ title }}</h1><p>{{ currentEpisode?.label }}</p><span class="playback-spinner loading-spinner" /><p v-if="longLoading" class="playback-long-loading">{{ word('正在查找可用线路', 'Finding an available stream') }}</p></div>
        </div>
        <component :is="'media-buffering-indicator'" :delay.prop="300" class="playback-buffering"><span class="playback-spinner" /></component>
        <component :is="'media-controls'">
          <component :is="'media-controls-content'" class="playback-chrome">
            <div class="playback-shade" />
            <header class="playback-header">
              <button class="playback-back playback-glass" :aria-label="word('返回', 'Back')" :title="word('返回', 'Back')" @click="back"><Icon name="back" :size="24" /></button>
              <div><h2>{{ title }}</h2><p>{{ currentGroup?.title }} · {{ currentEpisode?.label }}</p></div>
            </header>
            <div class="playback-bar playback-glass">
              <div class="playback-timeline">
                <component :is="'media-time'" type="current" />
                <component :is="'media-time-slider'" class="playback-seek" :step.prop="10" :disabled.prop="state.duration_ms === null || !state.session">
                  <component :is="'media-slider-track'" class="playback-track"><component :is="'media-slider-buffer'" /><component :is="'media-slider-fill'" /></component>
                  <component :is="'media-slider-thumb'" />
                  <component :is="'media-slider-preview'" class="playback-preview playback-glass"><component :is="'media-slider-value'" type="pointer" /></component>
                </component>
                <component :is="'media-time'" type="remaining" />
              </div>
              <div class="playback-buttons">
                <div class="playback-left">
                  <component :is="'media-seek-button'" class="playback-button playback-seek-back" :seconds.prop="-10" :title="word('后退10秒', 'Back 10 seconds')"><Icon name="history" :size="24" /><span class="playback-ten">10</span></component>
                  <component :is="'media-play-button'" id="playback-play" class="playback-button" :title="word('播放 / 暂停', 'Play / Pause')"><component :is="'media-icon'" family="compat" name="play" class="when-paused" /><component :is="'media-icon'" family="compat" name="pause" class="when-playing" /></component>
                  <component :is="'media-seek-button'" class="playback-button playback-seek-forward" :seconds.prop="10" :title="word('前进10秒', 'Forward 10 seconds')"><Icon name="history" :size="24" /><span class="playback-ten">10</span></component>
                </div>
                <div class="playback-right">
                  <component :is="'media-captions-button'" class="playback-button" :title="word('字幕', 'Captions')"><Icon name="captions" :size="24" /></component>
                  <div class="playback-volume">
                    <component :is="'media-mute-button'" id="playback-mute" class="playback-button" :title="word('静音', 'Mute')"><component :is="'media-icon'" family="compat" name="volume-high" class="when-unmuted" /><component :is="'media-icon'" family="compat" name="volume-off" class="when-muted" /></component>
                    <component :is="'media-volume-slider'" class="playback-volume-slider"><component :is="'media-slider-track'" class="playback-track"><component :is="'media-slider-fill'" /></component><component :is="'media-slider-thumb'" /></component>
                  </div>
                  <component :is="'media-playback-rate-button'" commandfor="playback-rate-menu" class="playback-button playback-rate" :title="word('倍速', 'Speed')" />
                  <component :is="'media-menu'" id="playback-rate-menu" class="playback-rate-menu playback-glass" side="top" align="end"><component :is="'media-menu-content'"><component :is="'media-playback-rate-radio-group'" /></component></component>
                  <button data-player-tray class="playback-button" :aria-label="word('选集托盘', 'Episode tray')" :title="word('选集托盘', 'Episode tray')" :aria-expanded="Boolean(tray)" @click="tray ? closeTray() : openTray('episodes', $event.currentTarget)"><Icon name="film" :size="24" /></button>
                  <component :is="'media-fullscreen-button'" id="playback-fullscreen" class="playback-button" :title="word('全屏', 'Fullscreen')"><component :is="'media-icon'" family="compat" name="fullscreen-enter" class="when-windowed" /><component :is="'media-icon'" family="compat" name="fullscreen-exit" class="when-fullscreen" /></component>
                </div>
              </div>
            </div>
            <component :is="'media-tooltip'" trigger="playback-play" class="playback-tooltip playback-glass"><component :is="'media-tooltip-label'" /></component>
            <component :is="'media-tooltip'" trigger="playback-mute" class="playback-tooltip playback-glass"><component :is="'media-tooltip-label'" /></component>
            <component :is="'media-tooltip'" trigger="playback-fullscreen" class="playback-tooltip playback-glass"><component :is="'media-tooltip-label'" /></component>
          </component>
        </component>
        <section class="playback-tray playback-glass" :class="{ open: tray }" :inert="!tray ? true : undefined" :aria-hidden="!tray" :aria-label="word('播放选项', 'Playback options')">
          <div class="playback-tray-header" role="tablist" :aria-label="word('播放选项', 'Playback options')">
            <button data-action="episodes" data-tray-tab="episodes" role="tab" :aria-label="word('选集', 'Episodes')" :title="word('选集', 'Episodes')" :aria-selected="tray === 'episodes'" @click="openTray('episodes')"><Icon name="film" :size="16" />{{ word('选集', 'Episodes') }}</button>
            <button data-tray-tab="streams" role="tab" :aria-selected="tray === 'streams'" @click="openTray('streams')">{{ word('线路', 'Streams') }}</button>
            <button data-tray-tab="tracks" role="tab" :aria-selected="tray === 'tracks'" @click="openTray('tracks')">{{ word('字幕与音频', 'Subtitles & audio') }}</button>
            <button class="playback-button playback-tray-close" :aria-label="word('关闭托盘', 'Close tray')" @click="closeTray"><Icon name="close" :size="24" /></button>
          </div>
          <div class="playback-tray-body">
            <div v-show="tray === 'episodes'" class="playback-episodes" role="tabpanel">
              <template v-for="group in groups" :key="group.id"><h3>{{ group.title }}</h3><div class="playback-episode-grid"><button v-for="episode in group.episodes" :key="episode.id" class="playback-episode" :data-video-id="metadataVideoId(episode.ref)" :aria-current="metadataVideoId(episode.ref) === currentVideo ? 'true' : undefined" :disabled="busy || !metadataVideoId(episode.ref)" @click="tray = null; emit('play', metadataVideoId(episode.ref), appState.details.data?.metadata_ref?.identity.namespace === 'maccms')"><img v-if="episodeImage(episode)" :src="episodeImage(episode)" alt="" loading="lazy" /><span>{{ episode.label }}</span></button></div></template>
              <p v-if="!groups.length">{{ word('暂无选集', 'No episodes available') }}</p>
              <div class="playback-business-controls">
                <button v-if="ui.autoplayBlocked" data-action="continue" :title="word('继续播放', 'Continue playback')" @click="playback.play()"><Icon name="play" />{{ word('继续播放', 'Continue playback') }}</button>
                <button data-action="stop" :aria-label="word('停止', 'Stop')" :title="word('停止', 'Stop')" @click="playback.stop()"><Icon name="stop" />{{ word('停止', 'Stop') }}</button>
                <button data-action="next" :aria-label="word('下一集', 'Next episode')" :title="word('下一集', 'Next episode')" :disabled="!nextVideo || busy" @click="playNext"><Icon name="next" />{{ word('下一集', 'Next episode') }}</button>
                <label><input data-action="auto-next" type="checkbox" :aria-label="word('自动下一集', 'Auto-play next')" :checked="autoNext" @change="emit('update:autoNext', ($event.target as HTMLInputElement).checked)" />{{ word('自动下一集', 'Auto-play next') }}</label>
              </div>
            </div>
            <div v-show="tray === 'streams'" role="tabpanel" class="playback-streams">
              <button v-if="props.context && props.context.metadata_ref.identity.namespace !== 'maccms'" data-action="change-line" class="playback-stream" :disabled="busy" @click="appController.match(props.context.metadata_video_id, null, false)">{{ word('选择其他线路', 'Choose another line') }}</button>
              <button v-for="stream in state.resolution?.streams ?? []" :key="stream.id" class="playback-stream" :data-stream-id="stream.id" :aria-current="stream.id === state.session?.stream_id ? 'true' : undefined" :disabled="!stream.playable || busy" @click="playback.selectStream(stream.id)"><span>{{ stream.label }}</span><span>{{ stream.quality_label ?? (stream.quality ? `${stream.quality}p` : word('清晰度未知', 'Unknown quality')) }}<small v-if="!stream.playable"> · {{ stream.reason ?? word('暂不可用', 'Unavailable') }}</small></span><Icon v-if="stream.id === state.session?.stream_id" name="check" /></button>
              <p v-if="!state.resolution?.streams.length">{{ word('暂无可用线路', 'No streams available') }}</p>
              <slot name="preferences" />
            </div>
            <div v-show="tray === 'tracks'" role="tabpanel" class="playback-tracks">
              <h3>{{ word('字幕', 'Subtitles') }}</h3><component :is="'media-captions-radio-group'" />
              <p v-if="!state.session?.subtitles.length">{{ word('暂无字幕', 'No subtitles available') }}</p>
              <h3>{{ word('音频', 'Audio') }}</h3><component :is="'media-audio-track-radio-group'" /><p class="playback-audio-fallback">{{ word('使用当前音轨', 'Using the current audio track') }}</p>
            </div>
          </div>
          <div class="playback-recovery-actions">
            <button v-if="ui.imageError" data-action="retry-images" @click="playback.retryImages()">{{ word('重试图片', 'Retry image') }}</button>
            <button v-if="ui.action === 'retry'" data-action="retry" @click="playback.retry()">{{ word('重试', 'Retry') }}</button>
            <button v-if="ui.action === 'reconnect'" data-action="reconnect" @click="playback.reconnect()">{{ word('重新连接', 'Reconnect') }}</button>
          </div>
        </section>
        <aside v-if="nextVisible && !tray" class="playback-next playback-glass" :aria-label="word('下一集', 'Next episode')">
          <img v-if="episodeImage(nextEpisode)" :src="episodeImage(nextEpisode)" alt="" /><div v-else class="playback-next-image" />
          <div class="playback-next-copy"><span class="playback-next-kicker">{{ word('下一集', 'Next episode') }}</span><h3>{{ nextEpisode?.label }}</h3><div class="playback-next-actions"><button class="playback-primary" :disabled="busy" @click="playNext">{{ word('立即播放', 'Play now') }}</button><button @click="cancelNext">{{ word('取消', 'Cancel') }}</button><span v-if="countdown !== null" class="playback-countdown" role="timer" :style="{ '--countdown': countdown }">{{ countdown }}</span></div></div>
        </aside>
        <div v-if="state.error && !ui.autoplayBlocked || ui.diagnostic" class="playback-error-backdrop">
          <section class="playback-error playback-glass" role="alert"><h2>{{ word('播放遇到问题', 'Playback failed') }}</h2><p>{{ state.error ? `${state.error.code} · ${state.error.message}` : ui.diagnostic }}</p><div><button class="playback-primary" @click="ui.action === 'reconnect' ? playback.reconnect() : playback.retry()">{{ word('重试', 'Retry') }}</button><button @click="openTray('streams', $event.currentTarget)">{{ word('换线路', 'Change stream') }}</button></div></section>
        </div>
        <p v-if="appController.lineMemory.error" class="playback-line-error" role="status">{{ appController.lineMemory.error }}</p>
        <button v-if="ui.autoplayBlocked" class="playback-start playback-glass" :aria-label="word('继续播放', 'Continue playback')" @click="playback.play()"><Icon name="play" :size="28" />{{ word('继续播放', 'Continue playback') }}</button>
        <component :is="'media-hotkey'" keys="Space" action="togglePaused" />
        <component :is="'media-hotkey'" keys="ArrowLeft" action="seekStep" :value.prop="-10" />
        <component :is="'media-hotkey'" keys="ArrowRight" action="seekStep" :value.prop="10" />
        <component :is="'media-hotkey'" keys="ArrowUp" action="volumeStep" :value.prop="0.05" />
        <component :is="'media-hotkey'" keys="ArrowDown" action="volumeStep" :value.prop="-0.05" />
        <component :is="'media-hotkey'" keys="f" action="toggleFullscreen" />
        <component :is="'media-hotkey'" keys="m" action="toggleMuted" />
        <component :is="'media-gesture'" type="tap" action="togglePaused" pointer="mouse" region="center" />
        <component :is="'media-gesture'" type="tap" action="toggleControls" pointer="touch" />
        <component :is="'media-gesture'" type="doubletap" action="seekStep" :value.prop="-10" region="left" />
        <component :is="'media-gesture'" type="doubletap" action="seekStep" :value.prop="10" region="right" />
        <component :is="'media-status-announcer'" class="playback-sr-only" />
      </component>
    </video-player>
    </media-i18n>
    <p class="playback-sr-only playback-now">{{ nowPlaying }}</p>
    <p class="playback-sr-only" role="status" data-playback-phase>{{ state.phase }}</p>
    <p v-if="state.session && state.duration_ms === null" class="playback-sr-only" role="status">{{ word('时长未知', 'Unknown duration') }}</p>
    <p v-if="!context" class="playback-sr-only" role="status">{{ word('请从影片详情选择已确认的剧集。', 'Choose a confirmed episode from film details.') }}</p>
    <p v-if="ui.imageError" class="playback-sr-only" role="alert">{{ ui.imageError }}</p>
  </main>
</template>
