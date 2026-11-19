<script setup lang="ts">
import TabList from "./TabList.vue";
import type { Tab } from "../../lib/types";
import type { TabRow } from "../../lib/tab-rows";
defineProps<{ rows: TabRow[]; doomedIds?: string[] }>();
const emit = defineEmits<{ menu: [event: MouseEvent, tab: Tab] }>();
</script>
<template>
  <TabList
    class="pinned-tabs"
    :rows="rows"
    :doomed-ids="doomedIds"
    aria-label="固定标签页"
    @menu="(e, t) => emit('menu', e, t)"
  />
</template>
