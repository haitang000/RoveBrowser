<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useBrowser } from "../../stores/browser";
import { liveTab } from "../../lib/bookmarks";
import type { Tab } from "../../lib/types";
import Icon from "../common/Icon.vue";
import PinnedTabs from "./PinnedTabs.vue";
import TabList from "./TabList.vue";
import { tabIsFaded } from "../../lib/tab-meta";

const props = defineProps<{ spaceId: string; savedScroll: number }>();
const emit = defineEmits<{ menu: [event: MouseEvent, tab: Tab]; scroll: [id: string, top: number] }>();
const b = useBrowser(), list = ref<HTMLElement>();
const space = computed(() => b.state.spaces.find((s) => s.id === props.spaceId));
const tabs = computed(() => b.state.tabs.filter((t) => t.spaceId === props.spaceId && !t.pinned && !b.state.favorites.some((f) => f.id === t.bookmarkId)));
const pins = computed<Tab[]>(() => (b.state.pins || []).filter((f) => f.spaceId === props.spaceId)
  .map((f) => ({ ...(liveTab(b.state, f.id) || { id: f.id, spaceId: props.spaceId, url: f.url, unloaded: true }), title: f.title, favicon: liveTab(b.state, f.id)?.favicon || f.favicon, pinned: true, bookmarkId: f.id })));
onMounted(() => { if (list.value) list.value.scrollTop = props.savedScroll; });
onBeforeUnmount(() => { if (list.value) emit("scroll", props.spaceId, list.value.scrollTop); });

// Zigzag's Clear button: step one closes faded tabs, step two the other tabs,
// step three the tab being looked at and its split partner. This copy only
// previews the step; the host owns the actual close and its Undo batch.
const clearPlan = computed(() => {
  const current = tabs.value.find((tab) => tab.id === b.state.activeTab);
  const partner = current
    ? current.splitWith || b.state.tabs.find((tab) => tab.splitWith === current.id)?.id
    : undefined;
  const others = tabs.value.filter((tab) => tab.id !== current?.id && tab.id !== partner);
  const faded = others.filter((tab) => tabIsFaded(tab.lastActivated, b.now) && !b.audio[tab.id]);
  if (faded.length) return { ids: faded.map((tab) => tab.id), faded: true };
  if (others.length) return { ids: others.map((tab) => tab.id), faded: false };
  const ids = current
    ? [current.id, ...(partner && tabs.value.some((tab) => tab.id === partner) ? [partner] : [])]
    : [];
  return { ids, faded: false };
});
// Clear only arms after it has been visible for a moment, so brushing the tab
// list never wipes it (Zigzag rearms after each step too).
const previewing = ref(false), armed = ref(false);
let armTimer: ReturnType<typeof setTimeout> | undefined;
function arm() {
  clearTimeout(armTimer);
  armed.value = false;
  armTimer = setTimeout(() => (armed.value = true), 350);
}
/** Focus can arrive between hover and click; it must not disarm that click. */
function focusClear() {
  if (!armed.value) arm();
  previewing.value = true;
}
function disarm() {
  clearTimeout(armTimer);
  armed.value = false;
  previewing.value = false;
}
function stepClear() {
  if (!armed.value) return;
  b.send({ type: "clearStep" });
  arm();
}
// For a few seconds after a batch close the same spot offers Zigzag's Undo
// instead of Clear, so the action and its reversal live in one place.
const undoActive = computed(() => b.undoAvailable);
const clearLabel = computed(() =>
  undoActive.value ? "Undo" : clearPlan.value.faded ? "Clear Old" : "Clear",
);
watch(undoActive, (active) => {
  if (active) {
    clearTimeout(armTimer);
    armed.value = false;
    previewing.value = false;
  }
});
function enterClear() {
  if (undoActive.value) return;
  arm();
  previewing.value = true;
}
function clickClear() {
  if (undoActive.value) {
    b.undoBatch();
    disarm();
    return;
  }
  stepClear();
}
</script>

<template>
  <section ref="list" class="space-list" :data-space-id="spaceId" :aria-label="space?.name" @scroll="emit('scroll', spaceId, list!.scrollTop)">
    <PinnedTabs :tabs="pins" :space-id="spaceId" @menu="(e, t) => emit('menu', e, t)" />
    <div class="section-label">
      <span>Tabs</span>
      <span class="section-label-end">
        <button
          v-if="tabs.length || undoActive"
          class="clear-tabs"
          :class="{ armed: armed && !undoActive, undo: undoActive }"
          :aria-label="undoActive ? '撤销刚才关闭的标签页' : clearPlan.faded ? `关闭 ${clearPlan.ids.length} 个旧标签页` : `关闭 ${clearPlan.ids.length} 个标签页`"
          :title="undoActive ? '撤销刚才的批量关闭（4 秒内）' : clearPlan.faded ? '先关闭很久没看的标签页；再次点击关闭其余' : '关闭其他标签页；再次点击关闭当前页'"
          @pointerenter="enterClear"
          @pointerleave="disarm()"
          @focus="focusClear"
          @blur="disarm()"
          @click="clickClear"
        >
          <Icon :name="undoActive ? 'history' : 'chevronDown'" :size="11" />
          {{ clearLabel }}
        </button>
        <span class="tab-count">{{ tabs.length }}</span>
      </span>
    </div>
    <button class="new-tab" @click="b.openCommand()"><Icon name="plus" :size="16" /><span>New Tab</span><kbd v-if="b.shortcutLabel('new')">{{ b.shortcutLabel('new') }}</kbd></button>
    <TabList :tabs="tabs" :doomed-ids="previewing && !undoActive ? clearPlan.ids : []" aria-label="标签页" @menu="(e, t) => emit('menu', e, t)" />
  </section>
</template>
