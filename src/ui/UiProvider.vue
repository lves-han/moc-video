<script setup lang="ts">
import { computed, watchEffect } from "vue";
import {
  NConfigProvider,
  NGlobalStyle,
  NMessageProvider,
  enUS,
  zhCN,
  useOsTheme,
} from "naive-ui";
import { locale } from "../i18n";
import { palette, resolveTheme } from "./theme";

const osTheme = useOsTheme();
const config = computed(() =>
  resolveTheme(palette.value, osTheme.value === "dark"),
);
watchEffect(() => {
  for (const [key, value] of Object.entries(config.value.variables)) {
    document.documentElement.style.setProperty(key, value);
  }
  document.documentElement.style.colorScheme =
    osTheme.value === "dark" ? "dark" : "light";
});
</script>

<template>
  <NConfigProvider
    :theme="config.theme"
    :theme-overrides="config.overrides"
    :locale="locale === 'zh-CN' ? zhCN : enUS"
    :component-options="{
      Button: { size: 'medium' },
    }"
  >
    <NGlobalStyle />
    <NMessageProvider
      placement="bottom"
      :container-style="{ bottom: 'calc(86px + env(safe-area-inset-bottom))' }"
      ><slot
    /></NMessageProvider>
  </NConfigProvider>
</template>
