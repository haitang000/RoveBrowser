<script setup lang="ts">
// Ported from Zigzag SidebarView.transitionList/handleSwipe/settleHome.
// Copyright 2026 Zigao Wang, Apache-2.0; modified for Vue and Web Animations.
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useBrowser } from "../../stores/browser";
import { SPACE_SWIPE, settleFrames, swipeCoasting, swipeLanding, swipeNewImpulse, swipeOffset, swipeOffsetVelocity, swipeReleaseDelay, swipeTravel, swipeVelocity, type SwipeSample } from "../../lib/space-swipe";
import type { Tab } from "../../lib/types";
import SpaceList from "./SpaceList.vue";

const props = defineProps<{ blocked: boolean }>();
const emit = defineEmits<{ menu: [event: MouseEvent, tab: Tab]; progress: [shares: Record<string, number> | null] }>();
const b = useBrowser(), viewport = ref<HTMLElement>(), track = ref<HTMLElement>();
const center = ref(b.state.activeSpace), width = ref(240), offset = ref(0);
const panes = ref([{ id: center.value, slot: 0 }]);
const tracking = ref(false), settling = ref(false);
const scroll = new Map<string, number>();
const motion = matchMedia("(prefers-reduced-motion: reduce)");
const systemReduced = ref(motion.matches);
const reduced = computed(() => systemReduced.value || b.settings.reduceMotion);
let phase: "idle" | "deciding" | "scrolling" | "tracking" | "finished" = "idle";
let lastEvent = -Infinity, lastDeltaX = 0, accumX = 0, accumY = 0, travel = 0;
let samples: SwipeSample[] = [], release: ReturnType<typeof setTimeout> | undefined;
let animations: Animation[] = [], generation = 0, observer: ResizeObserver | undefined;
const index = computed(() => b.state.spaces.findIndex((s) => s.id === center.value));
function geometry() { return { width: width.value, hasLeft: index.value > 0, hasRight: index.value + 1 < b.state.spaces.length }; }
function remember(id: string, top: number) { scroll.set(id, top); }
function clearRelease() { clearTimeout(release); release = undefined; }

/** Read the compositor's presentation value before cancelling an interrupted slide. */
function interrupt() {
  // During tracking Vue may not have painted the latest packet yet. Only an
  // active compositor animation needs a presentation read.
  const shown = animations.length && track.value ? new DOMMatrixReadOnly(getComputedStyle(track.value).transform).m41 : offset.value;
  generation++;
  animations.forEach((a) => a.cancel());
  animations = [];
  settling.value = false;
  offset.value = shown;
  return shown;
}

async function settle(from: number, velocity?: number, switching = false) {
  const ticket = ++generation;
  settling.value = true;
  offset.value = 0;
  emit("progress", null);
  if (reduced.value && !switching) {
    panes.value = [{ id: center.value, slot: 0 }];
    settling.value = false;
    return;
  }
  await nextTick();
  if (ticket !== generation || !track.value) return;
  if (reduced.value) {
    // Same-place cross-fade: reduced motion never translates either list.
    animations = Array.from(track.value.children).map((pane) => {
      const incoming = pane.getAttribute("data-pane-space") === center.value;
      return pane.animate({ opacity: incoming ? [0, 1] : [1, 0] }, {
        duration: SPACE_SWIPE.reducedDuration * (incoming ? 1 : 0.6), fill: "both", easing: SPACE_SWIPE.ease,
      });
    });
  } else if (Math.abs(from) > 0.1) {
    const spring = velocity === undefined ? null : settleFrames(from, velocity);
    animations = [track.value.animate(spring?.frames || [
      { transform: `translateX(${from}px)` }, { transform: "translateX(0px)" },
    ], { duration: spring?.duration ?? SPACE_SWIPE.switchDuration, easing: spring ? "linear" : SPACE_SWIPE.ease, fill: "both" })];
    if (velocity === undefined) {
      const outgoing = Array.from(track.value.children).find((pane) => pane.getAttribute("data-pane-space") !== center.value);
      if (outgoing) animations.push(outgoing.animate({ opacity: [1, 0] }, { duration: 180, fill: "both", easing: SPACE_SWIPE.ease }));
    }
  }
  // Start on the current timeline sample. Leaving WAAPI's start pending adds
  // another compositor frame after the release detector has already fired.
  const startedAt = document.timeline.currentTime;
  if (startedAt !== null) animations.forEach((animation) => { animation.startTime = startedAt; });
  void Promise.all(animations.map((a) => a.finished)).then(() => {
    if (ticket !== generation) return;
    panes.value = [{ id: center.value, slot: 0 }];
    animations.forEach((a) => a.cancel());
    animations = [];
    settling.value = false;
  }).catch(() => { /* A newer gesture owns the presentation now. */ });
}

