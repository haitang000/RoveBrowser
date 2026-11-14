<script setup lang="ts">
import { computed } from "vue";
const props = defineProps<{ filename: string }>();
const type = computed(() => {
  const ext = props.filename.split(".").pop()?.toLowerCase() ?? "";
  if (["zip", "rar", "7z", "gz", "tar"].includes(ext)) return { kind: "archive", label: "ZIP" };
  if (["mp4", "mkv", "mov", "mp3", "wav", "webm"].includes(ext)) return { kind: "media", label: ext.toUpperCase() };
  if (["png", "jpg", "jpeg", "gif", "webp", "svg"].includes(ext)) return { kind: "image", label: "IMG" };
  if (["exe", "msi", "msix"].includes(ext)) return { kind: "app", label: "APP" };
  return { kind: "document", label: ext.length <= 4 && props.filename.includes(".") ? ext.toUpperCase() : "FILE" };
});
</script>
<template>
  <span class="download-icon" :class="type.kind" aria-hidden="true">
    <svg width="30" height="36" viewBox="0 0 30 36" fill="none">
      <path d="M5 1h13l9 9v23a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V3a2 2 0 0 1 2-2Z" fill="currentColor" opacity=".15" />
      <path d="M18 1v7a2 2 0 0 0 2 2h7M5 1h13l9 9v23a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V3a2 2 0 0 1 2-2Z" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round" />
      <path v-if="type.kind === 'archive'" d="M13 2h3v3h-3zm3 3h3v3h-3zm-3 3h3v3h-3zm3 3h3v3h-3z" fill="currentColor" />
      <text x="15" y="26" text-anchor="middle" fill="currentColor" font-size="7.5" font-weight="700">{{ type.label }}</text>
    </svg>
  </span>
</template>
