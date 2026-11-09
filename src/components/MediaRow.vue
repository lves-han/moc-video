<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import Icon from "./Icon.vue";
import { t } from "../i18n";
import type { Bridge_MediaCard, Bridge_MediaRef, ImageAccessMap } from "../models";

const props = withDefaults(defineProps<{ title: string; items: Bridge_MediaCard[]; images?: ImageAccessMap; href?: string; landscape?: boolean; loading?: boolean }>(), { images: () => ({}) });
const emit = defineEmits<{ open: [ref: Bridge_MediaRef]; imageError: [id: string] }>();
const track = ref<HTMLElement>();
const atStart = ref(true), atEnd = ref(false);
const loaded = ref(new Set<string>()), broken = ref(new Set<string>());
const poster = (card: Bridge_MediaCard) => card.poster && !broken.value.has(card.poster.resource_id) ? props.images[card.poster.resource_id]?.url : undefined;
function edges() {
  if (track.value) {
    atStart.value = track.value.scrollLeft <= 1;
    atEnd.value = track.value.scrollLeft + track.value.clientWidth >= track.value.scrollWidth - 1;
  }
}
function page(direction: number) {
  const element = track.value;
  if (!element) return;
  const cards = [...element.querySelectorAll<HTMLElement>(".media-poster")];
  const edge = element.getBoundingClientRect();
  const target = direction > 0 ? cards.find(card => card.getBoundingClientRect().right > edge.right + 1)
    : [...cards].reverse().find(card => card.getBoundingClientRect().left < edge.left - 1);
  // Focus travels with paging so an arrow disappearing at the end cannot lose it.
  target?.focus({ preventScroll: true });
  target?.scrollIntoView({ inline: direction > 0 ? "start" : "end", block: "nearest", behavior: matchMedia("(prefers-reduced-motion: reduce)").matches ? "instant" : "smooth" });
}
function highlight(event: PointerEvent) {
  const element = event.currentTarget as HTMLElement;
  if (matchMedia("(prefers-reduced-motion: reduce)").matches) return;
  const r = element.getBoundingClientRect(), x = Math.max(0, Math.min(1, (event.clientX - r.x) / r.width)), y = Math.max(0, Math.min(1, (event.clientY - r.y) / r.height));
  element.style.setProperty("--tilt-x", `${(0.5 - y) * 8}deg`);
  element.style.setProperty("--tilt-y", `${(x - 0.5) * 8}deg`);
  element.style.setProperty("--highlight-x", `${x * 100}%`);
  element.style.setProperty("--highlight-opacity", String(x * .18));
}
function reset(event: PointerEvent) { (event.currentTarget as HTMLElement).style.cssText = ""; }
function failed(card: Bridge_MediaCard) {
  if (!card.poster) return;
  broken.value.add(card.poster.resource_id);
  emit("imageError", card.poster.resource_id);
}
watch(() => props.images, () => { loaded.value.clear(); broken.value.clear(); });
watch(() => props.items, async () => { await nextTick(); edges(); });
let observer: ResizeObserver | undefined;
onMounted(() => { observer = new ResizeObserver(edges); if (track.value) observer.observe(track.value); edges(); });
onBeforeUnmount(() => observer?.disconnect());
</script>

<template>
  <section class="media-row" :class="{ 'media-row-landscape': landscape, 'media-row-end': atEnd }" :aria-label="title" :aria-busy="loading">
    <div class="media-row-heading"><h2><a v-if="href" :href="href">{{ title }}<Icon name="chevron" :size="18" /></a><template v-else>{{ title }}</template></h2></div>
    <div class="media-row-body">
      <div ref="track" class="media-row-track" data-focus-row @scroll.passive="edges">
        <template v-if="loading"><div v-for="n in 8" :key="n" class="media-card media-loading" aria-hidden="true"><div class="media-poster" /><span /></div></template>
        <article v-for="card in loading ? [] : items" :key="card.id" class="media-card home-card" :data-card-id="card.id">
          <a class="media-poster poster-link" :href="`#/details/${encodeURIComponent(JSON.stringify(card.ref))}`" :aria-label="t('View details for {title}', { title: card.title })" @click.prevent="emit('open', card.ref)" @pointermove="highlight" @pointerleave="reset">
            <img v-if="poster(card)" :src="poster(card)" alt="" loading="lazy" :class="{ loaded: loaded.has(card.poster!.resource_id) }" @load="loaded.add(card.poster!.resource_id)" @error="failed(card)" />
            <span v-else class="media-poster-missing">{{ card.title }}</span>
          </a>
          <p class="media-card-title card-title" :title="card.title">{{ card.title }}</p>
          <p class="media-card-detail">{{ [card.year, card.update_label ?? card.subtitle, ...(card.genres ?? []).slice(0, 1)].filter(Boolean).join(' · ') }}</p>
        </article>
      </div>
      <button v-if="!atStart" class="media-row-arrow media-row-previous" :aria-label="t('Previous items')" @click="page(-1)"><Icon name="back" /></button>
      <button v-if="!atEnd" class="media-row-arrow media-row-next" :aria-label="t('Next items')" @click="page(1)"><Icon name="arrow" /></button>
    </div>
  </section>
