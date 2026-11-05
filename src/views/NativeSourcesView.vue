<script setup lang="ts">
import { computed, reactive, ref } from "vue";
import { NAlert, NButton, NEmpty, NModal } from "naive-ui";
import { appState as state, appController as controller, validateSourceDraft, type SourceDraft } from "../state";
import { locale, t } from "../i18n";
import type { AppSource, HttpSourceConfig } from "../models";

const emit = defineEmits<{ select: [id: string, role: "metadata" | "playback"] }>();
const word = (zh: string, en: string) => locale.value === "zh-CN" ? zh : en;
const providers = computed(() => state.sources.data?.providers ?? []);
const metadataSources = computed(() => providers.value.filter(source => source.kind === "stremio" && source.runtime_status === "ready" && source.capabilities.home && source.capabilities.details));
const playbackSources = computed(() => providers.value.filter(source => source.kind !== "stremio" && source.runtime_status === "ready" && source.capabilities.resolve));
const cloudstreamSources = computed(() => providers.value.filter(source => source.kind === "cloudstream"));
const configs = computed(() => state.settings.source_configs);
const plugins = computed(() => state.sources.data?.plugins ?? []);
const cloudstream = computed(() => state.runtime.cloudstream);

const providerOf = (config: HttpSourceConfig) => providers.value.find(source => source.kind === config.kind && source.origin.config_id === config.config_id);
const hostOf = (config: HttpSourceConfig) => (config.kind === "maccms" ? [config.api_url] : config.manifest_urls).map(url => { try { return new URL(url).host; } catch { return ""; } }).join(", ");
function lastRequest(config: HttpSourceConfig) {
  const source = providerOf(config), result = source && controller.lastSourceResult(source.id);
  return !result ? word('本次运行尚无请求', 'no request this session') : result.ok ? word('成功', 'succeeded') : result.message;
}
const kindLabel = (kind: string) => kind === "stremio" ? word("Stremio 元数据", "Stremio metadata") : kind === "maccms" ? word("MacCMS 播放", "MacCMS playback") : "Cloudstream";

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
const prepare = () => run(() => controller.prepareCloudstream());
const finishInteraction = (action: "completed" | "cancelled") => run(() => controller.finishInteraction(action));
const pendingDelete = ref<HttpSourceConfig | null>(null);
async function confirmDelete() {
  const config = pendingDelete.value!;
  pendingDelete.value = null;
  await run(() => controller.removeSourceConfig(config.config_id));
}

