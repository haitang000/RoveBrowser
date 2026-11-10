<script setup lang="ts">
// Zigzag's zoom Toast (BrowserActions.setZoom): a short capsule telling the
// page's zoom after Ctrl+= / Ctrl+- / Ctrl+0. It drops down from the page's
// top edge and slides back up on the way out.
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useBrowser } from "../../stores/browser";
const b = useBrowser();
const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");
const calm = ref(b.settings.reduceMotion || reducedMotion.matches);
function syncCalm() {
  calm.value = b.settings.reduceMotion || reducedMotion.matches;
}
watch(() => b.settings.reduceMotion, syncCalm);
onMounted(() => reducedMotion.addEventListener("change", syncCalm));
onBeforeUnmount(() => reducedMotion.removeEventListener("change", syncCalm));
// 160ms drop-down entrance and 120ms slide-up exit; Reduce Motion keeps a
// quick 80ms fade. The explicit duration unmounts the capsule after the exit.
const motion = computed(() => (calm.value ? 80 : { enter: 160, leave: 120 }));
</script>
<template>
  <Transition name="zoom-toast" :duration="motion">
    <div
      v-if="b.zoomNotice"
      class="status-toast"
      role="status"
      aria-live="polite"
    >
      <div class="status-toast-surface">
        {{ Math.round(b.zoomNotice.factor * 100) }}%
      </div>
    </div>
  </Transition>
</template>
