<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { NAlert, NButton } from "naive-ui";
import { appState, createPlaybackController } from "../state.ts";
import { settingsQueue } from "../api.ts";
import type { PlaybackContext } from "../models.ts";
import { locale } from "../i18n";

const props = defineProps<{ context: PlaybackContext | null }>();
const emit = defineEmits<{ ended: [context: PlaybackContext, epoch: number] }>();
const playback = createPlaybackController(appState, undefined, (context, epoch) => emit("ended", context, epoch));
const state = playback.state;
const ui = playback.ui;
const video = ref<HTMLVideoElement>();
const panel = ref<HTMLElement>();
const controlsVisible = ref(true);
const zh = computed(() => locale.value === "zh-CN");
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
function toggle() { if (playing.value) playback.pause(); else void playback.play(); }
function showControls() {
  controlsVisible.value = true;
  clearTimeout(hideTimer);
  if (playing.value) hideTimer = setTimeout(() => {
    if (!panel.value?.querySelector(".playback-controls")?.contains(document.activeElement)) controlsVisible.value = false;
  }, 3000);
}
function keydown(event: KeyboardEvent) {
  if (event.ctrlKey || event.altKey || event.metaKey || (event.target as HTMLElement).closest("input,select,button,a,textarea,[contenteditable=true]")) return;
  if (event.code === "Space") { event.preventDefault(); toggle(); }
  else if ((event.key === "ArrowLeft" || event.key === "ArrowRight") && state.duration_ms !== null) {
    event.preventDefault(); playback.seek(state.position_ms / 1000 + (event.key === "ArrowRight" ? 5 : -5));
  } else if (event.key.toLowerCase() === "m") playback.setMuted(!state.muted);
  else if (event.key.toLowerCase() === "f") void fullscreen();
  showControls();
}
async function fullscreen() {
  try { await playback.fullscreen(); }
  catch (cause) { ui.diagnostic = String(cause); }
}
watch(() => props.context, context => {
  if (!mounted) return;
  if (context) void playback.load(context); else void playback.stop();
});
watch(playing, showControls);
onMounted(async () => {
  mounted = true;
  playback.attach(video.value!);
  try { appState.settings = await settingsQueue.initialize(); }
  catch (cause) { ui.diagnostic = String(cause); }
  if (!mounted) return;
  state.volume = appState.settings.volume;
  state.muted = appState.settings.muted;
  if (props.context) void playback.load(props.context);
  panel.value?.focus({ preventScroll: true });
});
onBeforeUnmount(() => {
  mounted = false;
  clearTimeout(hideTimer);
  void playback.stop();
  if (priorFocus?.isConnected) priorFocus.focus({ preventScroll: true });
});
defineExpose({ playback });
</script>

<template>
  <main ref="panel" class="playback-view" tabindex="-1" @keydown="keydown" @pointermove="showControls" @focusin="showControls">
    <header><a href="#/home">{{ word('返回', 'Back') }}</a><h1>{{ word('正在播放', 'Now playing') }}</h1></header>
    <div v-if="poster" class="playback-title"><img :key="poster.key" :src="poster.url" :alt="poster.title" @error="poster.error" /><span>{{ poster.title }}</span></div>
    <p v-if="!context" role="status">{{ word('请从影片详情选择已确认的剧集。', 'Choose a confirmed episode from film details.') }}</p>
    <section class="playback-surface" :aria-label="word('视频播放器', 'Video player')">
      <video ref="video" playsinline :aria-label="word('视频', 'Video')" @click="toggle" />
      <div class="playback-controls" :class="{ concealed: !controlsVisible && playing }">
        <NButton v-if="ui.autoplayBlocked" data-action="continue" type="primary" @click="playback.play()">{{ word('继续播放', 'Continue playback') }}</NButton>
        <NButton data-action="play" :disabled="!state.session" @click="toggle">{{ playing ? word('暂停', 'Pause') : word('播放', 'Play') }}</NButton>
        <NButton data-action="stop" @click="playback.stop()">{{ word('停止', 'Stop') }}</NButton>
        <NButton data-action="fullscreen" :disabled="!state.session" @click="fullscreen">{{ word('全屏', 'Fullscreen') }}</NButton>
        <label class="playback-seek">{{ time(state.position_ms) }} / {{ time(state.duration_ms) }}
          <input :aria-label="word('播放进度', 'Playback position')" type="range" min="0" :max="(state.duration_ms ?? 0) / 1000" step="0.1" :disabled="state.duration_ms === null || !state.session" :value="state.position_ms / 1000" @input="playback.seek(Number(($event.target as HTMLInputElement).value))" />
        </label>
        <label>{{ word('音量', 'Volume') }} {{ Math.round(state.volume * 100) }}%
          <input :aria-label="word('音量', 'Volume')" type="range" min="0" max="1" step="0.01" :value="state.volume" @input="playback.setVolume(Number(($event.target as HTMLInputElement).value))" />
        </label>
        <label><input type="checkbox" :checked="state.muted" @change="playback.setMuted(($event.target as HTMLInputElement).checked)" />{{ word('静音', 'Mute') }}</label>
        <label>{{ word('倍速', 'Speed') }}<select :value="state.speed" @change="playback.setSpeed(Number(($event.target as HTMLSelectElement).value))"><option v-for="rate in [0.5, 1, 1.5, 2]" :key="rate" :value="rate">{{ rate }}×</option></select></label>
        <label>{{ word('字幕', 'Subtitles') }}<select :value="state.subtitle_id ?? ''" @change="playback.selectSubtitle(($event.target as HTMLSelectElement).value || null)"><option value="">{{ word('关闭', 'Off') }}</option><option v-for="track in state.session?.subtitles ?? []" :key="track.id" :value="track.id">{{ track.label }}</option></select></label>
        <label>{{ word('线路', 'Stream') }}<select :disabled="!state.session" :value="state.session?.stream_id ?? ''" @change="playback.selectStream(($event.target as HTMLSelectElement).value)"><option v-for="stream in state.resolution?.streams ?? []" :key="stream.id" :value="stream.id" :disabled="!stream.playable">{{ stream.label }}{{ stream.reason ? ` · ${stream.reason}` : '' }}</option></select></label>
      </div>
    </section>
    <p role="status" data-playback-phase>{{ state.phase }}</p>
    <NAlert v-if="state.error" type="error" role="alert">{{ state.error.code }} · {{ state.error.message }}</NAlert>
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
[role=alert] { overflow-wrap: anywhere; margin: 12px 0; } a:focus-visible, input:focus-visible, select:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
@media (max-width: 420px) { .playback-controls { gap: 8px; padding: 12px; } label:not(.playback-seek) { width: 100%; } }
</style>
