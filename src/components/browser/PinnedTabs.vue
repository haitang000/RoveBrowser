<script setup lang="ts">
import { useBrowser } from "../../stores/browser";
import TabList from "./TabList.vue";
import type { Tab } from "../../lib/types";
const b = useBrowser();
defineProps<{ tabs?: Tab[]; spaceId?: string }>();
const emit = defineEmits<{ menu: [event: MouseEvent, tab: Tab] }>();
</script>
<template>
  <TabList
    class="pinned-tabs"
    :tabs="tabs ?? b.pinnedTabs"
    aria-label="固定标签页"
    @dragover.prevent
    @drop.prevent.self="
      b.send({ type: 'bookmarkPin', id: $event.dataTransfer?.getData('text/era-tab'), spaceId: spaceId ?? b.state.activeSpace });
      b.dragging = false;
    "
    @menu="(e, t) => emit('menu', e, t)"
  />
</template>
