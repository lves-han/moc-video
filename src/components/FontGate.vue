<script setup lang="ts">
import { onMounted, ref } from 'vue';
import { appState } from '../state';
import { fontStatus, prepareFont, setFontScale } from '../ui/fonts';
import { t } from '../i18n';
const ready = ref(false);
async function start() {
  try {
    await prepareFont(appState.settings.font_family);
    setFontScale(appState.settings.font_scale);
    ready.value = true;
  } catch { /* Keep the bootstrap screen available for a visible retry. */ }
}
onMounted(start);
</script>
<template>
  <slot v-if="ready" />
  <main v-else class="font-startup" aria-live="polite">
    <h1>{{ t('Preparing app fonts') }}</h1>
    <p>{{ t('Fonts download once, then load from this device.') }}</p>
    <p v-if="fontStatus.busy">{{ t('Downloading font') }} · {{ Math.round(fontStatus.received / Math.max(1, fontStatus.total) * 100) }}%</p>
    <p v-if="fontStatus.error" role="alert">{{ t('Font could not be loaded. Check your connection and local storage, then retry.') }}<br>{{ fontStatus.error }}</p>
    <button v-if="!fontStatus.busy" @click="start">{{ t('Retry') }}</button>
  </main>
</template>
<style scoped>
.font-startup { max-width: 620px; margin: 14vh auto; padding: 32px; line-height: 1.7; }
</style>
