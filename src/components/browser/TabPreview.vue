<script setup lang="ts">
// Behavioral port of Zigzag's TabPreviewController, Copyright 2026 Zigao Wang,
// Apache-2.0; adapted to DOM hover and native child-window clipping.
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useBrowser } from "../../stores/browser";
import { previewFrame, previewPages, TAB_PREVIEW } from "../../lib/tab-preview";
import { displayUrl } from "../../lib/command";
import type { Rect } from "../../lib/types";
import Favicon from "../common/Favicon.vue";

const b = useBrowser();
const rendered = ref<string>();
const frame = ref<Rect>();
const card = ref<HTMLElement>();
const opacity = ref(0);
const phase = ref<"preparing" | "opening" | "open" | "closing">("preparing");
const slots = ref<Rect[]>([]);
const backdrops = ref<(string | null)[]>([]);
const reducedMotion = matchMedia("(prefers-reduced-motion: reduce)");
const closing = ref(0);
const pages = computed(() => rendered.value ? previewPages(b.state, rendered.value) : []);
const blocked = computed(() => b.overlay || !!b.peek || b.state.sidebarCollapsed);
let source: HTMLElement | undefined, pending: HTMLElement | undefined;
let showTimer: ReturnType<typeof setTimeout> | undefined, hideTimer: ReturnType<typeof setTimeout> | undefined;
let watchTimer: ReturnType<typeof setInterval> | undefined, refreshTimer: ReturnType<typeof setInterval> | undefined;
let ticket = 0;
let animation: Animation | undefined, instant = false;

function clearTimers() { clearTimeout(showTimer); clearTimeout(hideTimer); }
function stopRefresh() { clearInterval(refreshTimer); refreshTimer = undefined; }
function cancelAnimation() {
  const value = card.value ? Number(getComputedStyle(card.value).opacity) : opacity.value;
  animation?.cancel();
  animation = undefined;
  opacity.value = value;
  return value;
}
async function fade(to: number, generation: number, done?: () => void) {
  if (!card.value || generation !== ticket) return;
  const from = cancelAnimation();
  phase.value = to ? "opening" : "closing";
  const duration = reducedMotion.matches ? TAB_PREVIEW.reduced : to ? (instant ? 0 : TAB_PREVIEW.enter) : TAB_PREVIEW.leave;
  const current = card.value.animate([{ opacity: from }, { opacity: to }], { duration, easing: TAB_PREVIEW.ease, fill: "both" });
  animation = current;
  try { await current.finished; } catch { return; }
  if (generation !== ticket || animation !== current) return;
  opacity.value = to;
  await nextTick();
  if (generation !== ticket || animation !== current) return;
  current.cancel(); animation = undefined;
  if (to) phase.value = "open";
  done?.();
}
function finishClose(generation: number) {
  if (generation === ticket) b.send({ type: "tabPreview", id: null, rect: null, ticket: generation });
}
function hide(immediately = false) {
  clearTimers();
  stopRefresh();
  clearInterval(watchTimer);
  watchTimer = undefined;
  source = pending = undefined;
  b.previewTarget = undefined;
  if (!rendered.value || (closing.value && !immediately)) return;
  closing.value = ++ticket;
  if (immediately || phase.value === "preparing") {
    cancelAnimation();
    finishClose(ticket);
  } else if (b.native) {
    // Refresh only the pixels beneath the fixed cutout before fading the card.
    b.send({ type: "tabPreviewClose", ticket });
  } else void fade(0, ticket, () => finishClose(ticket));
}
function forceHide() { hide(true); }
function remove() {
  cancelAnimation();
  rendered.value = undefined; frame.value = undefined; closing.value = 0;
  opacity.value = 0; backdrops.value = []; b.previewBackdrop = null;
}
watch(() => b.previewHiddenTicket, (value) => {
  if (value === ticket) {
    stopRefresh();
    clearInterval(watchTimer);
    watchTimer = undefined;
    source = undefined;
    b.previewTarget = undefined;
    remove();
  }
});
watch(() => b.previewDismissals, forceHide);
watch([blocked, () => b.state.activeTab, () => b.state.activeSpace], forceHide);
watch(pages, (value) => { if (rendered.value && !value.length) forceHide(); });
watch(() => b.previewBackdrop, async (value) => {
  if (!value || value.ticket !== ticket || !rendered.value) return;
  try {
    await Promise.all(value.images.map(async (src) => { if (src) { const img = new Image(); img.src = src; await img.decode(); } }));
    if (value.ticket !== ticket) return;
    backdrops.value = value.images;
    await nextTick();
    requestAnimationFrame(() => requestAnimationFrame(() => {
      if (value.ticket === ticket) b.send({ type: "tabPreviewReady", ticket });
    }));
  } catch { if (value.ticket === ticket) b.send({ type: "tabPreviewFailed", ticket }); }
});
watch(() => b.previewPresentedTicket, (value) => {
  if (value !== ticket) return;
  if (closing.value) void fade(0, value, () => finishClose(value));
  else { void fade(1, value); startRefresh(); }
});

