<script setup lang="ts">
import { useBrowser } from "../../stores/browser";
import { host } from "../../lib/address";
import { isSettingsUrl } from "../../lib/settings";
import Icon from "../common/Icon.vue";
import { nextTick, onMounted, onBeforeUnmount, ref, watch } from "vue";
const b = useBrowser();
const maximize = ref<HTMLButtonElement>();
let observer: ResizeObserver;
function report() {
  if (!b.native || !maximize.value) return;
  const r = maximize.value.getBoundingClientRect(),
    style = getComputedStyle(document.documentElement);
  const canvas = document.createElement("canvas");
  canvas.width = canvas.height = 1;
  const context = canvas.getContext("2d")!;
  const frame = style.getPropertyValue("--frame").trim();
  const color = (...fills: string[]) => {
    context.clearRect(0, 0, 1, 1);
    for (const fill of fills) {
      context.fillStyle = fill;
      context.fillRect(0, 0, 1, 1);
    }
    return [...context.getImageData(0, 0, 1, 1).data].slice(0, 3);
  };
  b.send({
    type: "windowChrome",
    maximize: { x: r.x, y: r.y, width: r.width, height: r.height },
    viewportWidth: innerWidth,
    background: color(frame),
    hover: color(frame, style.getPropertyValue("--hover").trim()),
    foreground: color(frame, style.getPropertyValue("--icon").trim()),
  });
}
watch(
  () => [b.state.theme, b.space.color, b.space.translucency, b.space.clearGlass],
  async () => {
    await nextTick();
    report();
  },
);
onMounted(() => {
  observer = new ResizeObserver(report);
  if (maximize.value) observer.observe(maximize.value);
  window.addEventListener("resize", report);
  report();
});
onBeforeUnmount(() => {
  observer?.disconnect();
  window.removeEventListener("resize", report);
});
</script>
<template>
  <header class="titlebar">
    <div class="titlebar-identity"><span class="app-name">Rove</span></div>
    <div class="browser-toolbar">
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
          @click="b.send({ type: 'reload' })"
        ><Icon name="reload" :size="14" /></button>
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
