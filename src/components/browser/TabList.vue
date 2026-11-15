<script setup lang="ts">
import type { Tab } from "../../lib/types";
import TabItem from "./TabItem.vue";
import { computed, ref } from "vue";
import { useTabDrag } from "../../lib/tab-drag";

const props = defineProps<{ tabs: Tab[]; doomedIds?: string[] }>();
const emit = defineEmits<{ menu: [event: MouseEvent, tab: Tab] }>();
const group = ref<{ $el: HTMLElement }>();
const drag = useTabDrag();
const keyOf = (tab: Tab) => tab.bookmarkId || tab.id;
const sourceHere = computed(() => drag?.state.value?.source === group.value?.$el);
const sourceAway = computed(() => sourceHere.value && drag?.state.value?.target && drag.state.value.target !== group.value?.$el);
const entries = computed(() => {
  const state = drag?.state.value;
  const rows: { key: string; tab?: Tab }[] = props.tabs.map((tab) => ({ key: keyOf(tab), tab }));
  if (!state || state.target !== group.value?.$el) return rows;
  const source = rows.findIndex((row) => row.key === state.key);
  const moving = source >= 0 ? rows.splice(source, 1)[0]! : { key: "tab-drag-gap" };
  rows.splice(state.index, 0, moving);
  return rows;
});

function beforeLeave(element: Element) {
  const row = element as HTMLElement;
  // Keep the outgoing row in place while its neighbours fill the gap.
  Object.assign(row.style, {
    width: `${row.offsetWidth}px`,
    left: `${row.offsetLeft}px`,
    top: `${row.offsetTop}px`,
  });
  row.inert = true;
  row.setAttribute("aria-hidden", "true");
}
</script>

<template>
  <TransitionGroup
    ref="group"
    tag="div"
    name="tab"
    class="tab-list"
    role="tablist"
    @before-leave="beforeLeave"
    @dragenter="drag?.over($event, tabs)"
    @dragover="drag?.over($event, tabs)"
    @drop.stop="drag?.drop($event, tabs)"
  >
    <template v-for="entry in entries" :key="entry.key">
      <TabItem
        v-if="entry.tab"
        :tab="entry.tab"
        :class="{
          'tab-drag-source': sourceHere && entry.key === drag?.state.value?.key && (drag.state.value.dragging || sourceAway),
          'tab-drag-away': sourceAway && entry.key === drag?.state.value?.key,
        }"
        :doomed="doomedIds?.includes(entry.tab.id)"
        @menu="(event, tab) => emit('menu', event, tab)"
      />
      <div v-else class="tab-drag-gap" aria-hidden="true" />
    </template>
  </TransitionGroup>
</template>
