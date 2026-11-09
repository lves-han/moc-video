<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import CatalogHero from './CatalogHero.vue';
import CatalogRow from './CatalogRow.vue';
import FilterBar from './FilterBar.vue';
import MediaRow from './MediaRow.vue';
import Icon from './Icon.vue';
import WatchlistRow from './WatchlistRow.vue';
import { appController as app, appState as state, type HomeFilter } from '../state';
import { historyRepository, historyPoster } from '../data/history';
import type { HistoryEntry } from '../data/history-model';
import { api } from '../api';
import { locale, t } from '../i18n';
import type { Bridge_HomeSection, Bridge_MediaRef } from '../models';
const props = withDefaults(defineProps<{ category?: string; query?: string }>(), { category: '', query: '' });
const emit = defineEmits<{ open: [ref: Bridge_MediaRef]; resume: [entry: HistoryEntry] }>();
const zh = computed(() => locale.value === 'zh-CN');
const word = (cn: string, en: string) => zh.value ? cn : en;
const categoryTitle = computed(() => ({ movie: word('电影', 'Movies'), series: word('电视剧', 'Series'), anime: word('动画', 'Animation'), documentary: word('纪录片', 'Documentaries') }[props.category] ?? ''));
const sections = computed(() => state.home.phase === 'loading' ? [] : state.home.data?.page.sections ?? []);
const base = computed(() => sections.value.find(section => section.kind === (props.category === 'series' ? 'series' : 'movie')) ?? sections.value[0]);
const lockedGenre = computed(() => props.category === 'anime' ? base.value?.filters?.genre.options.find(item => /^(Animation|动画)$/i.test(item.value))?.value : props.category === 'documentary' ? base.value?.filters?.genre.options.find(item => /^(Documentary|纪录)$/i.test(item.value))?.value : undefined);
const selected = computed<HomeFilter>(() => {
  const params = new URLSearchParams(props.query), value: HomeFilter = {};
  for (const key of ['genre', 'year', 'language', 'region', 'sort'] as const) { const text = params.get(key); if (text) Object.assign(value, { [key]: text }); }
  return value;
});
const wallRef = computed(() => { const ref = new URLSearchParams(props.query).get('section'); return sections.value.find(section => section.section_ref === ref)?.section_ref ?? base.value?.section_ref; });
const wall = computed(() => !!props.category && (!!new URLSearchParams(props.query).get('section') || Object.keys(selected.value).length > 0));
const wallFilter = computed(() => ({ ...selected.value, ...(lockedGenre.value ? { genre: lockedGenre.value } : {}) }));
const wallRow = computed(() => wallRef.value ? app.catalogRow(wallRef.value, wallFilter.value) : null);
const wallSection = computed(() => wallRow.value?.data?.page.sections[0]);
function href(category: string, section: string, filter: HomeFilter) { const params = new URLSearchParams({ section }); for (const [key, value] of Object.entries(filter)) params.set(key, value); return `#/browse/${category}?${params}`; }
function filters(value: HomeFilter) { const params = new URLSearchParams(); for (const [key, text] of Object.entries(value)) params.set(key, text); location.hash = `#/browse/${props.category}${params.size ? '?' + params : ''}`; }
type Row = { title: string; section: Bridge_HomeSection; filter: HomeFilter; category: string };
const rows = computed<Row[]>(() => {
  const result: Row[] = [];
  function add(section: Bridge_HomeSection | undefined, title: string, filter: HomeFilter, category = props.category || 'movie') {
    if (section && Object.entries(filter).every(([key, value]) => section.filters?.[key as keyof HomeFilter].options.some(item => item.value === value))) result.push({ section, title, filter, category });
  }
  function genre(section: Bridge_HomeSection | undefined, pattern: RegExp) { return section?.filters?.genre.options.find(item => pattern.test(item.value)); }
  function years(section: Bridge_HomeSection | undefined, prefix = '') { for (const [year, title] of [[new Date().getFullYear(), word('今年佳作', 'This year')], [new Date().getFullYear() - 1, word('去年佳作', 'Last year')]] as const) {
    if (section?.filters?.year.options.some(item => item.value === String(year))) add(section, prefix + title, { ...(lockedGenre.value ? { genre: lockedGenre.value } : {}), year: String(year) });
  } }
  const movie = sections.value.find(item => item.kind === 'movie'), series = sections.value.find(item => item.kind === 'series');
  if (props.category) {
    const section = base.value, categoryFilter = lockedGenre.value ? { genre: lockedGenre.value } : {};
    if (['anime', 'documentary'].includes(props.category) && !lockedGenre.value) return [];
    add(section, word('本周趋势', 'Trending this week'), { ...categoryFilter, sort: 'trending_week' });
    add(section, word('最新上线', 'New releases'), { ...categoryFilter, sort: 'latest' });
    years(section);
    for (const option of section?.filters?.genre.options ?? []) { if (!lockedGenre.value || option.value === lockedGenre.value) add(section, option.name, { genre: option.value }); }
  } else {
    add(movie, word('本周趋势电影', 'Trending movies'), { sort: 'trending_week' });
    add(series, word('本周趋势剧集', 'Trending series'), { sort: 'trending_week' }, 'series');
    add(movie, word('最新上线', 'New releases'), { sort: 'latest' });
    add(movie, word('热门电影', 'Popular movies'), { sort: 'popular' });
    for (const [pattern, title] of [[/^(Action|动作)$/, word('动作', 'Action')], [/^(Comedy|喜剧)$/, word('喜剧', 'Comedy')], [/^(Science Fiction|Sci-Fi|科幻)$/, word('科幻', 'Science fiction')], [/^(Drama|剧情)$/, word('剧情', 'Drama')]] as const) { const option = genre(movie, pattern); if (option) add(movie, title, { genre: option.value }); }
    years(movie);
    for (const [pattern, title] of [[/Mandarin|Chinese|普通话|华语/i, word('华语精选', 'Chinese cinema')], [/Korean|韩语/i, word('韩语精选', 'Korean cinema')], [/Japanese|日语/i, word('日语精选', 'Japanese cinema')]] as const) { const option = movie?.filters?.language.options.find(item => pattern.test(item.value)); if (option) add(movie, title, { language: option.value }); }
    if (!result.length) for (const section of sections.value) add(section, section.title, {}, section.kind === 'series' ? 'series' : 'movie');
  }
  return result;
});
const heroRow = computed(() => rows.value[0]);
const heroData = computed(() => heroRow.value ? app.catalogRow(heroRow.value.section.section_ref, heroRow.value.filter).data : null);
function imageError(sectionRef: string, filter: HomeFilter, id: string) {
  void app.rowImageError(sectionRef, filter, id).catch(error => app.diagnostics.push({ code: 'UPSTREAM_FAILED', message: String(error), retryable: true }));
}
const history = ref<HistoryEntry[]>([]), posters = ref<Record<string, string>>({}), historyError = ref('');
let historyGeneration = 0;
async function loadHistory() {
  const generation = ++historyGeneration; historyError.value = '';
  try {
    const entries = await historyRepository.list(); if (generation !== historyGeneration) return;
    history.value = entries.filter(entry => !entry.completed).slice(0, 20);
    for (const entry of history.value) if (entry.record) { const url = await historyPoster(entry.record, state.sources.data?.providers ?? [], api.call.bind(api)); if (generation !== historyGeneration) return; if (url) posters.value[entry.content_id] = url; }
  } catch (error) { if (generation === historyGeneration) historyError.value = String(error); }
}
const root = ref<HTMLElement>(); let observer: IntersectionObserver;
function loadWall(next = false) { if (!wallRef.value || !wallRow.value || wallRow.value.phase === 'loading') return; const page = next ? wallSection.value?.next_page : 1; if (page) void app.loadRow(wallRef.value, wallFilter.value, page); }
async function observe() {
  await nextTick(); observer?.disconnect();
  observer = new IntersectionObserver(entries => { for (const entry of entries) if (entry.isIntersecting) {
    if (entry.target.classList.contains('hero-observer')) { const row = heroRow.value; if (row && app.catalogRow(row.section.section_ref, row.filter).phase === 'pending') void app.loadRow(row.section.section_ref, row.filter); }
    else if (entry.target.classList.contains('poster-wall')) { if (wallRow.value?.phase === 'pending') loadWall(); }
    else if (wallRow.value?.phase === 'ready') loadWall(true);
  } }, { rootMargin: '0px' });
  root.value?.querySelectorAll('.hero-observer,.poster-wall,.wall-sentinel').forEach(element => observer.observe(element));
}
watch([rows, wall, () => props.query, () => state.home.request_epoch, () => wallSection.value?.next_page], observe, { flush: 'post' });
watch(() => state.selected_metadata_source, loadHistory);
onMounted(() => { void loadHistory(); void observe(); }); onBeforeUnmount(() => { observer?.disconnect(); historyGeneration++; });
</script>
<template>
  <div ref="root" class="home-catalog" :class="{ 'browse-catalog': category }" :data-category="category" :data-query="query">
    <div class="hero-observer"><CatalogHero :items="heroData?.page.sections[0]?.items ?? []" :images="heroData?.images ?? {}" :category="categoryTitle" :compact="wall" @open="emit('open', $event)" @image-error="heroRow && imageError(heroRow.section.section_ref, heroRow.filter, $event)" /></div>
    <p v-if="state.home.phase === 'loading'" class="catalog-status" role="status">{{ t('Loading catalog') }}</p>
    <p v-if="state.home.error" class="home-failed" role="alert">{{ state.home.error.message }} <button class="home-retry glass-pill" @click="app.loadCatalog()">{{ t('Retry') }}</button></p>
    <p v-if="state.home.phase === 'empty'" class="home-empty">{{ word('该来源暂无栏目', 'No catalogs from this source') }}</p>
    <FilterBar v-if="category" :model-value="selected" :facets="base?.filters" :locked-genre="lockedGenre" @update:model-value="filters" />
    <div v-if="wall" class="catalog-wall-container">
      <p class="wall-progress" role="status">{{ word('已查找', 'Scanned') }} {{ wallSection?.scanned_count ?? 0 }} · {{ word('找到', 'Found') }} {{ wallSection?.matched_count ?? 0 }}</p>
      <div :key="JSON.stringify(wallFilter)" class="poster-wall home-grid"><MediaRow :title="categoryTitle" :items="wallSection?.items ?? []" :images="wallRow?.data?.images" :loading="wallRow?.phase === 'loading' && !wallSection" @open="emit('open', $event)" @image-error="wallRef && imageError(wallRef, wallFilter, $event)" /></div>
      <p v-if="wallRow?.error" class="catalog-row-error" role="alert">{{ wallRow.error.message }} <button class="glass-pill home-section-retry" @click="loadWall()">{{ t('Retry') }}</button></p>
      <div v-if="wallRow?.phase === 'ready' && !wallSection?.items.length" class="catalog-empty"><Icon name="search" :size="40" /><h2>{{ word('没有找到影片', 'No titles found') }}</h2><p>{{ word('试试减少筛选条件，发现更多故事。', 'Remove a filter to discover more stories.') }}</p><button v-for="(_, key) in selected" :key="key" class="glass-pill empty-remove" @click="filters(Object.fromEntries(Object.entries(selected).filter(([name]) => name !== key)))">{{ word('去掉', 'Remove') }} {{ selected[key] }}</button><button class="glass-pill" @click="filters({})">{{ word('清除全部', 'Clear all') }}</button></div>
      <div v-if="wallSection?.has_next" class="wall-sentinel"><button class="glass-pill wall-more" :disabled="wallRow?.phase === 'loading'" @click="loadWall(true)">{{ word('继续查找', 'Keep looking') }}</button></div>
    </div>
    <div v-else class="home-content-rows">
      <section v-if="!category && (history.length || historyError)" class="continue-watching home-section"><h2>{{ word('继续观看', 'Continue watching') }}</h2><p v-if="historyError" role="alert">{{ historyError }} <button class="glass-pill" @click="loadHistory">{{ t('Retry') }}</button></p><div class="continue-track" data-focus-row><article v-for="entry in history" :key="entry.content_id" class="continue-card"><button class="continue-poster" :aria-label="entry.title" @click="emit('resume', entry)"><img v-if="posters[entry.content_id]" :src="posters[entry.content_id]" alt="" /><span class="continue-episode">{{ word(`第 ${entry.episode} 集`, `Episode ${entry.episode}`) }}</span><span class="continue-play"><Icon name="play" :size="28" /></span><span class="continue-progress"><span :style="{ width: `${Math.min(100, entry.position_ms / (entry.duration_ms || 1) * 100)}%` }" /></span></button><p>{{ entry.title }}</p><p class="continue-remaining">{{ entry.duration_ms ? word(`剩 ${Math.max(0, Math.ceil((entry.duration_ms - entry.position_ms) / 60000))} 分钟`, `${Math.max(0, Math.ceil((entry.duration_ms - entry.position_ms) / 60000))} min left`) : word('继续观看', 'Continue watching') }}</p></article></div></section>
      <WatchlistRow v-if="!category" @open="emit('open', $event)" />
      <CatalogRow v-for="row in rows" :key="app.rowKey(row.section.section_ref, row.filter)" :title="row.title" :section-ref="row.section.section_ref" :filter="row.filter" :href="href(row.category, row.section.section_ref, row.filter)" @open="emit('open', $event)" />
      <p v-if="category && !rows.length && state.home.phase === 'ready'" class="catalog-empty">{{ word('当前来源未提供此分类', 'This category is unavailable from this source') }}</p>
    </div>
  </div>
</template>
