<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { NAlert, NButton, NEmpty, NModal, NProgress, NSkeleton, NTag } from 'naive-ui';
import Icon from '../components/Icon.vue';
import WatchlistRow from '../components/WatchlistRow.vue';
import { films, localizeFilm, initialSources, sourceText } from '../data/demo';
import { locale, t } from '../i18n';
import { historyRepository, historyPoster } from '../data/history';
import { appState } from '../state';
import { api } from '../api';
import type { Bridge_MediaRef } from '../models';
import { historyContentId, historyProgress, historyTime, type HistoryEntry } from '../data/history-model';

const native = Boolean(window.__TAURI__);
const props = defineProps<{ saveError: string | null }>();
const emit = defineEmits<{ open:[ref:Bridge_MediaRef]; resume: [entry: HistoryEntry]; retrySave: []; removed: [id: string | null] }>();
const desktop = matchMedia('(min-width:960px)');
const tab = ref<'continue' | 'watchlist' | 'history'>(desktop.matches ? 'continue' : 'history');
function layoutChanged() { tab.value = desktop.matches ? 'continue' : 'history'; }
const tabs = computed(() => [{ key:'continue' as const, label:t('Continue watching') }, { key:'watchlist' as const, label:locale.value === 'zh-CN' ? '待播清单' : 'Watchlist' }, { key:'history' as const, label:t('Watch history') }]);
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
const visibleGroups = computed(() => tab.value === 'continue' ? [{ label:t('Continue watching'), items:entries.value.filter(entry => !entry.completed && entry.position_ms > 0) }] : groups.value);
const visibleCount = computed(() => visibleGroups.value.reduce((sum, group) => sum + group.items.length, 0));
async function retrySave() { emit('retrySave'); await load(); }
function displaySource(entry: HistoryEntry) { const source = initialSources.find(item=>item.id===entry.source_id); return source ? sourceText(source).name : entry.source_name; }
function formatDate(value: number) {
  return new Intl.DateTimeFormat(locale.value === 'zh-CN' ? 'zh-CN' : 'en', { month:'short', day:'numeric', hour:'2-digit', minute:'2-digit' }).format(value);
}
async function loadPosters(rows: HistoryEntry[], sequence: number) {
  for (const row of rows) {
    if (sequence !== loadSequence) return;
    if (!row.record) continue;
    try {
      const poster = await historyPoster(row.record, appState.sources.data?.providers ?? [], (command, input, ...rest) => api.call(command, input, ...rest));
      if (sequence !== loadSequence) return;
      if (poster) row.poster = poster;
    } catch (failure) {
      if (sequence === loadSequence) error.value = failure instanceof Error ? failure.message : String(failure);
    }
  }
}
async function load() {
  const sequence = ++loadSequence;
  loading.value=true; error.value='';
  try { const rows = await historyRepository.list(); if (sequence===loadSequence) { entries.value=rows; clock.value=Date.now(); void loadPosters(entries.value, sequence); } }
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
onMounted(() => { void load(); desktop.addEventListener('change',layoutChanged); document.addEventListener('visibilitychange',onVisible); });
onBeforeUnmount(() => { ++loadSequence; desktop.removeEventListener('change',layoutChanged); document.removeEventListener('visibilitychange',onVisible); });
</script>

<template>
  <div class="page-heading library-heading"><div><h1>{{ locale === 'zh-CN' ? '资料库' : 'Library' }}</h1></div><button v-if="entries.length && tab === 'history'" class="history-clear" :disabled="busy || loading" @click="deletion = 'all'">{{ t('Clear history') }}</button></div>
  <div class="library-tabs" role="tablist" :aria-label="locale === 'zh-CN' ? '资料库' : 'Library'" data-focus-row><button v-for="item in tabs" :id="`library-tab-${item.key}`" :key="item.key" role="tab" :aria-selected="tab === item.key" :aria-controls="`library-panel-${item.key}`" @click="tab = item.key">{{ item.label }}</button></div>
  <div :id="`library-panel-${tab}`" class="library-panel" :class="{ 'library-continue':tab === 'continue', 'library-history':tab === 'history' }" role="tabpanel" :aria-labelledby="`library-tab-${tab}`">
  <WatchlistRow v-if="tab === 'watchlist'" wall @open="emit('open',$event)" />
  <template v-else>
  <p v-if="!native" class="history-preview-note">{{ t('Preview history resets when you reload this page.') }}</p>
  <NAlert v-if="saveError" type="error" class="history-alert" :title="t('Watch history could not be saved')">{{ saveError }}<NButton size="small" @click="retrySave">{{ t('Retry') }}</NButton></NAlert>
  <NAlert v-if="error" type="error" class="history-alert" :title="t('History is unavailable')">{{ error }}<NButton size="small" @click="load">{{ t('Retry') }}</NButton></NAlert>
  <div v-if="loading" class="history-loading" :aria-label="t('Loading watch history')" aria-busy="true"><NSkeleton v-for="n in 3" :key="n" height="144px" :sharp="false" /></div>
  <NEmpty v-else-if="!visibleCount && !error" class="history-empty" :description="t('Your next story starts here.')"><template #icon><Icon name="history" :size="48" /></template><p>{{ t('Watch a video and your progress will appear here automatically.') }}</p><template #extra><NButton tag="a" href="#/home" type="primary">{{ t('Start exploring') }}</NButton></template></NEmpty>
  <template v-else>
    <p v-if="entries.length" class="history-count">{{ entries.length === 1 ? t('1 watched title') : t('{count} watched titles', { count: entries.length }) }}</p>
    <section v-for="group in visibleGroups" :key="group.label" class="history-group"><h2>{{ group.label }}</h2><div class="history-wall" data-focus-row>
      <article v-for="entry in group.items" :key="entry.content_id" class="history-card">
        <img class="history-poster" :src="entry.poster" alt="" loading="lazy" @error="entry.poster = '/mark.svg'" />
        <div class="history-copy"><div class="history-title"><h3>{{ displayTitle(entry) }}</h3><NTag v-if="entry.completed" size="small" :bordered="false">{{ t(entry.kind !== 'movie' ? 'Episode finished' : 'Completed') }}</NTag></div><p>{{ entry.kind !== 'movie' ? t('Episode {number}', { number: entry.episode }) : t('Feature') }}<span>·</span>{{ displaySource(entry) }}</p><NProgress type="line" :percentage="entry.completed ? 100 : historyProgress(entry)" :height="4" :show-indicator="false" /><div class="history-progress"><span>{{ historyTime(entry.position_ms) }} / {{ historyTime(entry.duration_ms) }}</span><time :datetime="new Date(entry.updated_at).toISOString()">{{ formatDate(entry.updated_at) }}</time></div></div>
        <div class="history-actions"><NButton type="primary" :disabled="busy" @click="emit('resume',entry)"><template #icon><Icon :name="entry.completed ? 'repeat' : 'play'" :size="16" /></template>{{ t(entry.completed ? 'Play again' : 'Continue watching') }}</NButton><NButton class="history-delete" quaternary :aria-label="t('Remove {title} from history', { title: displayTitle(entry) })" :disabled="busy" @click="deletion = entry"><template #icon><Icon name="trash" :size="18" /></template></NButton></div>
      </article>
    </div></section>
  </template>
  </template>
  </div>
  <NModal :show="deletion !== null" preset="dialog" type="warning" class="tv-dialog history-confirmation" :aria-label="t(deletion === 'all' ? 'Clear history' : 'Remove from history')" :title="t(deletion === 'all' ? 'Clear history' : 'Remove from history')" :content="deletion === 'all' ? t('Clear all watch history? This removes saved progress from this device.') : deletion ? t('Remove {title} and its saved progress from this device?', { title: displayTitle(deletion) }) : ''" :positive-text="t(deletion === 'all' ? 'Clear history' : 'Remove')" :negative-text="t('Cancel')" :loading="busy" :mask-closable="false" @positive-click="confirmDeletion" @negative-click="deletion = null" @update:show="value => { if (!value) deletion = null; }" />
</template>
