<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted } from "vue";
import { useBrowser } from "../../stores/browser";
import Icon from "../common/Icon.vue";

const b = useBrowser();
const controls = computed(() => {
  const anchor = b.splitControls;
  return anchor?.id === b.active?.id && anchor?.other === b.splitPartner?.id ? anchor : undefined;
});
const fullscreen = computed(() => b.state.sidebarCollapsed && b.state.sidebarHidden);
const fullscreenLabel = computed(() => fullscreen.value ? b.t("退出全屏") : b.t("全屏"));

function close() { b.splitControls = undefined; }
function outside(event: PointerEvent) {
  if (controls.value && event.target instanceof Element && !event.target.closest(".split-controls, .split-divider")) close();
}
function swap() {
  const anchor = controls.value;
  if (!anchor) return;
  close();
  b.send({ type: "swapSplit", id: anchor.id, other: anchor.other });
}
function toggleFullscreen() {
  close();
  b.send({ type: "sidebarFocus" });
}
onMounted(() => {
  document.addEventListener("pointerdown", outside, true);
  if (b.popupWindow) window.addEventListener("blur", close);
});
onBeforeUnmount(() => {
  document.removeEventListener("pointerdown", outside, true);
  window.removeEventListener("blur", close);
});
</script>

<template>
  <Teleport to="body">
    <template v-if="controls && b.popupWindow">
      <div class="split-controls-dismiss" aria-hidden="true" :style="{ right: `calc(100% - ${controls.x - controls.gap / 2}px)` }" />
      <div class="split-controls-dismiss" aria-hidden="true" :style="{ left: `${controls.x + controls.gap / 2}px` }" />
    </template>
    <Transition name="split-controls">
      <div v-if="controls" class="split-controls" :style="{ left: `${controls.x}px`, top: `${controls.y}px` }">
        <div class="split-controls-surface" role="group" :aria-label="b.t('分屏操作')">
          <button class="icon-button" type="button" :title="b.t('对调')" :aria-label="b.t('对调')" @click="swap">
            <Icon name="swap" :size="16" />
          </button>
          <button class="icon-button" type="button" :title="fullscreenLabel" :aria-label="fullscreenLabel"
            @click="toggleFullscreen">
            <Icon :name="fullscreen ? 'contract' : 'expand'" :size="16" />
          </button>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>
