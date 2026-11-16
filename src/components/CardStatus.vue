<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { observeAvailability } from '../card-availability';
import { appState } from '../state';
import { locale } from '../i18n';
import type { Bridge_MediaCard } from '../models';

const props = defineProps<{ card: Bridge_MediaCard }>();
const root = ref<HTMLElement>();
const status = ref('unknown');
const label = computed(() => props.card.status === 'upcoming' || status.value === 'upcoming'
  ? (locale.value === 'zh-CN' ? '未上映' : 'Coming soon')
  : status.value === 'unavailable' ? (locale.value === 'zh-CN' ? '无可播放源' : 'No playable source') : '');
let observer: IntersectionObserver | undefined, unsubscribe: (() => void) | undefined;
let visible = false, timer: ReturnType<typeof setInterval> | undefined;
function refresh() {
  unsubscribe?.(); unsubscribe = undefined; status.value = 'unknown';
  if (visible) unsubscribe = observeAvailability(props.card, value => { status.value = value; });
}
onMounted(() => {
  observer = new IntersectionObserver(entries => {
    visible = entries.some(entry => entry.isIntersecting);
    refresh();
  }, { threshold: .1 });
  if (root.value) observer.observe(root.value);
  timer = setInterval(refresh, 5 * 60 * 1000);
});
watch(() => [JSON.stringify(props.card.ref), props.card.status, appState.runtime.engine_epoch,
  appState.settings.playback_service?.url, appState.settings.playback_service?.token,
  appState.settings.playback_service?.enabled, JSON.stringify(appState.settings.playback_service?.groups ?? [])], refresh);
onBeforeUnmount(() => { observer?.disconnect(); unsubscribe?.(); clearInterval(timer); });
</script>
<template><span ref="root" class="card-status-observer" aria-live="polite"><span v-if="label" class="card-status-watermark">{{ label }}</span></span></template>
<style scoped>
.card-status-observer { position: absolute; inset: 0; z-index: 2; pointer-events: none; }
.card-status-watermark { position: absolute; inset-inline: 0; top: 42%; padding: 9px 4px; background: rgba(25, 25, 30, .82); color: #fff; font-size: clamp(15px, 1.3vw, 20px); font-weight: 700; line-height: 1.3; text-align: center; letter-spacing: .08em; }
</style>
