<script setup lang="ts">
import { computed, onMounted, onBeforeUnmount, ref, watch } from "vue";
import { useBrowser } from "./stores/browser";
import Sidebar from "./components/browser/Sidebar.vue";
import WindowControls from "./components/browser/WindowControls.vue";
import PageArea from "./components/browser/PageArea.vue";
import CommandBar from "./components/browser/CommandBar.vue";
import Desk from "./components/browser/Desk.vue";
import SettingsPanel from "./components/browser/SettingsPanel.vue";
import FavoritesPanel from "./components/browser/FavoritesPanel.vue";
import Icon from "./components/common/Icon.vue";
import { SPACE_SWIPE } from "./lib/space-swipe";
import { coverage, isSeeThrough, splitGap, usesClearGlass } from "./lib/page-frame";
const b = useBrowser();
const shell = ref<HTMLElement>();
const reducedTransparency = matchMedia("(prefers-reduced-transparency: reduce)");
const transparencyReduced = ref(reducedTransparency.matches);
const glassAllowed = computed(() => !transparencyReduced.value && !b.glassBlocked);
const glassActive = computed(() => isSeeThrough(b.space?.translucency) && glassAllowed.value);
const shellStyle = computed(() => ({
  "--split-gap": `${splitGap({ sidebarVisible: !b.state.sidebarCollapsed, borderless: b.state.borderless !== false })}px`,
  ...(glassActive.value
    ? { "--frame-coverage": String(coverage(b.space?.translucency, usesClearGlass(b.space?.clearGlass, true))) }
    : {}),
}));
let frameAnimation: Animation | undefined;
const reducedMotion = matchMedia("(prefers-reduced-motion: reduce)");
function finishFrameAnimation() { frameAnimation?.cancel(); frameAnimation = undefined; }
// ColorView.setColor/CALayer.set: retarget from the color currently on screen.
// Submit only a background-color effect; the titlebar, list and native page stay live.
watch(
  [() => b.state.theme, () => b.space.color] as const,
  ([theme, color]) => {
    const from = shell.value ? getComputedStyle(shell.value).backgroundColor : undefined;
    document.documentElement.dataset.theme = theme;
    document.documentElement.dataset.color = color;
    finishFrameAnimation();
    if (!shell.value || !from || reducedMotion.matches || glassActive.value) return;
    const to = getComputedStyle(document.documentElement).getPropertyValue("--frame").trim();
    frameAnimation = shell.value.animate([{ backgroundColor: from }, { backgroundColor: to }], {
      duration: SPACE_SWIPE.backgroundDuration, easing: "ease",
    });
  },
  { immediate: true },
);
watch(
  [() => b.overlay, () => b.commandOpen],
  ([visible, command]) => b.send({ type: "overlay", visible, command }),
);
watch(glassActive, (enabled) => {
  if (b.native) b.send({ type: "glass", enabled });
}, { immediate: true });
function syncTransparency() { transparencyReduced.value = reducedTransparency.matches; }
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
  reducedMotion.addEventListener("change", finishFrameAnimation);
  reducedTransparency.addEventListener("change", syncTransparency);
  if (b.native) document.documentElement.dataset.native = "true";
  if (b.native) {
    // Wait for the mounted shell, fonts and a paint opportunity before revealing
    // the native window. Its session and theme are already present on first render.
    await document.fonts.ready;
    requestAnimationFrame(() => {
      requestAnimationFrame(() => b.send({ type: "presented" }));
    });
  }
});
onBeforeUnmount(() => {
  window.removeEventListener("keydown", key);
  reducedMotion.removeEventListener("change", finishFrameAnimation);
  reducedTransparency.removeEventListener("change", syncTransparency);
  finishFrameAnimation();
});
</script>
<template>
  <div
    ref="shell"
    class="browser-shell"
    :class="{
      collapsed: b.state.sidebarCollapsed,
      borderless: b.state.borderless !== false,
      glass: glassActive,
      'glass-neutral': glassActive && b.space?.color === 'graphite',
    }"
    :style="shellStyle"
  >
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
      ><PageArea><CommandBar /></PageArea>
    </div>
    <Desk /><SettingsPanel /><FavoritesPanel />
    <div v-if="b.error && !b.commandOpen" class="error-toast" role="alert">
      {{ b.error
      }}<button aria-label="关闭提示" @click="b.error = ''">×</button>
    </div>
  </div>
</template>
