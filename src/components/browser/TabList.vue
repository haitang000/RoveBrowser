<script setup lang="ts">
import type { PinFolder, Tab } from "../../lib/types";
import TabItem from "./TabItem.vue";
import { computed, ref } from "vue";
import { useTabDrag } from "../../lib/tab-drag";
import type { TabRow } from "../../lib/tab-rows";

const props = defineProps<{ rows: TabRow[]; folders?: PinFolder[]; order?: string[]; doomedIds?: string[] }>();
const emit = defineEmits<{ menu: [event: MouseEvent, tab: Tab] }>();
const group = ref<{ $el: HTMLElement }>();
const drag = useTabDrag();
const keyOf = (tab: Tab) => tab.bookmarkId || tab.id;
const sourceHere = computed(() => drag?.state.value?.source === group.value?.$el);
const sourceAway = computed(() => sourceHere.value && drag?.state.value?.target && drag.state.value.target !== group.value?.$el);
const entries = computed(() => {
  const state = drag?.state.value;
  const rows: { key: string; row?: TabRow; folder?: PinFolder }[] = props.rows.map((row) => ({ key: keyOf(row.tab), row }));
  if (props.order) {
    const ordered = new Map([...rows, ...(props.folders ?? []).map((folder) => ({ key: folder.id, folder }))].map((entry) => [entry.key, entry]));
    rows.splice(0, rows.length, ...props.order.flatMap((id) => {
      const entry = ordered.get(id);
      ordered.delete(id);
      return entry ? [entry] : [];
    }), ...ordered.values());
  }
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
  >
    <template v-for="entry in entries" :key="entry.key">
      <TabItem
        v-if="entry.row"
        v-bind="entry.row"
        :class="{
          'tab-drag-source': sourceHere && entry.key === drag?.state.value?.key && (drag.state.value.dragging || sourceAway),
          'tab-drag-away': sourceAway && entry.key === drag?.state.value?.key,
        }"
        :doomed-ids="doomedIds"
        @menu="(event, tab) => emit('menu', event, tab)"
      />
      <div v-else-if="entry.folder" class="pin-folder" :data-folder-id="entry.key"
        :class="{ 'pin-folder-drag-source': sourceHere && entry.key === drag?.state.value?.key && drag.state.value.dragging }">
        <slot name="folder" :folder="entry.folder" />
      </div>
      <div v-else class="tab-drag-gap" aria-hidden="true" />
    </template>
  </TransitionGroup>
</template>
