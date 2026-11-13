<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { NAlert } from "naive-ui";
import Icon from "../components/Icon.vue";
import { api, ApiFailure, describeError } from "../api";
import { appState as state } from "../state";
import { locale, t } from "../i18n";
import type { CommandData } from "../models";

type ChannelList = CommandData<"get_live_channels">;
const word = (zh: string, en: string) => locale.value === "zh-CN" ? zh : en;
const lists = computed(() => (state.settings.live_configs ?? []).filter(config => config.enabled));
const listId = ref("");
const list = ref<ChannelList | null>(null);
const group = ref("");
const loading = ref(false), error = ref("");
const playing = ref<{ index: number; session: string; url: string } | null>(null);
const openingIndex = ref<number | null>(null);
let playbackRequest = 0, listRequest = 0;
const message = (cause: unknown) => cause instanceof ApiFailure ? describeError(cause.error) : String(cause);

const groups = computed(() => [...new Set((list.value?.channels ?? []).map(channel => channel.group || word("未分组", "Ungrouped")))]);
const channels = computed(() => (list.value?.channels ?? []).filter(channel => (channel.group || word("未分组", "Ungrouped")) === group.value));

// Each list load or channel switch releases the previous gateway session.
async function stop() {
  const current = playing.value;
  playing.value = null;
  if (current) await api.call("close_playback", { session_id: current.session }).catch(() => undefined);
}
async function load(id: string) {
  const request = ++listRequest;
  ++playbackRequest; openingIndex.value = null;
  await stop();
  if (request !== listRequest) return;
  list.value = null; error.value = ""; group.value = "";
  if (!id) return;
  loading.value = true;
  try {
    const result = await api.call("get_live_channels", { config_id: id });
    if (request !== listRequest || listId.value !== id) return;
    list.value = result;
    group.value = groups.value[0] ?? "";
  } catch (cause) { if (request === listRequest && listId.value === id) error.value = message(cause); }
  finally { if (request === listRequest && listId.value === id) loading.value = false; }
}
async function play(index: number) {
  const current = list.value;
  if (!current) return;
  const request = ++playbackRequest;
  openingIndex.value = index; error.value = "";
  await stop();
  if (request !== playbackRequest) return;
  error.value = "";
  try {
    const view = await api.call("open_live", { config_id: current.config_id, revision: current.revision, index });
    if (request !== playbackRequest || list.value !== current) { await api.call("close_playback", { session_id: view.session_id }).catch(() => undefined); return; }
    playing.value = { index, session: view.session_id, url: view.media_url };
  } catch (cause) { if (request === playbackRequest) error.value = message(cause); }
  finally { if (request === playbackRequest) openingIndex.value = null; }
}
watch(lists, value => { if (!value.some(config => config.config_id === listId.value)) listId.value = value[0]?.config_id ?? ""; }, { immediate: true });
watch(listId, id => { void load(id); }, { immediate: true });
onBeforeUnmount(() => { ++playbackRequest; ++listRequest; void stop(); });
const current = computed(() => list.value?.channels[playing.value?.index ?? -1]);
// Live streams have no meaningful timeline, so the player offers no progress bar.
const video = ref<HTMLVideoElement>();
const paused = ref(false), muted = ref(false);
function togglePlay() { const el = video.value; if (!el) return; if (el.paused) void el.play().catch(cause => { error.value = String(cause); }); else el.pause(); }
function toggleMute() { if (video.value) video.value.muted = !video.value.muted; }
function fullscreen() { void video.value?.requestFullscreen?.().catch(() => undefined); }
function mediaError() { error.value = word("该频道无法播放，可能已失效或格式不受支持。", "This channel cannot be played; it may be offline or in an unsupported format."); }
</script>

