<script setup lang="ts">
import {
  computed,
  defineAsyncComponent,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  watch,
} from "vue";
import { useBrowser } from "../../stores/browser";
import { pageClip } from "../../lib/page-clip";
import type { Rect } from "../../lib/types";
import Icon from "../common/Icon.vue";
import Peek from "./Peek.vue";
import NewTabPage from "./NewTabPage.vue";
import PageToolbar from "./PageToolbar.vue";
import { isSettingsUrl } from "../../lib/settings";
import { pageDirection, type PageDirection } from "../../lib/page-motion";
const SettingsPage = defineAsyncComponent(() => import("./SettingsPage.vue"));
const b = useBrowser(),
  primary = ref<HTMLElement>(),
  secondary = ref<HTMLElement>(),
  container = ref<HTMLElement>(),
  ratio = ref(0.5),
  resizing = ref(false),
  sidebarMotion = ref(false),
  dropTarget = ref<HTMLElement>();
const other = computed(() => b.splitPartner);
const canDrop = computed(() => b.dragging && !!b.active && !b.spaceFormOpen && !b.peek);
watch([dropTarget, () => dropTarget.value && b.popupArea], ([target]) => {
  if (!b.native) return;
  if (!target) {
    b.send({ type: "tabDropTarget", rect: null });
    return;
  }
  const { x, y, width, height } = target.getBoundingClientRect();
  const radius = parseFloat(getComputedStyle(target).borderTopLeftRadius);
  b.send({ type: "tabDropTarget", rect: { x, y, width, height, corners: [radius, radius, radius, radius] } });
}, { flush: "post" });
let ro: ResizeObserver;
let finishResize: (() => void) | undefined;
let reported: { primary: Rect; secondary: Rect | null; sidebar: Rect | null } | undefined;
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
  // Native sidebar capture owns the HWNDs and renderer viewports. Feedback
  // changes only the shell; sending its intermediate bounds back creates a loop.
  if (b.sidebarResize?.resizing) return;
  if (!primary.value || !container.value) return;
  const card = container.value.getBoundingClientRect();
  const popup = { x: card.x + container.value.clientLeft, y: card.y + container.value.clientTop, width: container.value.clientWidth, height: container.value.clientHeight };
  if (popup.x !== b.popupArea.x || popup.y !== b.popupArea.y || popup.width !== b.popupArea.width || popup.height !== b.popupArea.height)
    b.popupArea = popup;
  const rect = (e: HTMLElement) => {
    const frame = other.value ? e.parentElement! : container.value!;
    return pageClip(e.getBoundingClientRect(), frame.getBoundingClientRect(), getComputedStyle(frame));
  };
  // ResizeObserver runs after layout and before paint. Report this frame's
  // geometry now; another rAF leaves native pages one frame behind the sidebar.
  reported = {
    sidebar: (() => {
      const r = !b.state.sidebarCollapsed && document.querySelector('.sidebar-resize-handle')?.getBoundingClientRect();
      return r ? { x: r.x, y: r.y, width: r.width, height: r.height } : null;
    })(),
    primary: rect(primary.value),
    secondary: secondary.value ? rect(secondary.value) : null,
  };
  b.send({
    type: "layout",
    ...reported,
    viewport: { width: innerWidth, height: innerHeight },
    liveResize: resizing.value || b.sidebarResizing,
    sidebarMotion: sidebarMotion.value,
  });
}
function resizeSidebar(delta: number) {
  if (!reported) return layout();
  const primary = { ...reported.primary, x: reported.primary.x + delta };
  let secondary = reported.secondary;
  if (secondary) {
    const total = primary.width + secondary.width;
    const gap = secondary.x - reported.primary.x - primary.width;
    primary.width = Math.max(0, total - delta) * primary.width / Math.max(1, total);
    secondary = { ...secondary, x: primary.x + primary.width + gap, width: Math.max(0, total - delta) - primary.width };
  } else primary.width = Math.max(0, primary.width - delta);
  primary.corners = [0, 0, 0, 0];
  if (secondary) secondary.corners = [0, 0, 0, 0];
  reported = { primary, secondary, sidebar: reported.sidebar ? { ...reported.sidebar, x: reported.sidebar.x + delta } : null };
  b.send({ type: 'layout', ...reported, liveResize: true, viewport: { width: innerWidth, height: innerHeight } });
}
defineExpose({ layout, resizeSidebar });
let sidebarGeneration = 0;
watch(() => b.state.sidebarCollapsed, () => {
  const ticket = ++sidebarGeneration;
  const track = container.value?.parentElement?.querySelector(".sidebar-track");
  // Reading the running CSS transition also flushes its new target. Retargeting
  // a transition keeps its current width when the shortcut reverses mid-flight.
  const motion = track?.getAnimations().find(animation =>
    animation instanceof CSSTransition && animation.transitionProperty === "width");
  sidebarMotion.value = Boolean(motion);
  layout();
  if (motion) void motion.finished.catch(() => {}).then(() => {
    if (ticket !== sidebarGeneration) return;
    sidebarMotion.value = false;
    layout();
  });
}, { flush: "post" });
watch(
  [
    () => b.active?.id,
    () => b.active?.splitWith,
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
  sidebarGeneration++;
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
      viewport: { width: innerWidth, height: innerHeight },
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
function allowTabDrop(e: DragEvent) {
  if (!canDrop.value || !e.dataTransfer?.types.includes("text/era-tab")) return;
  e.preventDefault();
  e.dataTransfer.dropEffect = "move";
}
async function split(e: DragEvent) {
  if (!canDrop.value) return;
  const id = e.dataTransfer?.getData("text/era-tab");
  const active = b.active;
  if (!id || !active) return;
  e.preventDefault();
  b.dragging = false;
  if (id === active.id || id === active.bookmarkId) return;
  // Restore the live page region before the host captures its split entrance.
  await nextTick();
  b.send({ type: "split", id: active.id, other: id });
}
</script>
<template>
  <main
    ref="container"
    class="page-area"
    :inert="b.spaceFormOpen"
    :aria-hidden="b.spaceFormOpen ? true : undefined"
    :class="{ split: other, resizing: resizing || b.sidebarResizing, 'sidebar-motion-preview': sidebarMotion && !b.native }"
    :style="{ '--split-ratio': ratio }"
    @dragover="allowTabDrop"
    @drop="split"
  >
    <div class="page-pane" :role="other ? 'region' : undefined" :aria-label="other ? '左侧窗格' : undefined">
      <PageToolbar v-if="other && b.active" :pane="b.active" data-chrome-motion="left-pane">
        <button
          class="icon-button"
          title="固定分屏组合"
          aria-label="固定分屏组合"
          @click="!b.active.pinned && b.send({ type: 'pin' })"
        >
          <Icon name="pin" :size="13" />
        </button>
      </PageToolbar>
      <div ref="primary" class="webview-slot" :data-page-dir="primaryDir">
        <Transition name="page">
        <SettingsPage v-if="b.active && isSettingsUrl(b.active.url)" :key="`settings:${b.active.id}`" :tab-id="b.active.id" :url="b.active.url" />
        <NewTabPage
          v-else-if="b.active?.url === 'era://newtab' || !b.active"
          :key="`newtab:${b.active?.id || 'empty'}`"
          :tab-id="b.active?.id"
        />
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
        :class="{ 'is-hovered': b.splitHovered, 'is-dragging': resizing }"
        role="separator"
        aria-label="调整分屏比例"
        aria-orientation="vertical"
        :aria-valuenow="Math.round(ratio * 100)"
        tabindex="0"
        @pointerdown="resize"
        @keydown.left.prevent="ratio = Math.max(0.2, ratio - 0.05)"
        @keydown.right.prevent="ratio = Math.min(0.8, ratio + 0.05)"
      />
      <div class="page-pane" role="region" aria-label="右侧窗格">
        <PageToolbar :pane="other" data-chrome-motion="right-pane">
          <button
            class="icon-button"
            aria-label="关闭分屏"
            title="关闭分屏"
            @click="b.send({ type: 'unsplit' })"
          >
            <Icon name="close" :size="13" />
          </button>
        </PageToolbar>
        <div ref="secondary" class="webview-slot" :data-page-dir="secondaryDir">
          <Transition name="page">
          <SettingsPage v-if="isSettingsUrl(other.url)" :key="`settings:${other.id}`" :tab-id="other.id" :url="other.url" />
          <NewTabPage v-else-if="other.url === 'era://newtab'" :key="`newtab:${other.id}`" :tab-id="other.id" />
          <div
            v-else-if="!b.native"
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
    <div v-if="canDrop" ref="dropTarget" class="split-drop-target">
      <Icon name="split" :size="24" /><span>拖到这里，打开左右分屏</span>
    </div>
    <Peek v-if="!b.native" />
    <slot />
  </main>
</template>
