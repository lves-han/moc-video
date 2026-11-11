<script setup lang="ts">
import { ref, watchEffect } from 'vue';
import { locale, setLocale, t } from '../i18n';
import { resetRecoveryInfo } from '../resetRecovery';
const info = resetRecoveryInfo();
const busy = ref(false), error = ref('');
watchEffect(() => { document.title = t('Reset recovery'); document.documentElement.lang = locale.value; });
async function recover(action: 'retry' | 'cancel') {
  busy.value = true; error.value = '';
  try {
    const core = window.__TAURI__?.core;
    if (!core) throw new Error(t('Reset is available in the desktop app.'));
    await core.invoke('app_reset_recovery', { action });
  } catch (cause) { error.value = String(cause); busy.value = false; }
}
async function quit() {
  try { await window.__TAURI__?.core.invoke('app_close_ready', {}); }
  catch (cause) { error.value = String(cause); }
}
</script>
<template>
  <main class="reset-recovery" :aria-busy="busy">
    <button class="glass-pill" :disabled="busy" @click="setLocale(locale === 'en' ? 'zh-CN' : 'en')">{{ locale === 'en' ? '简体中文' : 'English' }}</button>
    <h1>{{ t('Reset could not finish') }}</h1>
    <p>{{ t('Cleanup is paused while this recovery window is open. Reopening the app will retry the pending reset.') }}</p>
    <p>{{ t('Some files may already have been cleared. Close other apps using these files and retry, or cancel the remaining reset and reopen with the data that remains.') }}</p>
    <pre class="recovery-details">{{ info?.message }}</pre>
    <p v-if="error" role="alert">{{ error }}</p>
    <p v-if="busy" role="status">{{ t('Restarting…') }}</p>
    <div class="recovery-actions">
      <button class="glass-pill" :disabled="busy" @click="recover('retry')">{{ t('Retry reset') }}</button>
      <button class="glass-pill" :disabled="busy" @click="recover('cancel')">{{ t('Cancel reset and reopen') }}</button>
      <button class="glass-pill" :disabled="busy" @click="quit">{{ t('Quit app') }}</button>
    </div>
  </main>
</template>
<style scoped>
.reset-recovery { max-width: 640px; margin: 8vh auto; padding: 24px; line-height: 1.8; }
.recovery-details { white-space: pre-wrap; overflow-wrap: anywhere; max-height: 160px; overflow: auto; color: var(--muted); }
.recovery-actions { display: flex; flex-wrap: wrap; gap: 12px; margin-top: 24px; }
.recovery-actions button { min-height: 44px; }
</style>
