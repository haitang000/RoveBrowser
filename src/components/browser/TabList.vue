<script setup lang="ts">
import type { Tab } from "../../lib/types";
import TabItem from "./TabItem.vue";

defineProps<{ tabs: Tab[] }>();
const emit = defineEmits<{ menu: [event: MouseEvent, tab: Tab] }>();

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
    tag="div"
    name="tab"
    class="tab-list"
    role="tablist"
    @before-leave="beforeLeave"
  >
    <TabItem
      v-for="tab in tabs"
      :key="tab.bookmarkId || tab.id"
      :tab="tab"
      @menu="(event, tab) => emit('menu', event, tab)"
    />
  </TransitionGroup>
</template>
