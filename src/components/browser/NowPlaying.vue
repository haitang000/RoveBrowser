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
    playing: b.audio[tab.id] && !b.mediaPaused(tab.id) && !b.state.sidebarCollapsed,
    playback: media?.playback,
    glow: colors ? {
      "--playing-glow-one": colors[0],
      "--playing-glow-two": colors[1],
      "--playing-glow-three": colors[2],
    } : undefined,
  };
}));

const sampledCards = computed(() => b.native && !b.state.sidebarCollapsed ? b.nowPlayingCards : []);
let refreshTimer: number | undefined;
function refreshDetails() {
  if (!document.hidden) sampledCards.value.forEach(b.requestNowPlayingDetails);
}
watch(() => JSON.stringify(sampledCards.value.map(({ id, url }) => [id, url])), () => {
  window.clearInterval(refreshTimer);
  if (!sampledCards.value.length) return;
  refreshDetails();
  refreshTimer = window.setInterval(refreshDetails, 1000);
}, { immediate: true });
onScopeDispose(() => window.clearInterval(refreshTimer));

function formatTime(seconds: number | null | undefined) {
  if (seconds == null) return "–:––";
  const whole = Math.floor(seconds);
  const minutes = Math.floor(whole / 60) % 60;
  const hours = Math.floor(whole / 3600);
  const remainder = String(whole % 60).padStart(2, "0");
  return hours ? `${hours}:${String(minutes).padStart(2, "0")}:${remainder}` : `${minutes}:${remainder}`;
}

function seek(tab: Tab, event: Event) {
  const input = event.currentTarget as HTMLInputElement;
  b.controlMedia(tab, { seek: { position: input.valueAsNumber } });
}

function previewSeek(event: Event) {
  const input = event.currentTarget as HTMLInputElement;
  input.style.setProperty("--progress", `${input.valueAsNumber / Number(input.max) * 100}%`);
  input.setAttribute("aria-valuetext", formatTime(input.valueAsNumber));
}

