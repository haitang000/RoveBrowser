<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, watch } from "vue";
import { useBrowser } from "../../stores/browser";
import CommandBar from "./CommandBar.vue";
import Desk from "./Desk.vue";
import SettingsPanel from "./SettingsPanel.vue";
import Peek from "./Peek.vue";
import TabMenu from "./TabMenu.vue";
import PageContextMenu from "./PageContextMenu.vue";
import PinFolderMenu from "./PinFolderMenu.vue";
import FavoriteMenu from "./FavoriteMenu.vue";
import TabPreviewCard from "./TabPreviewCard.vue";
import TabSwitcher from "./TabSwitcher.vue";
import ErrorToast from "./ErrorToast.vue";
import FindBar from "./FindBar.vue";
import ReaderControls from "./ReaderControls.vue";
import StatusToast from "./StatusToast.vue";
import ActionToast from "./ActionToast.vue";
import DownloadsPanel from "./DownloadsPanel.vue";
import BrowserMenu from "./BrowserMenu.vue";
import QuitDialog from "./QuitDialog.vue";
import SplitControls from "./SplitControls.vue";
import PermissionPrompt from "./PermissionPrompt.vue";
import ExtensionDialog from "./ExtensionDialog.vue";
import ExtensionMenu from "./ExtensionMenu.vue";
import { browserKey, browserKeyUp } from "../../lib/browser-key";
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
let observer: MutationObserver, resize: ResizeObserver, last = "", focusKey = "";
let observedPreview: HTMLElement | null = null;
const key = (event: KeyboardEvent) => browserKey(b, event);
const keyUp = (event: KeyboardEvent) => browserKeyUp(b, event);
function dismissPreview() {
  if (b.hoverPreview) b.send({ type: "tabPreview", id: null, rect: null, ticket: b.hoverPreview.ticket });
}
function report() {
  const modals = ".quit-scrim, .extension-dialog-scrim, .extension-menu-layer, .command-scrim, .settings-scrim, .menu-scrim, .desk, .downloads-layer, .browser-menu-layer, .tab-switcher-layer";
  const surfaces = document.querySelectorAll<HTMLElement>(`${modals}, .peek-layer, .permission-layer, .tab-hover-preview, .error-toast, .find-bar, .status-toast, .action-toast, .reader-controls, .reader-options-layer, .split-controls, .split-controls-dismiss`);
  const preview = document.getElementById("tab-hover-preview");
  if (preview !== observedPreview) {
    if (observedPreview) resize.unobserve(observedPreview);
    if (preview) resize.observe(preview, { box: "border-box" });
    observedPreview = preview;
  }
  const rect = (element: HTMLElement) => {
    const r = element.getBoundingClientRect();
    return { x: r.x, y: r.y, width: r.width, height: r.height };
  };
  // Keep the Peek hole covered until a leaving modal has painted its final frame.
  const modal = Boolean(document.querySelector(modals));
  const downloads = Boolean(document.querySelector(".downloads-layer"));
  const content = document.querySelector<HTMLElement>(".peek-content");
  const cutout = !modal && !b.permissionPrompt && b.peek && b.peekPhase === "open" && content ? { ...rect(content), corners: [12, 12, 12, 12] } : null;
  const regions = Array.from(surfaces, (element) => {
    const bounds = rect(element);
    // The native opacity layer is clipped to these regions; a shadow must have
    // room or the clip cuts it into a square-cornered halo.
    return surfaceRegion(bounds, element.classList.contains("reader-options-layer") ? 24 : shadowBleed(element.className));
  });
  const next = JSON.stringify({ regions, cutout, downloads });
  const active = b.quitOpen ? "quit" : b.extensions.popup ? "extension" : b.extensionMenuAnchor ? "extensionsMenu" : b.commandOpen ? "command" : b.settingsOpen ? "settings" : b.deskOpen ? "desk" : b.contextMenu ? "menu" : b.browserMenuOpen ? "browserMenu" : b.pageContextMenu ? "pageMenu" : b.downloadsOpen ? "downloads" : b.permissionPrompt ? "permission" : b.tabSwitcher?.visible ? "tabSwitcher" : b.splitControls ? "splitControls" : b.findOpen ? "find" : "";
  const nextFocus = active === "permission" ? b.permissionPrompt?.id ?? active
    : active === "pageMenu" ? `pageMenu-${b.pageContextMenu?.ticket}` : active;
  const focus = Boolean(active && nextFocus !== focusKey);
  focusKey = nextFocus;
  if (next !== last || focus) {
    last = next;
    b.send({ type: "popupLayer", regions, cutout, focus, downloads });
  }
  if (focus) {
    const selector = active === "pageMenu" ? ".page-context-menu"
      : active === "tabSwitcher" ? ".tab-switcher-panel" : active === "quit" ? ".quit-dialog" : active === "extension" ? ".extension-dialog [autofocus]" : active === "extensionsMenu" ? ".extension-menu-panel" : active === "command" ? ".command-panel input" : active === "settings" ? ".settings-panel input" : active === "desk" ? ".desk input" : active === "find" ? ".find-bar input" : active === "downloads" ? ".downloads-panel" : active === "browserMenu" ? ".browser-menu button:not(:disabled)" : active === "splitControls" ? ".split-controls button" : active === "permission" ? ".permission-prompt" : ".context-menu input, .context-menu button, .pin-folder-editor input[type=text]";
    document.querySelector<HTMLElement>(selector)?.focus({ preventScroll: active === "pageMenu" || active === "menu" });
  }
}
onMounted(async () => {
  document.documentElement.dataset.native = "true";
  document.documentElement.dataset.popup = "true";
  window.addEventListener("keydown", key);
  window.addEventListener("keyup", keyUp);
  document.addEventListener("pointerdown", dismissPreview, true);
  document.addEventListener("wheel", dismissPreview, { capture: true, passive: true });
  resize = new ResizeObserver(report);
  resize.observe(document.documentElement);
  observer = new MutationObserver(report);
  observer.observe(document.body, { childList: true, subtree: true, attributes: true });
  await document.fonts.ready;
  await nextTick();
  report();
});
onBeforeUnmount(() => {
  observer?.disconnect(); resize?.disconnect();
  window.removeEventListener("keydown", key);
  window.removeEventListener("keyup", keyUp);
  document.removeEventListener("pointerdown", dismissPreview, true);
  document.removeEventListener("wheel", dismissPreview, true);
});
</script>
<template>
  <div class="popup-page-area" :style="area"><Peek /><CommandBar /><FindBar /><StatusToast /><ActionToast /></div>
  <Desk /><SettingsPanel /><TabMenu /><FavoriteMenu /><PinFolderMenu /><BrowserMenu /><TabPreviewCard /><ErrorToast /><DownloadsPanel />
  <PageContextMenu v-if="b.pageContextMenu" :key="b.pageContextMenu.ticket" :menu="b.pageContextMenu" />
  <ReaderControls />
  <SplitControls />
  <TabSwitcher />
  <PermissionPrompt v-if="b.permissionPrompt" :key="b.permissionPrompt.id" :prompt="b.permissionPrompt" />
  <ExtensionMenu />
  <ExtensionDialog v-if="b.extensions.popup && !b.quitOpen" />
  <QuitDialog v-if="b.quitOpen" />
</template>
