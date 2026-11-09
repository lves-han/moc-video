<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import Icon from './Icon.vue';
import { locale } from '../i18n';
import type { Bridge_MediaCard, Bridge_MediaRef, ImageAccessMap } from '../models';
const props = withDefaults(defineProps<{ items: Bridge_MediaCard[]; images: ImageAccessMap; category?: string; compact?: boolean }>(), { category: '', compact: false });
const emit = defineEmits<{ open: [ref: Bridge_MediaRef]; imageError: [id: string] }>();
const index = ref(0), hover = ref(false), focus = ref(false), hidden = ref(document.hidden), scroll = ref(scrollY);
const root = ref<HTMLElement>();
const paused = computed(() => hover.value || focus.value || hidden.value);
const cards = computed(() => props.items.slice(0, 5));
const active = computed(() => cards.value[index.value % Math.max(cards.value.length, 1)]);
const zh = computed(() => locale.value === 'zh-CN');
const image = (card: Bridge_MediaCard) => { const id = (card.backdrop ?? card.poster)?.resource_id; return id ? props.images[id]?.url : undefined; };
const height = computed(() => root.value?.offsetHeight ?? 760);
const shift = computed(() => Math.min(scroll.value, height.value) * .4);
const opacity = computed(() => 1 - Math.min(scroll.value / (height.value * .6), 1) * .75);
function visibility() { hidden.value = document.hidden; }
function scrolling() { scroll.value = scrollY; }
let timer: ReturnType<typeof setInterval>;
function restart() { clearInterval(timer); timer = setInterval(() => { if (!paused.value && cards.value.length > 1) index.value = (index.value + 1) % cards.value.length; }, 8000); }
watch([paused, () => props.items], restart);
onMounted(() => { restart(); document.addEventListener('visibilitychange', visibility); window.addEventListener('scroll', scrolling, { passive: true }); });
onBeforeUnmount(() => { clearInterval(timer); document.removeEventListener('visibilitychange', visibility); window.removeEventListener('scroll', scrolling); });
</script>
<template>
  <header ref="root" class="catalog-hero" :class="{ 'browse-hero': category, 'hero-compact': compact, 'hero-paused': paused }" :data-index="index" :data-paused="paused" @mouseenter="hover = true" @mouseleave="hover = false" @focusin="focus = true" @focusout="focus = !!root?.contains(($event as FocusEvent).relatedTarget as Node)">
    <div class="catalog-hero-visual" :style="{ transform: `translateY(${shift}px)`, opacity }">
      <div v-for="(card, i) in cards" :key="card.id" class="catalog-hero-slide" :class="{ current: i === index % cards.length }" :aria-hidden="i !== index % cards.length">
        <img v-if="image(card)" :src="image(card)" alt="" @error="emit('imageError', (card.backdrop ?? card.poster)!.resource_id)" />
      </div>
    </div>
    <div class="hero-bottom-mask" /><div class="hero-left-mask" />
    <div class="catalog-hero-copy">
      <p class="hero-eyebrow">{{ category || (zh ? '本周趋势 · 精选' : 'TRENDING THIS WEEK') }}</p>
      <h1>{{ compact ? category : active?.title || category || (zh ? '发现好故事' : 'Discover good stories') }}</h1>
      <p v-if="!compact && active" class="hero-information">{{ [active.year, active.rating ? `${active.rating} ★` : null, ...(active.genres ?? []).slice(0, 2)].filter(Boolean).join(' · ') }}</p>
      <p v-if="!compact && active?.plot" class="hero-plot">{{ active.plot }}</p>
      <div v-if="!compact && active" class="hero-actions"><button class="hero-primary" @click="emit('open', active.ref)"><Icon name="play" :size="20" />{{ zh ? '立即观看' : 'Watch now' }}</button><button class="hero-secondary" @click="emit('open', active.ref)">{{ zh ? '查看详情' : 'View details' }}</button></div>
    </div>
    <div v-if="!compact && cards.length > 1" class="hero-indicators" :aria-label="zh ? '精选影片' : 'Featured titles'"><button v-for="(card, i) in cards" :key="card.id" :class="{ active: i === index % cards.length }" :aria-label="card.title" :aria-pressed="i === index % cards.length" @click="index = i; restart()" /></div>
  </header>
</template>
