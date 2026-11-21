<script setup lang="ts">
// Copyright 2026 Zigao Wang, Apache-2.0. UI adapted for Rove.
import { computed, nextTick, onScopeDispose, watch } from "vue";
import { useBrowser } from "../../stores/browser";
import { tabDisplayTitle } from "../../lib/tab-meta";
import type { Tab } from "../../lib/types";
import Favicon from "../common/Favicon.vue";
import Icon from "../common/Icon.vue";
import NowPlayingTitle from "./NowPlayingTitle.vue";

const b = useBrowser();
const cards = computed(() => b.nowPlayingCards.map((tab) => {
  const media = b.nowPlayingDetails.get(tab.id);
  const colors = media?.colors?.map((rgb) => `rgb(${rgb.join(" ")})`);
  return {
    tab,
    title: media?.title || tabDisplayTitle(tab, b.language) || tab.url,
    paused: b.mediaPaused(tab.id),
    pending: b.mediaPending(tab.id),
    glow: colors ? {
      "--playing-glow-one": colors[0],
      "--playing-glow-two": colors[1],
      "--playing-glow-three": colors[2],
    } : undefined,
  };
}));

const playingCards = computed(() => b.native && !b.state.sidebarCollapsed
  ? b.nowPlayingCards.filter((tab) => b.audio[tab.id])
  : []);
let refreshTimer: number | undefined;
function refreshArt() {
  if (!document.hidden) playingCards.value.forEach(b.requestNowPlayingDetails);
}
watch(() => JSON.stringify(playingCards.value.map(({ id, url }) => [id, url])), () => {
  window.clearInterval(refreshTimer);
  if (!playingCards.value.length) return;
  refreshArt();
  refreshTimer = window.setInterval(refreshArt, 1600);
}, { immediate: true });
onScopeDispose(() => window.clearInterval(refreshTimer));

async function hide(tab: Tab, event: MouseEvent) {
  const index = cards.value.findIndex((card) => card.tab.id === tab.id);
  b.hideMedia(tab);
  if (event.detail !== 0) return;
  const stack = (event.currentTarget as HTMLButtonElement).closest(".now-playing-stack")!;
  await nextTick();
  const titles = stack.querySelectorAll<HTMLButtonElement>(".now-playing-card:not(.now-playing-leave-active) .now-playing-title");
  const next = titles[Math.min(index, titles.length - 1)];
  (next ?? stack.closest(".sidebar")?.querySelector<HTMLButtonElement>(".browser-menu-trigger"))?.focus();
}
</script>

<template>
  <TransitionGroup
    name="now-playing"
    tag="section"
    class="now-playing-stack"
    :aria-label="b.t('正在播放')"
    :aria-hidden="cards.length === 0 || undefined"
  >
    <article
      v-for="{ tab, title, paused, pending, glow } in cards"
      :key="tab.id"
      class="now-playing-card"
      :class="{
        'has-artwork': glow,
        'artwork-playing': glow && !paused && !b.state.sidebarCollapsed,
        'artwork-dark': b.state.theme === 'dark',
        'motion-reduced': b.settings.reduceMotion,
      }"
      role="group"
      :aria-label="title"
    >
      <span class="now-playing-glow" :style="glow" aria-hidden="true" />
      <button
        type="button"
        class="now-playing-title"
        :aria-label="b.t('转到正在播放的标签页：{name}', { name: title })"
        :title="title + ' · ' + tab.url"
        @click="b.send({ type: 'activate', id: tab.id })"
      >
        <Favicon :url="tab.url" :favicon="tab.favicon" />
        <NowPlayingTitle :title="title" :reduced="b.settings.reduceMotion || b.state.sidebarCollapsed" />
      </button>
      <button
        type="button"
        class="now-playing-action now-playing-toggle"
        :aria-disabled="pending || undefined"
        :aria-label="b.t(paused ? '继续播放' : '暂停播放')"
        :title="b.t(paused ? '继续播放' : '暂停播放')"
        @click="b.toggleMedia(tab)"
      ><Icon :name="paused ? 'play' : 'pause'" :size="13" /></button>
      <button
        type="button"
        class="now-playing-action"
        :aria-label="b.t(tab.muted ? '取消静音标签页' : '静音标签页')"
        :title="b.t(tab.muted ? '取消静音标签页' : '静音标签页')"
        @click="b.toggleMute(tab)"
      ><Icon :name="tab.muted ? 'speakerMuted' : 'speaker'" :size="13" /></button>
      <button
        type="button"
        class="now-playing-action"
        :aria-label="b.t('隐藏播放卡片')"
        :title="b.t('隐藏播放卡片')"
        @click="hide(tab, $event)"
      ><Icon name="close" :size="12" /></button>
    </article>
  </TransitionGroup>
