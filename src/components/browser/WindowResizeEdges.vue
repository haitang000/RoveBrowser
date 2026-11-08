<script setup lang="ts">
import { useBrowser } from "../../stores/browser";
const b = useBrowser();
</script>
<template>
  <div
    v-for="edge in ['n', 's', 'e', 'w', 'ne', 'nw', 'se', 'sw']"
    :key="edge"
    class="resize-edge"
    :class="`resize-${edge}`"
    aria-hidden="true"
    @pointerdown.prevent="b.send({ type: 'window', action: `resize-${edge}` })"
  />
</template>
<style scoped>
.resize-edge {
  position: fixed;
  z-index: 200;
  touch-action: none;
}
.resize-n,
.resize-s {
  left: 7px;
  right: 7px;
  height: 4px;
  cursor: ns-resize;
}
.resize-n {
  top: 0;
}
.resize-s {
  bottom: 0;
}
.resize-e,
.resize-w {
  top: 7px;
  bottom: 7px;
  width: 4px;
  cursor: ew-resize;
}
.resize-e {
  right: 0;
}
.resize-w {
  left: 0;
}
.resize-ne,
.resize-nw,
.resize-se,
.resize-sw {
  width: 7px;
  height: 7px;
}
.resize-ne,
.resize-nw {
  top: 0;
}
.resize-se,
.resize-sw {
  bottom: 0;
}
.resize-ne,
.resize-se {
  right: 0;
}
.resize-nw,
.resize-sw {
  left: 0;
}
.resize-ne,
.resize-sw {
  cursor: nesw-resize;
}
.resize-nw,
.resize-se {
  cursor: nwse-resize;
}
</style>
