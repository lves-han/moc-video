<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { NRadioGroup, NRadioButton, NSwitch, NTag, NButton, NAlert, useOsTheme } from "naive-ui";
import { api, ApiFailure, describeError, settingsQueue } from "../api";
import { appController, appState } from "../state";
import type { MediaCacheStatus } from "../models";
import Icon from "../components/Icon.vue";
import { locale, getStremioLanguage, t } from "../i18n";
import {
  palette,
  paletteOptions,
} from "../ui/theme";

const speed = defineModel<number>("speed", { required: true });
const props = defineProps<{ preview?: boolean }>();
const autoNext = defineModel<boolean>("autoNext", { required: true });
const emit = defineEmits<{ notice: [message: string]; language: [value: string]; palette: [value: string] }>();
const osTheme = useOsTheme();
const metadataLanguage = computed(() => getStremioLanguage(locale.value));
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
  <section class="settings-group">
    <h2>{{ t("Language & metadata") }}</h2>
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
    <div class="setting-row">
      <div>
        <h3>{{ t("Metadata provider") }}</h3>
        <p>
          {{ t(preview ? "Metadata language preview · Live Stremio connection pending" : "Follows the app: Chinese uses Chinese; all other languages use English.") }}
        </p>
      </div>
      <NTag :bordered="false">Stremio</NTag>
    </div>
    <div class="setting-row">
      <div>
        <h3>{{ t("Metadata language") }}</h3>
        <p>
          {{
            t(
              "Follows the app: Chinese uses Chinese; all other languages use English.",
            )
          }}
        </p>
      </div>
      <output class="metadata-language" :lang="metadataLanguage">{{
        t(metadataLanguage === "zh-CN" ? "Simplified Chinese" : "English")
      }}</output>
    </div>
      </div>
  </section>
  <section class="settings-group">
    <h2>{{ t("Playback preferences") }}</h2>
    <div class="settings-panel">
    <div class="setting-row">
      <div>
        <h3>{{ t("Automatically play next episode") }}</h3>
        <p>{{ t("Continue to the next episode when this one ends.") }}</p>
      </div>
      <NSwitch
        v-model:value="autoNext"
        :aria-label="t('Automatically play next episode')"
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
  <section class="settings-group">
    <h2>{{ t("About Moc Video") }}</h2>
    <div class="settings-panel">
    <div class="about-brand">
      <img src="/mark.svg" width="48" height="48" alt="" />
      <div>
        <h3>{{ t("Moc Video") }}</h3>
        <p>{{ t("A place for good stories.") }}</p>
      </div>
      <NTag :bordered="false">{{ preview ? t("Interface preview 0.1.0") : '0.1.0' }}</NTag>
    </div>
    <p class="about-copy">
      {{
        t(
          preview ? "Explore discovery, film details, episodes, embedded playback and source management. Artwork is original, playback uses a local 30-second demo, and playback preferences last for this page session." : "A place for good stories.",
        )
      }}
    </p>
      </div>
  </section>
  </div>
</template>

<style scoped>
.cache-limit-controls { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; max-width: 100%; }
.cache-limit-controls input { width: 130px; max-width: 100%; min-height: 44px; }
.media-cache-settings > p { color: var(--muted); margin-bottom: 12px; }
.media-cache-error, .media-cache-notice { margin-top: 12px; overflow-wrap: anywhere; }
</style>
