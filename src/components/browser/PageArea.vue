<script setup lang="ts">
import {
  computed,
  onBeforeUnmount,
  onMounted,
  ref,
  watch,
} from "vue";
import { useBrowser } from "../../stores/browser";
import { host } from "../../lib/address";
import { pageClip } from "../../lib/page-clip";
import Icon from "../common/Icon.vue";
import Peek from "./Peek.vue";
import SettingsPage from "./SettingsPage.vue";
import { isSettingsUrl } from "../../lib/settings";
import { pageDirection, type PageDirection } from "../../lib/page-motion";
const b = useBrowser(),
  primary = ref<HTMLElement>(),
  secondary = ref<HTMLElement>(),
  container = ref<HTMLElement>(),
  ratio = ref(0.5),
  resizing = ref(false),
  drop = ref(false);
const other = computed(() =>
  b.state.tabs.find((t) => t.id === b.active?.splitWith),
);
let ro: ResizeObserver;
let finishResize: (() => void) | undefined;
// Zigzag's native ResizeHandle owns the real frames while dragging. This event
// only keeps the shell headers and fallback content in step with native input.
watch(() => b.splitResize, (update) => {
  if (!update) return;
  resizing.value = update.resizing;
  ratio.value = update.ratio;
});
watch([() => b.active?.id, () => b.active?.splitWith], () => {
  finishResize?.();
  resizing.value = false;
});
const primaryDir = ref<PageDirection>(0);
const secondaryDir = ref<PageDirection>(0);
const spaceTabs = () => b.state.tabs.filter((tab) => tab.spaceId === b.space.id);
function moveDirection(
  next: string,
  prev: string | undefined,
  tabs: { id: string }[],
) {
  const [space, id, url] = next.split("\u0000");
  const [prevSpace, prevId, prevUrl] = (prev ?? next).split("\u0000");
  if (id !== prevId || space !== prevSpace)
    return pageDirection(tabs, prevId, id, space !== prevSpace);
  return url === prevUrl ? undefined : 0;
}
watch(
  () => [b.space.id, b.active?.id ?? "", b.active?.url ?? ""].join("\u0000"),
  (next, prev) => {
    const dir = moveDirection(next, prev, spaceTabs());
    if (dir !== undefined) primaryDir.value = dir;
  },
);
watch(
  () => [b.space.id, other.value?.id ?? "", other.value?.url ?? ""].join("\u0000"),
  (next, prev) => {
    const dir = moveDirection(next, prev, spaceTabs());
    if (dir !== undefined) secondaryDir.value = dir;
  },
);
function layout() {
  if (!primary.value || !container.value) return;
  const card = container.value.getBoundingClientRect();
  const style = getComputedStyle(container.value);
  const popup = { x: card.x + container.value.clientLeft, y: card.y + container.value.clientTop, width: container.value.clientWidth, height: container.value.clientHeight };
  if (popup.x !== b.popupArea.x || popup.y !== b.popupArea.y || popup.width !== b.popupArea.width || popup.height !== b.popupArea.height)
    b.popupArea = popup;
  const rect = (e: HTMLElement) => {
    const r = e.getBoundingClientRect();
    return pageClip(r, card, style);
  };
  // ResizeObserver runs after layout and before paint. Report this frame's
  // geometry now; another rAF leaves native pages one frame behind the sidebar.
  b.send({
    type: "layout",
    primary: rect(primary.value),
    secondary: secondary.value ? rect(secondary.value) : null,
    liveResize: resizing.value || b.sidebarResizing,
  });
}
watch(
  [
    () => b.active?.id,
    () => b.active?.splitWith,
    () => b.state.sidebarCollapsed,
    () => b.sidebarResizing,
    ratio,
    resizing,
  ],
  () => { if (!resizing.value) layout(); },
  { flush: "post" },
);
onMounted(() => {
  ro = new ResizeObserver(layout);
  if (container.value) ro.observe(container.value);
  layout();
  window.addEventListener("resize", layout);
});
onBeforeUnmount(() => {
  finishResize?.();
  ro?.disconnect();
  window.removeEventListener("resize", layout);
});
function resize(e: PointerEvent) {
  if (e.button !== 0) return;
  e.preventDefault();
  const handle = e.currentTarget as HTMLElement;
  finishResize?.();
  const grabOffset = e.clientX - handle.getBoundingClientRect().x;
  const area = container.value!;
  const card = area.getBoundingClientRect();
  const primaryRect = primary.value!.getBoundingClientRect();
  const secondaryRect = secondary.value!.getBoundingClientRect();
  const gap = handle.offsetWidth;
  const width = area.clientWidth - gap;
  const rect = (r: DOMRect) => ({ x: r.x, y: r.y, width: r.width, height: r.height, corners: [0, 0, 0, 0] });
  handle.setPointerCapture(e.pointerId);
  resizing.value = true;
  layout();
  // Pointer capture keeps dragging over native pages without covering them.
  // Only the live viewport bounds change; no overlay or thumbnail is needed.
  handle.onpointermove = (ev) => {
    if (width <= 0) return;
    const floor = Math.max(0.2, 260 / width), ceiling = Math.min(0.8, 1 - floor);
    const next = (ev.clientX - card.x - area.clientLeft - grabOffset) / width;
    // Match Zigzag's minimumPaneWidth and floor the shared edge once.
    ratio.value = Math.floor(width * (floor < ceiling ? Math.max(floor, Math.min(ceiling, next)) : 0.5)) / width;
    area.style.setProperty('--split-ratio', String(ratio.value));
    const leftWidth = width * ratio.value;
    // Resize real controllers in the input event, before waiting for Vue/layout
    // or a paint callback. Native keeps only the latest pending dimensions.
    b.send({ type: 'layout', liveResize: true,
      primary: { ...rect(primaryRect), width: leftWidth },
      secondary: { ...rect(secondaryRect), x: primaryRect.x + leftWidth + gap, width: width - leftWidth },
    });
  };
  const finish = () => {
    finishResize = undefined;
    resizing.value = false;
    handle.onpointermove = null;
    handle.onpointerup = null;
    handle.onpointercancel = null;
    handle.onlostpointercapture = null;
    if (handle.hasPointerCapture(e.pointerId)) handle.releasePointerCapture(e.pointerId);
  };
  finishResize = finish;
  handle.onpointerup = finish;
  handle.onpointercancel = finish;
  handle.onlostpointercapture = finish;
}
function split(e: DragEvent) {
  drop.value = false;
  const id = e.dataTransfer?.getData("text/era-tab");
  if (id) b.send({ type: "split", id: b.state.activeTab, other: id });
  b.dragging = false;
}
</script>
<template>
  <main
    ref="container"
    class="page-area"
    :class="{ split: other, dropping: drop, resizing: resizing || b.sidebarResizing }"
    :style="{ '--split-ratio': ratio }"
    @dragover.prevent="drop = true"
    @dragleave.self="drop = false"
    @drop.prevent="split"
  >
    <div class="page-pane">
      <div v-if="other" class="pane-header">
        <span>{{ host(b.active?.url || "") || "New Tab" }}</span
        ><button
          class="icon-button"
          title="固定分屏组合"
          aria-label="固定分屏组合"
          @click="!b.active?.pinned && b.send({ type: 'pin' })"
        >
          <Icon name="pin" :size="13" />
        </button>
      </div>
      <div ref="primary" class="webview-slot" :data-page-dir="primaryDir">
        <Transition name="page">
        <SettingsPage v-if="b.active && isSettingsUrl(b.active.url)" :key="`settings:${b.active.id}`" :tab-id="b.active.id" :url="b.active.url" />
        <div
          v-else-if="b.active?.url === 'era://newtab' || !b.active"
          :key="`newtab:${b.active?.id || 'empty'}`"
          class="new-tab-page"
        >
          <div class="new-tab-content">
            <span class="rove-wordmark">Rove</span>
            <p>A little room to explore.</p>
            <button class="start-search" @click="b.openCommand()">
              <Icon name="search" /><span>Search or enter address</span
              ><kbd v-if="b.shortcutLabel('address')">{{ b.shortcutLabel('address') }}</kbd>
            </button>
          </div>
          <div class="new-tab-hints">
            <span v-if="b.shortcutLabel('new')"><kbd>{{ b.shortcutLabel('new') }}</kbd> New tab</span
            ><span v-if="b.shortcutLabel('desk')"><kbd>{{ b.shortcutLabel('desk') }}</kbd> All tabs</span>
          </div>
        </div>
        <div v-else-if="!b.native" :key="`preview:${b.active.id}:${b.active.url}`" class="preview-page">
          <Icon name="globe" :size="28" />
          <h2>{{ b.active.title }}</h2>
          <p>在 Windows 应用中打开真实网页</p>
          <small>{{ b.active.url }}</small>
        </div>
        </Transition>
      </div>
    </div>
    <template v-if="other"
      ><div
        class="split-divider"
        role="separator"
        aria-label="调整分屏比例"
        aria-orientation="vertical"
        :aria-valuenow="Math.round(ratio * 100)"
        tabindex="0"
        @pointerdown="resize"
        @keydown.left.prevent="ratio = Math.max(0.2, ratio - 0.05)"
        @keydown.right.prevent="ratio = Math.min(0.8, ratio + 0.05)"
      />
      <div class="page-pane">
        <div class="pane-header">
          <span>{{ host(other.url) || "New Tab" }}</span
          ><button
            class="icon-button"
            aria-label="关闭分屏"
            title="关闭分屏"
            @click="b.send({ type: 'unsplit' })"
          >
            <Icon name="close" :size="13" />
          </button>
        </div>
        <div ref="secondary" class="webview-slot" :data-page-dir="secondaryDir">
          <Transition name="page">
          <SettingsPage v-if="isSettingsUrl(other.url)" :key="`settings:${other.id}`" :tab-id="other.id" :url="other.url" />
          <div
            v-else-if="other.url === 'era://newtab' || !b.native"
            :key="`preview:${other.id}:${other.url}`"
            class="preview-page"
          >
            <Icon name="globe" :size="28" />
            <h2>{{ other.title }}</h2>
            <small>{{ other.url }}</small>
          </div>
          </Transition>
        </div>
      </div></template
    >
    <div v-if="b.dragging" class="split-drop-target">
      <Icon name="split" :size="24" /><span>拖到这里，打开左右分屏</span>
    </div>
    <Peek v-if="!b.native" />
    <slot />
  </main>
</template>
