<script setup lang="ts">
import Icon from "./Icon.vue";
import { locale, t, type Message } from "../i18n";
import { kindLabels, type Film } from "../data/demo";
defineProps<{ film: Film; index?: number }>();
</script>

<template>
  <article class="film-card">
    <a
      :href="`#/film/${film.id}`"
      class="poster-link"
      :aria-label="t('View details for {title}', { title: film.title })"
    >
      <img
        :src="film.artwork"
        alt=""
        :style="{ objectPosition: film.position || 'center' }"
        loading="lazy"
        width="400"
        height="600"
      />
      <span class="poster-shade" :class="film.posterTone" />
      <span class="poster-kind">{{
        film.episodes > 1
          ? t("{count} episodes", { count: film.episodes })
          : t(kindLabels[film.kind])
      }}</span>
      <span class="poster-lettering"
        ><small v-if="locale === 'zh-CN'">{{ film.english }}</small
        ><strong>{{ film.title }}</strong></span
      >
      <span class="card-play"><Icon name="play" :size="21" /></span>
    </a>
    <a :href="`#/film/${film.id}`" class="card-title">{{ film.title }}</a>
    <p>
      {{ film.year }}<span>·</span
      >{{ film.genres.map((genre) => t(genre as Message)).join(" / ") }}
    </p>
  </article>
</template>
