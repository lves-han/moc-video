<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { NAlert, NButton } from "naive-ui";
import { appState, appController, createPlaybackController, metadataVideoId } from "../state.ts";
import { settingsQueue } from "../api.ts";
import type { PlaybackContext, ResumeSnapshot } from "../models.ts";
import { locale } from "../i18n";

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
const controlsVisible = ref(true);
const zh = computed(() => locale.value === "zh-CN");
const native = Boolean(window.__TAURI__);
const word = (cn: string, en: string) => zh.value ? cn : en;
const priorFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
let hideTimer: ReturnType<typeof setTimeout> | undefined;
let mounted = false;
const imageLoadEpoch = ref(0);
watch(() => appState.details.data, () => imageLoadEpoch.value++);
const poster = computed(() => {
  const data = appState.details.data;
  const id = data?.details.poster?.resource_id;
  const epoch = appState.details.request_epoch;
  return data && id && data.images[id] ? { url: data.images[id]!.url, key: `${epoch}:${imageLoadEpoch.value}:${id}`, title: data.details.title,
    error: () => playback.imageError(data, id, epoch) } : null;
});
const time = (ms: number | null) => ms === null ? word("时长未知", "Unknown duration") :
  `${Math.floor(ms / 60000).toString().padStart(2, "0")}:${Math.floor(ms / 1000 % 60).toString().padStart(2, "0")}`;
const playing = computed(() => state.phase === "playing" || state.phase === "buffering");
const groups = computed(() => appState.details.data?.details.groups ?? []);
const currentVideo = computed(() => props.context?.metadata_video_id ?? "");
const currentGroup = computed(() => groups.value.find(group => group.episodes.some(episode => metadataVideoId(episode.ref) === currentVideo.value)) ?? null);
const currentEpisode = computed(() => currentGroup.value?.episodes.find(episode => metadataVideoId(episode.ref) === currentVideo.value) ?? null);
const nowPlaying = computed(() => currentEpisode.value ? [appState.details.data!.details.title, currentGroup.value!.title, currentEpisode.value.label].filter(Boolean).join(" · ") : "");
const nextVideo = computed(() => currentVideo.value ? appController.nextVideo(currentVideo.value) : null);
const busy = computed(() => Boolean(appController.matching.pending));
const panelOpen = ref(false);
const isFullscreen = ref(false);
// Dragging the slider only saves its settled value.
let volumeTimer: ReturnType<typeof setTimeout> | undefined;
function savePlaybackAudio() {
  clearTimeout(volumeTimer);
  volumeTimer = setTimeout(() => {
    const volume = state.volume, muted = state.muted;
    if (appState.settings.volume === volume && appState.settings.muted === muted) return;
    appController.updateSettings(settings => ({ ...settings, volume, muted })).catch(cause => { ui.diagnostic = String(cause); });
  }, 300);
}
function setVolume(value: number) { playback.setVolume(Math.min(1, Math.max(0, Math.round(value * 100) / 100))); savePlaybackAudio(); }
function setMuted(value: boolean) { playback.setMuted(value); savePlaybackAudio(); }
function seekTo(seconds: number) { playback.seek(seconds); emit("seek"); }
function toggle() { if (playing.value) playback.pause(); else void playback.play(); }
function showControls() {
  controlsVisible.value = true;
  clearTimeout(hideTimer);
  if (playing.value) hideTimer = setTimeout(() => {
    if (!panel.value?.querySelector(".playback-controls")?.contains(document.activeElement)) controlsVisible.value = false;
  }, 3000);
}
function keydown(event: KeyboardEvent) {
  if (event.key === "Escape" && !document.querySelector('[role="dialog"], [role="alertdialog"]')) {
    if (panelOpen.value) { panelOpen.value = false; event.preventDefault(); }
    else if (isFullscreen.value) { event.preventDefault(); void fullscreen(); }
    return;
  }
  if (document.querySelector('[role="dialog"], [role="alertdialog"]') || event.ctrlKey || event.altKey || event.metaKey || (event.target as HTMLElement).closest("input,select,button,a,textarea,[role=switch],[role=combobox],[role=radio],[role=slider],[contenteditable=true]")) return;
  if (event.code === "Space") { event.preventDefault(); toggle(); }
  else if ((event.key === "ArrowLeft" || event.key === "ArrowRight") && state.duration_ms !== null) {
    event.preventDefault(); seekTo(state.position_ms / 1000 + (event.key === "ArrowRight" ? 5 : -5));
  } else if (event.key === "ArrowUp" || event.key === "ArrowDown") {
    event.preventDefault(); setVolume(state.volume + (event.key === "ArrowUp" ? .05 : -.05));
  } else if (event.key.toLowerCase() === "m") setMuted(!state.muted);
  else if (event.key.toLowerCase() === "f") void fullscreen();
  showControls();
}
async function fullscreen() {
  try { const result = await playback.fullscreen(); isFullscreen.value = result === true; }
  catch (cause) { ui.diagnostic = String(cause); }
}
watch(() => props.context, context => {
  if (!mounted) return;
  if (context) void playback.load(context, { resume: props.resume });
  // Already stopped by the controller: another stop would bump the epoch and discard its pending rematch.
  else if (state.context || state.session || state.resolution || state.phase !== "idle") void playback.stop();
});
watch(playing, showControls);
onMounted(async () => {
  mounted = true;
  playback.attach(video.value!);
  try { appState.settings = await settingsQueue.initialize(); }
  catch (cause) { ui.diagnostic = String(cause); }
  await nextTick();
  if (!mounted) return;
  state.volume = appState.settings.volume;
  state.muted = appState.settings.muted;
  if (props.context) void playback.load(props.context, { resume: props.resume });
  panel.value?.focus({ preventScroll: true });
});
onBeforeUnmount(() => {
  unbind();
  mounted = false;
  clearTimeout(hideTimer);
  clearTimeout(volumeTimer);
  if (state.context || state.session || state.resolution || state.phase !== "idle") void playback.stop();
  if (priorFocus?.isConnected) priorFocus.focus({ preventScroll: true });
});
defineExpose({ playback });
</script>