</template>

<style scoped>
@media (min-width: 960px) {
  .media-row { margin-block: 40px; min-width: 0; }
  .media-row-heading { height: 28px; margin-bottom: 12px; }
  h2 { font-size: 22px; line-height: 1.25; font-weight: 600; }
  h2 a { display: inline-flex; align-items: center; gap: 8px; }
  .media-row-body { position: relative; }
  .media-row-track { display: flex; gap: 16px; overflow-x: auto; scroll-snap-type: x mandatory; scrollbar-width: none; padding: 24px 12px; margin-inline: -12px; scroll-padding-inline: 12px; }
  .media-row-track::-webkit-scrollbar { display: none; }
  .media-row-body::after { content: ""; position: absolute; right: -12px; top: 0; bottom: 0; width: 48px; background: linear-gradient(90deg, transparent, var(--tv-bg)); pointer-events: none; }
  .media-row-end .media-row-body::after { opacity: 0; }
  .media-card { flex: 0 0 clamp(132px, 12.5vw, 220px); min-width: 0; scroll-snap-align: start; }
  .media-row-landscape .media-card { flex-basis: clamp(240px, 22vw, 360px); }
  .media-poster { display: block; position: relative; aspect-ratio: 2 / 3; border-radius: 12px; background: var(--tv-surface-1); overflow: hidden; isolation: isolate; transition: transform 200ms var(--tv-ease), opacity 200ms var(--tv-ease); }
  .media-row-landscape .media-poster { aspect-ratio: 16 / 9; }
  .media-poster img { width: 100%; height: 100%; object-fit: cover; opacity: 0; transition: opacity 200ms var(--tv-ease); }
  .media-poster img.loaded { opacity: 1; }
  .media-poster::after { content: ""; position: absolute; inset: 0; background: linear-gradient(135deg, rgba(255,255,255,var(--highlight-opacity,.18)), transparent 65%); background-position: var(--highlight-x,50%) 0; opacity: 0; pointer-events: none; transition: opacity 200ms var(--tv-ease); }
  .media-poster:is(:hover,:focus-visible) { transform: perspective(800px) scale(1.08) rotateX(var(--tilt-x,0deg)) rotateY(var(--tilt-y,0deg)); box-shadow: 0 18px 40px rgba(0,0,0,.55); z-index: 1; }
  .media-poster:is(:hover,:focus-visible)::after { opacity: 1; }
  .media-poster:focus-visible { outline: 2px solid var(--tv-text); outline-offset: -2px; }
  .media-poster-missing { position: absolute; inset: 0; display: grid; place-items: center; padding: 12px; font-size: 15px; line-height: 1.5; font-weight: 600; text-align: center; overflow-wrap: anywhere; }
  .media-card-title { font-size: 14px; line-height: 1.3; font-weight: 500; margin-top: 12px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .media-card-detail { font-size: 13px; line-height: 1.4; font-weight: 400; color: var(--tv-text-2); margin-top: 4px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; font-variant-numeric: tabular-nums; }
  .media-row-arrow { position: absolute; top: calc(2px + clamp(132px, 12.5vw, 220px) * .75); width: 44px; height: 44px; border-radius: 999px; background: var(--tv-glass); border: 1px solid var(--tv-hairline); -webkit-backdrop-filter: var(--tv-glass-filter); backdrop-filter: var(--tv-glass-filter); display: grid; place-items: center; z-index: 2; opacity: 0; transition: opacity 200ms var(--tv-ease), transform 200ms var(--tv-ease); }
  .media-row-landscape .media-row-arrow { top: calc(2px + clamp(240px, 22vw, 360px) * .28125); }
  .media-row-previous { left: -10px; } .media-row-next { right: -10px; }
  .media-row:is(:hover,:focus-within) .media-row-arrow { opacity: 1; }
  .media-row-arrow:is(:hover,:focus-visible) { transform: scale(1.04); }
  .media-loading .media-poster { animation: media-pulse 1.2s ease-in-out infinite alternate; }
  .media-loading > span { display: block; width: 75%; height: 18px; margin-top: 12px; background: var(--tv-surface-1); border-radius: 4px; }
  @keyframes media-pulse { from { opacity: .5; } to { opacity: .8; } }
}
@media (min-width: 1440px) { .media-row-track { gap: 20px; } }
@media (prefers-reduced-motion: reduce) {
  .media-loading .media-poster { animation: none; }
  .media-poster:is(:hover,:focus-visible), .media-row-arrow:is(:hover,:focus-visible) { transform: none; }
  .media-poster { transition-property: opacity; }
  .media-row-track { scroll-behavior: auto; }
}
</style>
