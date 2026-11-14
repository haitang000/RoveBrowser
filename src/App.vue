<script setup lang="ts">
import { computed, nextTick, onMounted, onBeforeUnmount, ref, watch } from "vue";
import { useBrowser } from "./stores/browser";
import Sidebar from "./components/browser/Sidebar.vue";
import SidebarResize from "./components/browser/SidebarResize.vue";
import WindowControls from "./components/browser/WindowControls.vue";
import PageArea from "./components/browser/PageArea.vue";
import CommandBar from "./components/browser/CommandBar.vue";
import Desk from "./components/browser/Desk.vue";
import SettingsPanel from "./components/browser/SettingsPanel.vue";
import TabPreview from "./components/browser/TabPreview.vue";
import FindBar from "./components/browser/FindBar.vue";
import StatusToast from "./components/browser/StatusToast.vue";
import ActionToast from "./components/browser/ActionToast.vue";
import ErrorToast from "./components/browser/ErrorToast.vue";
import OnboardingPage from "./components/browser/OnboardingPage.vue";
import Icon from "./components/common/Icon.vue";
import { browserKey } from "./lib/browser-key";
import { SPACE_SWIPE } from "./lib/space-swipe";
import { coverage, isSeeThrough } from "./lib/page-frame";
import { frameOf, lookOf } from "./lib/space-look";
import SpaceEditor from "./components/browser/SpaceEditor.vue";
import DownloadsPanel from "./components/browser/DownloadsPanel.vue";
const b = useBrowser();
const shell = ref<HTMLElement>();
const pageArea = ref<InstanceType<typeof PageArea>>();
const reducedTransparency = matchMedia("(prefers-reduced-transparency: reduce)");
const transparencyReduced = ref(reducedTransparency.matches);
const glassAllowed = computed(() => !transparencyReduced.value);
const shownSpace = computed(() => {
  const preview = b.spacePreview;
  return preview && preview.id === b.space?.id ? { ...b.space, ...preview.look } : b.space;
});
const painted = computed(() => frameOf(shownSpace.value, b.state.theme, glassAllowed.value));
const glassActive = computed(() => !b.onboardingOpen && isSeeThrough(shownSpace.value?.translucency) && glassAllowed.value);
watch(() => b.onboardingOpen, (open) => { document.documentElement.dataset.onboarding = String(open); }, { immediate: true, flush: "sync" });
const gradient = "linear-gradient(var(--frame-gradient-angle), var(--frame-gradient-start), var(--frame-gradient-end))";
const shellStyle = computed(() => ({
  ...(glassActive.value
    ? { "--frame-coverage": String(coverage(shownSpace.value?.translucency)) }
    : {}),
  ...(painted.value.custom
    ? {
        "--frame": painted.value.start,
        "--frame-h": String(painted.value.hue),
        "--shell-bg": painted.value.end ? gradient : painted.value.background,
        "--frame-gradient-start": frameColor(painted.value.start),
        "--frame-gradient-end": frameColor(painted.value.end ?? painted.value.start),
        "--frame-gradient-angle": `${painted.value.cssAngle}deg`,
      }
    : {}),
  "--grain-opacity": String(painted.value.grain * 0.18),
}));
function frameColor(color: string) {
  const alpha = glassActive.value ? coverage(shownSpace.value?.translucency) : 1;
  return `color-mix(in srgb, ${color} ${alpha * 100}%, transparent)`;
}
let frameAnimation: Animation | undefined;
let frameGeneration = 0;
let nativeGlass: boolean | undefined;
function syncNativeGlass(enabled: boolean) {
  if (!b.native || nativeGlass === enabled) return;
  nativeGlass = enabled;
  b.send({ type: "glass", enabled });
}
function cancelFrameAnimation() {
  frameGeneration++;
  frameAnimation?.cancel();
  frameAnimation = undefined;
}
function finishFrameAnimation() {
  cancelFrameAnimation();
  syncNativeGlass(glassActive.value);
  shell.value?.dispatchEvent(new Event("frame-background-change"));
}
watch(() => b.settings.reduceMotion, (value) => { document.documentElement.dataset.reduceMotion = String(value); if (value) finishFrameAnimation(); }, { immediate: true });
const reducedMotion = matchMedia("(prefers-reduced-motion: reduce)");
function frameSnapshot(element: HTMLElement) {
  const style = getComputedStyle(element);
  const hasGradient = style.backgroundImage !== "none";
  return {
    backgroundColor: style.backgroundColor,
    backgroundImage: style.backgroundImage,
    "--frame-gradient-start": hasGradient ? style.getPropertyValue("--frame-gradient-start").trim() : style.backgroundColor,
    "--frame-gradient-end": hasGradient ? style.getPropertyValue("--frame-gradient-end").trim() : style.backgroundColor,
    "--frame-gradient-angle": style.getPropertyValue("--frame-gradient-angle").trim(),
    "--grain-opacity": style.getPropertyValue("--grain-opacity").trim(),
  };
}
// Read the presentation before cancelling, including alpha and gradient stops.
// Only the background changes; chrome and page content remain interactive.
watch(
  // Saving a width or receiving a tab title must not restart the color tween.
  () => JSON.stringify([b.state.theme, lookOf(shownSpace.value), glassActive.value]),
  async () => {
    const theme = painted.value.theme ?? b.state.theme;
    const from = shell.value ? frameSnapshot(shell.value) : undefined;
    cancelFrameAnimation();
    const ticket = frameGeneration;
    document.documentElement.dataset.theme = theme;
    document.documentElement.dataset.color = painted.value.custom ? "custom" : b.space.color;
    if (painted.value.custom) {
      document.documentElement.style.setProperty("--frame", painted.value.start);
      document.documentElement.style.setProperty("--frame-h", String(painted.value.hue));
    } else {
      document.documentElement.style.removeProperty("--frame");
      document.documentElement.style.removeProperty("--frame-h");
    }
    // Enable the backdrop before alpha begins changing. Keep it behind an
    // outgoing glass space until the frame has become completely opaque.
    if (glassActive.value) syncNativeGlass(true);
    if (!shell.value || !from || reducedMotion.matches || b.settings.reduceMotion || transparencyReduced.value) {
      syncNativeGlass(glassActive.value);
      return;
    }
    await nextTick();
    if (ticket !== frameGeneration || !shell.value) return;
    const to = frameSnapshot(shell.value);
    if (JSON.stringify(from) === JSON.stringify(to)) {
      syncNativeGlass(glassActive.value);
      return;
    }
    const frames: Keyframe[] = [from, to];
    if (from.backgroundImage !== "none" || to.backgroundImage !== "none") {
      // A solid is a gradient with identical stops, so transitions to/from a
      // gradient can interpolate without an opaque overlay or an alpha flash.
      for (const frame of frames) {
        frame.backgroundColor = "transparent";
        frame.backgroundImage = gradient;
      }
    }
    const animation = shell.value.animate(frames, {
      duration: SPACE_SWIPE.backgroundDuration, easing: "ease",
    });
    frameAnimation = animation;
    void animation.finished.then(() => {
      if (ticket === frameGeneration) finishFrameAnimation();
    }).catch(() => { /* A newer switch owns the background. */ });
  },
  { immediate: true },
);
watch(
  [() => b.overlay, () => b.commandOpen],
  ([visible, command]) => b.send({ type: "overlay", visible, command }),
);
function syncTransparency() { transparencyReduced.value = reducedTransparency.matches; }
const key = (event: KeyboardEvent) => browserKey(b, event);
function dismissCommand() { if (b.native) b.commandOpen = false; }

