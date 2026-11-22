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
import SplitControls from "./SplitControls.vue";
import { isSettingsUrl } from "../../lib/settings";
import { isHistoryUrl } from "../../lib/address";
import { pageDirection, type PageDirection } from "../../lib/page-motion";
const SettingsPage = defineAsyncComponent(() => import("./SettingsPage.vue"));
const HistoryPage = defineAsyncComponent(() => import("./HistoryPage.vue"));
const b = useBrowser(),
  primary = ref<HTMLElement>(),
  secondary = ref<HTMLElement>(),
  leftPane = ref<HTMLElement>(),
  rightPane = ref<HTMLElement>(),
  divider = ref<HTMLElement>(),
  container = ref<HTMLElement>(),
  ratio = ref(0.5),
  resizing = ref(false),
  swapping = ref(false),
  sidebarMotion = ref(false);
const other = computed(() => b.splitPartner);
let ro: ResizeObserver;
let finishResize: (() => void) | undefined;
let paneAnimations: Animation[] = [];
let reported: { primary: Rect; secondary: Rect | null; sidebar: Rect | null } | undefined;
// Zigzag's native ResizeHandle owns the real frames while dragging. This event
// only keeps the shell headers and fallback content in step with native input.
watch(() => b.splitResize, (update) => {
  if (!update) return;
  resizing.value = update.resizing;
  ratio.value = update.ratio;
});
watch([() => b.active?.id, () => b.active?.splitWith], ([left, right], [previousLeft, previousRight]) => {
  finishResize?.();
  resizing.value = false;
  b.splitControls = undefined;
  const panes = [leftPane.value, rightPane.value];
  const swap = Boolean(left && right && left === previousRight && right === previousLeft);
  const from = swap && !b.native ? panes.map(pane => pane?.getBoundingClientRect()).reverse() : [];
  paneAnimations.forEach(animation => animation.cancel());
  paneAnimations = [];
  swapping.value = swap;
  if (!swap) return;
  if (!b.native) ratio.value = 1 - ratio.value;
  void nextTick(() => {
    if (b.active?.id !== left || other.value?.id !== right) return;
    swapping.value = false;
    if (b.native || b.settings.reduceMotion || matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    panes.forEach((pane, index) => {
      const start = from[index];
      if (!pane || !start) return;
      const end = pane.getBoundingClientRect();
      const x = start.x - end.x, y = start.y - end.y;
      const side = Math.sign(x);
      const frames = Array.from({ length: 13 }, (_, frame) => {
        const t = frame / 12;
        const slide = t ** 3 * (t * (6 * t - 15) + 10);
        const lift = Math.sin(Math.PI * t) ** 2;
        return { transform: `translate(${x * (1 - slide)}px, ${y * (1 - slide) + side * 24 * lift}px) perspective(900px) rotateY(${side * 9 * lift}deg) scale(${1 - 0.068 * lift})` };
      });
      paneAnimations.push(pane.animate(frames, { duration: 360 }));
    });
  });
});
watch([resizing, () => b.overlay, () => b.peek, () => b.sidebarResizing], ([dragging, overlay, peek, sidebar]) => {
  if (dragging || overlay || peek || sidebar) b.splitControls = undefined;
});
watch(() => b.splitClick, toggleSplitControls);
function positionSplitControls() {
  if (!divider.value || !b.active || !other.value) return;
  const { x, y, width, height } = divider.value.getBoundingClientRect();
  const center = { x: x + width / 2, y: y + height / 2, gap: width, id: b.active.id, other: other.value.id };
  if (b.splitControls?.x !== center.x || b.splitControls?.y !== center.y || b.splitControls?.gap !== width) b.splitControls = center;
}
function toggleSplitControls() {
  if (b.splitControls) b.splitControls = undefined;
  else if (!resizing.value && !b.overlay && !b.peek) positionSplitControls();
}
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
  if (b.splitControls) positionSplitControls();
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
  paneAnimations.forEach(animation => animation.cancel());
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
  // Pointer capture keeps dragging over native pages without covering them.
  // Only the live viewport bounds change; no overlay or thumbnail is needed.
  handle.onpointermove = (ev) => {
    if (width <= 0) return;
    if (!resizing.value) {
      if (Math.abs(ev.clientX - e.clientX) < 4) return;
      resizing.value = true;
    }
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
  handle.onpointerup = (ev) => {
    const click = !resizing.value && Math.abs(ev.clientX - e.clientX) < 4 && Math.abs(ev.clientY - e.clientY) < 4;
    finish();
    if (click) toggleSplitControls();
  };
  handle.onpointercancel = finish;
  handle.onlostpointercapture = finish;
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
  >
    <div ref="leftPane" class="page-pane" :role="other ? 'region' : undefined" :aria-label="other ? b.t('左侧窗格') : undefined">
      <PageToolbar v-if="other && b.active" :pane="b.active" data-chrome-motion="left-pane">
        <button
          class="icon-button"
          :title="b.t('固定分屏组合')"
          :aria-label="b.t('固定分屏组合')"
          @click="!b.active.pinned && b.send({ type: 'pin' })"
        >
          <Icon name="pin" :size="13" />
        </button>
      </PageToolbar>
      <div ref="primary" class="webview-slot" :data-page-dir="primaryDir">
        <Transition name="page" :css="!swapping">
        <SettingsPage v-if="b.active && isSettingsUrl(b.active.url)" :key="`settings:${b.active.id}`" :tab-id="b.active.id" :url="b.active.url" />
        <HistoryPage v-else-if="b.active && isHistoryUrl(b.active.url)" :key="`history:${b.active.id}`" :tab-id="b.active.id" />
        <NewTabPage
          v-else-if="b.active?.url === 'era://newtab' || !b.active"
          :key="`newtab:${b.active?.id || 'empty'}`"
          :tab-id="b.active?.id"
        />
        <div v-else-if="!b.native" :key="`preview:${b.active.id}:${b.active.url}`" class="preview-page">
          <Icon name="globe" :size="28" />
          <h2>{{ b.active.title }}</h2>
          <p>{{ b.t('在 Windows 应用中打开真实网页') }}</p>
          <small>{{ b.active.url }}</small>
        </div>
        </Transition>
      </div>
    </div>
    <template v-if="other"
      ><div
        ref="divider"
        class="split-divider"
        :class="{ 'is-hovered': b.splitHovered, 'is-dragging': resizing, 'is-expanded': b.splitControls }"
        role="separator"
        :aria-label="b.t('调整分屏比例')"
        aria-orientation="vertical"
        :aria-valuenow="Math.round(ratio * 100)"
        tabindex="0"
        :title="b.t('拖动调整比例，单击显示分屏操作')"
        @keydown.enter.prevent="toggleSplitControls"
        @keydown.space.prevent="toggleSplitControls"
        @pointerdown="resize"
        @keydown.left.prevent="ratio = Math.max(0.2, ratio - 0.05)"
        @keydown.right.prevent="ratio = Math.min(0.8, ratio + 0.05)"
      />
      <div ref="rightPane" class="page-pane" role="region" :aria-label="b.t('右侧窗格')">
        <PageToolbar :pane="other" data-chrome-motion="right-pane">
          <button
            class="icon-button"
            :aria-label="b.t('关闭分屏')"
            :title="b.t('关闭分屏')"
            @click="b.send({ type: 'unsplit' })"
          >
            <Icon name="close" :size="13" />
          </button>
        </PageToolbar>
        <div ref="secondary" class="webview-slot" :data-page-dir="secondaryDir">
          <Transition name="page" :css="!swapping">
          <SettingsPage v-if="isSettingsUrl(other.url)" :key="`settings:${other.id}`" :tab-id="other.id" :url="other.url" />
          <HistoryPage v-else-if="isHistoryUrl(other.url)" :key="`history:${other.id}`" :tab-id="other.id" />
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
    <SplitControls v-if="!b.native" />
    <div v-if="b.tabSplitGuide" class="split-drop-target">
      <Icon name="split" :size="24" /><span>{{ b.t('拖到这里，打开左右分屏') }}</span>
    </div>
    <Peek v-if="!b.native" />
    <slot />
  </main>
</template>
