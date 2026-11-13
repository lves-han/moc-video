<script setup lang="ts">
import { onMounted, reactive, ref, watch } from "vue";
import { NRadioGroup, NRadioButton, NSwitch, NTag, NButton, NAlert, NModal, useOsTheme } from "naive-ui";
import { api, ApiFailure, describeError, settingsQueue } from "../api";
import { appController, appState } from "../state";
import type { MediaCacheStatus, Settings } from "../models";
import Icon from "../components/Icon.vue";
import { fontOptions, setAppFont, setFontScale } from '../ui/fonts';
import sansLicense from '../../licenses/fonts/notosanssc-OFL.txt?raw';
import serifLicense from '../../licenses/fonts/notoserifsc-OFL.txt?raw';
import { locale, t } from "../i18n";
import {
  palette,
  paletteOptions,
} from "../ui/theme";

const speed = defineModel<number>("speed", { required: true });
const props = defineProps<{ preview?: boolean }>();
const autoNext = defineModel<boolean>("autoNext", { required: true });
const emit = defineEmits<{ notice: [message: string]; language: [value: string]; palette: [value: string]; select: [id: string, role: "metadata" | "playback"] }>();
const serviceDraft = reactive({ url: appState.settings.playback_service?.url ?? 'http://127.0.0.1:8787', token: appState.settings.playback_service?.token ?? '' });
const serviceBusy = ref(false), serviceError = ref('');
const word = (zh: string, en: string) => locale.value === 'zh-CN' ? zh : en;
async function connectService() {
  const url = serviceDraft.url.trim().replace(/\/+$/, ''), token = serviceDraft.token.trim();
  if (!url || !token) { serviceError.value = word('请输入服务地址和令牌。', 'Enter the service address and token.'); return; }
  serviceBusy.value = true; serviceError.value = '';
  try { await appController.connectPlaybackService(url, token); }
  catch (error) { serviceError.value = error instanceof Error ? error.message : String(error); }
  finally { serviceBusy.value = false; }
}
async function disconnectService() {
  serviceBusy.value = true; serviceError.value = '';
  try { await appController.disconnectPlaybackService(); }
  catch (error) { serviceError.value = error instanceof Error ? error.message : String(error); }
  finally { serviceBusy.value = false; }
}
const osTheme = useOsTheme();
const languageOptions = [
  { value: "en", label: "English" },
  { value: "zh-CN", label: "简体中文" },
];
const speedOptions = [0.5, 1, 1.25, 1.5, 2].map((value) => ({
  value,
  label: `${value}×`,
}));
function changeLanguage(value: string) {
  if (value !== "en" && value !== "zh-CN") return;
  emit("language", value);
}
function changePalette(value: string | number | boolean) {
  emit("palette", String(value));
}
const cache = ref<MediaCacheStatus | null>(null);
const resetConfirm = ref(false), resetBusy = ref(false), resetError = ref('');
async function resetApp() {
  if (resetBusy.value || props.preview) return false;
  resetBusy.value = true; resetError.value = '';
  try {
    const core = window.__TAURI__?.core;
    if (!core) throw new Error(t('Reset is available in the desktop app.'));
    // Native restart waits for the old process to exit, then clears every store
    // before creating the new Host/WebView. No frontend path is accepted.
    await core.invoke('app_reset', {});
  } catch (error) {
    resetError.value = `${t('Reset could not start. Your data has not been cleared.')} ${String(error)}`;
    resetBusy.value = false;
  }
  return false;
}
const fontSaving = ref(false), fontError = ref('');
async function changeFont(event: Event) {
  const value = (event.target as HTMLSelectElement).value;
  fontSaving.value = true; fontError.value = '';
  try {
    if (settingsQueue.paused) await appController.restoreSettings();
    await appController.updateSettings(settings => ({ ...settings, font_family: value as 'noto-sans-sc' | 'noto-serif-sc' }));
    setAppFont(value);
  } catch (error) {
    fontError.value = error instanceof Error ? error.message : String(error);
  } finally { fontSaving.value = false; }
}
async function changeFontSize(event: Event) {
  const value = Number((event.target as HTMLSelectElement).value);
  if (![90, 100, 110, 120].includes(value)) return;
  fontSaving.value = true; fontError.value = '';
  try {
    if (settingsQueue.paused) await appController.restoreSettings();
    const settings = await appController.updateSettings(settings => ({ ...settings, font_scale: value as NonNullable<Settings['font_scale']> }));
    setFontScale(settings.font_scale);
  } catch (error) { fontError.value = String(error); }
  finally { fontSaving.value = false; }
}
const cacheBusy = ref(false), cacheError = ref(''), cacheNotice = ref('');
const cacheMaxMiB = ref<number | null>((appState.settings.media_cache_max_bytes ?? 268435456) / 1048576);
watch(() => appState.settings.media_cache_max_bytes, value => { cacheMaxMiB.value = (value ?? 268435456) / 1048576; });
const cacheSize = (bytes: number) => `${(bytes / 1048576).toLocaleString(locale.value, { maximumFractionDigits: 2 })} MiB`;
function editCacheLimit(event: Event) {
  const value = (event.target as HTMLInputElement).valueAsNumber;
  cacheMaxMiB.value = Number.isFinite(value) ? value : null;
}
function cacheFailure(error: unknown) { return error instanceof ApiFailure ? describeError(error.error) : String(error); }
async function readCache() {
  cache.value = await api.call('get_media_cache', {});
  cacheError.value = cache.value.last_error ? describeError(cache.value.last_error) : '';
}
async function refreshCache() {
  if (props.preview) return;
  cacheBusy.value = true; cacheError.value = '';
  try { await readCache(); } catch (error) { cacheError.value = cacheFailure(error); }
  finally { cacheBusy.value = false; }
}
async function saveCacheLimit() {
  cacheNotice.value = '';
  if (cacheMaxMiB.value === null || !Number.isFinite(cacheMaxMiB.value) || cacheMaxMiB.value < 0 || !Number.isSafeInteger(cacheMaxMiB.value * 1048576)) {
    cacheError.value = t('Enter a valid non-negative cache size.'); return;
  }
  const bytes = cacheMaxMiB.value * 1048576;
  cacheBusy.value = true; cacheError.value = ''; cacheNotice.value = '';
  try {
    if (settingsQueue.paused) await appController.restoreSettings();
    await appController.updateSettings(settings => ({ ...settings, media_cache_max_bytes: bytes }));
    await readCache(); cacheNotice.value = t('Cache limit saved.');
  } catch (error) { cacheError.value = cacheFailure(error); }
  finally { cacheBusy.value = false; }
}
async function clearCache() {
  cacheBusy.value = true; cacheError.value = ''; cacheNotice.value = '';
  try {
    cache.value = await api.call('clear_media_cache', {});
    cacheNotice.value = t(cache.value.pending_bytes ? 'Cache cleared. In-use data will be released when playback finishes reading it.' : 'Media cache cleared.');
  } catch (error) {
    const message = cacheFailure(error);
    try { await readCache(); } catch { /* Preserve the clear failure when the status query also fails. */ }
    cacheError.value = message;
  } finally { cacheBusy.value = false; }
}
onMounted(() => { void refreshCache(); });
</script>

