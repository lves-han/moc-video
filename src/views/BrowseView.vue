<script setup lang="ts">
import HomeCatalog from '../components/HomeCatalog.vue';
import type { Bridge_MediaRef } from '../models';
defineProps<{ category: string; query: string }>();
const emit = defineEmits<{ open: [ref: Bridge_MediaRef] }>();
</script>
<template><HomeCatalog :category="category" :query="query" @open="emit('open', $event)" /></template>
