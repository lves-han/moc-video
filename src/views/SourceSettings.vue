<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import { NAlert, NButton, NModal, NSwitch } from "naive-ui";
import { appState as state, appController as controller, validateSourceDraft, TMDB_CONFIG_ID, TMDB_DEFAULT_INSTANCE, TMDB_API_URL, TMDB_IMAGE_URL, type SourceDraft } from "../state";
import { locale, t } from "../i18n";
import { describeError } from "../api";
import { unaddedPresets } from "../data/sourcePresets";
import type { HttpSourceConfig, Settings } from "../models";

// One component serves both settings groups so source editing keeps a single implementation.
const props = defineProps<{ section: "metadata" | "playback" }>();
const emit = defineEmits<{ select: [id: string, role: "metadata" | "playback"] }>();
const word = (zh: string, en: string) => locale.value === "zh-CN" ? zh : en;
const kind = computed(() => props.section === "metadata" ? "stremio" : "maccms");
const providers = computed(() => state.sources.data?.providers ?? []);
const metadataSources = computed(() => providers.value.filter(source => source.kind === "stremio" && source.runtime_status === "ready" && source.capabilities.home && source.capabilities.details));
const playbackSources = computed(() => providers.value.filter(source => source.kind !== "stremio" && source.runtime_status === "ready" && source.capabilities.resolve));
const configs = computed(() => state.settings.source_configs);
const sectionConfigs = computed(() => configs.value.filter(config => config.kind === kind.value));
// Presets not yet added appear after the configured sources, in the same list.
const presets = computed(() => props.section === "playback" ? unaddedPresets(configs.value) : []);

const providerOf = (config: { kind: HttpSourceConfig["kind"]; config_id: string }) => providers.value.find(source => source.kind === config.kind && source.origin.config_id === config.config_id);
function sourceError(config: { kind: HttpSourceConfig["kind"]; config_id: string }) {
  const provider = providerOf(config), result = provider && controller.lastSourceResult(provider.id);
  return provider?.error ? describeError(provider.error) : result && !result.ok ? result.message : '';
}
const hostOf = (config: HttpSourceConfig) => (config.kind === "maccms" ? [config.api_url] : config.manifest_urls).map(url => { try { return new URL(url).host; } catch { return ""; } }).join(", ");
const statusOf = (config: { kind: HttpSourceConfig["kind"]; config_id: string; enabled: boolean }) => !config.enabled ? 'disabled' : providerOf(config)?.runtime_status === 'ready' ? 'ready' : 'failed';
const statusText = (config: { kind: HttpSourceConfig["kind"]; config_id: string; enabled: boolean }) => !config.enabled ? word('已禁用', 'Disabled')
  : providerOf(config)?.runtime_status === 'ready' ? word('可用', 'Ready') : providerOf(config)?.runtime_status === 'failed' ? word('失败', 'Failed') : word('未同步', 'Not synced');

const busy = ref(false);
const failure = ref("");
async function run(work: () => Promise<unknown>) {
  busy.value = true; failure.value = "";
  try { await work(); return true; }
  catch (cause) { failure.value = cause instanceof Error ? cause.message : String(cause); return false; }
  finally { busy.value = false; }
}

