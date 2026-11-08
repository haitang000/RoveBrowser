<script setup lang="ts">
import { useBrowser } from "../../stores/browser";
import type { Tab } from "../../lib/types";
import Favicon from "../common/Favicon.vue";
import Icon from "../common/Icon.vue";
import { savedPage, samePinnedUrl } from "../../lib/bookmarks";
const props = defineProps<{ tab: Tab }>();
const emit = defineEmits<{ menu: [event: MouseEvent, tab: Tab] }>();
const b = useBrowser();
function away() {
  const f = props.tab.bookmarkId && savedPage(b.state, props.tab.bookmarkId);
  return f && !props.tab.unloaded && !samePinnedUrl(f.url, props.tab.url);
}
function close() {
  b.send(away() ? { type: "bookmarkReset", id: props.tab.bookmarkId } : props.tab.bookmarkId ? { type: "bookmarkUnload", id: props.tab.bookmarkId } : { type: "close", id: props.tab.id });
}
function activate() { b.send({ type: props.tab.bookmarkId ? "bookmarkOpen" : "activate", id: props.tab.bookmarkId || props.tab.id }); }
function unload() { b.send(props.tab.bookmarkId ? { type: "bookmarkUnload", id: props.tab.bookmarkId } : { type: "close", id: props.tab.id }); }
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
    :data-bookmark-id="tab.bookmarkId"
    :class="{ selected: b.state.activeTab === tab.id, loading: tab.loading, unloaded: tab.unloaded }"
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
    @auxclick.middle.prevent.stop="unload"
    @click="activate"
    @keydown.enter="activate"
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
      :aria-label="`${away() ? '返回固定网址' : tab.pinned ? '卸载' : '关闭'} ${tab.title}`"
      :title="away() ? '返回固定网址' : tab.pinned ? '卸载网页（保留固定项）' : '关闭标签页'"
      :disabled="tab.unloaded"
      @click.stop="close"
    >
      <Icon :name="away() ? 'back' : 'close'" :size="11" />
    </button>
  </div>
</template>
