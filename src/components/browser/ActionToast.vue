<script setup lang="ts">
// Bottom bar for Zigzag-style batch actions: Clear/Close-others confirmations with
// an Undo button, and small hints such as "link copied". It is owned by the
// trusted popup layer so the native window can clip it to the page area.
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
const motion = computed(() => (calm.value ? 80 : { enter: 160, leave: 120 }));
function undo() {
  b.send({ type: "undoBatch" });
  b.dismissToast();
}
</script>
<template>
  <Transition name="toast-bar" :duration="motion">
    <div v-if="b.actionToast" class="action-toast" role="status" aria-live="polite">
      <div class="action-toast-surface">
        <span class="action-toast-message">{{ b.actionToast.message }}</span>
        <button
          v-if="b.actionToast.action === 'undoBatch'"
          class="action-toast-undo"
          @click="undo"
        >
          撤销
        </button>
      </div>
    </div>
  </Transition>
</template>
