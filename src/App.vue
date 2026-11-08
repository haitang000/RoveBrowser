<script setup lang="ts">
import { onMounted, onBeforeUnmount, watch } from "vue";
import { useBrowser } from "./stores/browser";
import Sidebar from "./components/browser/Sidebar.vue";
import WindowControls from "./components/browser/WindowControls.vue";
import PageArea from "./components/browser/PageArea.vue";
import CommandBar from "./components/browser/CommandBar.vue";
import Desk from "./components/browser/Desk.vue";
import SettingsPanel from "./components/browser/SettingsPanel.vue";
import FavoritesPanel from "./components/browser/FavoritesPanel.vue";
import Icon from "./components/common/Icon.vue";
const b = useBrowser();
watch(
  () => b.state.theme,
  (t) => (document.documentElement.dataset.theme = t),
  { immediate: true },
);
watch(
  () => b.space.color,
  (c) => (document.documentElement.dataset.color = c),
  { immediate: true },
);
watch(
  () => b.overlay,
  (v) => b.send({ type: "overlay", visible: v }),
);
function key(e: KeyboardEvent) {
  const k = e.key.toLowerCase();
  if (e.ctrlKey && !e.altKey) {
    if (k === "l") {
      e.preventDefault();
      b.openCommand("edit");
    }
    if (k === "t") {
      e.preventDefault();
      if (e.shiftKey) b.send({ type: "restore" });
      else b.openCommand();
    }
    if (k === "w") {
      e.preventDefault();
      b.send({ type: "close" });
    }
    if (k === "tab") {
      e.preventDefault();
      b.cycle(e.shiftKey);
    }
    if (k === "b") {
      e.preventDefault();
      b.send({ type: "sidebar" });
    }
    if (k === "r") {
      e.preventDefault();
      b.send({ type: "reload" });
    }
    if (e.code === "Backslash" && e.shiftKey) {
      e.preventDefault();
      b.deskOpen = !b.deskOpen;
    }
    if (/^[1-9]$/.test(k)) {
      const s = b.state.spaces[Number(k) - 1];
      if (s) {
        e.preventDefault();
        b.send({ type: "space", id: s.id });
      }
    }
  }
  if (e.altKey && (k === "arrowleft" || k === "arrowright")) {
    e.preventDefault();
    b.send({ type: k === "arrowleft" ? "back" : "forward" });
  }
}
onMounted(async () => {
  window.addEventListener("keydown", key);
  if (b.native) {
    // Wait for the mounted shell, fonts and a paint opportunity before revealing
    // the native window. Its session and theme are already present on first render.
    await document.fonts.ready;
    requestAnimationFrame(() => {
      requestAnimationFrame(() => b.send({ type: "presented" }));
    });
  }
});
onBeforeUnmount(() => window.removeEventListener("keydown", key));
</script>
<template>
  <div class="browser-shell" :class="{ collapsed: b.state.sidebarCollapsed }">
    <WindowControls />
    <div class="browser-body">
      <div
        class="sidebar-track"
        :inert="b.state.sidebarCollapsed"
        :aria-hidden="b.state.sidebarCollapsed"
      >
        <Sidebar />
      </div>
      <button
        v-if="b.state.sidebarCollapsed"
        class="reveal-sidebar icon-button"
        aria-label="展开侧栏"
        title="展开侧栏 (Ctrl+B)"
        @click="b.send({ type: 'sidebar' })"
      >
        <Icon name="sidebar" /></button
      ><PageArea />
    </div>
    <Desk /><CommandBar /><SettingsPanel /><FavoritesPanel />
    <div v-if="b.error && !b.commandOpen" class="error-toast" role="alert">
      {{ b.error
      }}<button aria-label="关闭提示" @click="b.error = ''">×</button>
    </div>
  </div>
</template>
