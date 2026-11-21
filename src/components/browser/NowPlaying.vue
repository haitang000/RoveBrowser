<script setup lang="ts">
// Copyright 2026 Zigao Wang, Apache-2.0; adapted for Rove.
// Reference: https://github.com/ZigaoWang/zigzag/blob/main/Zigzag/Sidebar/NowPlaying.swift
import { computed, ref, watch } from "vue";
import { useBrowser } from "../../stores/browser";
import { tabDisplayTitle } from "../../lib/tab-meta";
import type { Tab } from "../../lib/types";
import Favicon from "../common/Favicon.vue";
import Icon from "../common/Icon.vue";

const b = useBrowser();
const pausedFromCard = ref(new Set<string>());
const hiddenPages = ref(new Map<string, string>());
const pages = ref(new Map<string, string>());
const order = ref<string[]>([]);
const maxCards = 3;

function pageOf(url: string) {
  try {
    const page = new URL(url);
    page.hash = "";
    return page.href;
  } catch {
    return url;
  }
}

function reconcile() {
  const tabs = b.state.tabs;
  const byId = new Map(tabs.map((tab) => [tab.id, tab]));
  const paused = new Set(pausedFromCard.value);
  const knownPages = new Map(pages.value);
  for (const [id] of knownPages) if (!byId.has(id)) knownPages.delete(id);
  for (const tab of tabs) {
    const page = pageOf(tab.url);
    if (knownPages.has(tab.id) && knownPages.get(tab.id) !== page) paused.delete(tab.id);
    knownPages.set(tab.id, page);
  }
  pages.value = knownPages;
  for (const id of paused) {
    if (!byId.has(id) || b.audio[id]) paused.delete(id);
  }
  pausedFromCard.value = paused;

  const hidden = new Map(hiddenPages.value);
  for (const [id, page] of hidden) {
    const tab = byId.get(id);
    if (!tab || pageOf(tab.url) !== page) hidden.delete(id);
  }
  hiddenPages.value = hidden;

  const sounding = new Set(tabs.filter((tab) => b.audio[tab.id] || paused.has(tab.id)).map((tab) => tab.id));
  order.value = order.value.filter((id) => sounding.has(id));
  const known = new Set(order.value);
  for (const tab of [...tabs].reverse()) {
    if (sounding.has(tab.id) && !known.has(tab.id)) {
      known.add(tab.id);
      order.value.push(tab.id);
    }
  }
}

watch(() => [b.state.tabs, b.state.activeTab, b.audio], reconcile, { deep: true, immediate: true });

const cards = computed(() => {
  const tabs = new Map(b.state.tabs.map((tab) => [tab.id, tab]));
  const selected = new Set([b.state.activeTab, b.splitPartner?.id].filter((id): id is string => Boolean(id)));
  return order.value
    .filter((id) => {
      const tab = tabs.get(id);
      return tab && !selected.has(id) && hiddenPages.value.get(id) !== pageOf(tab.url);
    })
    .slice(-maxCards)
    .reverse()
    .flatMap((id) => {
      const tab = tabs.get(id);
      return tab ? [tab] : [];
    });
});

function titleOf(tab: Tab) {
  return tabDisplayTitle(tab, b.language) || tab.url;
}

function activate(tab: Tab) {
  if (tab.bookmarkId && tab.unloaded) b.send({ type: "bookmarkOpen", id: tab.bookmarkId });
  else b.send({ type: "activate", id: tab.id });
}

function togglePlayback(tab: Tab) {
  const paused = pausedFromCard.value.has(tab.id);
  if (!paused) pausedFromCard.value = new Set(pausedFromCard.value).add(tab.id);
  b.send({ type: "mediaControl", id: tab.id, action: paused ? "play" : "pause" });
}

function hide(tab: Tab) {
  hiddenPages.value = new Map(hiddenPages.value).set(tab.id, pageOf(tab.url));
}

function isPaused(tab: Tab) {
  return pausedFromCard.value.has(tab.id);
}
</script>

<template>
  <TransitionGroup
    v-if="cards.length"
    name="now-playing"
    tag="section"
    class="now-playing-stack"
    :aria-label="b.t('正在播放')"
  >
    <article v-for="tab in cards" :key="tab.id" class="now-playing-card">
      <button
        class="now-playing-title"
        :aria-label="b.t('转到正在播放的标签页：{name}', { name: titleOf(tab) })"
        :title="`${titleOf(tab)} · ${tab.url}`"
        @click="activate(tab)"
      >
        <Favicon :url="tab.url" :favicon="tab.favicon" />
        <span>{{ titleOf(tab) }}</span>
      </button>
      <button
        class="now-playing-action"
        :aria-label="b.t(isPaused(tab) ? '继续播放' : '暂停播放')"
        :title="b.t(isPaused(tab) ? '继续播放' : '暂停播放')"
        @click="togglePlayback(tab)"
      ><Icon :name="isPaused(tab) ? 'play' : 'pause'" :size="13" /></button>
      <button
        class="now-playing-action"
        :aria-label="b.t(tab.muted ? '取消静音标签页' : '静音标签页')"
        :title="b.t(tab.muted ? '取消静音标签页' : '静音标签页')"
        @click="b.toggleMute(tab)"
      ><Icon :name="tab.muted ? 'speakerMuted' : 'speaker'" :size="13" /></button>
      <button
        class="now-playing-action"
        :aria-label="b.t('隐藏播放卡片')"
        :title="b.t('隐藏播放卡片')"
        @click="hide(tab)"
      ><Icon name="close" :size="12" /></button>
    </article>
  </TransitionGroup>
</template>

<style scoped>
.now-playing-stack {
  position: relative;
  display: grid;
  flex: 0 0 auto;
  gap: 2px;
  padding: 5px 0 4px;
}
.now-playing-card {
  display: flex;
  align-items: center;
  gap: 2px;
  height: 30px;
  min-width: 0;
  padding: 0 4px 0 5px;
  border-radius: 6px;
  background: var(--field);
  color: var(--ink);
  font-size: 13px;
}
.now-playing-card:hover { background: var(--field-hover); }
.now-playing-title {
  display: flex;
  align-items: center;
  flex: 1 1 auto;
  gap: 8px;
  min-width: 0;
  height: 28px;
  padding: 0;
  text-align: start;
}
.now-playing-title :deep(.favicon) { width: 16px; height: 16px; flex: 0 0 16px; }
.now-playing-title > span {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.now-playing-action {
  display: grid;
  flex: 0 0 20px;
  place-items: center;
  width: 20px;
  height: 20px;
  border-radius: 4px;
  color: var(--quiet);
}
.now-playing-action:hover { background: var(--pressed); color: var(--ink); }
.now-playing-enter-active,
.now-playing-leave-active,
.now-playing-move { transition: opacity 140ms var(--ease-out), transform 140ms var(--ease-out); }
.now-playing-enter-from,
.now-playing-leave-to { opacity: 0; transform: translateY(3px); }
.now-playing-leave-active { position: absolute; width: calc(100% - 12px); }
@media (prefers-reduced-motion: reduce) {
  .now-playing-enter-active,
  .now-playing-leave-active,
  .now-playing-move { transition: none; }
}
</style>
