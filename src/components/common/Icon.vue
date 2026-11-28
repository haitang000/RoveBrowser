<script setup lang="ts">
defineProps<{ name: string; size?: number }>();
const paths: Record<string, string> = {
  back: "m10 3-5 5 5 5",
  forward: "m6 3 5 5-5 5",
  reload: "M13 6a5 5 0 1 0 .3 3M13 2v4H9",
  reader: "M3 1.5h7l3 3v10H3zM10 1.5v3h3M5.5 7h5M5.5 9.5h5M5.5 12h3",
  spinner: "M13 8a5 5 0 1 1-5-5",
  plus: "M8 2v12M2 8h12",
  close: "m4 4 8 8M12 4l-8 8",
  sidebar: "M2 3h12v10H2zM6 3v10",
  search: "M10.5 10.5 14 14M11.5 6.5a5 5 0 1 1-10 0 5 5 0 0 1 10 0",
  filter: "M14.5 8a6.5 6.5 0 1 1-13 0 6.5 6.5 0 1 1 13 0M4.6 5.8h6.8M5.8 8h4.4M7 10.2h2",
  grid: "M2 2h5v5H2zM10 2h4v5h-4zM2 10h5v4H2zM10 10h4v4h-4z",
  split: "M2 2h12v12H2zM8 2v12",
  moon: "M13 10a6 6 0 0 1-7-7 6 6 0 1 0 7 7",
  sun: "M8 1v1M8 14v1M1 8h1M14 8h1M3 3l1 1M12 12l1 1M3 13l1-1M12 4l1-1M11 8a3 3 0 1 1-6 0 3 3 0 0 1 6 0",
  settings: "M3 4h10M3 12h10M6 2v4M10 10v4",
  spaces: "M2 5h9v9H2zM5 2h9v9",
  key: "M10 2a4 4 0 1 1-2 7l-5 5H1v-2l5-5a4 4 0 0 1 4-5M11 5h.01",
  shield: "M8 1l6 2v5c0 4-6 7-6 7S2 12 2 8V3zM5 8l2 2 4-4",
  camera: "M2 4h3l1-1.5h4L11 4h3v9H2zM10.5 8.5a2.5 2.5 0 1 1-5 0 2.5 2.5 0 0 1 5 0z",
  microphone: "M6 3a2 2 0 1 1 4 0v4a2 2 0 1 1-4 0zM3.5 7a4.5 4.5 0 0 0 9 0M8 11.5V15M6 15h4",
  location: "M13 6c0 3-5 8-5 8S3 9 3 6a5 5 0 0 1 10 0zM9.5 6a1.5 1.5 0 1 1-3 0 1.5 1.5 0 0 1 3 0z",
  bell: "M6 13a2 2 0 0 0 4 0M3 11h10l-1.5-2V6a3.5 3.5 0 1 0-7 0v3z",
  blocked: "M14 8A6 6 0 1 1 2 8a6 6 0 1 1 12 0M3.8 3.8l8.4 8.4",
  screen: "M2 2.5h12v8.5H2zM8 11v2.5M5 13.5h6",
  translate: "M1 3h9M5 1v2M8 3c-1 4-3 6-6 8M3 5c1 3 3 5 5 6M9 14l3-7 3 7M10 12h4",
  extension: "M2 2h4a2 2 0 1 1 4 0h4v4a2 2 0 1 0 0 4v4H9a2 2 0 1 0-4 0H2V9a2 2 0 1 0 0-4z",
  agent: "M8 2v2M5 4h6a3 3 0 0 1 3 3v4a3 3 0 0 1-3 3H5a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3zM5.5 8v1M10.5 8v1",
  flask: "M6 2h4M6.5 2v4l-4 6.5a1 1 0 0 0 .9 1.5h9.2a1 1 0 0 0 .9-1.5L9.5 6V2M4.6 9h6.8",
  keyboard: "M1 3h14v10H1zM4 6h1M7 6h1M10 6h1M4 9h1M7 9h1M10 9h1M5 11h6",
  trash: "M2 4h12M6 1h4l1 3M4 4l1 10h6l1-10M7 6v6M9 6v6",
  pin: "m6 2 5 5M4 4l3-3 6 6-3 3M4 4l-1 4 5 5 4-1M1 15l5-5",
  globe:
    "M14 8A6 6 0 1 1 2 8a6 6 0 0 1 12 0M2 8h12M8 2c-4 4-4 8 0 12 4-4 4-8 0-12",
  history: "M2 7a6 6 0 1 1 1 5M1 3v4h4M8 4v4l3 2",
  check: "m3 8 3 3 7-7",
  info: "M14 8A6 6 0 1 1 2 8a6 6 0 0 1 12 0M8 7.5v3.25M8 5.25h.01",
  accessibility: "M8 1.9a1.4 1.4 0 1 1 0 2.8 1.4 1.4 0 0 1 0-2.8M3.3 7.15h9.4M8 6.2v3.5M5.25 14.25 8 9.7l2.75 4.55",
  warning: "M7.1 2.5a1 1 0 0 1 1.8 0l5.7 10a1 1 0 0 1-.9 1.5H2.3a1 1 0 0 1-.9-1.5zM8 6v3m0 2.5v.1",
  minimize: "M3 8h10",
  maximize: "M3 3h10v10H3z",
  expand: "M2 6V2h4M10 14h4v-4M2 2l4 4M14 14l-4-4",
  contract: "M2 2l4 4M2 6h4V2M14 14l-4-4M14 10h-4v4",
  swap: "M2 5h12m-3-3 3 3-3 3M14 11H2m3-3-3 3 3 3",
  restore: "M5 2h9v9M2 5h9v9H2z",
  arrow: "M3 8h10M9 4l4 4-4 4",
  chevron: "m5 6 3 3 3-3",
  chevronUp: "m4 10 4-4 4 4",
  chevronDown: "m4 6 4 4 4-4",
  minus: "M3 8h10",
  speaker:
    "M2 6h3l4-3v10l-4-3H2zM11 5.5a3.5 3.5 0 0 1 0 5M12.5 3.5a6 6 0 0 1 0 9",
  speakerMuted: "M2 6h3l4-3v10l-4-3H2zM10.5 6l4 4M14.5 6l-4 4",
  edit: "M2 14l1-3 8-8 2 2-8 8-3 1M9 5l2 2",
  copy: "M5 5h8v8H5zM3 3h8v2M3 3v8h2",
  duplicate: "M2 2h6v6H2zM8 8h6v6H8zM10 2h4v4",
  download: "M8 2v9M4.5 7.5 8 11l3.5-3.5M2 14h12",
  savePage: "M3 2h7l3 3v9H3zM10 2v3h3M8 7v4m-2-2 2 2 2-2",
  print: "M4 6V2h8v4M4 11H2V6h12v5h-2M4 9h8v5H4zM4 7.5h.01",
  folder: "M2 4h4l1.5 2H14v8H2z",
  pause: "M5 3v10M11 3v10",
  play: "M5 3v10l8-5z",
  stop: "M4 4h8v8H4z",
  more: "M4 8a1 1 0 1 1-2 0 1 1 0 0 1 2 0M9 8a1 1 0 1 1-2 0 1 1 0 0 1 2 0M14 8a1 1 0 1 1-2 0 1 1 0 0 1 2 0",
  mediaPictureInPicture: "M8 13H2a1 1 0 0 1-1-1V3a1 1 0 0 1 1-1h11a1 1 0 0 1 1 1v4M4 5l3 3M5 8h2V6",
  mediaVolume: "M11 5.5a3.5 3.5 0 0 1 0 5M13 4a5.5 5.5 0 0 1 0 8",
  mediaMuted: "m11 6 4 4m0-4-4 4",
};
const fills: Record<string, string> = {
  mediaPrevious: "M2 3a.7.7 0 1 1 1.4 0v10a.7.7 0 0 1-1.4 0zM14 3.9v8.2a1 1 0 0 1-1.5.86L4.8 8.86a1 1 0 0 1 0-1.72l7.7-4.1a1 1 0 0 1 1.5.86z",
  mediaNext: "M14 3a.7.7 0 1 0-1.4 0v10a.7.7 0 1 0 1.4 0zM2 3.9v8.2a1 1 0 0 0 1.5.86l7.7-4.1a1 1 0 0 0 0-1.72L3.5 3.04A1 1 0 0 0 2 3.9z",
  mediaPlay: "M4 2.9v10.2a1.2 1.2 0 0 0 1.8 1L14 9a1.2 1.2 0 0 0 0-2L5.8 1.9A1.2 1.2 0 0 0 4 2.9z",
  mediaPause: "M2 3.8a1.8 1.8 0 0 1 1.8-1.8h.8a1.8 1.8 0 0 1 1.8 1.8v8.4A1.8 1.8 0 0 1 4.6 14h-.8A1.8 1.8 0 0 1 2 12.2zM9.6 3.8A1.8 1.8 0 0 1 11.4 2h.8A1.8 1.8 0 0 1 14 3.8v8.4a1.8 1.8 0 0 1-1.8 1.8h-.8a1.8 1.8 0 0 1-1.8-1.8z",
  mediaVolume: "M2.2 5h2L8 2.5a1 1 0 0 1 1.6.8v9.4a1 1 0 0 1-1.6.8L4.2 11h-2A1.7 1.7 0 0 1 .5 9.3V6.7A1.7 1.7 0 0 1 2.2 5z",
  mediaPictureInPicture: "M10 9h5a1 1 0 0 1 1 1v3a1 1 0 0 1-1 1h-5a1 1 0 0 1-1-1v-3a1 1 0 0 1 1-1z",
  musicNote: "M7 2h1c.5 1.7 2 2.9 4 3.2v1.5a6 6 0 0 1-3.5-1.5v6.3c0 1.4-1.2 2.5-2.8 2.5S3 13.2 3 12c0-1.4 1.2-2.4 2.8-2.4.4 0 .8 0 1.2.2z",
  musicNotes: "M5 3.5 14 2v8.7c0 1.3-1.1 2.3-2.6 2.3S9 12.3 9 11.2c0-1.3 1.1-2.2 2.6-2.2.3 0 .6 0 .9.1V5L6.5 6v6.1c0 1.3-1.1 2.4-2.6 2.4S1.5 13.7 1.5 12.6c0-1.3 1.1-2.2 2.6-2.2.3 0 .6 0 .9.1z",
};
fills.mediaMuted = fills.mediaVolume;
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
    <path v-if="fills[name]" :d="fills[name]" fill="currentColor" stroke="none" />
    <path v-if="paths[name] || !fills[name]" :d="paths[name] || paths.globe" />
  </svg>
</template>
