<script setup lang="ts">
import { useBrowser } from "../../stores/browser";
import { titlebarDragRegions } from "../../lib/window-chrome";
import Icon from "../common/Icon.vue";
import PageToolbar from "./PageToolbar.vue";
import { nextTick, onMounted, onBeforeUnmount, ref, watch } from "vue";
const props = defineProps<{ split: boolean }>();
const b = useBrowser();
const maximize = ref<HTMLButtonElement>();
const header = ref<HTMLElement>();
let observer: ResizeObserver;
let paletteObserver: MutationObserver;
let paletteKey = "";
let palette: { background: number[]; hover: number[]; foreground: number[] };
let context: CanvasRenderingContext2D | null;
const controls = "[data-window-control]";
function dragSurfaces() {
  const shell = header.value?.closest(".browser-shell");
  return props.split && shell
    ? [...shell.querySelectorAll<HTMLElement>("[data-window-drag]")]
    : header.value ? [header.value] : [];
}
function report() {
  if (!b.native || !maximize.value) return;
  const shell = maximize.value.closest('.browser-shell')!;
  const r = maximize.value.getBoundingClientRect(), style = getComputedStyle(shell);
  const frame = style.backgroundImage === "none"
    ? style.backgroundColor
    : style.getPropertyValue("--frame-gradient-start").trim();
  const hover = style.getPropertyValue("--hover").trim();
  const foreground = style.getPropertyValue("--icon").trim();
  const nextPalette = JSON.stringify([frame, hover, foreground]);
  if (nextPalette !== paletteKey) {
    if (!context) {
      const canvas = document.createElement("canvas");
      canvas.width = canvas.height = 1;
      context = canvas.getContext("2d", { willReadFrequently: true });
    }
    if (!context) return;
    const color = (fill: string) => {
      context!.clearRect(0, 0, 1, 1);
      context!.fillStyle = fill;
      context!.fillRect(0, 0, 1, 1);
      return [...context!.getImageData(0, 0, 1, 1).data];
    };
    palette = { background: color(frame), hover: color(hover), foreground: color(foreground) };
    paletteKey = nextPalette;
  }
  const caption = maximize.value.parentElement!.getBoundingClientRect();
  const dragRegions = dragSurfaces().flatMap(surface => {
    const bounds = surface.getBoundingClientRect();
    if (surface.matches(".split-sidebar-top")) {
      if (b.state.sidebarCollapsed) return [];
      bounds.width = Math.max(0, Math.min(bounds.width, surface.closest(".sidebar-track")!.getBoundingClientRect().right - bounds.x));
    }
    if (!bounds.width || !bounds.height) return [];
    return titlebarDragRegions(bounds, [
      caption,
      ...[...surface.querySelectorAll(controls)].map(element => element.getBoundingClientRect()),
    ]);
  });
  b.send({
    type: "windowChrome",
    maximize: { x: r.x, y: r.y, width: r.width, height: r.height },
    viewportWidth: innerWidth,
    anchorLeft: props.split && !b.state.sidebarCollapsed,
    dragRegions,
    ...palette,
  });
}
function observeChrome() {
  if (!observer || !maximize.value) return;
  observer.disconnect();
  observer.observe(maximize.value);
  for (const surface of dragSurfaces()) {
    observer.observe(surface);
    surface.querySelectorAll(`.titlebar-identity, ${controls}`).forEach(element => observer.observe(element));
  }
  const track = header.value?.closest(".browser-shell")?.querySelector(".sidebar-track");
  if (props.split && track) observer.observe(track);
  report();
}
watch([() => props.split, () => b.onboardingOpen], observeChrome, { flush: "post" });
watch(
  [() => b.state.theme, () => b.space.color, () => b.space.translucency],
  async () => {
    await nextTick();
    report();
  },
);
onMounted(() => {
  if (!b.native) return;
  observer = new ResizeObserver(report);
  const shell = maximize.value?.closest('.browser-shell');
  shell?.addEventListener("frame-background-change", report);
  paletteObserver = new MutationObserver(report);
  if (shell) paletteObserver.observe(shell, { attributes: true, attributeFilter: ['class', 'style'] });
  window.addEventListener("resize", report);
  observeChrome();
});
onBeforeUnmount(() => {
  maximize.value?.closest('.browser-shell')?.removeEventListener("frame-background-change", report);
  observer?.disconnect();
  paletteObserver?.disconnect();
  window.removeEventListener("resize", report);
});
</script>
<template>
  <header ref="header" class="window-chrome" :class="split ? 'split-window-chrome' : 'titlebar'">
    <div v-if="!split" class="titlebar-identity" data-chrome-motion="app-name"><span class="app-name">Rove</span></div>
    <PageToolbar v-if="!split && !b.onboardingOpen" data-chrome-motion="titlebar" />
    <div class="window-controls">
      <button
        :aria-label="b.t('最小化')"
        :title="b.t('最小化')"
        @click="b.send({ type: 'window', action: 'minimize' })"
      >
        <Icon name="minimize" :size="12" /></button
      ><button
        ref="maximize"
        :aria-label="b.t('最大化或还原')"
        :title="b.t('最大化或还原')"
        @click="b.send({ type: 'window', action: 'maximize' })"
      >
        <Icon name="maximize" :size="11" /></button
      ><button
        class="window-close"
        :aria-label="b.t('关闭窗口')"
        :title="b.t('关闭窗口')"
        @click="b.send({ type: 'window', action: 'close' })"
      >
        <Icon name="close" :size="13" />
      </button>
    </div>
  </header>
</template>
