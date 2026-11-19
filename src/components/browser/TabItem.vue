<script setup lang="ts">
import { computed } from "vue";
import { useBrowser } from "../../stores/browser";
import type { Tab } from "../../lib/types";
import { samePinnedUrl } from "../../lib/bookmarks";
import { isSettingsUrl } from "../../lib/settings";
import { tabDisplayTitle, tabFreshness } from "../../lib/tab-meta";
import Favicon from "../common/Favicon.vue";
import Icon from "../common/Icon.vue";

const props = defineProps<{ tab: Tab; panes?: [Tab, Tab]; doomedIds?: string[] }>();
const emit = defineEmits<{ menu: [event: MouseEvent, tab: Tab] }>();
const b = useBrowser();
const primary = computed(() => props.panes?.[0] ?? props.tab);
const previewed = computed(() => b.previewTarget === (props.tab.bookmarkId || props.tab.id));
const labels = computed(() => (props.panes ?? [props.tab]).map((tab) => {
  const sleeping = Boolean(b.asleep[tab.id]);
  return {
    tab,
    title: tabDisplayTitle(tab),
    tooltip: sleeping ? `已休眠以节省内存 · ${tab.url}` : tab.url,
    sleeping,
    audio: Boolean(b.audio[tab.id] || tab.muted),
    doomed: props.doomedIds?.includes(tab.id),
    fade: tabFreshness(tab.lastActivated, b.now),
  };
}));
const selected = computed(() => labels.value.some(({ tab }) => tab.id === b.state.activeTab));
const sleeping = computed(() => labels.value.some((label) => label.sleeping));
const title = computed(() => labels.value.map((label) => label.title).join(" / "));
const rowTitle = computed(() => props.panes
  ? labels.value.map((label) => `${label.title} · ${label.tooltip}`).join("\n")
  : labels.value[0].tooltip);
const away = computed(() => {
  const { bookmarkId, unloaded, url } = primary.value;
  const bookmark = bookmarkId && b.savedBookmarks.get(bookmarkId);
  return Boolean(bookmark && !unloaded && !samePinnedUrl(bookmark.url, url));
});
const closeLabel = computed(() => away.value ? "返回固定网址" : primary.value.bookmarkId ? "卸载" : "关闭");
function unload(tab: Tab) {
  b.send(tab.bookmarkId ? { type: "bookmarkUnload", id: tab.bookmarkId } : { type: "close", id: tab.id });
}
function close() {
  if (away.value) b.send({ type: "bookmarkReset", id: primary.value.bookmarkId });
  else unload(primary.value);
}
function activate() {
  const tab = primary.value;
  if (tab.bookmarkId && (!props.panes || tab.unloaded)) b.send({ type: "bookmarkOpen", id: tab.bookmarkId });
  else b.send({ type: "activate", id: tab.id });
}
</script>

<template>
  <div
    class="tab-row"
    :data-tab-id="tab.id"
    :data-bookmark-id="tab.bookmarkId"
    :data-sleeping="sleeping || undefined"
    :class="{
      selected,
      'tab-row-split': panes,
      unloaded: primary.unloaded,
      sleeping,
      doomed: labels.every((label) => label.doomed),
    }"
    role="tab"
    :aria-label="panes ? `分屏：${title}` : title"
    :aria-selected="selected"
    :tabindex="selected ? 0 : -1"
    :title="previewed ? undefined : rowTitle"
    :aria-describedby="previewed ? 'tab-hover-preview' : undefined"
    draggable="true"
    @mousedown.middle.prevent
    @auxclick.middle.prevent.stop="unload(primary)"
    @click="activate"
    @keydown.enter.self.prevent="activate"
    @contextmenu.prevent="emit('menu', $event, primary)"
  >
    <span
      v-for="label in labels"
      :key="label.tab.bookmarkId || label.tab.id"
      class="tab-label"
      :class="{ sleeping: label.sleeping, loading: label.tab.loading, unloaded: label.tab.unloaded, doomed: label.doomed }"
      :style="{ '--tab-freshness': label.fade }"
      @auxclick.middle.prevent.stop="unload(label.tab)"
      @contextmenu.prevent.stop="emit('menu', $event, label.tab)"
    >
      <span class="tab-picture">
        <Icon v-if="isSettingsUrl(label.tab.url)" name="settings" />
        <Favicon v-else :url="label.tab.url" :favicon="label.tab.favicon" />
      </span>
      <span class="tab-title">{{ label.title }}</span>
      <button
        v-if="label.audio"
        class="tab-audio"
        :class="{ muted: label.tab.muted }"
        :aria-label="`${label.tab.muted ? '取消静音' : '静音'} ${label.title}`"
        :title="label.tab.muted ? '取消静音标签页' : '静音标签页'"
        @click.stop="b.toggleMute(label.tab)"
      >
        <Icon :name="label.tab.muted ? 'speakerMuted' : 'speaker'" :size="12" />
      </button>
    </span>
    <button
      class="tab-close"
      :aria-label="`${closeLabel} ${tabDisplayTitle(primary)}`"
      :title="away ? closeLabel : primary.bookmarkId ? '卸载网页（保留固定项）' : `关闭 ${tabDisplayTitle(primary)}`"
      :disabled="primary.unloaded"
      @click.stop="close"
    >
      <Icon :name="away ? 'back' : 'close'" :size="11" />
    </button>
  </div>
</template>
