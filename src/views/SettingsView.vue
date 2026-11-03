<script setup lang="ts">
import { computed } from "vue";
import { NRadioGroup, NRadioButton, NSwitch, NTag, useOsTheme } from "naive-ui";
import Icon from "../components/Icon.vue";
import { locale, getStremioLanguage, t } from "../i18n";
import {
  palette,
  paletteOptions,
} from "../ui/theme";

const speed = defineModel<number>("speed", { required: true });
defineProps<{ preview?: boolean }>();
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
</script>

<template>
  <div class="page-heading">
    <div>
      <p class="eyebrow">MAKE YOURSELF COMFORTABLE</p>
      <h1>{{ t("Make yourself comfortable.") }}</h1>
      <p class="page-description">
        {{ t("A few small adjustments for a better viewing experience.") }}
      </p>
    </div>
  </div>
  <section class="settings-panel appearance-panel">
    <h2>{{ t("Appearance") }}</h2>
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
      <NRadioGroup
        :value="palette"
        name="color-palette"
        class="palette-grid"
        aria-labelledby="palette-label"
        aria-describedby="palette-help"
        @update:value="changePalette"
      >
        <div v-for="item in paletteOptions" :key="item.value">
          <NRadioButton
            :value="item.value"
            :style="{
              width: '100%',
              padding: '6px',
              '--preview-day': item.day,
              '--preview-night': item.night,
            }"
          >
            <span class="palette-preview" aria-hidden="true"
              ><i class="palette-day"><b /><em /></i
              ><i class="palette-night"><b /><em /></i
            ></span>
            <span class="palette-name"
              ><strong>{{ t(item.label) }}</strong
              ><Icon v-if="palette === item.value" name="check" :size="16"
            /></span>
          </NRadioButton>
        </div>
      </NRadioGroup>
    </div>
  </section>
  <section class="settings-panel">
    <h2>{{ t("Language & metadata") }}</h2>
    <div class="setting-row">
      <div>
        <h3>{{ t("App language") }}</h3>
        <p>
          {{ t("Change the interface language and save it on this device.") }}
        </p>
      </div>
      <select
        class="setting-select compact-select"
        :value="locale"
        :aria-label="t('App language')"
        @change="changeLanguage(($event.target as HTMLSelectElement).value)"
      >
        <option
          v-for="item in languageOptions"
          :key="item.value"
          :value="item.value"
        >
          {{ item.label }}
        </option>
      </select>
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
  </section>
  <section class="settings-panel">
    <h2>{{ t("Playback preferences") }}</h2>
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
  </section>
  <section class="settings-panel">
    <h2>{{ t("About Yingxiang Video") }}</h2>
    <div class="about-brand">
      <img src="/mark.svg" width="48" height="48" alt="" />
      <div>
        <h3>{{ t("Yingxiang Video") }}</h3>
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
  </section>
</template>