<template>
  <div class="tv-settings">
  <div class="page-heading">
    <div>
      <p class="eyebrow">MAKE YOURSELF COMFORTABLE</p>
      <h1>{{ t("Settings") }}</h1>
      <p class="page-description">
        {{ t("A few small adjustments for a better viewing experience.") }}
      </p>
    </div>
  </div>
  <section v-if="!preview" class="settings-group service-settings">
    <h2>{{ word('服务连接', 'Service connection') }}</h2>
    <div class="settings-panel">
      <div class="setting-row"><label for="service-address">{{ word('服务地址', 'Service address') }}</label><input id="service-address" v-model="serviceDraft.url" class="setting-select" :disabled="serviceBusy" /></div>
      <div class="setting-row"><label for="service-token">{{ word('访问令牌', 'Access token') }}</label><input id="service-token" v-model="serviceDraft.token" type="password" autocomplete="off" class="setting-select" :disabled="serviceBusy" /></div>
      <div class="setting-row"><span>{{ appState.settings.playback_service?.enabled ? word('已连接', 'Connected') : word('未连接', 'Disconnected') }}</span><div class="cache-limit-controls"><NButton type="primary" :disabled="serviceBusy" @click="connectService">{{ word('连接', 'Connect') }}</NButton><NButton v-if="appState.settings.playback_service?.enabled" :disabled="serviceBusy" @click="disconnectService">{{ word('断开', 'Disconnect') }}</NButton></div></div>
      <NAlert v-if="serviceError" type="error" role="alert">{{ serviceError }}</NAlert>
    </div>
  </section>
  <section class="settings-group appearance-panel">
    <h2>{{ t("Appearance") }}</h2>
    <div class="settings-panel">
    <div class="setting-row system-mode-row">
      <div>
        <h3>{{ t("Display mode") }}</h3>
        <p>{{ t("Light and dark mode automatically follow your system.") }}</p>
      </div>
      <output class="system-mode" aria-live="polite"
        ><Icon :name="osTheme === 'dark' ? 'moon' : 'sun'" :size="17" />{{
          t("Following system · {mode}", {
            mode: t(osTheme === "dark" ? "Dark mode" : "Light mode"),
          })
        }}</output
      >
    </div>
    <div class="palette-picker">
      <h3 id="palette-label">{{ t("Color palette") }}</h3>
      <p id="palette-help">
        {{
          t("Choose your colors. Each palette adapts to light and dark mode.")
        }}
      </p>
      <div class="palette-swatches" role="radiogroup" aria-labelledby="palette-label" aria-describedby="palette-help">
        <button v-for="item in paletteOptions" :key="item.value" class="palette-swatch" role="radio" :aria-checked="palette === item.value" :aria-label="t(item.label)" :title="t(item.label)" :style="{ background:item.night }" @click="changePalette(item.value)"><Icon v-if="palette === item.value" name="check" :size="16" /></button>
      </div>
      <NRadioGroup class="palette-grid narrow-palette" :value="palette" name="color-palette" aria-labelledby="palette-label" aria-describedby="palette-help" @update:value="changePalette">
        <div v-for="item in paletteOptions" :key="item.value"><NRadioButton :value="item.value" :style="{ width:'100%', padding:'6px', '--preview-day':item.day, '--preview-night':item.night }"><span class="palette-preview" aria-hidden="true"><i class="palette-day"><b /><em /></i><i class="palette-night"><b /><em /></i></span><span class="palette-name"><strong>{{ t(item.label) }}</strong><Icon v-if="palette === item.value" name="check" :size="16" /></span></NRadioButton></div>
      </NRadioGroup>
    </div>
      </div>
  </section>
  <section class="settings-group font-settings">
    <h2>{{ t('Fonts') }}</h2>
    <div class="settings-panel">
      <div class="setting-row"><div><h3>{{ t('App font') }}</h3><p>{{ t('Choose a font included with the app.') }}</p></div>
        <select class="setting-select" :aria-label="t('App font')" :value="appState.settings.font_family ?? 'noto-sans-sc'" :disabled="fontSaving" @change="changeFont"><option v-for="font in fontOptions" :key="font.id" :value="font.id">{{ font.label }}</option></select>
      </div>
      <div class="setting-row"><div><h3>{{ t('Text size') }}</h3><p>{{ t('Scale text and controls together to keep layouts readable.') }}</p></div><select class="setting-select" :aria-label="t('Text size')" :value="appState.settings.font_scale ?? 100" :disabled="fontSaving" @change="changeFontSize"><option v-for="size in [90,100,110,120]" :key="size" :value="size">{{ size }}%</option></select></div>
      <NAlert v-if="fontError" type="error">{{ fontError }}</NAlert>
      <p class="font-sample">{{ t('A place for good stories.') }} · 字体预览 Aa 0123456789</p>
      <details><summary>{{ t('Font licenses (SIL OFL 1.1)') }}</summary><pre class="font-license">{{ sansLicense }}\n{{ serifLicense }}</pre></details>
    </div>
  </section>
  <section class="settings-group">
    <h2>{{ word("语言", "Language") }}</h2>
    <div class="settings-panel">
    <div class="setting-row">
      <div>
        <h3>{{ t("App language") }}</h3>
        <p>
          {{ t("Change the interface language and save it on this device.") }}
        </p>
      </div>
      <div class="settings-language library-tabs" role="radiogroup" :aria-label="t('App language')" data-focus-row><button v-for="item in languageOptions" :key="item.value" role="radio" :aria-checked="locale === item.value" @click="changeLanguage(item.value)">{{ item.label }}</button></div>
      <select class="setting-select compact-select narrow-language" :value="locale" :aria-label="t('App language')" @change="changeLanguage(($event.target as HTMLSelectElement).value)"><option v-for="item in languageOptions" :key="item.value" :value="item.value">{{ item.label }}</option></select>
    </div>
      </div>
  </section>
  <section class="settings-group">
    <h2>{{ t("Playback preferences") }}</h2>
    <div class="settings-panel">
    <div class="setting-row">
      <div>
        <h3>{{ word("自动查找下一集线路", "Find next episode automatically") }}</h3>
        <p>{{ word("播放结束后显示下一集可选线路。", "Show the next episode’s available lines when playback ends.") }}</p>
      </div>
      <NSwitch
        v-model:value="autoNext"
        :aria-label="word('自动查找下一集线路', 'Find next episode automatically')"
      />
    </div>
    <div class="setting-row">
      <div>
        <h3>{{ t("Default playback speed") }}</h3>
        <p>{{ t("Use this speed for all local demo clips.") }}</p>
      </div>
      <select
        class="setting-select compact-select"
        v-model="speed"
        :aria-label="t('Default playback speed')"
      >
        <option
          v-for="item in speedOptions"
          :key="item.value"
          :value="item.value"
        >
          {{ item.label }}
        </option>
      </select>
    </div>
      </div>
  </section>
  <details class="settings-group storage-settings"><summary>{{ word('存储与数据', 'Storage and data') }}</summary>
  <section class="settings-group media-cache-settings" :aria-busy="cacheBusy">
    <h2>{{ t('Media cache') }}</h2>
    <div class="settings-panel">
    <p>{{ t('Played media is cached on this device. Older cache is removed automatically when the limit is reached.') }}</p>
    <p v-if="preview">{{ t('Manage media cache in the desktop app.') }}</p>
    <div class="setting-row">
      <div><h3><label for="media-cache-max">{{ t('Maximum cache size') }}</label></h3><p>{{ t('Set to 0 to turn off caching.') }}</p></div>
      <div class="cache-limit-controls"><input id="media-cache-max" class="setting-select compact-select" type="number" min="0" step="1" :aria-label="t('Maximum cache size (MiB)')" :value="cacheMaxMiB ?? ''" :disabled="preview || cacheBusy" @input="editCacheLimit" /><span>MiB</span><NButton data-action="cache-save" :disabled="preview || cacheBusy" @click="saveCacheLimit">{{ t('Save') }}</NButton></div>
    </div>
    <div class="setting-row"><div><h3>{{ t('Cache used') }}</h3><output v-if="cache" class="media-cache-used" :data-bytes="cache.used_bytes">{{ cacheSize(cache.used_bytes) }} / {{ cacheSize(cache.max_bytes) }}</output><p v-else>{{ t('Cache usage unavailable') }}</p><p v-if="cache?.pending_bytes">{{ t('Waiting to release') }}: {{ cacheSize(cache.pending_bytes) }}</p></div><div class="cache-limit-controls"><NButton data-action="cache-refresh" :disabled="preview || cacheBusy" @click="refreshCache">{{ t('Refresh') }}</NButton><NButton data-action="cache-clear" :disabled="preview || cacheBusy" @click="clearCache">{{ t('Clear media cache') }}</NButton></div></div>
    <NAlert v-if="cacheError" type="error" class="media-cache-error">{{ cacheError }}</NAlert>
    <p v-if="cacheNotice" role="status" class="media-cache-notice">{{ cacheNotice }}</p>
      </div>
  </section>
  <section class="settings-group reset-settings" :aria-busy="resetBusy">
    <h2>{{ t('Reset app') }}</h2>
    <div class="settings-panel">
      <div class="setting-row"><div><h3>{{ t('Clear all data and reset') }}</h3><p>{{ t('Delete settings, watch history, watchlist and all caches. The app will restart with defaults.') }}</p></div><NButton data-action="app-reset" type="error" secondary :disabled="preview || resetBusy || cacheBusy || fontSaving" @click="resetError = ''; resetConfirm = true">{{ t('Clear all data and reset') }}</NButton></div>
      <p v-if="preview">{{ t('Reset is available in the desktop app.') }}</p>
      <p v-if="resetBusy" role="status">{{ t('Resetting app and restarting…') }}</p>
    </div>
  </section>
  <NModal v-model:show="resetConfirm" preset="dialog" type="warning" :title="t('Reset app?')" :positive-text="t('Clear all data and reset')" :negative-text="t('Cancel')" :loading="resetBusy" :closable="!resetBusy" :mask-closable="!resetBusy" :close-on-esc="!resetBusy" :negative-button-props="{ disabled: resetBusy }" :positive-button-props="{ type: 'error' }" @positive-click="resetApp">
    <p>{{ t('This permanently deletes all app settings, watch history, watchlist and cached data. Defaults will be restored.') }}</p>
    <NAlert v-if="resetError" type="error" role="alert">{{ resetError }}</NAlert>
  </NModal>
  </details>
  </div>
</template>

<style scoped>
.font-license { white-space: pre-wrap; overflow-wrap: anywhere; max-height: 280px; overflow: auto; font: inherit; }
.font-sample { font-size: 1.2em; padding: 16px 0; }
.cache-limit-controls { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; max-width: 100%; }
.cache-limit-controls input { width: 130px; max-width: 100%; min-height: 44px; }
.media-cache-settings > p { color: var(--muted); margin-bottom: 12px; }
.media-cache-error, .media-cache-notice { margin-top: 12px; overflow-wrap: anywhere; }
.reset-settings .setting-row > div { min-width: 0; }
.reset-settings .setting-row p { max-width: 650px; overflow-wrap: anywhere; }
@media (max-width: 600px) { .reset-settings .setting-row { align-items: stretch; flex-direction: column; gap: 16px; } }
</style>
