<script setup lang="ts">
import { useBrowser } from "../../stores/browser";
import type { Tab } from "../../lib/types";
import Favicon from "../common/Favicon.vue";
import Icon from "../common/Icon.vue";
const props = defineProps<{ tab: Tab }>();
const emit = defineEmits<{ menu: [event: MouseEvent, tab: Tab] }>();
const b = useBrowser();
function drag(e: DragEvent, id: string) {
  e.dataTransfer?.setData("text/era-tab", id);
  if (e.dataTransfer) e.dataTransfer.effectAllowed = "move";
  b.dragging = true;
}
function drop(e: DragEvent) {
  const id = e.dataTransfer?.getData("text/era-tab");
  if (id && id !== props.tab.id)
    b.send({ type: "reorder", id, before: props.tab.id });
  b.dragging = false;
}
</script>
<template>
  <div
    class="tab-row"
    :data-tab-id="tab.id"
    :class="{ selected: b.state.activeTab === tab.id, loading: tab.loading }"
    role="tab"
    :aria-selected="b.state.activeTab === tab.id"
    :tabindex="b.state.activeTab === tab.id ? 0 : -1"
    :title="tab.url"
    draggable="true"
    @dragstart="drag($event, tab.id)"
    @dragend="b.dragging = false"
    @dragover.prevent
    @drop.prevent.stop="drop"
    @mousedown.middle.prevent
    @auxclick.middle.prevent.stop="b.send({ type: 'close', id: tab.id })"
    @click="b.send({ type: 'activate', id: tab.id })"
    @keydown.enter="b.send({ type: 'activate', id: tab.id })"
    @contextmenu.prevent="emit('menu', $event, tab)"
  >
    <span class="tab-picture"
      ><Icon v-if="tab.splitWith" name="split" /><Favicon
        v-else
        :url="tab.url"
        :favicon="tab.favicon" /></span
    ><span class="tab-title">{{ tab.title }}</span
    ><button
      class="tab-close"
      :aria-label="`关闭 ${tab.title}`"
      title="关闭标签页"
      @click.stop="b.send({ type: 'close', id: tab.id })"
    >
      <Icon name="close" :size="11" />
    </button>
  </div>
</template>
