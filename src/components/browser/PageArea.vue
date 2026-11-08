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
const b = useBrowser(),
  primary = ref<HTMLElement>(),
  secondary = ref<HTMLElement>(),
  container = ref<HTMLElement>(),
  ratio = ref(0.5),
  drop = ref(false);
const other = computed(() =>
  b.state.tabs.find((t) => t.id === b.active?.splitWith),
);
let ro: ResizeObserver;
function layout() {
  if (!primary.value || !container.value) return;
  const card = container.value.getBoundingClientRect();
  const style = getComputedStyle(container.value);
  b.popupArea = { x: card.x + container.value.clientLeft, y: card.y + container.value.clientTop, width: container.value.clientWidth, height: container.value.clientHeight };
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
  });
}
watch(
  [
    () => b.active?.id,
    () => b.active?.splitWith,
    () => b.state.sidebarCollapsed,
    ratio,
  ],
  layout,
  { flush: "post" },
);
onMounted(() => {
  ro = new ResizeObserver(layout);
  if (container.value) ro.observe(container.value);
  layout();
  window.addEventListener("resize", layout);
});
onBeforeUnmount(() => {
  ro?.disconnect();
  window.removeEventListener("resize", layout);
});
function resize(e: PointerEvent) {
  const handle = e.currentTarget as HTMLElement;
  handle.setPointerCapture(e.pointerId);
  b.send({ type: "overlay", visible: true });
  handle.onpointermove = (ev) => {
    const r = container.value!.getBoundingClientRect();
    ratio.value = Math.max(0.2, Math.min(0.8, (ev.clientX - r.x) / r.width));
  };
  handle.onpointerup = () => {
    handle.onpointermove = null;
    layout();
    b.send({ type: "overlay", visible: b.overlay });
  };
  handle.onlostpointercapture = () => {
    handle.onpointermove = null;
    b.send({ type: "overlay", visible: b.overlay });
  };
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
    :class="{ split: other, dropping: drop }"
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
      <div ref="primary" class="webview-slot">
        <SettingsPage v-if="b.active && isSettingsUrl(b.active.url)" :key="b.active.id" :tab-id="b.active.id" :url="b.active.url" />
        <div
          v-else-if="b.active?.url === 'era://newtab' || !b.active"
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
        <div v-else-if="!b.native" class="preview-page">
          <Icon name="globe" :size="28" />
          <h2>{{ b.active.title }}</h2>
          <p>在 Windows 应用中打开真实网页</p>
          <small>{{ b.active.url }}</small>
        </div>
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
        <div ref="secondary" class="webview-slot">
          <SettingsPage v-if="isSettingsUrl(other.url)" :key="other.id" :tab-id="other.id" :url="other.url" />
          <div
            v-else-if="other.url === 'era://newtab' || !b.native"
            class="preview-page"
          >
            <Icon name="globe" :size="28" />
            <h2>{{ other.title }}</h2>
            <small>{{ other.url }}</small>
          </div>
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
