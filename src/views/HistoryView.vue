<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { NAlert, NButton, NEmpty, NModal, NProgress, NSkeleton, NTag } from 'naive-ui';
import Icon from '../components/Icon.vue';
import { films, localizeFilm, initialSources, sourceText } from '../data/demo';
import { locale, t } from '../i18n';
import { historyRepository } from '../data/history';
import { historyContentId, historyProgress, historyTime, type HistoryEntry } from '../data/history-model';

const native = Boolean(window.__TAURI__);
const props = defineProps<{ saveError: string | null }>();
const emit = defineEmits<{ resume: [entry: HistoryEntry]; retrySave: []; removed: [id: string | null] }>();
const entries = ref<HistoryEntry[]>([]);
const loading = ref(true);
const error = ref('');
const busy = ref(false);
const deletion = ref<HistoryEntry | 'all' | null>(null);
const clock = ref(Date.now());
let loadSequence = 0;
function displayTitle(entry: HistoryEntry) {
  const film = films.find(item => historyContentId(item.kind,item.id) === entry.content_id);
  return film ? localizeFilm(film).title : entry.title;
}
const groups = computed(() => {
  const today = new Date(clock.value); today.setHours(0,0,0,0);
  const yesterday = new Date(today); yesterday.setDate(yesterday.getDate()-1);
  return [
    { label: t('Today'), items: entries.value.filter(item => item.updated_at >= today.getTime()) },
    { label: t('Yesterday'), items: entries.value.filter(item => item.updated_at < today.getTime() && item.updated_at >= yesterday.getTime()) },
    { label: t('Earlier'), items: entries.value.filter(item => item.updated_at < yesterday.getTime()) },
  ].filter(group=>group.items.length);
});
async function retrySave() { emit('retrySave'); await load(); }
function displaySource(entry: HistoryEntry) { const source = initialSources.find(item=>item.id===entry.source_id); return source ? sourceText(source).name : entry.source_name; }
function formatDate(value: number) {
  return new Intl.DateTimeFormat(locale.value === 'zh-CN' ? 'zh-CN' : 'en', { month:'short', day:'numeric', hour:'2-digit', minute:'2-digit' }).format(value);
}
async function load() {
  const sequence = ++loadSequence;
  loading.value=true; error.value='';
  try { const rows = await historyRepository.list(); if (sequence===loadSequence) { entries.value=rows; clock.value=Date.now(); } }
  catch (failure) { if (sequence===loadSequence) error.value = failure instanceof Error ? failure.message : String(failure); }
  finally { if (sequence===loadSequence) loading.value=false; }
}
async function remove(entry: HistoryEntry) {
  ++loadSequence; loading.value=false;
  busy.value=true; error.value='';
  try { await historyRepository.remove(entry.content_id); entries.value=entries.value.filter(item=>item.content_id!==entry.content_id); emit('removed',entry.content_id); }
  catch (failure) { error.value = failure instanceof Error ? failure.message : String(failure); }
  finally { busy.value=false; }
}
async function clear() {
  ++loadSequence; loading.value=false;
  busy.value=true; error.value='';
  try { await historyRepository.clear(); entries.value=[]; emit('removed',null); }
  catch (failure) { error.value = failure instanceof Error ? failure.message : String(failure); }
  finally { busy.value=false; }
}
async function confirmDeletion() {
  const target = deletion.value;
  if (!target || busy.value) return;
  if (target === 'all') await clear(); else await remove(target);
  deletion.value = null;
}
function onVisible() { if (document.visibilityState === 'visible' && !busy.value) void load(); }
onMounted(() => { void load(); document.addEventListener('visibilitychange',onVisible); });
onBeforeUnmount(() => { ++loadSequence; document.removeEventListener('visibilitychange',onVisible); });
</script>

