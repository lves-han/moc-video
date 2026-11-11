<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from 'vue';
import type { Bridge_HomeSection } from '../models';
import type { HomeFilter } from '../state';
import { locale } from '../i18n';
const props = defineProps<{ modelValue: HomeFilter; facets?: Bridge_HomeSection['filters']; lockedGenre?: string }>();
const emit = defineEmits<{ 'update:modelValue': [value: HomeFilter] }>();
type Dimension = keyof HomeFilter;
const dimensions: Dimension[] = ['genre', 'year', 'language', 'region', 'sort'];
const labels = computed(() => locale.value === 'zh-CN' ? ['类型', '年份', '语言', '地区', '排序'] : ['Genre', 'Year', 'Language', 'Region', 'Sort']);
const open = ref<Dimension | null>(null), root = ref<HTMLElement>(), notice = ref('');
const zh = computed(() => locale.value === 'zh-CN');
const disabled = (key: Dimension) => !props.facets?.[key].available || key === 'genre' && !!props.lockedGenre || key === 'sort' && !!(props.modelValue.year || props.modelValue.language);
const reason = (key: Dimension) => key === 'sort' && (props.modelValue.year || props.modelValue.language) ? (zh.value ? '清除年份和语言后可排序' : 'Clear year and language to sort') : key === 'genre' && props.lockedGenre ? (zh.value ? '由当前分类确定' : 'Set by this category') : (zh.value ? '当前来源未提供此筛选' : 'Unavailable from this source');
function choose(key: Dimension, value?: string) {
  const next = { ...props.modelValue };
  if (value === next[key]) delete next[key];
  else if (value) Object.assign(next, { [key]: value });
  else delete next[key];
  if ((key === 'year' || key === 'language') && value && next.sort) { delete next.sort; notice.value = zh.value ? '已清除排序，以应用年份或语言筛选' : 'Sort cleared to apply year or language'; }
  emit('update:modelValue', next); open.value = null;
  void nextTick(() => root.value?.querySelector<HTMLButtonElement>(`[data-dimension="${key}"]`)?.focus());
}
async function toggle(key: Dimension) { open.value = open.value === key ? null : key; await nextTick(); root.value?.querySelector<HTMLButtonElement>('.filter-panel button')?.focus(); }
function keyboard(event: KeyboardEvent) {
  if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); const key = open.value; open.value = null; root.value?.querySelector<HTMLButtonElement>(`[data-dimension="${key}"]`)?.focus(); return; }
  if (!['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(event.key)) return;
  const target = event.currentTarget as HTMLElement, buttons = [...target.querySelectorAll<HTMLButtonElement>('button')];
  const current = buttons.indexOf(document.activeElement as HTMLButtonElement), columns = Math.max(1, Math.floor(target.clientWidth / 96));
  const offset = { ArrowLeft: -1, ArrowRight: 1, ArrowUp: -columns, ArrowDown: columns }[event.key]!;
  event.preventDefault(); event.stopPropagation(); buttons[(current + offset + buttons.length) % buttons.length]?.focus();
}
function outside(event: PointerEvent) { if (!root.value?.contains(event.target as Node)) open.value = null; }
onMounted(() => document.addEventListener('pointerdown', outside)); onBeforeUnmount(() => document.removeEventListener('pointerdown', outside));
</script>
<template>
  <div ref="root" class="filter-bar">
    <div class="filter-capsules" data-focus-row><div v-for="(key, i) in dimensions" :key="key" class="filter-dimension">
      <button class="filter-pill" :class="{ selected: modelValue[key] }" :data-dimension="key" :disabled="disabled(key)" :title="disabled(key) ? reason(key) : labels[i]" :aria-expanded="open === key" @click="toggle(key)">{{ facets?.[key].options.find(item => item.value === modelValue[key])?.name ?? modelValue[key] ?? labels[i] }}<span v-if="modelValue[key]" aria-hidden="true">×</span></button>
      <span v-if="disabled(key)" class="filter-unavailable">{{ reason(key) }}</span>
      <div v-if="open === key" class="filter-panel" role="dialog" :aria-label="labels[i]" @keydown="keyboard"><div class="filter-option-grid"><button @click="choose(key)">{{ zh ? '全部' : 'All' }}</button><button v-for="option in facets?.[key].options" :key="option.value" :aria-pressed="modelValue[key] === option.value" @click="choose(key, option.value)">{{ option.name }}<span v-if="modelValue[key] === option.value" class="filter-check">✓</span></button></div></div>
    </div><button v-if="Object.keys(modelValue).length" class="filter-clear" @click="emit('update:modelValue', {})">{{ zh ? '清除全部' : 'Clear all' }}</button></div>
    <p v-if="notice" class="filter-notice" role="status">{{ notice }}</p>
  </div>
</template>
