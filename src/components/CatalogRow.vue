<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import MediaRow from './MediaRow.vue';
import { appController as app, type HomeFilter } from '../state';
import { locale, t } from '../i18n';
import type { Bridge_MediaRef } from '../models';
const props = defineProps<{ title: string; sectionRef: string; filter: HomeFilter; href: string }>();
const emit = defineEmits<{ open: [ref: Bridge_MediaRef] }>();
const root = ref<HTMLElement>();
const row = computed(() => app.catalogRow(props.sectionRef, props.filter));
const section = computed(() => row.value.data?.page.sections[0]);
let observer: IntersectionObserver;
function load() { if (row.value.phase === 'pending') void app.loadRow(props.sectionRef, props.filter); }
function observe() { observer?.disconnect(); observer = new IntersectionObserver(entries => { if (entries.some(entry => entry.isIntersecting)) load(); }, { rootMargin: '0px' }); if (root.value) observer.observe(root.value); }
watch(() => app.rowKey(props.sectionRef, props.filter), observe, { flush: 'post' });
onMounted(observe); onBeforeUnmount(() => observer?.disconnect());
</script>
<template>
  <section ref="root" class="home-section catalog-lazy-row" :data-row-key="app.rowKey(sectionRef, filter)" :data-filter="JSON.stringify(filter)" :data-phase="row.phase">
    <MediaRow :title="title" :items="section?.items ?? []" :images="row.data?.images" :href="href" :loading="['pending', 'loading'].includes(row.phase) && !section" @open="emit('open', $event)" @image-error="app.rowImageError(sectionRef, filter, $event).catch(error => app.diagnostics.push({ code: 'UPSTREAM_FAILED', message: String(error), retryable: true }))" />
    <p v-if="row.error" class="catalog-row-error" role="alert">{{ row.error.message }} <button class="home-section-retry glass-pill" @click="app.loadRow(sectionRef, filter)">{{ t('Retry') }}</button></p>
    <p v-if="row.phase === 'ready' && !section?.items.length" class="catalog-row-empty">{{ locale === 'zh-CN' ? '该栏目暂无内容' : 'No titles in this row yet' }}</p>
  </section>
</template>