type Tmdb = NonNullable<Settings["tmdb"]>;
const tmdb = computed<Tmdb>(() => state.settings.tmdb ?? { enabled: true, instance_url: TMDB_DEFAULT_INSTANCE, language: "follow" });
const tmdbConfig = computed(() => ({ kind: "stremio" as const, config_id: TMDB_CONFIG_ID, enabled: tmdb.value.enabled }));
const instanceDraft = ref(tmdb.value.instance_url);
watch(() => tmdb.value.instance_url, value => { instanceDraft.value = value; });
const instanceError = ref("");
const languageOptions = computed(() => [
  { value: "follow", label: word("跟随应用语言", "Follow app language") },
  { value: "zh-CN", label: "简体中文" },
  { value: "en", label: "English" },
]);
const saveTmdb = (change: Partial<Tmdb>) => run(() => controller.saveTmdb({ ...tmdb.value, ...change }));
const apiMode = computed(() => tmdb.value.mode === "api");
// Switching to API mode first shows the key form; the mode is saved together with a key.
const choosingApi = ref(false);
const apiDraft = reactive({ api_key: "", api_url: TMDB_API_URL, image_url: TMDB_IMAGE_URL });
watch(tmdb, value => Object.assign(apiDraft, { api_key: value.api_key ?? "", api_url: value.api_url ?? TMDB_API_URL, image_url: value.image_url ?? TMDB_IMAGE_URL }), { immediate: true });
const apiError = ref("");
function changeMode(value: string) {
  apiError.value = "";
  if (value === "api") { choosingApi.value = true; return; }
  choosingApi.value = false;
  if (apiMode.value) void saveTmdb({ mode: "stremio" });
}
function saveApi() {
  const fields = { api_key: apiDraft.api_key.trim(), api_url: apiDraft.api_url.trim().replace(/\/+$/, ""), image_url: apiDraft.image_url.trim().replace(/\/+$/, "") };
  const valid = (value: string) => { try { const url = new URL(value); return ["http:", "https:"].includes(url.protocol) && !url.username && !url.password && !url.hash && !url.search; } catch { return false; } };
  if (!fields.api_key) { apiError.value = word("请输入 API Key。", "Enter an API key."); return; }
  if (!valid(fields.api_url) || !valid(fields.image_url)) { apiError.value = word("API 地址和图片地址须为不含账号、查询参数和片段的 http(s) 地址。", "API and image addresses must be http(s) without credentials, query or fragment."); return; }
  apiError.value = ""; choosingApi.value = false;
  void saveTmdb({ ...fields, mode: "api" });
}
function saveInstance() {
  const value = instanceDraft.value.trim().replace(/\/+$/, "");
  let parsed: URL | null = null;
  try { parsed = new URL(value); } catch { /* reported below */ }
  if (!parsed || !["http:", "https:"].includes(parsed.protocol) || parsed.username || parsed.password || parsed.hash || parsed.search) {
    instanceError.value = word("请输入不含账号、查询参数和片段的 http(s) 地址。", "Enter an http(s) address without credentials, query or fragment."); return;
  }
  instanceError.value = "";
  void saveTmdb({ instance_url: value });
}

