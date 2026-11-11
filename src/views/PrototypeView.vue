<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";
import { capabilities, createPlayer, type PlaybackSource, type Player, type PlayerEvent } from "../player";

const video = ref<HTMLVideoElement>();
const state = ref("ready");
const error = ref("");
const position = ref(0);
const duration = ref(0);
const volume = ref(1);
const speed = ref(1);
const subtitles = ref(true);
const playing = ref(false);
const supported = ref("");
let player: Player;
let source: PlaybackSource = { url: "/demo.mp4", kind: "mp4", subtitle: "/demo.vtt" };

function event(value: PlayerEvent) {
  state.value = value.state;
  position.value = value.position_ms / 1000;
  duration.value = (value.duration_ms ?? 0) / 1000;
  playing.value = !video.value?.paused;
  if (value.state === "preparing" || value.state === "playing") error.value = "";
  if (value.error) error.value = value.error;
}
async function play() {
  try { if (video.value?.paused) await player.play(); else player.pause(); }
  catch (cause) { error.value = String(cause); }
}
function open(next: PlaybackSource) { source = next; return player.open(next); }
function reopen() { open(source); }
async function fullscreen() {
  try { await player.fullscreen(); } catch (cause) { error.value = String(cause); }
}
onMounted(async () => {
  player = createPlayer(video.value!, event);
  supported.value = JSON.stringify(capabilities(video.value!));
  if (import.meta.env.MODE === "t01") {
    const { automate } = await import("../../tests/fixtures/t01/automation");
    await automate(video.value!, player, open);
  } else reopen();
});
onBeforeUnmount(() => player?.stop());
</script>

<template>
  <main class="prototype">
    <header><strong>墨客视频</strong><span>视频技术样机 · T01</span></header>
    <h1>内嵌视频播放</h1>
    <p class="prototype-intro">本地自有测试片段 · MP4 / HLS</p>
    <section class="prototype-player" aria-label="视频播放器">
      <video ref="video" playsinline aria-label="自有测试视频" />
      <div class="prototype-controls">
        <button data-action="play" @click="play">{{ playing ? '暂停' : '继续播放' }}</button>
        <button data-action="stop" @click="player.stop()">停止</button>
        <button data-action="reopen" @click="reopen">重新打开</button>
        <button data-action="fullscreen" @click="fullscreen">全屏</button>
        <label class="prototype-seek">进度 {{ position.toFixed(1) }} / {{ duration.toFixed(1) }} 秒
          <input aria-label="播放进度" type="range" min="0" :max="duration || 30" step="0.1" :value="position" @input="player.seek(Number(($event.target as HTMLInputElement).value))" />
        </label>
        <label>音量 {{ Math.round(volume * 100) }}%
          <input aria-label="音量" v-model.number="volume" type="range" min="0" max="1" step="0.01" @input="player.setVolume(volume)" />
        </label>
        <label>倍速 <select aria-label="倍速" v-model.number="speed" @change="player.setSpeed(speed)"><option v-for="rate in [0.5, 1, 1.5, 2]" :key="rate" :value="rate">{{ rate }}×</option></select></label>
        <label><input aria-label="字幕" v-model="subtitles" type="checkbox" @change="player.selectSubtitle(subtitles ? 0 : null)" />字幕</label>
      </div>
    </section>
    <p role="status" class="prototype-state">{{ state }}</p>
    <p v-if="error" role="alert" class="prototype-error">{{ error }}</p>
    <details><summary>播放能力</summary><pre>{{ supported }}</pre></details>
  </main>
</template>

<style scoped>
.prototype { max-width: 1200px; margin: auto; padding: 24px clamp(12px, 3vw, 36px); }
header { display: flex; flex-wrap: wrap; justify-content: space-between; gap: 12px; margin-bottom: 32px; }
header strong { font-weight: 650; } header span, .prototype-intro, details { color: var(--muted, #666); }
h1 { font-size: clamp(24px, 4vw, 32px); } .prototype-intro { margin: 10px 0 24px; }
.prototype-player { border: 1px solid var(--border, #d9dde0); border-radius: 12px; overflow: hidden; background: var(--surface, white); }
video { display: block; width: 100%; aspect-ratio: 16 / 9; background: black; object-fit: contain; }
.prototype-controls { display: flex; flex-wrap: wrap; gap: 12px; align-items: center; padding: 16px; }
button, select { min-height: 44px; padding: 8px 12px; background: var(--background, #f2f4f5); border: 1px solid var(--border, #d9dde0); }
label { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; min-height: 44px; max-width: 100%; }
input { max-width: 100%; } input[type=range] { min-height: 44px; }
.prototype-seek { width: 100%; } .prototype-seek input { width: 100%; }
.prototype-state { margin: 16px 0; } .prototype-error { color: #be2839; overflow-wrap: anywhere; }
pre { white-space: pre-wrap; overflow-wrap: anywhere; }
@media (max-width: 420px) { .prototype-controls { gap: 8px; padding: 12px; } label:not(.prototype-seek) { width: 100%; } }
</style>
