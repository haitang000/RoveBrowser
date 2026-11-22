<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";

const props = defineProps<{ title: string; reduced: boolean }>();
const viewport = ref<HTMLElement>();
const line = ref<HTMLElement>();
const scrolling = ref(false);
const systemMotion = matchMedia("(prefers-reduced-motion: reduce)");
let observer: ResizeObserver | undefined;
let visibilityObserver: IntersectionObserver | undefined;
let animation: Animation | undefined;
let visible = false;
let lastTitle = "";
let lastDistance = 0;

function measure() {
  if (!viewport.value || !line.value) return;
  const distance = Math.max(0, line.value.scrollWidth - viewport.value.clientWidth);
  const enabled = visible && viewport.value.clientWidth > 0 && distance > 2 && !props.reduced && !systemMotion.matches;
  if (animation && enabled && distance === lastDistance && props.title === lastTitle) return;
  animation?.cancel();
  animation = undefined;
  scrolling.value = enabled;
  if (!enabled) return;

  lastTitle = props.title;
  lastDistance = distance;
  const pause = 1000;
  const duration = Math.max(1000, distance / 30 * 1000) + 2 * pause;
  const direction = getComputedStyle(viewport.value).direction === "rtl" ? 1 : -1;
  const end = `translateX(${direction * distance}px)`;
  animation = line.value.animate([
    { transform: "translateX(0)", offset: 0 },
    { transform: "translateX(0)", offset: pause / duration },
    { transform: end, offset: 1 - pause / duration },
    { transform: end, offset: 1 },
  ], { duration, iterations: Infinity, direction: "alternate" });
}

watch(() => [props.title, props.reduced], measure, { flush: "post" });
onMounted(() => {
  observer = new ResizeObserver(measure);
  visibilityObserver = new IntersectionObserver(([entry]) => {
    visible = entry.intersectionRatio > 0;
    measure();
  }, { threshold: 0.1 });
  if (viewport.value) {
    observer.observe(viewport.value);
    visibilityObserver.observe(viewport.value);
  }
  systemMotion.addEventListener("change", measure);
  void document.fonts.ready.then(measure);
  measure();
});
onBeforeUnmount(() => {
  observer?.disconnect();
  visibilityObserver?.disconnect();
  systemMotion.removeEventListener("change", measure);
  animation?.cancel();
});
</script>

<template>
  <span ref="viewport" class="now-playing-title-viewport" :class="{ scrolling }" dir="auto">
    <span ref="line" class="now-playing-title-text">{{ title }}</span>
  </span>
</template>

<style scoped>
.now-playing-title-viewport {
  display: block;
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
}
.now-playing-title-text {
  display: block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.scrolling .now-playing-title-text {
  width: max-content;
}
.scrolling {
  mask-image: linear-gradient(to right, transparent, #000 4px, #000 calc(100% - 8px), transparent);
}
</style>