function rowOf(target: EventTarget | null): HTMLElement | undefined {
  if (!(target instanceof Element)) return;
  return target.closest<HTMLElement>(".sidebar .tab-row, .sidebar .favorite-tile") || undefined;
}
function idOf(row: HTMLElement) { return row.dataset.bookmarkId || row.dataset.tabId; }
function eligible(row: HTMLElement) {
  const id = idOf(row);
  return !blocked.value && !document.hidden && row.isConnected && !row.closest("[inert], [aria-hidden='true']") && id && previewPages(b.state, id).length > 0;
}
function geometry(row: HTMLElement) {
  const sidebar = row.closest(".sidebar"), page = document.querySelector(".page-area");
  if (!sidebar || !page) return;
  return previewFrame(row.getBoundingClientRect(), sidebar.getBoundingClientRect(), page.getBoundingClientRect(), { width: innerWidth, height: innerHeight });
}
function refresh() {
  if (rendered.value && !closing.value) b.send({ type: "tabPreviewRefresh", id: rendered.value });
}
async function present(row: HTMLElement) {
  if (!eligible(row)) { hide(); return; }
  const rect = geometry(row);
  if (!rect) { hide(); return; }
  clearTimers();
  stopRefresh();
  instant = !!rendered.value && !closing.value;
  cancelAnimation();
  if (!rendered.value) opacity.value = 0;
  const generation = ++ticket;
  closing.value = 0;
  source = row;
  pending = undefined;
  rendered.value = idOf(row);
  b.previewTarget = rendered.value;
  for (const page of pages.value) delete b.previewFailures[page.tabId || page.key];
  frame.value = rect;
  slots.value = Array.from(document.querySelectorAll<HTMLElement>('.page-pane > .webview-slot')).map(e => {
    const r = e.getBoundingClientRect();
    return { x: r.x, y: r.y, width: r.width, height: r.height };
  });
  phase.value = "preparing";
  await nextTick();
  // Give the trusted UI a paint opportunity before cutting through native pages.
  requestAnimationFrame(() => requestAnimationFrame(() => {
    if (ticket !== generation || source !== row || !eligible(row)) return;
    if (b.native) b.send({ type: "tabPreview", id: rendered.value, rect, ticket: generation });
    else { void fade(1, generation); startRefresh(); }
  }));
}
function startRefresh() {
  stopRefresh();
  refresh();
  refreshTimer = setInterval(refresh, TAB_PREVIEW.refresh);
  watchTimer ??= setInterval(() => {
    if (source && (!source.matches(":hover") || !eligible(source))) hide();
  }, 250);
}
function enter(row: HTMLElement) {
  clearTimeout(hideTimer);
  if (!eligible(row)) { hide(); return; }
  if (source === row && !closing.value) return;
  clearTimeout(showTimer);
  if (rendered.value) { void present(row); return; }
  pending = row;
  showTimer = setTimeout(() => {
    if (pending === row && row.matches(":hover") && eligible(row)) void present(row);
  }, TAB_PREVIEW.delay);
}
function over(event: PointerEvent) {
  if (event.pointerType !== "mouse" || event.buttons) return;
  const row = rowOf(event.target);
  if (row && row !== rowOf(event.relatedTarget)) enter(row);
}
function out(event: PointerEvent) {
  const row = rowOf(event.target);
  if (!row || row === rowOf(event.relatedTarget)) return;
  clearTimeout(showTimer);
  pending = undefined;
  clearTimeout(hideTimer);
  hideTimer = setTimeout(() => hide(), TAB_PREVIEW.hideDelay);
}
function visibility() { if (document.hidden) forceHide(); }
function key(event: KeyboardEvent) { if (event.key === "Escape") forceHide(); }