async function hide(tab: Tab, event: MouseEvent) {
  const index = cards.value.findIndex((card) => card.tab.id === tab.id);
  b.hideMedia(tab);
  if (event.detail !== 0) return;
  const stack = (event.currentTarget as HTMLButtonElement).closest(".now-playing-stack")!;
  await nextTick();
  const sources = stack.querySelectorAll<HTMLButtonElement>(".now-playing-card:not(.now-playing-leave-active) .now-playing-source");
  const next = sources[Math.min(index, sources.length - 1)];
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
      v-for="{ tab, title, paused, pending, playing, playback, glow } in cards"
      :key="tab.id"
      class="now-playing-card"
      :class="{
        'has-artwork': glow,
        'artwork-playing': glow && playing,
        'artwork-dark': b.state.theme === 'dark',
        'motion-reduced': b.settings.reduceMotion,
      }"
      role="group"
      :aria-label="title"
    >
      <span class="now-playing-glow" :style="glow" aria-hidden="true" />
      <div class="now-playing-controls">
        <button
          type="button"
          class="now-playing-action now-playing-source"
          :aria-label="b.t('转到正在播放的标签页：{name}', { name: title })"
          :title="title + ' · ' + tab.url"
          @click="b.send({ type: 'activate', id: tab.id })"
        >
          <Favicon :url="tab.url" :favicon="tab.favicon" />
          <span v-if="playing" class="now-playing-notes" aria-hidden="true">
            <Icon name="musicNotes" :size="13" />
            <Icon name="musicNote" :size="10" />
            <Icon name="musicNotes" :size="9" />
          </span>
        </button>
        <button type="button" class="now-playing-action" :disabled="!playback?.previous"
          :aria-label="b.t('上一首')" :title="b.t('上一首')" @click="b.controlMedia(tab, 'previous')"
        ><Icon name="mediaPrevious" :size="17" /></button>
        <button type="button" class="now-playing-action now-playing-toggle" :aria-disabled="pending || undefined"
          :aria-label="b.t(paused ? '继续播放' : '暂停播放')" :title="b.t(paused ? '继续播放' : '暂停播放')"
          @click="b.toggleMedia(tab)"
        ><Icon :name="paused ? 'mediaPlay' : 'mediaPause'" :size="17" /></button>
        <button type="button" class="now-playing-action" :disabled="!playback?.next"
          :aria-label="b.t('下一首')" :title="b.t('下一首')" @click="b.controlMedia(tab, 'next')"
        ><Icon name="mediaNext" :size="17" /></button>
        <button type="button" class="now-playing-action"
          :aria-label="b.t(tab.muted ? '取消静音标签页' : '静音标签页')"
          :title="b.t(tab.muted ? '取消静音标签页' : '静音标签页')" @click="b.toggleMute(tab)"
        ><Icon :name="tab.muted ? 'mediaMuted' : 'mediaVolume'" :size="17" /></button>
      </div>
      <div class="now-playing-expansion">
        <div class="now-playing-details">
          <div class="now-playing-header">
            <NowPlayingTitle :title="title" :reduced="b.settings.reduceMotion || b.state.sidebarCollapsed" />
            <button type="button" class="now-playing-action" :disabled="!playback?.pictureInPicture"
              :aria-label="b.t('画中画')" :title="b.t('画中画')" @click="b.controlMedia(tab, 'pictureInPicture')"
            ><Icon name="mediaPictureInPicture" :size="16" /></button>
            <button type="button" class="now-playing-action"
              :aria-label="b.t('隐藏播放卡片')" :title="b.t('隐藏播放卡片')" @click="hide(tab, $event)"
            ><Icon name="close" :size="16" /></button>
          </div>
          <div class="now-playing-timeline">
            <span>{{ formatTime(playback?.position) }}</span>
            <input type="range" class="now-playing-progress" min="0" :max="playback?.duration ?? 1" step="any"
              :value="playback?.position ?? 0" :aria-label="b.t('播放进度')"
              :aria-valuetext="formatTime(playback?.position)" :disabled="!playback?.seekable"
              :style="{ '--progress': `${playback?.duration ? (playback.position ?? 0) / playback.duration * 100 : 0}%` }"
              @input="previewSeek"
              @change="seek(tab, $event)"
            />
            <span>{{ formatTime(playback?.duration) }}</span>
          </div>
        </div>
      </div>
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
  --playing-reveal: var(--motion-standard);
  position: relative;
  isolation: isolate;
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  grid-template-rows: minmax(0, var(--playing-details-size, 0fr)) auto;
  min-width: 0;
  border-radius: var(--radius-field);
  background: color-mix(in srgb, var(--panel) 96%, var(--frame));
  color: var(--ink);
  font-size: 13px;
  transition: grid-template-rows var(--playing-reveal) var(--ease-out);
}
.now-playing-card:has(:focus-visible) {
  --playing-details-size: 1fr;
  --playing-details-visibility: visible;
  --playing-details-opacity: 1;
  --playing-notes-opacity: 0;
  --playing-source-background: var(--separator);
  --playing-reveal: 0ms;
}
@media (any-hover: hover) {
  .now-playing-card:hover {
    --playing-details-size: 1fr;
    --playing-details-visibility: visible;
    --playing-details-opacity: 1;
    --playing-notes-opacity: 0;
    --playing-source-background: var(--separator);
  }
}
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
.has-artwork .now-playing-glow { opacity: calc(var(--playing-details-opacity, 0) * 0.45); }
.artwork-playing .now-playing-glow::before { animation-play-state: running; }
@keyframes now-playing-drift {
  from { transform: translate(-3%, -5%) scale(1.02); }
  to { transform: translate(4%, 6%) scale(1.08); }
}
.now-playing-controls {
  display: grid;
  grid-row: 2;
  grid-template-columns: repeat(5, minmax(0, 1fr));
  align-items: center;
  justify-items: center;
  min-height: 36px;
}
.now-playing-source {
  position: relative;
  background: var(--playing-source-background, transparent);
}
.now-playing-source :deep(.favicon) {
  width: 17px;
  height: 17px;
  object-fit: contain;
}
.now-playing-action {
  display: grid;
  flex: 0 0 auto;
  place-items: center;
  width: 28px;
  height: 28px;
  border-radius: var(--radius-control);
  color: var(--icon);
  transition: background-color var(--motion-quick) ease, transform var(--motion-quick) var(--ease-out);
}
.now-playing-action:not(:disabled):hover { background: color-mix(in srgb, var(--ink) 10%, transparent); }
.now-playing-action:active { transform: scale(0.97); }
.now-playing-toggle[aria-disabled="true"] { opacity: 0.55; cursor: progress; }
.now-playing-notes {
  position: absolute;
  inset: 0;
  pointer-events: none;
  opacity: var(--playing-notes-opacity, 1);
  transition: opacity var(--playing-reveal) ease;
}
.now-playing-notes > svg {
  position: absolute;
  left: 8px;
  bottom: 16px;
  opacity: 0;
  animation: now-playing-note 1.8s linear infinite;
  animation-delay: var(--note-delay, 0s);
}
.now-playing-notes > svg:nth-child(2) {
  left: 13px;
  --note-drift: 5px;
  --note-delay: -0.6s;
}
.now-playing-notes > svg:nth-child(3) {
  --note-drift: -2px;
  --note-delay: -1.2s;
}
@keyframes now-playing-note {
  0% { opacity: 0; transform: translateY(2px) rotate(-10deg) scale(0.9); }
  20% { opacity: 0.78; }
  75% { opacity: 0.37; }
  100% { opacity: 0; transform: translate(var(--note-drift, -6px), -23px) rotate(10deg); }
}
.now-playing-expansion {
  grid-row: 1;
  min-height: 0;
  overflow: clip;
  visibility: var(--playing-details-visibility, hidden);
  opacity: var(--playing-details-opacity, 0);
  transition: opacity var(--playing-reveal) ease, visibility var(--playing-reveal);
}
.now-playing-details {
  padding: 5px 9px 3px 12px;
}
.now-playing-header {
  display: flex;
  align-items: center;
  gap: 2px;
  height: 28px;
  font-weight: 500;
}
.now-playing-timeline {
  display: grid;
  grid-template-columns: max-content minmax(0, 1fr) max-content;
  align-items: center;
  gap: 8px;
  min-height: 26px;
  color: var(--quiet);
  font-size: 11px;
  font-variant-numeric: tabular-nums;
  padding-inline-end: 3px;
}
.now-playing-progress {
  appearance: none;
  width: 100%;
  height: 16px;
  margin: 0;
  background: none;
  border-radius: var(--radius-small);
  cursor: pointer;
}
.now-playing-progress:disabled { cursor: default; }
.now-playing-progress::-webkit-slider-runnable-track {
  height: 4px;
  border-radius: 2px;
  background: linear-gradient(to right, var(--accent) var(--progress), var(--separator) var(--progress));
}
.now-playing-progress::-webkit-slider-thumb {
  appearance: none;
  width: 8px;
  height: 8px;
  margin-top: -2px;
  border-radius: 50%;
  background: var(--accent);
  opacity: 0;
  transition: opacity var(--motion-quick) ease;
}
.now-playing-progress:not(:disabled):is(:hover, :focus-visible)::-webkit-slider-thumb { opacity: 1; }
.now-playing-move,
.now-playing-enter-active,
.now-playing-leave-active {
  transition: grid-template-rows var(--playing-reveal) var(--ease-out),
    opacity var(--motion-selection) var(--ease-out), transform var(--motion-selection) var(--ease-out);
}
.now-playing-enter-from,
.now-playing-leave-to { opacity: 0; transform: translateY(3px); }
.now-playing-leave-active {
  position: absolute;
  inset-inline: 0;
  pointer-events: none;
}
.now-playing-card.motion-reduced { --playing-reveal: 0ms; transition: none; }
.motion-reduced .now-playing-action { transition-property: background-color; }
.motion-reduced .now-playing-action:active { transform: none; }
.motion-reduced .now-playing-glow::before,
.motion-reduced .now-playing-notes > svg { animation: none; }
.motion-reduced .now-playing-notes > svg:first-child { opacity: 0.65; transform: translate(-5px, -8px); }
@media (prefers-reduced-motion: reduce) {
  .now-playing-card { --playing-reveal: 0ms; transition: none; }
  .now-playing-action { transition-property: background-color; }
  .now-playing-action:active { transform: none; }
  .now-playing-glow::before,
  .now-playing-notes > svg { animation: none; }
  .now-playing-notes > svg:first-child { opacity: 0.65; transform: translate(-5px, -8px); }
}
</style>
