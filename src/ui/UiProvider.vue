<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watchEffect } from "vue";
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
const desktopQuery = matchMedia("(min-width: 960px)");
const desktop = ref(desktopQuery.matches);
const resize = () => { desktop.value = desktopQuery.matches; };
desktopQuery.addEventListener("change", resize);
onBeforeUnmount(() => desktopQuery.removeEventListener("change", resize));
const config = computed(() =>
  resolveTheme(palette.value, osTheme.value === "dark", desktop.value),
);
let previousKeys: string[] = [];
watchEffect(() => {
  for (const key of previousKeys) document.documentElement.style.removeProperty(key);
  for (const [key, value] of Object.entries(config.value.variables)) {
    document.documentElement.style.setProperty(key, value);
  }
  previousKeys = Object.keys(config.value.variables);
  document.documentElement.style.colorScheme =
    desktop.value || osTheme.value === "dark" ? "dark" : "light";
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
