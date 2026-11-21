<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useBrowser } from "../../stores/browser";

const b = useBrowser();
const heading = ref<HTMLElement>();
const art = ref<SVGSVGElement>();
const hasFinished = ref(false);
const reducedMotion = matchMedia("(prefers-reduced-motion: reduce)");
const duration = 2800;
let strokes: { path: SVGPathElement; length: number }[] = [];
let totalLength = 0;
let frame = 0;

function draw(distance: number) {
  for (const { path, length } of strokes) {
    const drawn = Math.min(length, Math.max(0, distance));
    path.style.strokeDashoffset = String(length - drawn);
    path.style.visibility = drawn > 0 ? "visible" : "hidden";
    distance -= length;
  }
}

function showComplete() {
  cancelAnimationFrame(frame);
  draw(totalLength);
  hasFinished.value = true;
}

function play() {
  if (reducedMotion.matches || b.settings.reduceMotion) {
    showComplete();
    return;
  }
  cancelAnimationFrame(frame);
  draw(0);
  let start: number | undefined;
  const tick = (now: number) => {
    start ??= now;
    const time = Math.min((now - start) / duration, 1);
    // Ease the shared pen distance so both segments follow the same pace.
    const progress = 0.18 * time + 0.82 * time * time * (3 - 2 * time);
    draw(progress * totalLength);
    if (time < 1) frame = requestAnimationFrame(tick);
    else hasFinished.value = true;
  };
  frame = requestAnimationFrame(tick);
}

function syncMotion() {
  if (reducedMotion.matches || b.settings.reduceMotion) showComplete();
}

watch(() => b.settings.reduceMotion, syncMotion);
onMounted(() => {
  heading.value?.focus({ preventScroll: true });
  strokes = [...art.value!.querySelectorAll("path")].map(path => {
    const length = path.getTotalLength();
    path.style.strokeDasharray = `${length} ${length}`;
    return { path, length };
  });
  totalLength = strokes.reduce((total, stroke) => total + stroke.length, 0);
  reducedMotion.addEventListener("change", syncMotion);
  play();
});
onBeforeUnmount(() => {
  cancelAnimationFrame(frame);
  reducedMotion.removeEventListener("change", syncMotion);
});
</script>

<template>
  <section class="onboarding-greeting" aria-labelledby="onboarding-title">
    <h1 id="onboarding-title" ref="heading" class="sr-only" tabindex="-1">引导完成</h1>
    <button class="onboarding-greeting-replay" type="button" aria-label="重播 hello 手写动画" @click="play">
      <svg ref="art" class="onboarding-greeting-art" viewBox="44 6 184 80" fill="none" stroke="currentColor" stroke-width="6.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <path visibility="hidden" d="M50.7 63.6
          C63.3 53.4 73.6 42.7 73.6 27.2
          C73.6 19.4 70.7 13.2 65 13.2
          C59.4 13.2 56.8 18.7 56.8 26.9
          L56.8 78.6" />
        <path visibility="hidden" d="M56.8 63.3
          C60 56.9 63.8 50 70.1 50
          C78 50 80.3 56.9 80.3 65.8
          C80.3 74.6 82.1 79 88.2 79
          C96.2 79 114.9 68.8 114.9 57.6
          C114.9 52.2 111.6 49.2 107.2 49.2
          C101.2 49.2 97.6 54.8 97.6 62.3
          C97.6 72.8 103.3 79 113.4 79
          C128.9 79 149.5 53.3 149.5 29.4
          C149.5 19.5 146.8 13.3 141 13.3
          C134.7 13.3 132 20.4 132 30.9
          L132 62.4
          C132 73.2 135.8 79 145.1 79
          C161.2 79 181.5 53.4 181.5 29.4
          C181.5 19.4 178.8 13.3 173 13.3
          C166.6 13.3 163.9 20.4 163.9 30.9
          L163.9 62.4
          C163.9 73.2 167.7 79 176.5 79
          C184.8 79 190.2 73.9 193.7 65.9
          C193.4 56.7 199 49.5 207.1 49.5
          C215.2 49.5 220.8 56 220.8 64.3
          C220.8 72.6 215 78.9 207.1 78.9
          C199.2 78.9 193.6 73.2 193.7 65.9" />
      </svg>
    </button>
    <button v-if="hasFinished" class="onboarding-primary onboarding-primary-light onboarding-greeting-continue" type="button" @click="b.dismissOnboardingGreeting()">开始浏览</button>
  </section>
</template>