const editing = ref(false);
const draft = reactive<SourceDraft & { config_id: string | null; enabled: boolean }>({ config_id: null, kind: "maccms", name: "", urls: "", enabled: true });
const errors = ref<string[]>([]);
function openDraft(config?: HttpSourceConfig) {
  Object.assign(draft, config ? { config_id: config.config_id, kind: config.kind, name: config.name, enabled: config.enabled, urls: config.kind === "maccms" ? config.api_url : config.manifest_urls.join("\n") }
    : { config_id: null, kind: kind.value, name: "", urls: "", enabled: true });
  errors.value = []; failure.value = ""; editing.value = true;
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
  <div class="source-settings" :data-section="section">
    <NAlert v-if="state.sources.error" class="sources-list-error" type="error">{{ state.sources.error.message }}<NButton size="small" class="sources-action" @click="controller.loadSources()">{{ t('Retry') }}</NButton></NAlert>
    <NAlert v-if="failure" class="sources-failure" type="error" role="alert">{{ failure }}</NAlert>

    <template v-if="section === 'metadata'">
      <div class="setting-row tmdb-row">
        <div>
          <h3>TMDB</h3>
          <p>{{ word('通用 TMDB 元数据源', 'General TMDB metadata source') }} · <span class="tmdb-status">{{ statusText(tmdbConfig) }}</span></p>
          <p v-if="sourceError(tmdbConfig)" class="sources-row-error" role="alert">{{ sourceError(tmdbConfig) }}</p>
        </div>
        <NSwitch class="tmdb-enabled" :value="tmdb.enabled" :disabled="busy" :aria-label="word('启用 TMDB', 'Enable TMDB')" @update:value="saveTmdb({ enabled: $event })" />
      </div>
      <div class="setting-row">
        <div><h3><label for="tmdb-mode">{{ word('接入方式', 'Connection') }}</label></h3><p>{{ apiMode ? word('使用自己的 TMDB API Key 直接访问官方接口。', 'Calls the official TMDB API with your own key.') : word('通过 TMDB Stremio 扩展获取，无需 API Key。', 'Uses the TMDB Stremio add-on; no API key needed.') }}</p></div>
        <select id="tmdb-mode" class="setting-select compact-select tmdb-mode" :value="apiMode || choosingApi ? 'api' : 'stremio'" :disabled="busy" @change="changeMode(($event.target as HTMLSelectElement).value)"><option value="stremio">{{ t('Stremio add-on') }}</option><option value="api">{{ word('TMDB 官方 API', 'TMDB official API') }}</option></select>
      </div>
      <template v-if="apiMode || choosingApi">
        <div class="setting-row tmdb-api-row">
          <div><h3><label for="tmdb-api-key">API Key</label></h3><p>{{ word('v3 API Key 或 v4 读访问令牌，保存在本机设置文件中。', 'A v3 API key or v4 read access token, kept in the settings file on this device.') }}</p></div>
          <input id="tmdb-api-key" v-model="apiDraft.api_key" type="password" autocomplete="off" class="setting-select compact-select tmdb-instance tmdb-api-key" :disabled="busy" />
        </div>
        <div class="setting-row tmdb-api-row">
          <div><h3><label for="tmdb-api-url">{{ word('API 地址', 'API address') }}</label></h3><p>{{ word('无法直连时可改为代理地址。', 'Use a proxy address if the official one is unreachable.') }}</p></div>
          <input id="tmdb-api-url" v-model="apiDraft.api_url" class="setting-select compact-select tmdb-instance tmdb-api-url" :disabled="busy" />
        </div>
        <div class="setting-row tmdb-api-row">
          <div><h3><label for="tmdb-image-url">{{ word('图片地址', 'Image address') }}</label></h3><p>{{ word('海报和剧照的地址前缀。', 'Address prefix for posters and stills.') }}</p></div>
          <div class="tmdb-instance-controls"><input id="tmdb-image-url" v-model="apiDraft.image_url" class="setting-select compact-select tmdb-instance tmdb-image-url" :disabled="busy" /><NButton class="tmdb-api-save" type="primary" :disabled="busy" @click="saveApi">{{ t('Save') }}</NButton></div>
        </div>
        <p v-if="apiError" class="sources-row-error" role="alert">{{ apiError }}</p>
      </template>
      <div v-else class="setting-row">
        <div><h3><label for="tmdb-instance">{{ word('实例地址', 'Instance address') }}</label></h3><p>{{ word('默认使用 elfhosted 公共实例，可改为自己部署的地址。', 'Uses the public elfhosted instance by default; you can use your own.') }}</p></div>
        <div class="tmdb-instance-controls"><input id="tmdb-instance" v-model="instanceDraft" class="setting-select compact-select tmdb-instance" :disabled="busy" @keydown.enter.prevent="saveInstance" /><NButton class="tmdb-instance-save" :disabled="busy || instanceDraft === tmdb.instance_url" @click="saveInstance">{{ t('Save') }}</NButton></div>
      </div>
      <p v-if="instanceError && !apiMode" class="sources-row-error" role="alert">{{ instanceError }}</p>
      <div class="setting-row">
        <div><h3><label for="tmdb-language">{{ t('Metadata language') }}</label></h3><p>{{ word('默认跟随应用语言。', 'Follows the app language by default.') }}</p></div>
        <select id="tmdb-language" class="setting-select compact-select tmdb-language" :value="tmdb.language" :disabled="busy" @change="saveTmdb({ language: ($event.target as HTMLSelectElement).value as Tmdb['language'] })"><option v-for="item in languageOptions" :key="item.value" :value="item.value">{{ item.label }}</option></select>
      </div>
      <div class="sources-heading"><h3>{{ word('其他 Stremio 扩展', 'Other Stremio add-ons') }}</h3><NButton class="sources-add" :disabled="busy" @click="openDraft()">{{ word('添加扩展', 'Add add-on') }}</NButton></div>
    </template>

    <template v-else>
      <NAlert v-if="state.runtime.error" class="sources-runtime-error" type="error">{{ state.runtime.error.message }}</NAlert>
      <div class="sources-pickers">
        <label>{{ t('Metadata provider') }}<select class="sources-metadata" :value="state.selected_metadata_source?.source_id ?? ''" @change="emit('select', ($event.target as HTMLSelectElement).value, 'metadata')"><option value="" disabled>{{ t('Choose a metadata source') }}</option><option v-for="source in metadataSources" :key="source.id" :value="source.id">{{ source.name }}</option></select></label>
        <label>{{ word('播放源', 'Playback source') }}<select class="sources-playback" :value="state.selected_playback_source?.source_id ?? ''" @change="emit('select', ($event.target as HTMLSelectElement).value, 'playback')"><option value="" disabled>{{ t('Choose a video source') }}</option><option v-for="source in playbackSources" :key="source.id" :value="source.id">{{ source.name }}</option></select></label>
      </div>
      <p v-if="!metadataSources.length" class="sources-guidance" role="status">{{ word('请启用 TMDB 或添加 Stremio 扩展，首页才能显示内容。', 'Enable TMDB or add a Stremio add-on so the home page has content.') }}</p>
      <p v-if="!playbackSources.length" class="sources-guidance" role="status">{{ word('请添加一个播放源。', 'Add a playback source.') }}</p>
      <div class="sources-heading"><h3>{{ word('播放源', 'Playback sources') }}</h3><NButton class="sources-add" :disabled="busy" @click="openDraft()">{{ word('添加来源', 'Add source') }}</NButton></div>
      <p class="sources-presets-note">{{ word('仅供个人使用。第三方来源的内容和可用性以来源为准。', 'For personal use only. Third-party sources decide their own content and availability.') }}</p>
    </template>

    <p v-if="section === 'metadata' && !sectionConfigs.length" class="sources-empty">{{ word('尚未添加其他扩展', 'No other add-ons') }}</p>
    <article v-for="config in sectionConfigs" :key="config.config_id" class="sources-row" :data-config-id="config.config_id">
      <span class="source-status" :class="statusOf(config)" :aria-label="statusText(config)" /><div class="sources-row-text">
        <h3>{{ config.name }}</h3>
        <p>{{ hostOf(config) }} · <span class="sources-state">{{ statusText(config) }}</span></p>
        <p v-if="sourceError(config)" class="sources-row-error" role="alert">{{ sourceError(config) }}</p>
      </div>
      <div class="sources-row-actions">
        <NButton class="sources-edit" :disabled="busy" @click="openDraft(config)">{{ word('编辑', 'Edit') }}</NButton>
        <NButton class="sources-toggle" :disabled="busy" @click="toggleConfig(config)">{{ config.enabled ? word('禁用', 'Disable') : word('启用', 'Enable') }}</NButton>
        <NButton class="sources-delete" :disabled="busy" @click="pendingDelete = config">{{ word('删除', 'Delete') }}</NButton>
      </div>
    </article>
    <article v-for="preset in presets" :key="preset.api_url" class="sources-row sources-preset" :data-preset="preset.name">
      <span class="source-status disabled" aria-hidden="true" /><div class="sources-row-text"><h3>{{ preset.name }}</h3><p>{{ word('未添加', 'Not added') }}</p></div>
      <div class="sources-row-actions"><NButton class="sources-preset-add" :disabled="busy" @click="addPreset(preset)">{{ word('添加', 'Add') }}</NButton></div>
    </article>

    <details v-if="section === 'playback'" class="sources-diagnostics">
      <summary>{{ word('诊断', 'Diagnostics') }}</summary>
      <NButton class="sources-copy" size="small" @click="copyDiagnostics">{{ word('复制脱敏诊断', 'Copy redacted diagnostics') }}</NButton>
      <p v-if="copyState" class="sources-copy-state" role="status">{{ copyState }}</p>
      <pre class="sources-report">{{ report }}</pre>
    </details>
  </div>

  <NModal :show="editing" preset="card" role="dialog" class="tv-dialog sources-dialog" :title="draft.config_id ? word('编辑来源', 'Edit source') : word('添加来源', 'Add source')" :aria-label="draft.config_id ? word('编辑来源', 'Edit source') : word('添加来源', 'Add source')" :style="{ width: 'min(560px, calc(100vw - 32px))' }" @update:show="editing = $event">
    <form class="sources-form" @submit.prevent="submit">
      <label>{{ word('名称', 'Name') }}<input class="sources-name" v-model="draft.name" maxlength="128" /></label>
      <label>{{ draft.kind === 'stremio' ? word('Manifest 地址（每行一个，最多 16 个）', 'Manifest URLs (one per line, up to 16)') : word('MacCMS JSON API 地址', 'MacCMS JSON API URL') }}<textarea class="sources-urls" v-model="draft.urls" rows="3" /></label>
      <NAlert v-if="errors.length" class="sources-errors" type="error" role="alert"><p v-for="error in errors" :key="error">{{ error }}</p></NAlert>
      <div class="sources-form-actions"><NButton class="sources-form-cancel" @click="editing = false">{{ t('Cancel') }}</NButton><NButton class="sources-save" attr-type="submit" type="primary">{{ word('保存', 'Save') }}</NButton></div>
    </form>
  </NModal>
  <NModal :show="Boolean(pendingDelete)" preset="card" role="dialog" class="tv-dialog sources-confirm" :title="word('删除来源', 'Delete source')" :aria-label="word('删除来源', 'Delete source')" :style="{ width: 'min(480px, calc(100vw - 32px))' }" @update:show="pendingDelete = null">
    <p>{{ word('删除后，正在使用它的播放会停止。', 'Playback using it will stop.') }} {{ pendingDelete?.name }}</p>
    <div class="sources-form-actions"><NButton class="sources-confirm-cancel" @click="pendingDelete = null">{{ t('Cancel') }}</NButton><NButton class="sources-confirm-delete" type="error" @click="confirmDelete">{{ word('删除', 'Delete') }}</NButton></div>
  </NModal>
</template>

<style scoped>
.source-settings { display: grid; gap: 4px; min-width: 0; }
.sources-heading, .sources-form-actions { display: flex; flex-wrap: wrap; justify-content: space-between; align-items: center; gap: 12px; margin-top: 12px; }
.sources-form-actions { justify-content: flex-end; }
.sources-pickers { display: flex; flex-wrap: wrap; gap: 16px; }
.sources-row { display: flex; flex-wrap: wrap; justify-content: space-between; gap: 16px; padding: 14px 0; border-top: 1px solid var(--border); }
.sources-row-text { min-width: 0; flex: 1 1 240px; overflow-wrap: anywhere; }
.sources-row-text p, .sources-presets-note, .sources-empty { color: var(--muted); }
.sources-row-actions { display: flex; flex-wrap: wrap; gap: 8px; align-items: flex-start; }
.sources-action { margin-inline-start: 12px; }
.sources-row-error { color: var(--danger, #d03050); overflow-wrap: anywhere; }
.sources-diagnostics { margin-top: 12px; }
.sources-diagnostics summary { cursor: pointer; min-height: 44px; display: flex; align-items: center; }
.sources-report { white-space: pre-wrap; overflow-wrap: anywhere; max-height: 280px; overflow: auto; padding: 12px; margin-top: 8px; background: var(--control-surface); border-radius: 8px; user-select: text; }
.tmdb-instance-controls { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; max-width: 100%; }
.tmdb-instance { width: min(320px, 100%); }
.sources-form { display: grid; gap: 12px; }
label { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; min-width: 0; }
.sources-form label { display: grid; }
select, input, textarea { min-height: 44px; max-width: 100%; padding: 8px; color: var(--text); background: var(--surface); border: 1px solid var(--border); }
.sources-form input, .sources-form textarea { width: 100%; }
.sources-pickers select { width: auto; max-width: min(100%, 340px); }
</style>
