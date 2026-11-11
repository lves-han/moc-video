<script setup lang="ts">
withDefaults(defineProps<{ name: string; size?: number }>(), { size: 20 });
const paths: Record<string, string[]> = {
  search: ["M21 21l-5-5", "M18 10a8 8 0 1 1-16 0 8 8 0 0 1 16 0Z"],
  library: ["M4 3h4v18H4zM11 3h4v18h-4zM18 4l3 16"],
  sidebar: ["M3 4h18v16H3zM9 4v16m7-12-4 4 4 4"],
  variety: ["M3 6h18v14H3zM7 3l5 3 5-3M8 11v4m8-4v4"],
  documentary: ["M3 5h13v14H3zM16 10l5-3v10l-5-3"],
  short: ["M7 2h10v20H7zM10 8l5 4-5 4"],
  sports: ["M16 5a2 2 0 1 0 0-4 2 2 0 0 0 0 4ZM4 9l6-3 4 4 6 1M7 21l4-8 4 3 2 5M10 6l-1 7-6 3"],
  watchlist: ["M5 3h14v18l-7-4-7 4V3Z", "M9 9h6m-3-3v6"],
  history: ["M3 10a9 9 0 1 1 .7 7M3 4v6h6", "M12 7v5l3 2"],
  trash: ["M3 6h18M9 6V3h6v3M6 6l1 15h10l1-15M10 10v7m4-7v7"],
  compass: [
    "M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z",
    "m16.2 7.8-2.7 5.7-5.7 2.7 2.7-5.7 5.7-2.7Z",
  ],
  film: ["M4 4h16v16H4z", "M8 4v16M16 4v16M4 8h4m-4 8h4m8-8h4m-4 8h4"],
  tv: ["M4 7h16v13H4z", "m8 3 4 4 4-4"],
  sparkles: [
    "m12 3 2.4 6.6L21 12l-6.6 2.4L12 21l-2.4-6.6L3 12l6.6-2.4L12 3Z",
    "M20 2v4m-2-2h4",
  ],
  layers: ["m12 3 10 5-10 5L2 8l10-5Z", "m2 12 10 5 10-5M2 16l10 5 10-5"],
  settings: ["M4 6h7m4 0h5M4 12h2m4 0h10M4 18h10m4 0h2", "M11 3v6M6 9v6m8 0v6"],
  play: ["m9 5 11 7-11 7V5Z"],
  stop: ["M5 5h14v14H5z"],
  speed: ["M4 19a9 9 0 1 1 16 0", "m12 13 5-5M5 13H3m18 0h-2M12 4V2"],
  "fullscreen-exit": ["M3 9h6V3m6 0v6h6M9 21v-6H3m18 0h-6v6"],
  pause: ["M7 5v14M17 5v14"],
  arrow: ["M4 12h16m-6-6 6 6-6 6"],
  back: ["M20 12H4m6-6-6 6 6 6"],
  chevron: ["m9 5 7 7-7 7"],
  down: ["m6 9 6 6 6-6"],
  close: ["m6 6 12 12M18 6 6 18"],
  check: ["m5 12 4 4L19 6"],
  plus: ["M12 5v14M5 12h14"],
  info: ["M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z", "M12 11v6m0-10v.1"],
  volume: [
    "m11 4-6 5H2v6h3l6 5V4Z",
    "M15 8a6 6 0 0 1 0 8m3-11a10 10 0 0 1 0 14",
  ],
  muted: ["m11 4-6 5H2v6h3l6 5V4Z", "m16 9 6 6m0-6-6 6"],
  fullscreen: ["M9 3H3v6m12-6h6v6M3 15v6h6m12-6v6h-6"],
  next: ["m5 5 10 7-10 7V5Z", "M19 5v14"],
  captions: ["M3 5h18v14H3z", "M10 9H7v6h3m7-6h-3v6h3"],
  repeat: [
    "m18 2 4 4-4 4M2 12V8a2 2 0 0 1 2-2h18",
    "m6 22-4-4 4-4m16-2v4a2 2 0 0 1-2 2H2",
  ],
  link: [
    "M10 13a5 5 0 0 0 7 .4l3-3a5 5 0 0 0-7-7l-2 2",
    "M14 11a5 5 0 0 0-7-.4l-3 3a5 5 0 0 0 7 7l2-2",
  ],
  palette: [
    "M21 12a9 9 0 1 0-9 9h1a2 2 0 0 0 1.4-3.4 1.5 1.5 0 0 1 1.1-2.6H18a3 3 0 0 0 3-3Z",
    "M7 10v.1M10 6v.1M15 6v.1M18 10v.1",
  ],
  moon: ["M20.8 13a9 9 0 1 1-9.8-9.8 7 7 0 0 0 9.8 9.8Z"],
  sun: [
    "M16 12a4 4 0 1 1-8 0 4 4 0 0 1 8 0Z",
    "M12 2v2m0 16v2M2 12h2m16 0h2M5 5l1.5 1.5m11 11L19 19M5 19l1.5-1.5m11-11L19 5",
  ],
};
</script>

<template>
  <svg
    :width="size"
    :height="size"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.65"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
  >
    <path
      v-for="(path, index) in paths[name] || paths.info"
      :key="index"
      :d="path"
    />
  </svg>
</template>
