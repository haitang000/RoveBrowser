<script setup lang="ts">
import { useBrowser } from "../../stores/browser";
import TabList from "./TabList.vue";
import type { Tab } from "../../lib/types";
const b = useBrowser();
const emit = defineEmits<{ menu: [event: MouseEvent, tab: Tab] }>();
</script>
<template>
  <TabList
    class="pinned-tabs"
    :tabs="b.tabs.filter((t) => t.pinned)"
    aria-label="固定标签页"
    @dragover.prevent
    @drop.prevent.self="
      b.send({ type: 'pin', id: $event.dataTransfer?.getData('text/era-tab') });
      b.dragging = false;
    "
    @menu="(e, t) => emit('menu', e, t)"
  />
</template>