<template>
  <main id="main-content" ref="panel" class="playback-view" tabindex="-1" @keydown="keydown" @pointermove="showControls" @focusin="showControls">
    <header><a :href="context ? native ? `#/details/${encodeURIComponent(JSON.stringify(context.metadata_ref.content_ref))}` : `#/film/${context.metadata_ref.identity.meta_id}` : '#/home'">{{ word('返回', 'Back') }}</a><h1>{{ word('正在播放', 'Now playing') }}</h1></header>
    <div v-if="poster" class="playback-title"><img :key="poster.key" :src="poster.url" :alt="poster.title" @error="poster.error" /><span>{{ poster.title }}</span></div>
    <p v-if="!context" role="status">{{ word('请从影片详情选择已确认的剧集。', 'Choose a confirmed episode from film details.') }}</p>
    <p v-if="nowPlaying" class="playback-now">{{ nowPlaying }}</p>
    <section class="playback-surface" :aria-label="word('视频播放器', 'Video player')">
      <video ref="video" playsinline :aria-label="word('视频', 'Video')" @click="toggle" />
      <div class="playback-controls" :class="{ concealed: !controlsVisible && playing }">
        <NButton v-if="ui.autoplayBlocked" data-action="continue" type="primary" @click="playback.play()">{{ word('继续播放', 'Continue playback') }}</NButton>
        <NButton data-action="play" :disabled="!state.session" @click="toggle">{{ playing ? word('暂停', 'Pause') : word('播放', 'Play') }}</NButton>
        <NButton data-action="stop" @click="playback.stop()">{{ word('停止', 'Stop') }}</NButton>
        <NButton v-if="groups.length" data-action="next" :disabled="!nextVideo || busy" @click="emit('play', nextVideo!)">{{ word('下一集', 'Next episode') }}</NButton>
        <NButton v-if="groups.length" data-action="episodes" :aria-expanded="panelOpen" @click="panelOpen = !panelOpen">{{ word('选集', 'Episodes') }}</NButton>
        <NButton data-action="fullscreen" :disabled="!state.session" @click="fullscreen">{{ word('全屏', 'Fullscreen') }}</NButton>
        <label class="playback-seek">{{ time(state.position_ms) }} / {{ time(state.duration_ms) }}
          <input :aria-label="word('播放进度', 'Playback position')" type="range" min="0" :max="(state.duration_ms ?? 0) / 1000" step="0.1" :disabled="state.duration_ms === null || !state.session" :value="state.position_ms / 1000" @input="seekTo(Number(($event.target as HTMLInputElement).value))" />
        </label>
        <label>{{ word('音量', 'Volume') }} {{ Math.round(state.volume * 100) }}%
          <input :aria-label="word('音量', 'Volume')" type="range" min="0" max="1" step="0.01" :value="state.volume" @input="setVolume(Number(($event.target as HTMLInputElement).value))" />
        </label>
        <label><input type="checkbox" :checked="state.muted" @change="setMuted(($event.target as HTMLInputElement).checked)" />{{ word('静音', 'Mute') }}</label>
        <label v-if="groups.length"><input data-action="auto-next" type="checkbox" :checked="autoNext" @change="emit('update:autoNext', ($event.target as HTMLInputElement).checked)" />{{ word('自动下一集', 'Auto-play next') }}</label>
        <label>{{ word('倍速', 'Speed') }}<select :value="state.speed" @change="playback.setSpeed(Number(($event.target as HTMLSelectElement).value))"><option v-for="rate in [0.5, 1, 1.25, 1.5, 2]" :key="rate" :value="rate">{{ rate }}×</option></select></label>
        <label>{{ word('字幕', 'Subtitles') }}<select :value="state.subtitle_id ?? ''" @change="playback.selectSubtitle(($event.target as HTMLSelectElement).value || null)"><option value="">{{ word('关闭', 'Off') }}</option><option v-for="track in state.session?.subtitles ?? []" :key="track.id" :value="track.id">{{ track.label }}</option></select></label>
        <label>{{ word('线路', 'Stream') }}<select :disabled="!state.session" :value="state.session?.stream_id ?? ''" @change="playback.selectStream(($event.target as HTMLSelectElement).value)"><option v-for="stream in state.resolution?.streams ?? []" :key="stream.id" :value="stream.id" :disabled="!stream.playable">{{ stream.label }}{{ stream.reason ? ` · ${stream.reason}` : '' }}</option></select></label>
      </div>
    </section>
    <section v-if="panelOpen" class="playback-episodes" :aria-label="word('选集', 'Episodes')">
      <template v-for="group in groups" :key="group.id"><h2>{{ group.title }}</h2><div class="playback-episode-grid"><NButton v-for="episode in group.episodes" :key="episode.id" class="playback-episode" :data-video-id="metadataVideoId(episode.ref)" :type="metadataVideoId(episode.ref) === currentVideo ? 'primary' : 'default'" :aria-current="metadataVideoId(episode.ref) === currentVideo ? 'true' : undefined" :disabled="busy || !metadataVideoId(episode.ref)" @click="emit('play', metadataVideoId(episode.ref))">{{ episode.label }}</NButton></div></template>
    </section>
    <p role="status" data-playback-phase>{{ state.phase }}</p>
    <NAlert v-if="state.error" class="playback-error" type="error" role="alert">{{ state.error.code }} · {{ state.error.message }}</NAlert>
    <p v-if="ui.diagnostic" role="alert">{{ ui.diagnostic }}</p>
    <p v-if="ui.imageError" role="alert">{{ ui.imageError }}</p>
    <NButton v-if="ui.imageError" data-action="retry-images" @click="playback.retryImages()">{{ word('重试图片', 'Retry image') }}</NButton>
    <NButton v-if="ui.action === 'retry'" data-action="retry" @click="playback.retry()">{{ word('重试', 'Retry') }}</NButton>
    <NButton v-if="ui.action === 'reconnect'" data-action="reconnect" @click="playback.reconnect()">{{ word('重新连接', 'Reconnect') }}</NButton>
  </main>