async function switchTo(id: string, velocity?: number, send = false) {
  if (id === center.value) return;
  const targetIndex = b.state.spaces.findIndex((s) => s.id === id);
  if (targetIndex < 0) return;
  clearRelease();
  const side = targetIndex < index.value ? -1 : 1;
  const shown = interrupt();
  const neighbor = panes.value.find((p) => p.id === id);
  const from = neighbor ? neighbor.slot * width.value + shown : side * width.value + shown;
  panes.value = [{ id: center.value, slot: -side }, { id, slot: 0 }];
  center.value = id;
  phase = "finished";
  tracking.value = false;
  // Submit the compositor slide before native page activation/color updates can do work.
  await settle(from, velocity, true);
  if (send && center.value === id) b.send({ type: "space", id });
}

function neighbors() {
  panes.value = b.state.spaces.slice(Math.max(0, index.value - 1), index.value + 2)
    .map((s) => ({ id: s.id, slot: b.state.spaces.indexOf(s) - index.value }));
}
function updateProgress() {
  if (reduced.value) return;
  const share = Math.min(Math.abs(offset.value) / width.value, 1);
  const neighbor = b.state.spaces[index.value + (offset.value > 0 ? -1 : 1)];
  emit("progress", { [center.value]: 1 - share, ...(neighbor ? { [neighbor.id]: share } : {}) });
}
function finish(cancelled = false) {
  clearRelease();
  if (phase !== "tracking") return;
  phase = "finished";
  tracking.value = false;
  // Use the last input time, not the silence timer, so a flick retains its release velocity.
  const velocity = cancelled ? 0 : swipeVelocity(samples, lastEvent);
  const direction = cancelled ? 0 : swipeLanding(swipeOffset(travel, geometry()), velocity, width.value);
  const visibleVelocity = swipeOffsetVelocity(travel, velocity, geometry());
  const target = b.state.spaces[index.value + direction];
  if (direction && target) void switchTo(target.id, visibleVelocity, true);
  else void settle(interrupt(), visibleVelocity);
}
function cancel() {
  clearRelease();
  if (phase === "tracking") finish(true);
  phase = "finished";
}
function begin() { phase = "deciding"; accumX = 0; accumY = 0; samples = []; clearRelease(); }

