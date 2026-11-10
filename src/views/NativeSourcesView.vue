<script setup lang="ts">
import { computed, reactive, ref } from "vue";
import { NAlert, NButton, NEmpty, NModal } from "naive-ui";
import { appState as state, appController as controller, validateSourceDraft, type SourceDraft } from "../state";
import { locale, t } from "../i18n";
import { describeError } from "../api";
import { SOURCE_PRESETS } from "../data/sourcePresets";
import type { HttpSourceConfig } from "../models";

const emit = defineEmits<{ select: [id: string, role: "metadata" | "playback"] }>();
const word = (zh: string, en: string) => locale.value === "zh-CN" ? zh : en;
const providers = computed(() => state.sources.data?.providers ?? []);
const metadataSources = computed(() => providers.value.filter(source => source.kind === "stremio" && source.runtime_status === "ready" && source.capabilities.home && source.capabilities.details));
const playbackSources = computed(() => providers.value.filter(source => source.kind !== "stremio" && source.runtime_status === "ready" && source.capabilities.resolve));
const configs = computed(() => state.settings.source_configs);
const presets = computed(() => SOURCE_PRESETS.map(preset => ({ ...preset, added: configs.value.some(config => config.kind === "maccms" && config.api_url === preset.api_url) })));
// A saved choice stays until the user picks a new source, so older Cloudstream preferences are not silently replaced.
const legacyPreference = computed(() => {
  const key = state.settings.last_playback_source_key ?? state.settings.last_source_key;
  return Boolean(key && state.sources.data && !providers.value.some(source => source.id === key));
});

const providerOf = (config: HttpSourceConfig) => providers.value.find(source => source.kind === config.kind && source.origin.config_id === config.config_id);
function sourceError(config: HttpSourceConfig) {
  const provider = providerOf(config), result = provider && controller.lastSourceResult(provider.id);
  return provider?.error ? describeError(provider.error) : result && !result.ok ? result.message : '';
}
const hostOf = (config: HttpSourceConfig) => (config.kind === "maccms" ? [config.api_url] : config.manifest_urls).map(url => { try { return new URL(url).host; } catch { return ""; } }).join(", ");
function lastRequest(config: HttpSourceConfig) {
  const source = providerOf(config), result = source && controller.lastSourceResult(source.id);
  return !result ? word('本次运行尚无请求', 'no request this session') : result.ok ? word('成功', 'succeeded') : result.message;
}
const kindLabel = (kind: string) => kind === "stremio" ? word("Stremio 元数据", "Stremio metadata") : word("MacCMS 播放", "MacCMS playback");

const editing = ref(false);
const draft = reactive<SourceDraft & { config_id: string | null; enabled: boolean }>({ config_id: null, kind: "maccms", name: "", urls: "", enabled: true });
const errors = ref<string[]>([]);
const busy = ref(false);
const failure = ref("");
function openDraft(config?: HttpSourceConfig) {
  Object.assign(draft, config ? { config_id: config.config_id, kind: config.kind, name: config.name, enabled: config.enabled, urls: config.kind === "maccms" ? config.api_url : config.manifest_urls.join("\n") }
    : { config_id: null, kind: "maccms", name: "", urls: "", enabled: true });
  errors.value = []; failure.value = ""; editing.value = true;
}
async function run(work: () => Promise<unknown>) {
  busy.value = true; failure.value = "";
  try { await work(); return true; }
  catch (cause) { failure.value = cause instanceof Error ? cause.message : String(cause); return false; }
  finally { busy.value = false; }
}
async function submit() {
  errors.value = validateSourceDraft(draft, configs.value.length - (draft.config_id ? 1 : 0));
  if (errors.value.length) return;
  editing.value = false;
  await run(() => controller.saveSourceConfig(draft));
}
const toggleConfig = (config: HttpSourceConfig) => run(() => controller.setSourceEnabled(config.config_id, !config.enabled));
async function addPreset(preset: { name: string; api_url: string }) {
  const [limit] = validateSourceDraft({ kind: "maccms", name: preset.name, urls: preset.api_url }, configs.value.length);
  if (limit) { failure.value = limit; return; }
  await run(() => controller.saveSourceConfig({ config_id: null, kind: "maccms", name: preset.name, urls: preset.api_url, enabled: true }));
}
const pendingDelete = ref<HttpSourceConfig | null>(null);
async function confirmDelete() {
  const config = pendingDelete.value!;
  pendingDelete.value = null;
  await run(() => controller.removeSourceConfig(config.config_id));
}

const report = computed(() => controller.diagnosticsReport());
const copyState = ref("");
async function copyDiagnostics() {
  try { await navigator.clipboard.writeText(report.value); copyState.value = word("已复制", "Copied"); }
  catch (cause) { copyState.value = `${word("复制失败，请手动选择下方文本", "Copy failed; select the text below")}: ${String(cause)}`; }
}
</script>

