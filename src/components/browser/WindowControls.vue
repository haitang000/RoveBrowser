<script setup lang="ts">
import { useBrowser } from "../../stores/browser";
import { host } from "../../lib/address";
import { isSettingsUrl } from "../../lib/settings";
import { titlebarDragRegions } from "../../lib/window-chrome";
import Icon from "../common/Icon.vue";
import { nextTick, onMounted, onBeforeUnmount, ref, watch } from "vue";
const b = useBrowser();
const maximize = ref<HTMLButtonElement>();
let observer: ResizeObserver;
let paletteObserver: MutationObserver;
let paletteKey = "";
let palette: { background: number[]; hover: number[]; foreground: number[] };
let context: CanvasRenderingContext2D | null;
function report() {
  if (!b.native || !maximize.value) return;
  const shell = maximize.value.closest('.browser-shell')!;
  const header = maximize.value.closest('header')!;
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
  b.send({
    type: "windowChrome",
    maximize: { x: r.x, y: r.y, width: r.width, height: r.height },
    viewportWidth: innerWidth,
    dragRegions: titlebarDragRegions(header.getBoundingClientRect(),
      [...header.querySelectorAll('.browser-navigation, .address-field, .reader-toggle, .window-controls')]
        .map(element => element.getBoundingClientRect())),
    ...palette,
  });
}
watch(
  [() => b.state.theme, () => b.space.color, () => b.space.translucency, () => b.onboardingOpen],
  async () => {
    await nextTick();
    report();
  },
);
onMounted(() => {
  observer = new ResizeObserver(report);
  if (maximize.value) {
    observer.observe(maximize.value);
    const header = maximize.value.closest('header')!;
    observer.observe(header);
    header.querySelectorAll('.titlebar-identity, .browser-navigation, .address-field').forEach(element => observer.observe(element));
  }
  const shell = maximize.value?.closest('.browser-shell');
  shell?.addEventListener("frame-background-change", report);
  paletteObserver = new MutationObserver(report);
  if (shell) paletteObserver.observe(shell, { attributes: true, attributeFilter: ['class', 'style'] });
  window.addEventListener("resize", report);
  report();
});
onBeforeUnmount(() => {
  maximize.value?.closest('.browser-shell')?.removeEventListener("frame-background-change", report);
  observer?.disconnect();
  paletteObserver?.disconnect();
  window.removeEventListener("resize", report);
});
</script>
<template>
  <header class="titlebar">
    <div class="titlebar-identity"><span class="app-name">Rove</span></div>
    <div v-if="!b.onboardingOpen" class="browser-toolbar">
      <nav class="browser-navigation" aria-label="网页导航">
        <button
          class="icon-button"
          :title="b.shortcutTitle('后退', 'back')"
          aria-label="后退"
          @click="b.send({ type: 'back' })"
        ><Icon name="back" :size="14" /></button>
        <button
          class="icon-button"
          :title="b.shortcutTitle('前进', 'forward')"
          aria-label="前进"
          @click="b.send({ type: 'forward' })"
        ><Icon name="forward" :size="14" /></button>
        <button
          class="icon-button"
          :title="b.shortcutTitle('刷新', 'reload')"
          aria-label="刷新"
          :aria-busy="b.active?.loading || undefined"
          @click="b.send({ type: 'reload' })"
        ><Icon
          :name="b.active?.loading ? 'spinner' : 'reload'"
          :class="{ 'reload-spinner': b.active?.loading }"
          :size="14"
        /></button>
      </nav>
      <button
        class="address-field"
        aria-label="搜索或输入地址"
        :title="b.shortcutTitle('搜索或输入地址', 'address')"
        @click="b.openCommand('edit')"
      >
        <span>{{ isSettingsUrl(b.active?.url) ? '设置' : host(b.active?.url || "") || "Search or enter address" }}</span>
        <Icon name="settings" :size="13" />
      </button>
      <button v-if="b.reader.available.includes(b.state.activeTab) || b.reader.id === b.state.activeTab || b.reader.waiting === b.state.activeTab"
        class="icon-button reader-toggle" :class="{ chosen: b.reader.id === b.state.activeTab }"
        :aria-label="b.reader.id === b.state.activeTab ? '退出阅读模式' : b.reader.waiting === b.state.activeTab ? '取消阅读模式' : '进入阅读模式'"
        :title="b.shortcutTitle(b.reader.id === b.state.activeTab ? '退出阅读模式' : '阅读模式', 'reader')"
        :aria-pressed="b.reader.id === b.state.activeTab" :aria-busy="b.reader.waiting === b.state.activeTab || undefined"
        @click="b.send({ type: 'reader' })"><Icon :name="b.reader.waiting === b.state.activeTab ? 'spinner' : 'reader'" :size="14" /></button>
    </div>
    <div class="window-controls">
      <button
        aria-label="最小化"
        title="最小化"
        @click="b.send({ type: 'window', action: 'minimize' })"
      >
        <Icon name="minimize" :size="12" /></button
      ><button
        ref="maximize"
        aria-label="最大化或还原"
        title="最大化或还原"
        @click="b.send({ type: 'window', action: 'maximize' })"
      >
        <Icon name="maximize" :size="11" /></button
      ><button
        class="window-close"
        aria-label="关闭窗口"
        title="关闭窗口"
        @click="b.send({ type: 'window', action: 'close' })"
      >
        <Icon name="close" :size="13" />
      </button>
    </div>
  </header>
</template>
