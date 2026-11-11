<script setup lang="ts">
import HomeCatalog from '../components/HomeCatalog.vue';
import MacCmsCatalog from '../components/MacCmsCatalog.vue';
import type { Bridge_MediaRef } from '../models';
defineProps<{ category: string; query: string }>();
const emit = defineEmits<{ open: [ref: Bridge_MediaRef] }>();
</script>
<template><MacCmsCatalog v-if="['variety', 'short', 'sports'].includes(category)" :category="category" :query="query" @open="emit('open', $event)" /><HomeCatalog v-else :category="category" :query="query" @open="emit('open', $event)" /></template>
