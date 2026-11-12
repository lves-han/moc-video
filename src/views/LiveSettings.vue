<script setup lang="ts">
import { computed, reactive, ref } from "vue";
import { NAlert, NButton, NModal } from "naive-ui";
import { appState as state, appController as controller } from "../state";
import { locale, t } from "../i18n";
import type { Settings } from "../models";

type LiveConfig = NonNullable<Settings["live_configs"]>[number];
const word = (zh: string, en: string) => locale.value === "zh-CN" ? zh : en;
const configs = computed(() => state.settings.live_configs ?? []);
const busy = ref(false), failure = ref("");
async function save(next: LiveConfig[]) {
  busy.value = true; failure.value = "";
  try { await controller.updateSettings(settings => ({ ...settings, live_configs: next })); }
  catch (cause) { failure.value = cause instanceof Error ? cause.message : String(cause); }
  finally { busy.value = false; }
}

const editing = ref(false);
const draft = reactive({ config_id: null as string | null, name: "", url: "", epg_url: "", enabled: true });
const errors = ref<string[]>([]);
function openDraft(config?: LiveConfig) {
  Object.assign(draft, config ? { ...config, epg_url: config.epg_url ?? "" } : { config_id: null, name: "", url: "", epg_url: "", enabled: true });
  errors.value = []; editing.value = true;
}
const httpUrl = (value: string) => {
  try { const url = new URL(value); return ["http:", "https:"].includes(url.protocol) && !url.username && !url.password && !url.hash; }
  catch { return false; }
};
async function submit() {
  const name = draft.name.trim(), url = draft.url.trim(), epg = draft.epg_url.trim();
  errors.value = [
    ...(!name ? [word("请输入名称。", "Enter a name.")] : []),
    ...(!httpUrl(url) ? [word("列表地址须为不含账号和片段的 http(s) 地址。", "The list address must be http(s) without credentials or a fragment.")] : []),
    ...(epg && !httpUrl(epg) ? [word("节目单地址须为 http(s) 地址。", "The EPG address must be http(s).")] : []),
    ...(!draft.config_id && configs.value.length >= 32 ? [word("最多 32 个直播列表。", "At most 32 live lists.")] : []),
  ];
  if (errors.value.length) return;
  const config: LiveConfig = { config_id: draft.config_id ?? crypto.randomUUID(), name, url, epg_url: epg || null, enabled: draft.enabled };
  editing.value = false;
  await save(draft.config_id ? configs.value.map(row => row.config_id === config.config_id ? config : row) : [...configs.value, config]);
}
const toggle = (config: LiveConfig) => save(configs.value.map(row => row.config_id === config.config_id ? { ...row, enabled: !row.enabled } : row));
const pendingDelete = ref<LiveConfig | null>(null);
async function confirmDelete() {
  const config = pendingDelete.value!; pendingDelete.value = null;
  await save(configs.value.filter(row => row.config_id !== config.config_id));
}
const host = (value: string) => { try { return new URL(value).host; } catch { return value; } };
</script>

<template>
  <div class="live-settings">
    <NAlert v-if="failure" type="error" role="alert">{{ failure }}</NAlert>
    <div class="sources-heading"><p>{{ word('支持 IPTV（M3U/M3U8）和直播源（TXT）列表，按内容自动识别。启用后左侧菜单显示「直播」。', 'Accepts IPTV (M3U/M3U8) and live source (TXT) lists, detected from the content. Live appears in the sidebar once a list is enabled.') }}</p><NButton class="live-add" :disabled="busy" @click="openDraft()">{{ word('添加列表', 'Add list') }}</NButton></div>
    <p v-if="!configs.length" class="sources-empty">{{ word('尚未添加直播列表', 'No live lists') }}</p>
    <article v-for="config in configs" :key="config.config_id" class="sources-row live-row" :data-config-id="config.config_id">
      <span class="source-status" :class="config.enabled ? 'ready' : 'disabled'" aria-hidden="true" /><div class="sources-row-text">
        <h3>{{ config.name }}</h3>
        <p>{{ host(config.url) }} · {{ config.enabled ? word('已启用', 'Enabled') : word('已禁用', 'Disabled') }}<template v-if="config.epg_url"> · EPG</template></p>
      </div>
      <div class="sources-row-actions">
        <NButton class="live-edit" :disabled="busy" @click="openDraft(config)">{{ word('编辑', 'Edit') }}</NButton>
        <NButton class="live-toggle" :disabled="busy" @click="toggle(config)">{{ config.enabled ? word('禁用', 'Disable') : word('启用', 'Enable') }}</NButton>
        <NButton class="live-delete" :disabled="busy" @click="pendingDelete = config">{{ word('删除', 'Delete') }}</NButton>
      </div>
    </article>
  </div>
  <NModal :show="editing" preset="card" role="dialog" class="tv-dialog live-dialog" :title="draft.config_id ? word('编辑直播列表', 'Edit live list') : word('添加直播列表', 'Add live list')" :style="{ width: 'min(560px, calc(100vw - 32px))' }" @update:show="editing = $event">
    <form class="sources-form" @submit.prevent="submit">
      <label>{{ word('名称', 'Name') }}<input class="live-name" v-model="draft.name" maxlength="128" /></label>
      <label>{{ word('列表地址（M3U 或 TXT）', 'List address (M3U or TXT)') }}<input class="live-url" v-model="draft.url" maxlength="2048" /></label>
      <label>{{ word('节目单地址（EPG，可选，暂只保存）', 'EPG address (optional, saved only for now)') }}<input class="live-epg" v-model="draft.epg_url" maxlength="2048" /></label>
      <NAlert v-if="errors.length" class="live-errors" type="error" role="alert"><p v-for="error in errors" :key="error">{{ error }}</p></NAlert>
      <div class="sources-form-actions"><NButton @click="editing = false">{{ t('Cancel') }}</NButton><NButton class="live-save" attr-type="submit" type="primary">{{ word('保存', 'Save') }}</NButton></div>
    </form>
  </NModal>
  <NModal :show="Boolean(pendingDelete)" preset="card" role="dialog" class="tv-dialog live-confirm" :title="word('删除直播列表', 'Delete live list')" :style="{ width: 'min(480px, calc(100vw - 32px))' }" @update:show="pendingDelete = null">
    <p>{{ pendingDelete?.name }}</p>
    <div class="sources-form-actions"><NButton @click="pendingDelete = null">{{ t('Cancel') }}</NButton><NButton class="live-confirm-delete" type="error" @click="confirmDelete">{{ word('删除', 'Delete') }}</NButton></div>
  </NModal>
</template>

<style scoped>
.live-settings { display: grid; gap: 4px; min-width: 0; }
.sources-heading, .sources-form-actions { display: flex; flex-wrap: wrap; justify-content: space-between; align-items: center; gap: 12px; }
.sources-heading p, .sources-empty, .sources-row-text p { color: var(--muted); }
.sources-heading p { flex: 1 1 280px; }
.sources-form-actions { justify-content: flex-end; margin-top: 12px; }
.sources-row { display: flex; flex-wrap: wrap; justify-content: space-between; gap: 16px; padding: 14px 0; border-top: 1px solid var(--border); }
.sources-row-text { min-width: 0; flex: 1 1 240px; overflow-wrap: anywhere; }
.sources-row-actions { display: flex; flex-wrap: wrap; gap: 8px; align-items: flex-start; }
.sources-form { display: grid; gap: 12px; }
.sources-form label { display: grid; gap: 8px; }
input { min-height: 44px; width: 100%; padding: 8px; color: var(--text); background: var(--surface); border: 1px solid var(--border); }
</style>
