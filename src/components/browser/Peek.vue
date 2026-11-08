<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useBrowser } from "../../stores/browser";
import { host } from "../../lib/address";
import { peekInsets } from "../../lib/peek-layout";
import {
  collapsedPeekTransform,
  freezePeekAnimation,
  PEEK_MOTION,
} from "../../lib/peek-motion";
import Icon from "../common/Icon.vue";
import Favicon from "../common/Favicon.vue";
const b = useBrowser(),
  layer = ref<HTMLElement>(),
  content = ref<HTMLElement>(),
  panel = ref<HTMLElement>(),
  scrim = ref<HTMLElement>(),
  surfaceImage = ref<HTMLImageElement>(),
  insets = ref({ x: 32, y: 24 }),
  backgroundBounds = ref<Record<number, Record<string, string>>>({});
let observer: ResizeObserver,
  frame = 0;
let panelAnimation: Animation | undefined,
  scrimAnimation: Animation | undefined,
  epoch = 0,
  prepared = "",
  disposed = false;
const reduced = matchMedia("(prefers-reduced-motion: reduce)");
async function measure() {
  if (!layer.value || !panel.value || !b.peek) return;
  insets.value = peekInsets(layer.value.clientWidth, layer.value.clientHeight);
  const outer = layer.value.getBoundingClientRect();
  layer.value.parentElement
    ?.querySelectorAll<HTMLElement>(".page-pane > .webview-slot")
    .forEach((slot, pane) => {
      const r = slot.getBoundingClientRect();
      backgroundBounds.value[pane] = {
        left: ((r.x - outer.x) / outer.width) * 100 + "%",
        top: ((r.y - outer.y) / outer.height) * 100 + "%",
        width: (r.width / outer.width) * 100 + "%",
        height: (r.height / outer.height) * 100 + "%",
      };
    });
  await nextTick();
  if (!content.value || !b.peek) return;
  // offset/computed sizes are layout coordinates. getBoundingClientRect on the
  // transformed card would incorrectly resize the real native viewport mid-flight.
  const card = cardBounds();
  if (!card) return;
  const contentStyle = getComputedStyle(content.value);
  const r = {
    x: card.x + panel.value.clientLeft + content.value.offsetLeft,
    y: card.y + panel.value.clientTop + content.value.offsetTop,
    width: parseFloat(contentStyle.width),
    height: parseFloat(contentStyle.height),
  };
  const radius = parseFloat(getComputedStyle(content.value).borderRadius) || 12;
  b.send({
    type: "peekLayout",
    id: b.peek.id,
    rect: {
      x: r.x,
      y: r.y,
      width: r.width,
      height: r.height,
      corners: [radius, radius, radius, radius],
    },
  });
}
function layout() {
  cancelAnimationFrame(frame);
  frame = requestAnimationFrame(() => void measure());
}
function cardBounds() {
  if (!layer.value || !panel.value) return;
  const root = layer.value.getBoundingClientRect(),
    style = getComputedStyle(panel.value);
  return {
    x: root.x + panel.value.offsetLeft,
    y: root.y + panel.value.offsetTop,
    width: parseFloat(style.width),
    height: parseFloat(style.height),
  };
}
function freeze() {
  freezePeekAnimation(panelAnimation);
  freezePeekAnimation(scrimAnimation);
  panelAnimation = undefined;
  scrimAnimation = undefined;
}
function current(id: string, ticket: number, generation: number) {
  return (
    !disposed &&
    b.peek?.id === id &&
    b.peekTicket === ticket &&
    epoch === generation
  );
}
async function prepare() {
  if (!b.peek || b.peekPhase === "open" || !b.peekBackgroundReady) return;
  const id = b.peek.id,
    ticket = b.peekTicket,
    key = `${id}:${ticket}`;
  if (prepared === key) return;
  const generation = ++epoch;
  freeze();
  if (b.peekPhase === "opening" && panel.value) {
    panel.value.style.opacity = "0";
    if (scrim.value) scrim.value.style.opacity = "0";
  }
  observer?.disconnect();
  await nextTick();
  if (!current(id, ticket, generation)) return;
  if (layer.value) observer?.observe(layer.value);
  await measure();
  if (!current(id, ticket, generation)) return;
  // The UI is still below the live source HWNDs here. Decode every split-pane
  // backdrop before asking Rust to raise it, otherwise the white PageArea leaks.
  try {
    await Promise.all(
      Array.from(layer.value?.querySelectorAll<HTMLImageElement>(".peek-background") || [])
        .map((image) => image.decode()),
    );
  } catch {
    // Leave the live native page visible if a local capture cannot be decoded.
    if (current(id, ticket, generation)) b.send({ type: "peekBackdropFailed", id, ticket });
    return;
  }
  if (b.peekPhase === "closing") {
    // Closing needs a decoded image before Rust hides the live child HWND.
    if (b.peekSurface?.ticket !== ticket) return;
    await nextTick();
    try {
      await surfaceImage.value?.decode();
    } catch {
      /* A capture failure keeps the themed loading surface. */
    }
  }
  if (!current(id, ticket, generation)) return;
  // decode() only guarantees pixels in memory. Give Chromium a paint opportunity
  // before the native sibling Z-order handoff (also needed for the exit proxy).
  await new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));
  if (!current(id, ticket, generation)) return;
  prepared = key;
  b.send({ type: "peekMotionStart", id, ticket });
}
async function play() {
  if (
    !b.peek ||
    !panel.value ||
    !scrim.value ||
    b.peekMoving !== b.peekTicket ||
    b.peekPhase === "open"
  )
    return;
  const id = b.peek.id,
    ticket = b.peekTicket,
    generation = epoch,
    closing = b.peekPhase === "closing";
  const card = cardBounds(),
    viewport = layer.value?.getBoundingClientRect();
  if (!card || !viewport) return;
  freeze();
  const collapsed = collapsedPeekTransform(card, b.peekOrigin, viewport);
  const transform = reduced.matches ? "none" : collapsed;
  const duration = reduced.matches
    ? PEEK_MOTION.reduced
    : closing
      ? PEEK_MOTION.exit
      : PEEK_MOTION.enter;
  const start = closing
    ? {
        transform: reduced.matches
          ? "none"
          : getComputedStyle(panel.value).transform,
        opacity: getComputedStyle(panel.value).opacity,
      }
    : { transform, opacity: "0" };
  const end = closing
    ? { transform, opacity: "0" }
    : { transform: "none", opacity: "1" };
  panelAnimation = panel.value.animate([start, end], {
    duration,
    easing: PEEK_MOTION.ease,
    fill: "both",
  });
  scrimAnimation = scrim.value.animate(
    [
      { opacity: closing ? getComputedStyle(scrim.value).opacity : "0" },
      { opacity: closing ? "0" : "1" },
    ],
    { duration, easing: PEEK_MOTION.ease, fill: "both" },
  );
  try {
    await Promise.all([panelAnimation.finished, scrimAnimation.finished]);
  } catch {
    return;
  }
  if (!current(id, ticket, generation)) return;
  freeze();
  b.send({ type: "peekMotionComplete", id, ticket });
}
function close() {
  b.send({ type: "peekClose" });
}
function key(e: KeyboardEvent) {
  if (b.peek && !b.overlay && e.key === "Escape") {
    e.preventDefault();
    e.stopImmediatePropagation();
    close();
  }
}
watch(
  () => b.peekBackgroundReady,
  () => void prepare(),
);
watch(
  () => [b.peek?.id, b.peekPhase, b.peekTicket],
  () => {
    if (!b.peek) {
      epoch++;
      prepared = "";
      freeze();
      observer?.disconnect();
      return;
    }
    if (b.peekPhase !== "open") void prepare();
  },
);
watch(
  () => b.peekSurface,
  () => {
    if (b.peekPhase === "closing") void prepare();
  },
);
watch(
  () => b.peekMoving,
  () => void play(),
);
function motionPreference() {
  panelAnimation?.finish();
  scrimAnimation?.finish();
}
onMounted(() => {
  observer = new ResizeObserver(layout);
  if (layer.value) observer.observe(layer.value);
  window.addEventListener("keydown", key, true);
  reduced.addEventListener("change", motionPreference);
});
onBeforeUnmount(() => {
  observer?.disconnect();
  disposed = true;
  epoch++;
  freeze();
  cancelAnimationFrame(frame);
  window.removeEventListener("keydown", key, true);
  reduced.removeEventListener("change", motionPreference);
});
</script>
<template>
  <section
    v-if="b.peek"
    ref="layer"
    class="peek-layer"
    aria-label="网页预览区域"
    :data-phase="b.peekPhase"
    :data-ticket="b.peekTicket"
    :data-moving="b.peekMoving"
    :data-peek-id="b.peek.id"
    :data-origin-x="b.peekOrigin?.x"
    :data-origin-y="b.peekOrigin?.y"
  >
    <img
      v-for="(image, pane) in b.peekBackground"
      :key="pane"
      class="peek-background"
      :style="backgroundBounds[Number(pane)]"
      :src="image"
      alt=""
      draggable="false"
    />
    <div
      ref="scrim"
      class="peek-scrim"
      aria-hidden="true"
      @pointerdown="close"
    />
    <section
      class="peek-panel"
      ref="panel"
      role="dialog"
      aria-modal="true"
      :aria-label="`Peek · ${b.peek.title}`"
      :style="{
        left: insets.x + 'px',
        right: insets.x + 'px',
        top: insets.y + 'px',
        bottom: insets.y + 'px',
      }"
    >
      <header class="peek-header">
        <span class="peek-favicon" :class="{ loading: b.peek.loading }"
          ><Favicon :url="b.peek.url" :favicon="b.peek.favicon" /></span
        ><span class="peek-host" :title="b.peek.url">{{
          host(b.peek.url)
        }}</span
        ><span class="peek-loading" role="status">{{
          b.peek.loading ? "正在加载…" : ""
        }}</span
        ><button
          class="peek-button"
          aria-label="将预览转为标签页"
          title="Open as Tab"
          @click="b.send({ type: 'peekPromote' })"
        >
          <Icon name="expand" :size="17" /></button
        ><button
          class="peek-button"
          aria-label="关闭 Peek"
          title="关闭 Peek (Esc)"
          @click="close"
        >
          <Icon name="close" :size="17" />
        </button>
      </header>
      <div ref="content" class="peek-content">
        <img v-if="b.commandBackdrop?.images[2]" class="command-background" :src="b.commandBackdrop.images[2]" alt="" draggable="false" />
        <img
          v-if="b.native && b.peekSurface?.image"
          ref="surfaceImage"
          class="peek-surface"
          :src="b.peekSurface.image"
          alt=""
          draggable="false"
        />
        <div v-else-if="b.native" class="peek-loading-page">
          <Icon name="globe" :size="24" /><span>正在打开预览…</span>
        </div>
        <div v-if="!b.native" class="preview-page">
          <Icon name="globe" :size="28" />
          <h2>{{ b.peek.title }}</h2>
          <p>预览网页由 Windows 应用中的独立 WebView2 加载</p>
          <small>{{ b.peek.url }}</small>
        </div>
      </div>
    </section>
  </section>
