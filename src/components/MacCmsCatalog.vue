<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import CatalogHero from './CatalogHero.vue';
import FilterBar from './FilterBar.vue';
import MediaRow from './MediaRow.vue';
import { api, ApiFailure } from '../api';
import { appState as state, mergeHome, type HomeFilter } from '../state';
import { locale } from '../i18n';
import type { Bridge_HomeSection, Bridge_MediaRef, HomeViewData } from '../models';

const props = defineProps<{ category: string; query: string }>();
const emit = defineEmits<{ open: [ref: Bridge_MediaRef] }>();
const word = (cn: string, en: string) => locale.value === 'zh-CN' ? cn : en;
const title = computed(() => ({ variety: word('综艺', 'Variety'), short: word('短剧', 'Short dramas'), sports: word('体育', 'Sports') }[props.category] ?? ''));
const source = computed(() => state.sources.data?.providers.find(item => item.kind === 'maccms' && item.runtime_status === 'ready'
  && item.id === state.selected_playback_source?.source_id && item.source_revision === state.selected_playback_source?.source_revision));
const data = ref<HomeViewData | null>(null), loading = ref(false), error = ref('');
const filters = computed<HomeFilter>(() => {
  const params = new URLSearchParams(props.query), value: HomeFilter = {};
  for (const key of ['genre', 'year', 'language', 'region'] as const) { const text = params.get(key); if (text) value[key] = text; }
  return value;
});
function belongs(section: Pick<Bridge_HomeSection, 'title'>) {
  return props.category === 'variety' ? /综艺/.test(section.title) : props.category === 'sports'
    ? /体育|足球|篮球|网球|斯诺克|台球|赛事|电竞/.test(section.title)
    : /短剧|古装仙侠|现代都市|穿越年代|年代穿越|言情总裁|重生民国|反转爽剧|脑洞悬疑|女频恋爱|都市脑洞|逆袭/.test(section.title);
}
const sections = computed(() => (data.value?.page.sections ?? []).filter(belongs));
const failures = computed(() => (data.value?.page.failed_sections ?? []).filter(belongs));
const section = computed(() => sections.value.find(item => item.title === new URLSearchParams(props.query).get('subcategory')) ?? sections.value[0]);
const subcategories = computed(() => [...new Set(sections.value.map(item => item.title))]);
let requestId: string | null = null, generation = 0;
let queried = new Set<string>(), refreshed = false;
function cancel() { generation++; if (requestId) void api.cancel(requestId).catch(cause => { error.value = String(cause); }); requestId = null; }
async function load(target: Pick<Bridge_HomeSection, 'section_ref'> | null = null, page = 1, recovery = false) {
  const selected = source.value, host = state.runtime.engine_epoch;
  cancel();
  if (!selected || !host) { data.value = null; loading.value = false; return; }
  const sequence = generation, id = crypto.randomUUID(); requestId = id;
  const input = { source_id: selected.id, source_revision: selected.source_revision, page,
    ...(target ? { section_ref: target.section_ref, filter: { ...filters.value } } : {}) };
  loading.value = true; error.value = ''; queried = new Set(); if (!recovery) refreshed = false;
  try {
    const response = await api.call('get_home', input, id, host);
    if (sequence !== generation || source.value?.id !== selected.id || source.value?.source_revision !== selected.source_revision || host !== state.runtime.engine_epoch) return;
    const previous = data.value?.page.sections.find(item => item.section_ref === target?.section_ref);
    const merged = mergeHome(target ? data.value : null, response, input);
    if (target && page > 1) {
      const appended = merged.page.sections.find(item => item.section_ref === target.section_ref)!;
      // MacCMS reports each bounded scan batch; the wall presents all loaded pages.
      appended.scanned_count = (previous?.scanned_count ?? 0) + (appended.scanned_count ?? 0);
      appended.matched_count = appended.items.length;
    }
    data.value = merged;
  } catch (cause) { if (sequence === generation) error.value = cause instanceof ApiFailure ? cause.error.message : String(cause); }
  finally { if (sequence === generation) { loading.value = false; requestId = null; } }
}
async function refresh() {
  data.value = null; await load();
  if (section.value && Object.keys(filters.value).length) await load(section.value);
}
function change(value: HomeFilter, subcategory = section.value?.title) {
  const params = new URLSearchParams(); if (subcategory) params.set('subcategory', subcategory);
  for (const [key, text] of Object.entries(value)) params.set(key, text);
  location.hash = `#/browse/${props.category}${params.size ? '?' + params : ''}`;
}
async function imageError(id: string) {
  const snapshot = data.value, sequence = generation, host = state.runtime.engine_epoch;
  if (!snapshot?.images[id] || loading.value || queried.has(id) || !host) return;
  queried.add(id);
  try {
    const status = await api.call('get_resource_status', { kind: 'image', target_id: id }, crypto.randomUUID(), host);
    if (data.value !== snapshot || sequence !== generation || host !== state.runtime.engine_epoch) return;
    if (status.kind !== 'image' || status.target_id !== id) throw new Error('Image status target differs');
    if (status.error?.code === 'SESSION_EXPIRED' && !refreshed) { refreshed = true; await load(section.value ?? null, 1, true); }
    else error.value = status.error?.message ?? 'Image failed to load';
  } catch (cause) { if (data.value === snapshot && sequence === generation) error.value = String(cause); }
}
watch([() => source.value?.id, () => source.value?.source_revision, () => state.runtime.engine_epoch, () => locale.value, () => props.category], refresh, { immediate: true });
watch(() => props.query, () => { if (section.value) void load(section.value); });
onBeforeUnmount(cancel);
</script>