<template>
  <div class="native-sources">
    <h1>{{ t('Source manager') }}</h1>
    <NAlert v-if="state.runtime.error" class="sources-runtime-error" type="error">{{ state.runtime.error.message }}</NAlert>
    <NAlert v-if="state.sources.error" class="sources-list-error" type="error">{{ state.sources.error.message }}<NButton size="small" class="sources-action" @click="controller.loadSources()">{{ t('Retry') }}</NButton></NAlert>
    <NAlert v-if="failure" class="sources-failure" type="error" role="alert">{{ failure }}</NAlert>

    <section class="settings-group">
      <h2>{{ word('当前选择', 'Current selection') }}</h2>
      <div class="settings-panel">
      <div class="sources-pickers">
        <label>{{ t('Metadata provider') }}<select class="sources-metadata" :value="state.selected_metadata_source?.source_id ?? ''" @change="emit('select', ($event.target as HTMLSelectElement).value, 'metadata')"><option value="" disabled>{{ t('Choose a metadata source') }}</option><option v-for="source in metadataSources" :key="source.id" :value="source.id">{{ source.name }}</option></select></label>
        <label>{{ word('播放源', 'Playback source') }}<select class="sources-playback" :value="state.selected_playback_source?.source_id ?? ''" @change="emit('select', ($event.target as HTMLSelectElement).value, 'playback')"><option value="" disabled>{{ t('Choose a video source') }}</option><option v-for="source in playbackSources" :key="source.id" :value="source.id">{{ source.name }}</option></select></label>
      </div>
      <p v-if="legacyPreference" class="sources-legacy" role="status">{{ word('之前选择的播放源已不可用，可能是已停止支持的 Cloudstream 来源。原偏好和历史都已保留，选择新的播放源后才会更新。', 'The previously selected playback source is unavailable; it may be a Cloudstream source, which is no longer supported. The preference and history are kept until you choose a new playback source.') }}<NButton v-if="state.selected_playback_source" size="small" class="sources-legacy-adopt sources-action" @click="emit('select', state.selected_playback_source.source_id, 'playback')">{{ word('改用当前播放源', 'Use the current playback source') }}</NButton></p>
      <p v-if="!metadataSources.length" class="sources-guidance" role="status">{{ word('请添加 Stremio 扩展的 manifest 地址，首页才能显示内容。', 'Add a Stremio add-on manifest URL so the home page has content.') }}</p>
      <p v-if="!playbackSources.length" class="sources-guidance" role="status">{{ word('请添加一个播放源。', 'Add a playback source.') }}</p>
          </div>
    </section>

    <section class="settings-group sources-presets">
      <h2>{{ word('可添加的播放源', 'Available playback sources') }}</h2>
      <div class="settings-panel">
      <p class="sources-presets-note">{{ word('仅供个人使用。这些来源由第三方提供，内容和可用性以来源为准。', 'For personal use only. These third-party sources decide their own content and availability.') }}</p>
      <article v-for="preset in presets" :key="preset.api_url" class="sources-preset" :data-preset="preset.name">
        <h3>{{ preset.name }}</h3>
        <NButton class="sources-preset-add" :disabled="busy || preset.added" @click="addPreset(preset)">{{ preset.added ? word('已添加', 'Added') : word('添加', 'Add') }}</NButton>
      </article>
          </div>
    </section>

    <section class="settings-group">
      <div class="sources-heading"><h2>{{ word('HTTP 来源', 'HTTP sources') }}</h2><NButton class="sources-add" type="primary" :loading="busy" @click="openDraft()">{{ word('添加来源', 'Add source') }}</NButton></div>
      <div class="settings-panel">
      <NEmpty v-if="!configs.length" class="sources-empty" :description="word('尚未配置来源', 'No sources configured')" />
      <article v-for="config in configs" :key="config.config_id" class="sources-row" :data-config-id="config.config_id">
        <span class="source-status" :class="!config.enabled ? 'disabled' : providerOf(config)?.runtime_status === 'ready' ? 'ready' : 'failed'" :aria-label="config.enabled ? providerOf(config)?.runtime_status : 'disabled'" /><div class="sources-row-text">
          <h3>{{ config.name }}</h3>
          <p>{{ kindLabel(config.kind) }} · {{ hostOf(config) }}</p>
          <p class="sources-state">{{ word('登记状态', 'Registration') }}: {{ config.enabled ? providerOf(config)?.runtime_status ?? word('未同步', 'not synced') : word('已禁用', 'disabled') }}</p>
          <p class="sources-last">{{ word('最近请求', 'Last request') }}: {{ lastRequest(config) }}</p>
          <p v-if="sourceError(config)" class="sources-row-error" role="alert">{{ sourceError(config) }}</p>
        </div>
        <div class="sources-row-actions">
          <NButton class="sources-edit" :disabled="busy" @click="openDraft(config)">{{ word('编辑', 'Edit') }}</NButton>
          <NButton class="sources-toggle" :disabled="busy" @click="toggleConfig(config)">{{ config.enabled ? word('禁用', 'Disable') : word('启用', 'Enable') }}</NButton>
          <NButton class="sources-delete" :disabled="busy" @click="pendingDelete = config">{{ word('删除', 'Delete') }}</NButton>
        </div>
      </article>
          </div>
    </section>

    <section class="settings-group sources-diagnostics">
      <div class="sources-heading"><h2>{{ word('诊断', 'Diagnostics') }}</h2><NButton class="sources-copy" @click="copyDiagnostics">{{ word('复制脱敏诊断', 'Copy redacted diagnostics') }}</NButton></div>
      <div class="settings-panel">
      <p v-if="copyState" class="sources-copy-state" role="status">{{ copyState }}</p>
      <pre class="sources-report">{{ report }}</pre>
          </div>
    </section>
  </div>

  <NModal :show="editing" preset="card" role="dialog" class="sources-dialog" :title="draft.config_id ? word('编辑来源', 'Edit source') : word('添加来源', 'Add source')" :aria-label="draft.config_id ? word('编辑来源', 'Edit source') : word('添加来源', 'Add source')" :style="{ width: 'min(560px, calc(100vw - 32px))' }" @update:show="editing = $event">
    <form class="sources-form" @submit.prevent="submit">
      <label>{{ word('类型', 'Type') }}<select class="sources-kind" v-model="draft.kind" :disabled="Boolean(draft.config_id)"><option value="maccms">MacCMS JSON</option><option value="stremio">{{ t('Stremio add-on') }}</option></select></label>
      <label>{{ word('名称', 'Name') }}<input class="sources-name" v-model="draft.name" maxlength="128" /></label>
      <label>{{ draft.kind === 'stremio' ? word('Manifest 地址（每行一个，最多 16 个）', 'Manifest URLs (one per line, up to 16)') : word('API 地址', 'API URL') }}<textarea class="sources-urls" v-model="draft.urls" rows="3" /></label>
      <NAlert v-if="errors.length" class="sources-errors" type="error" role="alert"><p v-for="error in errors" :key="error">{{ error }}</p></NAlert>
      <div class="sources-form-actions"><NButton class="sources-form-cancel" @click="editing = false">{{ t('Cancel') }}</NButton><NButton class="sources-save" attr-type="submit" type="primary">{{ word('保存', 'Save') }}</NButton></div>
    </form>
  </NModal>
  <NModal :show="Boolean(pendingDelete)" preset="card" role="dialog" class="sources-confirm" :title="word('删除来源', 'Delete source')" :aria-label="word('删除来源', 'Delete source')" :style="{ width: 'min(480px, calc(100vw - 32px))' }" @update:show="pendingDelete = null">
    <p>{{ word('删除后，正在使用它的播放会停止。', 'Playback using it will stop.') }} {{ pendingDelete?.name }}</p>
    <div class="sources-form-actions"><NButton class="sources-confirm-cancel" @click="pendingDelete = null">{{ t('Cancel') }}</NButton><NButton class="sources-confirm-delete" type="error" @click="confirmDelete">{{ word('删除', 'Delete') }}</NButton></div>
  </NModal>
