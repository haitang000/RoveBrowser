<script setup lang="ts">
// Behavioral port of Zigzag's TabPreviewController, Copyright 2026 Zigao Wang,
// Apache-2.0; adapted to DOM hover and an independent popup renderer.
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useBrowser } from "../../stores/browser";
import { previewFrame, previewPages, TAB_PREVIEW } from "../../lib/tab-preview";
import TabPreviewCard from "./TabPreviewCard.vue";
import type { Rect } from "../../lib/types";

const b = useBrowser();
const rendered = ref<string>();
const frame = ref<Rect>();
const opacity = ref(0);
const phase = ref<"preparing" | "opening" | "open" | "closing">("preparing");
const reducedMotion = matchMedia("(prefers-reduced-motion: reduce)");
const closing = ref(0);
const pages = computed(() => rendered.value ? previewPages(b.state, rendered.value) : []);
const blocked = computed(() => b.overlay || !!b.peek || b.state.sidebarCollapsed || b.sidebarResizing);
let source: HTMLElement | undefined, pending: HTMLElement | undefined;
let showTimer: ReturnType<typeof setTimeout> | undefined, hideTimer: ReturnType<typeof setTimeout> | undefined;
let watchTimer: ReturnType<typeof setInterval> | undefined, refreshTimer: ReturnType<typeof setInterval> | undefined;
let ticket = 0;
let instant = false;

function clearTimers() { clearTimeout(showTimer); clearTimeout(hideTimer); }
function stopRefresh() { clearInterval(refreshTimer); refreshTimer = undefined; }
function syncCard() {
  b.hoverPreview = rendered.value && frame.value ? { id: rendered.value, frame: frame.value, opacity: opacity.value, phase: phase.value, closing: !!closing.value, ticket, instant } : undefined;
}
watch([rendered, frame, opacity, phase, closing], syncCard, { flush: "sync" });
function fade(to: number, generation: number) {
  if (generation !== ticket) return;
  phase.value = to ? "opening" : "closing";
  opacity.value = to;
}
watch(() => b.previewAnimationDone, (value) => {
  if (value !== ticket) return;
  if (closing.value) finishClose(value);
  else phase.value = "open";
});

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
    finishClose(ticket);
  } else fade(0, ticket);
}
function forceHide() { hide(true); }
function remove() {
  rendered.value = undefined; frame.value = undefined; closing.value = 0;
  opacity.value = 0;
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
watch(() => b.previewPresentedTicket, (value) => {
  if (value !== ticket) return;
  if (closing.value) fade(0, value);
  else { fade(1, value); startRefresh(); }
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
  if (!rendered.value) opacity.value = 0;
  const generation = ++ticket;
  closing.value = 0;
  source = row;
  pending = undefined;
  rendered.value = idOf(row);
  b.previewTarget = rendered.value;
  for (const page of pages.value) delete b.previewFailures[page.tabId || page.key];
  frame.value = rect;
  phase.value = "preparing";
  await nextTick();
  // Mount the popup card before starting its transition.
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

<template><TabPreviewCard v-if="!b.native" /></template>
