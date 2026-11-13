<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { NAlert, NButton } from "naive-ui";
import Icon from "../components/Icon.vue";
import { api, ApiFailure, describeError } from "../api";
import { appState as state, appController as controller } from "../state";
import { historyRepository } from "../data/history";
import { locale, t } from "../i18n";
import type { CommandData } from "../models";

type Library = CommandData<"list_local_videos">;
type Playback = CommandData<"open_local">;
const emit = defineEmits<{ play: [value: Playback & { position_ms: number }] }>();
const native = Boolean(window.__TAURI__);
const word = (zh: string, en: string) => locale.value === "zh-CN" ? zh : en;
const library = ref<Library | null>(null);
const progress = ref<Record<string, number>>({});
const loading = ref(false), error = ref("");
const message = (cause: unknown) => cause instanceof ApiFailure ? describeError(cause.error) : String(cause);
const videoExtensions = ["mp4", "m4v", "mov", "mkv", "webm", "avi", "ts", "flv", "wmv"];

async function load() {
  if (!native) return;
  loading.value = true; error.value = "";
  try {
    library.value = await api.call("list_local_videos", {});
    // Resume positions come from watch history, keyed by the local file id.
    const rows = await historyRepository.list();
    progress.value = Object.fromEntries(rows.filter(row => row.record?.metadata_identity.namespace === "local")
      .map(row => [row.record!.video_id ?? "", row.duration_ms ? Math.round(row.position_ms / row.duration_ms * 100) : 0]));
  } catch (cause) { error.value = message(cause); }
  finally { loading.value = false; }
}

async function pick(directory: boolean) {
  error.value = "";
  try {
    const chosen = await window.__TAURI__!.core.invoke("plugin:dialog|open", { options: {
      multiple: true, directory, ...(directory ? {} : { filters: [{ name: word("视频", "Video"), extensions: videoExtensions }] }) } }) as string | string[] | null;
    const paths = (Array.isArray(chosen) ? chosen : chosen ? [chosen] : []).filter(path => typeof path === "string");
    if (!paths.length) return;
    await controller.updateSettings(settings => ({ ...settings, local_paths: [...new Set([...(settings.local_paths ?? []), ...paths])].slice(0, 256) }));
    await load();
  } catch (cause) { error.value = message(cause); }
}
async function removeRoot(path: string) {
  ++playbackRequest;
  try {
    await controller.updateSettings(settings => ({ ...settings, local_paths: (settings.local_paths ?? []).filter(row => row !== path) }));
    await load();
  } catch (cause) { error.value = message(cause); }
}

let playbackRequest = 0, mounted = true;
async function play(id: string) {
  const request = ++playbackRequest;
  error.value = "";
  let opened: Playback | null = null;
  try {
    opened = await api.call("open_local", { id });
    if (request !== playbackRequest) return;
    const saved = await historyRepository.get(opened.record.content_id);
    if (request !== playbackRequest) return;
    emit('play', { ...opened, position_ms: saved && !saved.completed ? saved.position_ms : 0 });
    opened = null;
  } catch (cause) { if (request === playbackRequest) error.value = message(cause); }
  finally { if (opened) await api.call("close_playback", { session_id: opened.session.session_id }).catch(() => undefined); }
}

const groups = computed(() => (library.value?.roots ?? []).map(root => ({ ...root, items: (library.value?.items ?? []).filter(item => item.root === root.path) })));
const size = (bytes: number) => bytes >= 1073741824 ? `${(bytes / 1073741824).toFixed(1)} GB` : `${Math.max(1, Math.round(bytes / 1048576))} MB`;
// The History page sends #/local?play=<id> to resume a file.
async function playRequested() {
  const id = new URLSearchParams(location.hash.split("?")[1] ?? "").get("play");
  if (id && library.value?.items.some(item => item.id === id)) await play(id);
  else if (id && library.value) {
    ++playbackRequest;
    if (mounted) error.value = word("找不到该视频文件，可能已移动或删除。", "This video file was moved or deleted.");
  }
}
function hashChanged() { if (location.hash.startsWith("#/local?") && library.value) void playRequested(); }
onMounted(async () => {
  window.addEventListener("hashchange", hashChanged);
  await load();
  if (mounted) await playRequested();
});
watch(() => state.settings.local_paths, () => { if (!loading.value) void load(); });
onBeforeUnmount(() => { mounted = false; ++playbackRequest; window.removeEventListener("hashchange", hashChanged); });
</script>