function wheel(event: WheelEvent) {
  if (props.blocked || b.state.sidebarCollapsed || event.ctrlKey || event.metaKey || event.altKey) { cancel(); return; }
  const keys = (event.target as Element).closest(".space-keys");
  if (keys && keys.scrollWidth > keys.clientWidth) return;
  const unit = event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? width.value : 1;
  // DOM deltas describe scrolling; NSEvent deltas describe the list's movement.
  const dx = -(event.shiftKey && !event.deltaX ? event.deltaY : event.deltaX) * unit;
  const dy = (event.shiftKey && !event.deltaX ? 0 : event.deltaY) * unit;
  if (!dx && !dy) return;
  const now = performance.now() / 1000, gap = now - lastEvent;
  const reversed = phase === "finished" && dx * accumX < 0 && Math.abs(dx) >= SPACE_SWIPE.axisLock;
  const precise = event.deltaMode === 0;
  const renewed = precise && phase === "finished" && Math.abs(dx) > Math.abs(dy)
    && (swipeNewImpulse(dx, lastDeltaX)
      || (gap * 1000 > swipeReleaseDelay(samples) && Math.abs(dx) >= SPACE_SWIPE.axisLock && Math.abs(dx) >= Math.abs(lastDeltaX)));
  if (gap > SPACE_SWIPE.wheelTurnGap || phase === "idle" || reversed || renewed) {
    if (phase === "tracking") finish();
    begin();
  }
  lastEvent = now;
  lastDeltaX = dx;
  if (phase === "scrolling") return;
  if (phase === "finished") { if (Math.abs(dx) > Math.abs(dy)) event.preventDefault(); return; }
  accumX += dx; accumY += dy;
  if (phase === "deciding") {
    if (Math.abs(accumY) >= SPACE_SWIPE.axisLock && Math.abs(accumY) > Math.abs(accumX)) { phase = "scrolling"; return; }
    if (Math.abs(accumX) < SPACE_SWIPE.axisLock || Math.abs(accumX) <= Math.abs(accumY)) {
      if (Math.abs(dx) > Math.abs(dy)) event.preventDefault();
      return;
    }
    if (event.deltaMode !== 0) {
      event.preventDefault();
      phase = "finished";
      const target = b.state.spaces[index.value + (accumX > 0 ? -1 : 1)];
      if (target) void switchTo(target.id, undefined, true);
      return;
    }
    const shown = interrupt();
    neighbors();
    travel = swipeTravel(shown, geometry()) + accumX;
    samples = [{ time: now, dx: accumX }];
    phase = "tracking";
    tracking.value = true;
  } else {
    travel += dx;
    samples.push({ time: now, dx });
    // Keep enough packets to distinguish deceleration from a single noisy delta;
    // swipeVelocity still limits release speed to its own 80ms time window.
    samples = samples.filter((s, i) => samples.length - i <= 5 || now - s.time <= SPACE_SWIPE.velocityWindow);
  }
  event.preventDefault();
  const pulled = swipeOffset(travel, geometry());
  offset.value = reduced.value ? 0 : pulled;
  updateProgress();
  const direction = swipeLanding(pulled, swipeVelocity(samples, now), width.value);
  // Coasting releases both a successful swipe and a spring home. At the edges
  // there is no target space; its inertia must not keep stretching the list.
  if (swipeCoasting(samples)
    || (direction && b.state.spaces[index.value + direction] && Math.abs(pulled) >= width.value)) {
    finish();
    return;
  }
  clearRelease();
  release = setTimeout(() => finish(), swipeReleaseDelay(samples));
}

function reset() {
  clearRelease(); interrupt(); phase = "idle"; tracking.value = false;
  offset.value = 0; panes.value = [{ id: center.value, slot: 0 }]; emit("progress", null);
}
function motionChanged() { systemReduced.value = motion.matches; reset(); }
watch(() => b.settings.reduceMotion, reset);
function visibilityChanged() { if (document.hidden) cancel(); }
watch(() => b.state.activeSpace, (id) => { if (id !== center.value) void switchTo(id); });
watch(() => props.blocked || b.state.sidebarCollapsed, (blocked) => { if (blocked) cancel(); });
onMounted(() => {
  observer = new ResizeObserver(() => {
    const measured = viewport.value?.closest(".sidebar")?.getBoundingClientRect().width;
    if (measured && measured !== width.value) { reset(); width.value = measured; }
  });
  if (viewport.value) observer.observe(viewport.value);
  motion.addEventListener("change", motionChanged);
  window.addEventListener("blur", cancel);
  document.addEventListener("visibilitychange", visibilityChanged);
});
onBeforeUnmount(() => {
  clearRelease(); generation++; observer?.disconnect(); animations.forEach((a) => a.cancel());
  motion.removeEventListener("change", motionChanged); window.removeEventListener("blur", cancel);
  document.removeEventListener("visibilitychange", visibilityChanged);
});
defineExpose({ wheel });
</script>

<template>
  <div ref="viewport" class="space-viewport" :class="{ 'space-tracking': tracking, 'space-settling': settling, 'space-reduced': reduced }">
    <div ref="track" class="space-track" :style="{ transform: `translateX(${offset}px)` }">
      <SpaceList v-for="pane in panes" :key="pane.id" :space-id="pane.id" :saved-scroll="scroll.get(pane.id) || 0"
        :data-pane-space="pane.id" :inert="pane.id !== b.state.activeSpace" :aria-hidden="pane.id !== b.state.activeSpace"
        :style="{ transform: reduced ? 'none' : `translateX(${pane.slot * width}px)` }"
        @scroll="remember" @menu="(e, t) => emit('menu', e, t)" />
    </div>
  </div>
</template>
