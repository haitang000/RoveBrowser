<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, watch } from "vue";
import { useBrowser } from "../../stores/browser";
import CommandBar from "./CommandBar.vue";
import Desk from "./Desk.vue";
import SettingsPanel from "./SettingsPanel.vue";
import Peek from "./Peek.vue";
import TabMenu from "./TabMenu.vue";
import FavoriteMenu from "./FavoriteMenu.vue";
import TabPreviewCard from "./TabPreviewCard.vue";
import ErrorToast from "./ErrorToast.vue";
import FindBar from "./FindBar.vue";
import StatusToast from "./StatusToast.vue";
import ActionToast from "./ActionToast.vue";
import DownloadsPanel from "./DownloadsPanel.vue";
import { browserKey } from "../../lib/browser-key";
import { shadowBleed, surfaceRegion } from "../../lib/popup-layer";
import { frameOf } from "../../lib/space-look";
const b = useBrowser();
watch(() => b.settings.reduceMotion, (value) => document.documentElement.dataset.reduceMotion = String(value), { immediate: true });
const area = computed(() => ({ left: b.popupArea.x + "px", top: b.popupArea.y + "px", width: b.popupArea.width + "px", height: b.popupArea.height + "px" }));
watch(() => frameOf(b.space, b.state.theme, false), (frame) => {
  const root = document.documentElement;
  root.dataset.theme = frame.theme ?? b.state.theme;
  root.dataset.color = frame.custom ? "custom" : b.space.color;
  if (frame.custom) {
    root.style.setProperty("--frame", frame.start);
    root.style.setProperty("--frame-h", String(frame.hue));
  } else {
    root.style.removeProperty("--frame");
    root.style.removeProperty("--frame-h");
  }
}, { immediate: true });
let observer: MutationObserver, resize: ResizeObserver, frame = 0, last = "", focusKey = "";
const key = (event: KeyboardEvent) => browserKey(b, event);
function dismissPreview() {
  if (b.hoverPreview) b.send({ type: "tabPreview", id: null, rect: null, ticket: b.hoverPreview.ticket });
}
function report() {
  const surfaces = document.querySelectorAll<HTMLElement>(".command-scrim, .settings-scrim, .menu-scrim, .desk, .peek-layer, .tab-hover-preview, .error-toast, .find-bar, .status-toast, .action-toast, .downloads-layer");
  const rect = (element: HTMLElement) => {
    const r = element.getBoundingClientRect();
    return { x: r.x, y: r.y, width: r.width, height: r.height };
  };
  // Keep the Peek hole covered until a leaving modal has painted its final frame.
  const modal = Boolean(document.querySelector(".command-scrim, .settings-scrim, .menu-scrim, .desk, .downloads-layer"));
  const downloads = Boolean(document.querySelector(".downloads-layer"));
  const content = document.querySelector<HTMLElement>(".peek-content");
  const cutout = !modal && b.peek && b.peekPhase === "open" && content ? { ...rect(content), corners: [12, 12, 12, 12] } : null;
  const regions = Array.from(surfaces, (element) => {
    const bounds = rect(element);
    // The native opacity layer is clipped to these regions; a shadow must have
    // room or the clip cuts it into a square-cornered halo.
    return surfaceRegion(bounds, shadowBleed(element.className));
  });
  const next = JSON.stringify({ regions, cutout, downloads });
  const active = b.commandOpen ? "command" : b.settingsOpen ? "settings" : b.deskOpen ? "desk" : b.contextMenu ? "menu" : b.downloadsOpen ? "downloads" : b.findOpen ? "find" : "";
  const focus = Boolean(active && active !== focusKey);
  focusKey = active;
  if (next !== last || focus) {
    last = next;
    b.send({ type: "popupLayer", regions, cutout, focus, downloads });
  }
  if (focus) {
    const selector = active === "command" ? ".command-panel input" : active === "settings" ? ".settings-panel input" : active === "desk" ? ".desk input" : active === "find" ? ".find-bar input" : active === "downloads" ? ".downloads-panel" : ".context-menu input, .context-menu button";
    document.querySelector<HTMLElement>(selector)?.focus();
  }
}
function schedule() { cancelAnimationFrame(frame); frame = requestAnimationFrame(report); }
onMounted(async () => {
  document.documentElement.dataset.native = "true";
  document.documentElement.dataset.popup = "true";
  window.addEventListener("keydown", key);
  document.addEventListener("pointerdown", dismissPreview, true);
  document.addEventListener("wheel", dismissPreview, { capture: true, passive: true });
  observer = new MutationObserver(report);
  observer.observe(document.body, { childList: true, subtree: true, attributes: true });
  resize = new ResizeObserver(schedule);
  resize.observe(document.documentElement);
  await document.fonts.ready;
  await nextTick();
  report();
});
onBeforeUnmount(() => {
  observer?.disconnect(); resize?.disconnect(); cancelAnimationFrame(frame);
  window.removeEventListener("keydown", key);
  document.removeEventListener("pointerdown", dismissPreview, true);
  document.removeEventListener("wheel", dismissPreview, true);
});
</script>
<template>
  <div class="popup-page-area" :style="area"><Peek /><CommandBar /><FindBar /><StatusToast /><ActionToast /></div>
  <Desk /><SettingsPanel /><TabMenu /><FavoriteMenu /><TabPreviewCard /><ErrorToast /><DownloadsPanel />
</template>
