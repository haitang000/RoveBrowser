<script setup lang="ts">
defineProps<{ name: string; size?: number }>();
const paths: Record<string, string> = {
  back: "m10 3-5 5 5 5",
  forward: "m6 3 5 5-5 5",
  reload: "M13 6a5 5 0 1 0 .3 3M13 2v4H9",
  plus: "M8 2v12M2 8h12",
  close: "m4 4 8 8M12 4l-8 8",
  sidebar: "M2 3h12v10H2zM6 3v10",
  search: "M10.5 10.5 14 14M11.5 6.5a5 5 0 1 1-10 0 5 5 0 0 1 10 0",
  grid: "M2 2h5v5H2zM10 2h4v5h-4zM2 10h5v4H2zM10 10h4v4h-4z",
  split: "M2 2h12v12H2zM8 2v12",
  moon: "M13 10a6 6 0 0 1-7-7 6 6 0 1 0 7 7",
  sun: "M8 1v1M8 14v1M1 8h1M14 8h1M3 3l1 1M12 12l1 1M3 13l1-1M12 4l1-1M11 8a3 3 0 1 1-6 0 3 3 0 0 1 6 0",
  settings: "M3 4h10M3 12h10M6 2v4M10 10v4",
  spaces: "M2 5h9v9H2zM5 2h9v9",
  key: "M10 2a4 4 0 1 1-2 7l-5 5H1v-2l5-5a4 4 0 0 1 4-5M11 5h.01",
  shield: "M8 1l6 2v5c0 4-6 7-6 7S2 12 2 8V3zM5 8l2 2 4-4",
  translate: "M1 3h9M5 1v2M8 3c-1 4-3 6-6 8M3 5c1 3 3 5 5 6M9 14l3-7 3 7M10 12h4",
  extension: "M2 2h4a2 2 0 1 1 4 0h4v4a2 2 0 1 0 0 4v4H9a2 2 0 1 0-4 0H2V9a2 2 0 1 0 0-4z",
  keyboard: "M1 3h14v10H1zM4 6h1M7 6h1M10 6h1M4 9h1M7 9h1M10 9h1M5 11h6",
  trash: "M2 4h12M6 1h4l1 3M4 4l1 10h6l1-10M7 6v6M9 6v6",
  pin: "m6 2 5 5M4 4l3-3 6 6-3 3M4 4l-1 4 5 5 4-1M1 15l5-5",
  globe:
    "M14 8A6 6 0 1 1 2 8a6 6 0 0 1 12 0M2 8h12M8 2c-4 4-4 8 0 12 4-4 4-8 0-12",
  history: "M2 7a6 6 0 1 1 1 5M1 3v4h4M8 4v4l3 2",
  check: "m3 8 3 3 7-7",
  minimize: "M3 8h10",
  maximize: "M3 3h10v10H3z",
  expand: "M2 6V2h4M10 14h4v-4M2 2l4 4M14 14l-4-4",
  restore: "M5 2h9v9M2 5h9v9H2z",
  arrow: "M3 8h10M9 4l4 4-4 4",
  chevron: "m5 6 3 3 3-3",
  chevronUp: "m4 10 4-4 4 4",
  chevronDown: "m4 6 4 4 4-4",
};
</script>
<template>
  <svg
    :width="size || 16"
    :height="size || 16"
    viewBox="0 0 16 16"
    fill="none"
    stroke="currentColor"
    stroke-width="1.1"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
  >
    <path :d="paths[name] || paths.globe" />
  </svg>
</template>
