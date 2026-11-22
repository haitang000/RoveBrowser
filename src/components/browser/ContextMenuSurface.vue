<script setup lang="ts">
import { computed, ref, watchPostEffect } from "vue";
import type { ContextMenuAnchor } from "../../lib/context-menu";
import type { Rect } from "../../lib/types";
import { useBrowser } from "../../stores/browser";

const props = defineProps<{ anchor: ContextMenuAnchor }>();
const emit = defineEmits<{ scroll: [event: Event] }>();
const b = useBrowser();
const panel = ref<HTMLElement>();
const position = ref({ x: 0, y: 0 });
const style = computed(() => ({ left: `${position.value.x}px`, top: `${position.value.y}px` }));
defineExpose({ panel });

function backdropStyle(rect: Rect, layer: number) {
  return {
    left: `${rect.x - position.value.x}px`, top: `${rect.y - position.value.y}px`,
    width: `${rect.width}px`, height: `${rect.height}px`, zIndex: layer,
    borderRadius: rect.corners?.map((radius) => `${radius}px`).join(" "),
  };
}

watchPostEffect((cleanup) => {
  const element = panel.value;
  const { x, y, side } = props.anchor;
  const scrim = element?.parentElement;
  if (!element || !scrim) return;

  const place = () => {
    const viewport = scrim.getBoundingClientRect();
    const width = element.offsetWidth;
    const right = viewport.right - 8;
    const left = viewport.left + 8;
    const top = viewport.top + 8;
    let start = x;
    if (side) {
      const rightSpace = right - side.right;
      const leftSpace = side.left - left;
      start = rightSpace >= width - 4 || rightSpace >= leftSpace
        ? side.right - 4 : side.left - width + 4;
    }
    const nextX = Math.max(left, Math.min(start, right - width));
    const nextY = Math.max(top, Math.min(y, viewport.bottom - element.offsetHeight - 8));
    if (position.value.x !== nextX || position.value.y !== nextY) position.value = { x: nextX, y: nextY };
  };
  place();
  const resize = new ResizeObserver(place);
  resize.observe(scrim);
  resize.observe(element);
  cleanup(() => resize.disconnect());
});
</script>

<template>
  <div ref="panel" class="context-menu" :style="style">
    <div v-if="b.contextMenuBackdrops.length" class="context-menu-backdrop" aria-hidden="true">
      <img v-for="{ layer, rect, image } in b.contextMenuBackdrops" :key="layer" :src="image"
        :style="backdropStyle(rect, layer)" alt="" draggable="false" />
    </div>
    <div class="context-menu-scroll" @scroll="emit('scroll', $event)"><slot /></div>
  </div>
</template>