<template>
  <section class="local-view">
    <div class="local-heading">
      <h1>{{ t("Local video") }}</h1>
      <div v-if="native" class="local-actions"><NButton class="local-add-files" @click="pick(false)"><template #icon><Icon name="plus" :size="16" /></template>{{ word('添加文件', 'Add files') }}</NButton><NButton class="local-add-folder" @click="pick(true)"><template #icon><Icon name="folder" :size="16" /></template>{{ word('添加文件夹', 'Add folder') }}</NButton></div>
    </div>
    <p v-if="!native" class="local-empty"><Icon name="folder" :size="40" />{{ word('本地视频需要在桌面应用中使用。', 'Local video is available in the desktop app.') }}</p>
    <NAlert v-if="error" type="error" class="local-error" role="alert">{{ error }}</NAlert>
    <p v-if="loading" role="status">{{ t("Loading…") }}</p>
    <p v-else-if="native && library && !library.roots.length" class="local-empty"><Icon name="folder" :size="40" />{{ word('添加视频文件或文件夹后，会显示在这里。', 'Add video files or folders and they appear here.') }}</p>
    <section v-for="group in groups" :key="group.path" class="local-group">
      <div class="local-group-heading"><h2 :title="group.path">{{ group.path }}</h2><span v-if="!group.available" class="local-missing">{{ word('找不到该位置', 'Location not found') }}</span><NButton size="small" class="local-remove" @click="removeRoot(group.path)">{{ word('移除', 'Remove') }}</NButton></div>
      <p v-if="group.available && !group.items.length" class="local-none">{{ word('没有找到视频文件', 'No video files found') }}</p>
      <ul class="local-items" data-focus-row>
        <li v-for="item in group.items" :key="item.id">
          <button type="button" :disabled="!item.supported" :title="item.path" @click="play(item.id)">
            <span class="local-name">{{ item.name }}</span>
            <small>{{ item.supported ? size(item.size) : word('格式不支持', 'Format not supported') }}<template v-if="progress[item.id]"> · {{ word(`已看 ${progress[item.id]}%`, `${progress[item.id]}% watched`) }}</template></small>
          </button>
        </li>
      </ul>
    </section>
  </section>
</template>

<style scoped>
.local-heading, .local-group-heading { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 12px; }
.local-heading h1 { margin-top: 0; font-size: 34px; }
.local-actions { display: flex; flex-wrap: wrap; gap: 8px; }
.local-empty { display: grid; justify-items: center; gap: 12px; padding: 96px 16px; color: var(--muted); text-align: center; }
.local-error { margin: 12px 0; overflow-wrap: anywhere; }
.local-none, .local-items small, .local-missing { color: var(--muted); }
.local-group { margin-top: 20px; }
.local-group-heading h2 { font-size: 15px; margin: 0; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; flex: 1 1 200px; }
.local-items { list-style: none; margin: 8px 0 0; padding: 0; display: grid; gap: 4px; }
.local-items button { width: 100%; min-height: 48px; display: flex; justify-content: space-between; align-items: center; gap: 12px; padding: 8px 12px; border: 0; border-radius: 10px; color: var(--text); background: transparent; text-align: start; cursor: pointer; }
.local-items button:hover, .local-items button[aria-current="true"] { background: var(--control-surface, rgba(255,255,255,.1)); }
.local-items button:disabled { color: var(--muted); cursor: not-allowed; }
.local-name { min-width: 0; overflow-wrap: anywhere; }
</style>