</template>

<style scoped>
.playback-view { max-width: 1200px; margin: auto; padding: 24px clamp(12px, 3vw, 36px); }
header { display: flex; align-items: center; gap: 24px; margin-bottom: 24px; } h1 { font-size: clamp(24px, 4vw, 32px); }
.playback-title { display: flex; align-items: center; gap: 16px; margin-bottom: 16px; } .playback-title img { width: 48px; height: 72px; object-fit: cover; }
.playback-surface { border: 1px solid var(--border); border-radius: 12px; overflow: hidden; background: var(--surface); }
video { display: block; width: 100%; aspect-ratio: 16 / 9; object-fit: contain; background: black; }
.playback-controls { display: flex; flex-wrap: wrap; align-items: center; gap: 12px; padding: 16px; }
.playback-controls.concealed { opacity: 0; pointer-events: none; } .playback-controls:focus-within { opacity: 1; pointer-events: auto; }
label { display: flex; align-items: center; flex-wrap: wrap; gap: 8px; min-height: 44px; max-width: 100%; }
.playback-seek, .playback-seek input { width: 100%; } input { max-width: 100%; } input[type=range] { min-height: 44px; }
select, :deep(button) { min-height: 44px; } select { max-width: 100%; padding: 8px; background: var(--surface); color: var(--text); border: 1px solid var(--border); }
.playback-now { overflow-wrap: anywhere; margin-bottom: 12px; }
.playback-episodes { margin-top: 16px; } .playback-episodes h2 { font-size: 16px; margin: 12px 0 8px; }
.playback-episode-grid { display: flex; flex-wrap: wrap; gap: 8px; }
[role=alert] { overflow-wrap: anywhere; margin: 12px 0; } a:focus-visible, input:focus-visible, select:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
@media (max-width: 420px) { .playback-controls { gap: 8px; padding: 12px; } label:not(.playback-seek) { width: 100%; } }
</style>