</template>

<style scoped>
.native-sources { display: grid; gap: 20px; min-width: 0; }
.settings-panel { min-width: 0; }
.sources-heading, .sources-form-actions { display: flex; flex-wrap: wrap; justify-content: space-between; align-items: center; gap: 12px; }
.sources-form-actions { justify-content: flex-end; margin-top: 12px; }
.sources-pickers { display: flex; flex-wrap: wrap; gap: 16px; }
.sources-row { display: flex; flex-wrap: wrap; justify-content: space-between; gap: 16px; padding: 14px 0; border-top: 1px solid var(--border); }
.sources-row-text { min-width: 0; flex: 1 1 240px; overflow-wrap: anywhere; }
.sources-row-actions { display: flex; flex-wrap: wrap; gap: 8px; align-items: flex-start; }
.sources-preset { display: flex; flex-wrap: wrap; justify-content: space-between; align-items: center; gap: 12px; padding: 10px 0; border-top: 1px solid var(--border); overflow-wrap: anywhere; }
.sources-action { margin-inline-start: 12px; }
.sources-report { white-space: pre-wrap; overflow-wrap: anywhere; max-height: 280px; overflow: auto; padding: 12px; background: var(--control-surface); border-radius: 8px; user-select: text; }
.sources-form { display: grid; gap: 12px; }
label { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; min-width: 0; }
.sources-form label { display: grid; }
select, input, textarea { min-height: 44px; max-width: 100%; width: 100%; padding: 8px; color: var(--text); background: var(--surface); border: 1px solid var(--border); }
.sources-pickers select { width: auto; max-width: min(100%, 340px); }
</style>