<template>
  <div class="home-catalog browse-catalog maccms-catalog" :data-category="category" :data-source="source?.id" :data-query="query">
    <div class="hero-observer"><CatalogHero :items="section?.items ?? []" :images="data?.images ?? {}" :category="title" :compact="Object.keys(filters).length > 0" @open="emit('open', $event)" @image-error="imageError" /></div>
    <div class="maccms-subcategories" role="group" :aria-label="word('子分类', 'Subcategories')" data-focus-row>
      <button v-for="name in subcategories" :key="name" class="filter-pill" :class="{ selected: name === section?.title }" :aria-pressed="name === section?.title" @click="change(filters, name)">{{ name }}</button>
    </div>
    <FilterBar :model-value="filters" :facets="section?.filters" @update:model-value="change($event)" />
    <p v-if="error" class="catalog-row-error" role="alert">{{ error }} <button class="glass-pill maccms-retry" @click="section ? load(section) : refresh()">{{ word('重试', 'Retry') }}</button></p>
    <p v-for="failure in failures" :key="failure.section_ref!" class="catalog-row-error" role="alert">{{ failure.title }} · {{ failure.error.message }} <button class="glass-pill maccms-retry" @click="failure.section_ref && load({ section_ref: failure.section_ref })">{{ word('重试', 'Retry') }}</button></p>
    <p v-if="loading" class="catalog-status" role="status">{{ word('加载中', 'Loading catalog') }}</p>
    <p class="wall-progress" role="status">{{ word('已查找', 'Scanned') }} {{ section?.scanned_count ?? 0 }} · {{ word('找到', 'Found') }} {{ section?.matched_count ?? 0 }}</p>
    <div class="poster-wall home-grid"><MediaRow :title="section?.title ?? title" :items="section?.items ?? []" :images="data?.images" :landscape="category === 'sports'" :loading="loading && !section" @open="emit('open', $event)" @image-error="imageError" /></div>
    <div v-if="!loading && !error && !failures.length && !section?.items.length" class="catalog-empty"><h2>{{ word('没有找到影片', 'No titles found') }}</h2><p>{{ word('试试减少筛选条件，或选择其他播放源。', 'Remove a filter or choose another playback source.') }}</p><button v-if="Object.keys(filters).length" class="glass-pill" @click="change({})">{{ word('清除全部', 'Clear all') }}</button></div>
    <button v-if="section?.has_next" class="glass-pill wall-more" :disabled="loading" @click="load(section, section.next_page!)">{{ word('继续查找', 'Keep looking') }}</button>
  </div>
</template>