onMounted(() => {
  document.addEventListener("pointerover", over);
  document.addEventListener("pointerout", out);
  document.addEventListener("pointerdown", forceHide, true);
  document.addEventListener("contextmenu", forceHide, true);
  document.addEventListener("wheel", forceHide, { capture: true, passive: true });
  document.addEventListener("scroll", forceHide, true);
  document.addEventListener("keydown", key, true);
  document.addEventListener("visibilitychange", visibility);
  window.addEventListener("blur", forceHide);
  window.addEventListener("resize", forceHide);
  reducedMotion.addEventListener("change", forceHide);
});
onBeforeUnmount(() => {
  forceHide();
  clearTimers();
  stopRefresh();
  clearInterval(watchTimer);
  document.removeEventListener("pointerover", over);
  document.removeEventListener("pointerout", out);
  document.removeEventListener("pointerdown", forceHide, true);
  document.removeEventListener("contextmenu", forceHide, true);
  document.removeEventListener("wheel", forceHide, true);
  document.removeEventListener("scroll", forceHide, true);
  document.removeEventListener("keydown", key, true);
  document.removeEventListener("visibilitychange", visibility);
  window.removeEventListener("blur", forceHide);
  window.removeEventListener("resize", forceHide);
  reducedMotion.removeEventListener("change", forceHide);
});
</script>

<template>
  <Teleport to="body">
    <template v-if="rendered && b.native">
      <template v-for="(image, index) in backdrops" :key="index">
        <img v-if="image && slots[index]" class="tab-preview-background" :src="image" alt="" draggable="false"
          :style="{ left: slots[index].x + 'px', top: slots[index].y + 'px', width: slots[index].width + 'px', height: slots[index].height + 'px' }" />
      </template>
    </template>
    <section v-if="rendered && frame && pages.length" id="tab-hover-preview" ref="card" class="tab-hover-preview" role="tooltip"
      :data-preview-id="rendered" :data-closing="!!closing" :data-phase="phase"
      :style="{ left: frame.x + 'px', top: frame.y + 'px', width: frame.width + 'px', height: frame.height + 'px', opacity }">
      <div class="tab-preview-picture" :class="{ 'is-split': pages.length === 2 }">
        <div v-for="page in pages" :key="page.key" class="tab-preview-pane">
          <img v-if="b.thumbnails[page.tabId || page.key]" :src="b.thumbnails[page.tabId || page.key]" :alt="`${page.title} 的网页缩略图`" draggable="false" />
          <div v-else-if="page.url === 'era://newtab'" class="tab-preview-empty tab-preview-newtab"><span>Rove</span><small>A little room to explore.</small></div>
          <div v-else class="tab-preview-empty"><Favicon :url="page.url" :favicon="page.favicon" /><small>{{ !b.native ? '界面预览' : b.previewFailures[page.tabId || page.key] ? '暂时无法生成预览' : '正在生成预览…' }}</small></div>
        </div>
      </div>
      <div class="tab-preview-captions">
        <div v-for="page in pages" :key="page.key" class="tab-preview-caption">
          <span>{{ page.title }}</span><small>{{ displayUrl(page.url) }}</small>
        </div>
      </div>
    </section>
  </Teleport>
</template>

<style>
.tab-preview-background { position: fixed; z-index: 44; object-fit: fill; pointer-events: none; }
.tab-preview-newtab > span { font-size: 36px; font-weight: 300; letter-spacing: -1px; color: var(--ink); }
.tab-hover-preview {
  position: fixed;
  z-index: 45;
  display: flex;
  flex-direction: column;
  padding: 4px;
  border: 1px solid var(--panel-edge);
  border-radius: var(--radius-panel);
  background: var(--panel);
  box-shadow: 0 3px 16px var(--panel-shadow);
  overflow: hidden;
  pointer-events: none;
}
/* Native pages are cut to the card's outline, keeping every other pixel live. */
html[data-native] .tab-hover-preview { box-shadow: none; }
.tab-preview-picture { display: flex; flex: 1; min-height: 0; overflow: hidden; border-radius: 10px; background: var(--surface); }
.tab-preview-pane { flex: 1; min-width: 0; overflow: hidden; }
.tab-preview-pane + .tab-preview-pane { border-left: 1px solid var(--panel-edge); }
.tab-preview-pane img { display: block; width: 100%; height: 100%; object-fit: cover; object-position: top; }
.tab-preview-empty { height: 100%; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 12px; color: var(--tertiary); }
.tab-preview-empty .favicon { width: 32px; height: 32px; }
.tab-preview-empty small { font-size: 11px; }
.tab-preview-captions { display: flex; flex: 0 0 43px; align-items: center; }
.tab-preview-caption { flex: 1; min-width: 0; padding: 0 8px; }
.tab-preview-caption span, .tab-preview-caption small { display: block; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
.tab-preview-caption span { font-size: 13px; line-height: 19px; }
.tab-preview-caption small { font-size: 11px; line-height: 16px; color: var(--quiet); }
</style>