</template>
<style scoped>
/* Structure/metrics adapted from Zigzag PeekController.swift, Copyright 2026 Zigao Wang,
   Apache-2.0. Modified for Vue, CSS and native WebView2 slot bounds. */
.peek-layer {
  position: absolute;
  inset: 0;
  z-index: 80;
  isolation: isolate;
  overflow: hidden;
}
.peek-background {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  object-fit: fill;
  pointer-events: none;
}
.peek-scrim {
  position: absolute;
  inset: 0;
  background: var(--peek-scrim);
  opacity: 0;
}
.peek-panel {
  position: absolute;
  border-radius: var(--radius-large);
  border: 1px solid var(--panel-edge);
  background: var(--panel);
  box-shadow: 0 3px 16px var(--panel-shadow);
  overflow: hidden;
  transform-origin: 0 0;
  opacity: 0;
}
.peek-header {
  height: 40px;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 0 5px 0 13px;
  user-select: none;
}
.peek-favicon {
  display: grid;
  place-items: center;
  width: 18px;
  height: 18px;
  flex-shrink: 0;
}
.peek-host {
  font-size: 13px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  flex: 1;
}
.peek-favicon :deep(.favicon) {
  width: 18px;
  height: 18px;
}
.peek-loading {
  font-size: 11px;
  color: var(--quiet);
}
.peek-button {
  width: 32px;
  height: 32px;
  border-radius: 6px;
  display: grid;
  place-items: center;
  flex-shrink: 0;
}
.peek-button:hover {
  background: var(--hover);
}
.peek-content {
  position: absolute;
  top: 40px;
  left: 4px;
  right: 4px;
  bottom: 4px;
  border-radius: 12px;
  background: var(--surface);
  overflow: hidden;
}
.peek-surface {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  object-fit: fill;
  pointer-events: none;
}
.peek-loading-page {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  color: var(--quiet);
  font-size: 12px;
}
.loading {
  animation: peek-loading 1s ease-in-out infinite;
}
@keyframes peek-loading {
  50% {
    opacity: 0.3;
  }
}
</style>
