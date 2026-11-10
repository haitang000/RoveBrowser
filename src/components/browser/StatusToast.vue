<script setup lang="ts">
// Zigzag's zoom Toast (BrowserActions.setZoom) and Now Playing state surface:
// a short capsule telling the page's zoom after Ctrl+= / Ctrl+- / Ctrl+0.
import { useBrowser } from "../../stores/browser";
const b = useBrowser();
</script>
<template>
  <div v-if="b.zoomNotice" class="status-toast" role="status" aria-live="polite">
    {{ Math.round(b.zoomNotice.factor * 100) }}%
  </div>
</template>
