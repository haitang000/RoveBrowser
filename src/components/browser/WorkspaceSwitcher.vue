<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useBrowser } from "../../stores/browser";
import Icon from "../common/Icon.vue";
import BrowserMenu from "./BrowserMenu.vue";

const b = useBrowser();
const trigger = ref<HTMLButtonElement>();
// Keep the drag source and drop targets in the shell WebView on Windows.
const dragMenuOpen = ref(false);

function syncAnchor() {
  const rect = trigger.value?.getBoundingClientRect();
  if (!rect) return;
  const { x, y, width, height } = rect;
  const anchor = b.browserMenuAnchor;
  if (anchor && x === anchor.x && y === anchor.y && width === anchor.width && height === anchor.height) return;
  b.browserMenuAnchor = { x, y, width, height };
}
function toggleMenu() {
  if (b.browserMenuOpen) { b.browserMenuOpen = false; return; }
  syncAnchor();
  b.commandOpen = false;
  b.settingsOpen = false;
  b.deskOpen = false;
  b.contextMenu = undefined;
  b.closeSpaceEditor();
  b.closeDownloads();
  b.browserMenuOpen = true;
}
function openDropMenu(event: DragEvent) {
  if (!b.dragging || !event.dataTransfer?.types.includes("text/era-tab")) return;
  event.preventDefault();
  if (dragMenuOpen.value) return;
  syncAnchor();
  dragMenuOpen.value = true;
}

let observer: ResizeObserver;
watch(() => b.dragging, (dragging) => { if (!dragging) dragMenuOpen.value = false; });
watch(() => [b.downloadsOpen, b.state.sidebarCollapsed, b.state.sidebarWidth], syncAnchor, { flush: "post" });
onMounted(() => {
  observer = new ResizeObserver(syncAnchor);
  if (trigger.value?.parentElement) observer.observe(trigger.value.parentElement);
  window.addEventListener("resize", syncAnchor);
  syncAnchor();
});
onBeforeUnmount(() => {
  observer?.disconnect();
  window.removeEventListener("resize", syncAnchor);
});
</script>

<template>
  <footer class="workspace-switcher">
    <button
      ref="trigger"
      class="icon-button browser-menu-trigger"
      :class="{ active: b.browserMenuOpen || dragMenuOpen || b.downloadsOpen || b.activeDownloads }"
      :title="b.activeDownloads ? `浏览器菜单 · ${b.activeDownloads} 个下载进行中` : '浏览器菜单'"
      aria-label="浏览器菜单"
      :aria-description="b.activeDownloads ? `${b.activeDownloads} 个下载进行中` : undefined"
      aria-haspopup="menu"
      :aria-expanded="b.browserMenuOpen || dragMenuOpen"
      :aria-controls="dragMenuOpen ? 'browser-menu-drop-targets' : 'browser-menu'"
      @click="toggleMenu"
      @keydown.down.prevent="!b.browserMenuOpen && toggleMenu()"
      @dragover="openDropMenu"
    >
      <Icon name="more" :size="15" />
      <span v-if="b.activeDownloads" class="downloads-badge" aria-hidden="true">{{ b.activeDownloads }}</span>
    </button>
    <button
      class="icon-button"
      :title="b.shortcutTitle('全部标签页', 'desk')"
      aria-label="全部标签页"
      @click="b.deskOpen = true"
    ><Icon name="grid" :size="14" /></button>
    <BrowserMenu drop-targets-only :drop-menu-open="dragMenuOpen" />
  </footer>
</template>
