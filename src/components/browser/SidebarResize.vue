<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useBrowser } from "../../stores/browser";
import { MIN_SIDEBAR_WIDTH, normalizeSidebarWidth, sidebarWidthLimit, visibleSidebarWidth } from "../../lib/sidebar-width";

const b = useBrowser();
const emit = defineEmits<{ resize: [] }>();
const preferred = ref(normalizeSidebarWidth(b.state.sidebarWidth));
const viewport = ref(innerWidth);
const width = computed(() => visibleSidebarWidth(preferred.value, viewport.value));
const maximum = computed(() => sidebarWidthLimit(viewport.value));
let finishResize: ((commit?: boolean) => void) | undefined;

function paint() {
  const style = document.documentElement.style;
  if (preferred.value === undefined) style.removeProperty("--sidebar-width");
  else style.setProperty("--sidebar-width", `${width.value}px`);
}
watch(() => b.state.sidebarWidth, (value) => {
  if (b.sidebarResizing) return;
  preferred.value = normalizeSidebarWidth(value);
  paint();
});
// Apply the saved preference before PageArea's first layout and window reveal.
paint();

function save() { b.send({ type: "sidebarWidth", width: preferred.value ?? null }); }
function reset() {
  finishResize?.(false);
  preferred.value = undefined;
  paint();
  save();
}
function resize(event: PointerEvent) {
  if (event.button !== 0 || !event.isPrimary) return;
  event.preventDefault();
  finishResize?.();
  const handle = event.currentTarget as HTMLElement;
  const track = handle.parentElement!;
  const startX = event.clientX;
  const startWidth = track.getBoundingClientRect().width;
  const previous = preferred.value;
  let moved = false;
  handle.focus({ preventScroll: true });
  handle.setPointerCapture(event.pointerId);
  b.sidebarResizing = true;
  // Stop the collapse transition immediately, including when grabbed mid-flight.
  track.style.transition = "none";
  const move = (next: PointerEvent) => {
    if (next.pointerId !== event.pointerId) return;
    const value = visibleSidebarWidth(startWidth + next.clientX - startX, viewport.value);
    if (value === width.value) return;
    moved = true;
    preferred.value = value;
    paint();
    // Submit the new page bounds in this input callback, before the next
    // ResizeObserver/paint turn can leave the real WebViews behind the track.
    emit("resize");
  };
  const finish = (commit = true) => {
    finishResize = undefined;
    handle.removeEventListener("pointermove", move);
    handle.removeEventListener("pointerup", release);
    handle.removeEventListener("pointercancel", release);
    handle.removeEventListener("lostpointercapture", release);
    if (handle.hasPointerCapture(event.pointerId)) handle.releasePointerCapture(event.pointerId);
    b.sidebarResizing = false;
    track.style.removeProperty("transition");
    if (!commit) { preferred.value = previous; paint(); }
    else if (moved) save();
  };
  const release = () => finish();
  finishResize = finish;
  handle.addEventListener("pointermove", move);
  handle.addEventListener("pointerup", release);
  handle.addEventListener("pointercancel", release);
  handle.addEventListener("lostpointercapture", release);
}
function key(event: KeyboardEvent) {
  if (event.key === "Escape" && finishResize) {
    event.preventDefault();
    finishResize(false);
    return;
  }
  const step = event.shiftKey ? 32 : 8;
  let next: number;
  switch (event.key) {
    case "ArrowLeft": next = width.value - step; break;
    case "ArrowRight": next = width.value + step; break;
    case "Home": next = MIN_SIDEBAR_WIDTH; break;
    case "End": next = maximum.value; break;
    case "Enter": event.preventDefault(); reset(); return;
    default: return;
  }
  event.preventDefault();
  finishResize?.();
  preferred.value = visibleSidebarWidth(next, viewport.value);
  paint();
  save();
}
function windowResize() { finishResize?.(); viewport.value = innerWidth; paint(); }
function blur() { finishResize?.(); }
function visibility() { if (document.hidden) blur(); }
watch(() => b.state.sidebarCollapsed || b.overlay || !!b.peek, (blocked) => { if (blocked) blur(); });
onMounted(() => {
  window.addEventListener("resize", windowResize);
  window.addEventListener("blur", blur);
  document.addEventListener("visibilitychange", visibility);
});
onBeforeUnmount(() => {
  finishResize?.();
  window.removeEventListener("resize", windowResize);
  window.removeEventListener("blur", blur);
  document.removeEventListener("visibilitychange", visibility);
  document.documentElement.style.removeProperty("--sidebar-width");
});
</script>

<template>
  <div class="sidebar-resize-handle" role="separator" aria-label="调整侧栏宽度" aria-orientation="vertical"
    :aria-valuemin="MIN_SIDEBAR_WIDTH" :aria-valuemax="maximum" :aria-valuenow="width" :aria-valuetext="`${width} 像素`"
    tabindex="0" title="拖动调整侧栏宽度，双击恢复默认" @pointerdown="resize" @dblclick.prevent="reset" @keydown="key" />
</template>