<template>
  <section class="live-view">
    <div class="live-heading">
      <h1>{{ t("Live") }}</h1>
      <select v-if="lists.length > 1" v-model="listId" class="compact-select live-list-select" :aria-label="word('直播列表', 'Live list')"><option v-for="config in lists" :key="config.config_id" :value="config.config_id">{{ config.name }}</option></select>
    </div>
    <p v-if="!lists.length" class="live-empty"><Icon name="broadcast" :size="32" />{{ word('请在设置的「直播」中添加并启用一个列表。', 'Add and enable a list under Live in Settings.') }}</p>
    <NAlert v-if="error" type="error" class="live-error" role="alert">{{ error }}</NAlert>
    <p v-if="loading" class="live-loading" role="status">{{ t("Loading…") }}</p>
    <div v-if="list" class="live-layout">
      <nav class="live-groups" :aria-label="word('频道分组', 'Channel groups')" data-focus-row>
        <button v-for="name in groups" :key="name" type="button" :aria-pressed="group === name" @click="group = name">{{ name }}</button>
      </nav>
      <ul class="live-channels" data-focus-row>
        <li v-for="channel in channels" :key="channel.index">
          <button type="button" :disabled="!channel.supported" :aria-current="playing?.index === channel.index ? 'true' : undefined" :title="channel.supported ? channel.name : word('不支持的协议', 'Unsupported protocol')" @click="play(channel.index)">
            <span>{{ channel.name }}</span><small v-if="!channel.supported">{{ word('不支持', 'Unsupported') }}</small>
          </button>
        </li>
      </ul>
      <div class="live-player">
        <template v-if="playing">
          <video ref="video" :key="playing.session" class="live-video" :src="playing.url" autoplay playsinline @play="paused = false" @pause="paused = true" @volumechange="muted = Boolean(video?.muted)" @error="mediaError" @dblclick="fullscreen" />
          <div class="live-controls" data-focus-row>
            <button type="button" class="live-play" @click="togglePlay">{{ paused ? word('播放', 'Play') : word('暂停', 'Pause') }}</button>
            <button type="button" class="live-mute" @click="toggleMute">{{ muted ? word('取消静音', 'Unmute') : word('静音', 'Mute') }}</button>
            <button type="button" class="live-fullscreen" @click="fullscreen">{{ word('全屏', 'Full screen') }}</button>
          </div>
        </template>
        <p v-else class="live-hint" role="status">{{ openingIndex !== null ? word('正在切换频道…', 'Opening channel…') : word('选择一个频道开始播放。', 'Choose a channel to start watching.') }}</p>
        <p v-if="current" class="live-now">{{ current.name }}</p>
      </div>
    </div>
  </section>
</template>

<style scoped>
.live-heading { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 12px; }
.live-heading h1 { margin-top: 0; font-size: 34px; }
.live-empty { display: grid; justify-items: center; gap: 12px; padding: 96px 16px; color: var(--muted); text-align: center; }
.live-error { margin: 12px 0; overflow-wrap: anywhere; }
.live-loading, .live-hint, .live-now { color: var(--muted); }
.live-layout { display: grid; grid-template-columns: minmax(120px, 180px) minmax(160px, 260px) minmax(0, 1fr); gap: 16px; min-height: 60vh; }
.live-groups, .live-channels { display: flex; flex-direction: column; gap: 4px; margin: 0; padding: 0; list-style: none; overflow: auto; max-height: 72vh; }
.live-groups button, .live-channels button { width: 100%; min-height: 40px; padding: 8px 12px; border: 0; border-radius: 10px; color: var(--text); background: transparent; text-align: start; cursor: pointer; display: flex; justify-content: space-between; gap: 8px; }
.live-groups button[aria-pressed="true"], .live-channels button[aria-current="true"] { background: var(--control-surface, rgba(255,255,255,.1)); }
.live-channels button:disabled { color: var(--muted); cursor: not-allowed; }
.live-channels small { color: var(--muted); }
.live-player { min-width: 0; }
.live-video { width: 100%; aspect-ratio: 16 / 9; background: #000; border-radius: 12px; }
.live-controls { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 8px; }
.live-controls button { min-height: 36px; padding: 0 14px; border: 0; border-radius: 10px; color: var(--text); background: var(--control-surface, rgba(255,255,255,.1)); cursor: pointer; }
@media (max-width: 760px) { .live-layout { grid-template-columns: 1fr; } .live-groups { flex-direction: row; overflow-x: auto; } .live-player { order: -1; } }
</style>