onMounted(async () => {
  window.addEventListener("keydown", key);
  document.addEventListener("pointerdown", dismissCommand, true);
  reducedMotion.addEventListener("change", finishFrameAnimation);
  reducedTransparency.addEventListener("change", syncTransparency);
  if (b.native) document.documentElement.dataset.native = "true";
  if (b.native) {
    // Wait for the mounted shell, fonts and a paint opportunity before revealing
    // the native window. Its session and theme are already present on first render.
    await document.fonts.ready;
    requestAnimationFrame(() => {
      requestAnimationFrame(() => b.send({ type: "presented" }));
    });
  }
});
onBeforeUnmount(() => {
  window.removeEventListener("keydown", key);
  document.removeEventListener("pointerdown", dismissCommand, true);
  reducedMotion.removeEventListener("change", finishFrameAnimation);
  reducedTransparency.removeEventListener("change", syncTransparency);
  finishFrameAnimation();
});
</script>
<template>
  <div
    ref="shell"
    class="browser-shell"
    :class="{
      collapsed: b.state.sidebarCollapsed,
      'sidebar-resizing': b.sidebarResizing,
      glass: glassActive,
      'glass-neutral': glassActive && painted.neutralGlass,
      'custom-frame': painted.custom,
    }"
    :style="shellStyle"
  >
    <WindowControls />
    <OnboardingPage v-if="b.onboardingOpen" />
    <template v-else>
    <div class="browser-body">
      <div
        class="sidebar-track"
        :inert="b.state.sidebarCollapsed"
        :aria-hidden="b.state.sidebarCollapsed"
      >
        <Sidebar />
        <SidebarResize @resize="pageArea?.layout()" />
      </div>
      <button
        v-if="b.state.sidebarCollapsed"
        class="reveal-sidebar icon-button"
        aria-label="展开侧栏"
        title="展开侧栏 (Ctrl+B)"
        @click="b.send({ type: 'sidebar' })"
      >
        <Icon name="sidebar" /></button
      ><PageArea ref="pageArea"><CommandBar v-if="!b.native" /><FindBar v-if="!b.native" /><StatusToast v-if="!b.native" /><ActionToast v-if="!b.native" /></PageArea>
    </div>
    <Desk v-if="!b.native" /><SettingsPanel v-if="!b.native" /><SpaceEditor v-if="b.spaceEditor" :key="b.spaceEditor" /><TabPreview />
    <ErrorToast v-if="!b.native" /><DownloadsPanel v-if="!b.native" />
    </template>
  </div>
</template>