<template>
  <div class="page-heading"><div><p class="eyebrow">PICK UP WHERE YOU LEFT OFF</p><h1>{{ t('Watch history') }}</h1><p class="page-description">{{ t('Your stories, ready to continue.') }}</p></div><NButton v-if="entries.length" :disabled="busy || loading" @click="deletion = 'all'">{{ t('Clear history') }}</NButton></div>
  <p v-if="!native" class="history-preview-note">{{ t('Preview history resets when you reload this page.') }}</p>
  <NAlert v-if="saveError" type="error" class="history-alert" :title="t('Watch history could not be saved')">{{ saveError }}<NButton size="small" @click="retrySave">{{ t('Retry') }}</NButton></NAlert>
  <NAlert v-if="error" type="error" class="history-alert" :title="t('History is unavailable')">{{ error }}<NButton size="small" @click="load">{{ t('Retry') }}</NButton></NAlert>
  <div v-if="loading" class="history-loading" :aria-label="t('Loading watch history')" aria-busy="true"><NSkeleton v-for="n in 3" :key="n" height="144px" :sharp="false" /></div>
  <NEmpty v-else-if="!entries.length && !error" class="history-empty" :description="t('Your next story starts here.')"><template #icon><Icon name="history" :size="48" /></template><p>{{ t('Watch a video and your progress will appear here automatically.') }}</p><template #extra><NButton tag="a" href="#/home" type="primary">{{ t('Start exploring') }}</NButton></template></NEmpty>
  <template v-else>
    <p v-if="entries.length" class="history-count">{{ entries.length === 1 ? t('1 watched title') : t('{count} watched titles', { count: entries.length }) }}</p>
    <section v-for="group in groups" :key="group.label" class="history-group"><h2>{{ group.label }}</h2>
      <article v-for="entry in group.items" :key="entry.content_id" class="history-card">
        <img class="history-poster" :src="entry.poster" alt="" loading="lazy" />
        <div class="history-copy"><div class="history-title"><h3>{{ displayTitle(entry) }}</h3><NTag v-if="entry.completed" size="small" :bordered="false">{{ t(entry.episode_total > 1 ? 'Episode finished' : 'Completed') }}</NTag></div><p>{{ entry.episode_total > 1 ? t('Episode {number}', { number: entry.episode }) : t('Feature') }}<span>·</span>{{ displaySource(entry) }}</p><NProgress type="line" :percentage="entry.completed ? 100 : historyProgress(entry)" :height="4" :show-indicator="false" /><div class="history-progress"><span>{{ historyTime(entry.position_ms) }} / {{ historyTime(entry.duration_ms) }}</span><time :datetime="new Date(entry.updated_at).toISOString()">{{ formatDate(entry.updated_at) }}</time></div></div>
        <div class="history-actions"><NButton type="primary" :disabled="busy" @click="emit('resume',entry)"><template #icon><Icon :name="entry.completed ? 'repeat' : 'play'" :size="16" /></template>{{ t(entry.completed ? 'Play again' : 'Continue watching') }}</NButton><NButton quaternary :aria-label="t('Remove {title} from history', { title: displayTitle(entry) })" :disabled="busy" @click="deletion = entry"><template #icon><Icon name="trash" :size="18" /></template></NButton></div>
      </article>
    </section>
  </template>
  <NModal :show="deletion !== null" preset="dialog" type="warning" class="history-confirmation" :aria-label="t(deletion === 'all' ? 'Clear history' : 'Remove from history')" :title="t(deletion === 'all' ? 'Clear history' : 'Remove from history')" :content="deletion === 'all' ? t('Clear all watch history? This removes saved progress from this device.') : deletion ? t('Remove {title} and its saved progress from this device?', { title: displayTitle(deletion) }) : ''" :positive-text="t(deletion === 'all' ? 'Clear history' : 'Remove')" :negative-text="t('Cancel')" :loading="busy" :mask-closable="false" @positive-click="confirmDeletion" @negative-click="deletion = null" @update:show="value => { if (!value) deletion = null; }" />
</template>