const interactionSource = computed(() => cloudstreamSources.value.find(source => source.error?.code === "INTERACTION_REQUIRED") ?? null);
const pendingInteraction = ref<AppSource | null>(null);
async function confirmInteraction() {
  const source = pendingInteraction.value!;
  pendingInteraction.value = null;
  await run(() => controller.startInteraction(source));
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

    <section class="settings-panel">
      <h2>{{ word('当前选择', 'Current selection') }}</h2>
      <div class="sources-pickers">
        <label>{{ t('Metadata provider') }}<select class="sources-metadata" :value="state.selected_metadata_source?.source_id ?? ''" @change="emit('select', ($event.target as HTMLSelectElement).value, 'metadata')"><option value="" disabled>{{ t('Choose a metadata source') }}</option><option v-for="source in metadataSources" :key="source.id" :value="source.id">{{ source.name }}</option></select></label>
        <label>{{ word('播放源', 'Playback source') }}<select class="sources-playback" :value="state.selected_playback_source?.source_id ?? ''" @change="emit('select', ($event.target as HTMLSelectElement).value, 'playback')"><option value="" disabled>{{ t('Choose a video source') }}</option><option v-for="source in playbackSources" :key="source.id" :value="source.id">{{ source.name }}</option></select></label>
      </div>
      <p v-if="!metadataSources.length" class="sources-guidance" role="status">{{ word('请添加 Stremio 扩展的 manifest 地址，首页才能显示内容。', 'Add a Stremio add-on manifest URL so the home page has content.') }}</p>
      <p v-if="!playbackSources.length" class="sources-guidance" role="status">{{ word('请准备 Cloudstream，或添加 MacCMS 播放源。', 'Prepare Cloudstream or add a MacCMS playback source.') }}</p>
    </section>

    <section class="settings-panel">
      <div class="sources-heading"><h2>{{ word('HTTP 来源', 'HTTP sources') }}</h2><NButton class="sources-add" type="primary" :loading="busy" @click="openDraft()">{{ word('添加来源', 'Add source') }}</NButton></div>
      <NEmpty v-if="!configs.length" class="sources-empty" :description="word('尚未配置来源', 'No sources configured')" />
      <article v-for="config in configs" :key="config.config_id" class="sources-row" :data-config-id="config.config_id">
        <div class="sources-row-text">
          <h3>{{ config.name }}</h3>
          <p>{{ kindLabel(config.kind) }} · {{ hostOf(config) }}</p>
          <p class="sources-state">{{ word('登记状态', 'Registration') }}: {{ config.enabled ? providerOf(config)?.runtime_status ?? word('未同步', 'not synced') : word('已禁用', 'disabled') }}</p>
          <p class="sources-last">{{ word('最近请求', 'Last request') }}: {{ lastRequest(config) }}</p>
          <NAlert v-if="providerOf(config)?.error" class="sources-row-error" type="error">{{ providerOf(config)!.error!.message }}</NAlert>
        </div>
        <div class="sources-row-actions">
          <NButton class="sources-edit" :disabled="busy" @click="openDraft(config)">{{ word('编辑', 'Edit') }}</NButton>
          <NButton class="sources-toggle" :disabled="busy" @click="toggleConfig(config)">{{ config.enabled ? word('禁用', 'Disable') : word('启用', 'Enable') }}</NButton>
          <NButton class="sources-delete" :disabled="busy" @click="pendingDelete = config">{{ word('删除', 'Delete') }}</NButton>
        </div>
      </article>
    </section>

    <section class="settings-panel sources-cloudstream">
      <div class="sources-heading"><h2>Cloudstream</h2><NButton class="sources-prepare" :loading="busy" :disabled="cloudstream.phase === 'starting'" @click="prepare">{{ word('准备引擎', 'Prepare engine') }}</NButton></div>
      <p class="sources-guest-state">{{ word('客体状态', 'Guest state') }}: {{ cloudstream.phase }} · {{ cloudstream.display_mode }}</p>
      <NAlert v-if="cloudstream.error" class="sources-guest-error" type="error">{{ cloudstream.error.message }}</NAlert>
      <article v-for="plugin in plugins" :key="plugin.internal_name" class="sources-plugin">
        <h3>{{ plugin.internal_name }} · v{{ plugin.version }} · {{ plugin.artifact_sha256.slice(0, 12) }}</h3>
        <p>{{ plugin.status }} · {{ word('实例', 'providers') }} {{ plugin.provider_ids.length }}</p>
        <NAlert v-if="plugin.error" type="error">{{ plugin.error.message }}</NAlert>
      </article>
      <template v-if="interactionSource">
        <p class="sources-interaction-needed" role="status">{{ word('来源需要人工交互', 'A source needs interaction') }}: {{ interactionSource.name }}</p>
        <NButton class="sources-interaction-open" :disabled="busy || Boolean(controller.interaction.active)" @click="pendingInteraction = interactionSource">{{ word('打开源站交互', 'Open source interaction') }}</NButton>
      </template>
      <NAlert v-if="controller.interaction.last?.status === 'failed'" class="sources-interaction-failed" type="error" role="alert">{{ controller.interaction.last.message }}</NAlert>
      <div v-if="controller.interaction.active" class="sources-interaction" role="status">
        <p>{{ controller.interaction.active.message }} · {{ controller.interaction.active.status }}</p>
        <NButton class="sources-interaction-done" type="primary" :disabled="busy" @click="finishInteraction('completed')">{{ word('我已完成', 'I finished') }}</NButton>
        <NButton class="sources-interaction-cancel" :disabled="busy" @click="finishInteraction('cancelled')">{{ t('Cancel') }}</NButton>
      </div>
    </section>

    <section class="settings-panel sources-diagnostics">
      <div class="sources-heading"><h2>{{ word('诊断', 'Diagnostics') }}</h2><NButton class="sources-copy" @click="copyDiagnostics">{{ word('复制脱敏诊断', 'Copy redacted diagnostics') }}</NButton></div>
      <p v-if="copyState" class="sources-copy-state" role="status">{{ copyState }}</p>
      <pre class="sources-report">{{ report }}</pre>
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
  <NModal :show="Boolean(pendingInteraction)" preset="card" role="dialog" class="sources-confirm" :title="word('打开源站交互', 'Open source interaction')" :aria-label="word('打开源站交互', 'Open source interaction')" :style="{ width: 'min(480px, calc(100vw - 32px))' }" @update:show="pendingInteraction = null">
    <p>{{ word('当前播放会停止，引擎将以可见窗口重启。HTTP 来源不受影响。', 'Current playback stops and the engine restarts with a visible window. HTTP sources are unaffected.') }}</p>
    <div class="sources-form-actions"><NButton class="sources-interaction-reject" @click="pendingInteraction = null">{{ t('Cancel') }}</NButton><NButton class="sources-interaction-confirm" type="primary" @click="confirmInteraction">{{ word('继续', 'Continue') }}</NButton></div>
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
.sources-plugin, .sources-interaction { padding: 10px 0; overflow-wrap: anywhere; }
.sources-action { margin-inline-start: 12px; }
.sources-report { white-space: pre-wrap; overflow-wrap: anywhere; max-height: 280px; overflow: auto; padding: 12px; background: var(--control-surface); border-radius: 8px; user-select: text; }
.sources-form { display: grid; gap: 12px; }
label { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; min-width: 0; }
.sources-form label { display: grid; }
select, input, textarea { min-height: 44px; max-width: 100%; width: 100%; padding: 8px; color: var(--text); background: var(--surface); border: 1px solid var(--border); }
.sources-pickers select { width: auto; max-width: min(100%, 340px); }
</style>