</template>

<style scoped>
.now-playing-stack {
  position: relative;
  display: grid;
  flex: 0 0 auto;
  gap: 4px;
}
.now-playing-stack:not(:empty) { margin: 4px 0; }
.now-playing-card {
  position: relative;
  isolation: isolate;
  display: flex;
  align-items: center;
  gap: var(--row-gap);
  min-width: 0;
  height: calc(var(--row-height) + 10px);
  padding-inline: 8px 5px;
  border-radius: var(--radius-row);
  background: var(--field);
  color: var(--ink);
  font-size: 13px;
}
.now-playing-card:is(:hover, :focus-within) { background: var(--field-hover); }
@property --playing-glow-one { syntax: "<color>"; inherits: true; initial-value: transparent; }
@property --playing-glow-two { syntax: "<color>"; inherits: true; initial-value: transparent; }
@property --playing-glow-three { syntax: "<color>"; inherits: true; initial-value: transparent; }
.now-playing-glow {
  position: absolute;
  z-index: -1;
  inset: 0;
  overflow: hidden;
  pointer-events: none;
  border-radius: inherit;
  opacity: 0;
  box-shadow:
    0 3px 10px -3px color-mix(in srgb, var(--playing-glow-one) 50%, transparent),
    0 5px 13px -6px color-mix(in srgb, var(--playing-glow-two) 42%, transparent);
  transition: opacity var(--motion-standard) ease,
    --playing-glow-one 280ms ease, --playing-glow-two 280ms ease, --playing-glow-three 280ms ease;
}
.now-playing-glow::before {
  content: "";
  position: absolute;
  inset: -20% -10%;
  background:
    radial-gradient(ellipse at 14% 6%, var(--playing-glow-one), transparent 68%),
    radial-gradient(ellipse at 84% 17%, var(--playing-glow-two), transparent 65%),
    radial-gradient(ellipse at 43% 114%, var(--playing-glow-three), transparent 72%),
    var(--surface);
  animation: now-playing-drift 16s ease-in-out infinite alternate paused;
}
/* Keep a consistent text contrast across covers with very different brightness. */
.now-playing-glow::after {
  content: "";
  position: absolute;
  inset: 0;
  background: color-mix(in srgb, var(--surface) 65%, transparent);
}
.artwork-dark .now-playing-glow::after {
  background: color-mix(in srgb, var(--surface) 69%, transparent);
}
.has-artwork .now-playing-glow { opacity: 1; }
.artwork-playing .now-playing-glow::before { animation-play-state: running; }
@keyframes now-playing-drift {
  from { transform: translate(-3%, -5%) scale(1.02); }
  to { transform: translate(4%, 6%) scale(1.08); }
}
.now-playing-title {
  display: flex;
  align-items: center;
  flex: 1 1 auto;
  gap: var(--icon-gap);
  min-width: 0;
  height: 30px;
  padding: 0;
  text-align: start;
  border-radius: var(--radius-control);
}
.now-playing-title :deep(.favicon) {
  width: var(--favicon);
  height: var(--favicon);
  flex: 0 0 var(--favicon);
}
.now-playing-action {
  display: grid;
  flex: 0 0 22px;
  place-items: center;
  width: 22px;
  height: 22px;
  border-radius: var(--radius-small);
  color: var(--quiet);
}
.now-playing-toggle { color: var(--ink); }
.now-playing-action:hover { background: var(--pressed); color: var(--ink); }
.now-playing-action:active { background: var(--selected); }
.now-playing-action[aria-disabled="true"] { opacity: 0.55; cursor: progress; }
.now-playing-action :deep(svg) { color: currentColor; }
.has-artwork .now-playing-action { color: var(--ink); }
.has-artwork .now-playing-action:hover { background: color-mix(in srgb, var(--ink) 12%, transparent); }
.now-playing-enter-active,
.now-playing-leave-active,
.now-playing-move { transition: opacity var(--motion-selection) var(--ease-out), transform var(--motion-selection) var(--ease-out); }
.now-playing-enter-from,
.now-playing-leave-to { opacity: 0; transform: translateY(3px); }
.now-playing-leave-active {
  position: absolute;
  inset-inline: 0;
  pointer-events: none;
}
.now-playing-card.motion-reduced { transition: none; }
.motion-reduced .now-playing-glow::before { animation: none; }
@media (prefers-reduced-motion: reduce) {
  .now-playing-card { transition: none; }
  .now-playing-glow::before { animation: none; }
}
</style>
