<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useBrowser } from "../../stores/browser";
import { isHistoryUrl } from "../../lib/address";
import { isSettingsUrl } from "../../lib/settings";
import { tabDisplayTitle } from "../../lib/tab-meta";
import Favicon from "../common/Favicon.vue";
import Icon from "../common/Icon.vue";

const b = useBrowser();
const panel = ref<HTMLElement>();
const selected = computed(() => b.tabSwitcher?.selected);
const cards = computed(() => {
  if (!b.tabSwitcher?.visible) return [];
  const tabs = new Map(b.state.tabs.map((tab) => [tab.id, tab]));
  return b.tabSwitcher.ids.flatMap((id) => {
    const tab = tabs.get(id);
    if (!tab) return [];
    const partner = tab.splitWith ? tabs.get(tab.splitWith) : undefined;
    const panes = (partner ? [tab, partner] : [tab]).map((pane) => ({
      ...pane,
      icon: isSettingsUrl(pane.url) ? "settings" : isHistoryUrl(pane.url) ? "history" : pane.url === "era://newtab" ? "plus" : undefined,
    }));
    return [{ id, panes, title: panes.map((pane) => tabDisplayTitle(pane, b.language)).join(" / ") }];
  });
});
const optionId = (id: string) => "tab-switcher-option-" + id;
let previousFocus: HTMLElement | null = null;
watch(panel, (element, previous) => {
  if (b.native) return;
  if (element) {
    previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    element.focus({ preventScroll: true });
  } else {
    if (previous?.contains(document.activeElement) || document.activeElement === document.body) {
      previousFocus?.focus({ preventScroll: true });
    }
    previousFocus = null;
  }
}, { flush: "post" });
watch([panel, selected], ([element]) => {
  element?.querySelector('[aria-selected="true"]')?.scrollIntoView({ block: "nearest", inline: "nearest" });
}, { flush: "post" });
</script>

<template>
  <div v-if="cards.length" class="tab-switcher-layer" @click.self="b.switchTab({ action: 'cancel' })">
    <div
      ref="panel"
      class="tab-switcher-panel"
      role="listbox"
      tabindex="-1"
      :aria-label="b.t('切换标签页')"
      :aria-activedescendant="selected ? optionId(selected) : undefined"
    >
      <button
        v-for="card in cards"
        :id="optionId(card.id)"
        :key="card.id"
        class="tab-switcher-card"
        type="button"
        role="option"
        tabindex="-1"
        :aria-selected="card.id === selected"
        :title="card.title"
        @click="b.switchTab({ action: 'commit', id: card.id })"
      >
        <span class="tab-switcher-picture" aria-hidden="true">
          <span v-for="tab in card.panes" :key="tab.id" class="tab-switcher-pane">
            <img v-if="b.thumbnails[tab.id]" class="tab-switcher-snapshot" :src="b.thumbnails[tab.id]" alt="" draggable="false" />
            <Icon v-else-if="tab.icon" :name="tab.icon" :size="30" />
            <Favicon v-else :url="tab.url" :favicon="tab.favicon" />
          </span>
        </span>
        <span class="tab-switcher-title">{{ card.title }}</span>
      </button>
    </div>
  </div>
</template>

<style scoped>
.tab-switcher-layer {
  position: fixed;
  inset: 0;
  z-index: 65;
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  place-items: center;
  padding: 20px;
}
.tab-switcher-panel {
  display: flex;
  gap: 12px;
  max-width: 100%;
  padding: 20px;
  overflow-x: auto;
  overscroll-behavior-x: contain;
  scrollbar-width: none;
  border: 1px solid rgb(255 255 255 / 19%);
  border-radius: 28px;
  background: rgb(24 24 26 / 93%);
  backdrop-filter: blur(24px);
  box-shadow: 0 12px 36px rgb(0 0 0 / 28%), 0 2px 8px rgb(0 0 0 / 20%);
  color: #f5f5f7;
  outline: none;
}
.tab-switcher-panel:focus-visible {
  outline: none;
}
.tab-switcher-card {
  flex: 0 0 156px;
  min-width: 0;
  display: grid;
  gap: 12px;
  padding: 9px;
  border: 1px solid transparent;
  border-radius: 18px;
  background: transparent;
  color: inherit;
  cursor: pointer;
  outline: none;
}
.tab-switcher-card:hover {
  background: rgb(255 255 255 / 8%);
}
.tab-switcher-card[aria-selected="true"] {
  border-color: rgb(255 255 255 / 28%);
  background: rgb(255 255 255 / 16%);
}
.tab-switcher-picture {
  display: flex;
  gap: 1px;
  aspect-ratio: 3 / 2;
  overflow: hidden;
  border-radius: 12px;
  background: rgb(255 255 255 / 8%);
}
.tab-switcher-pane {
  flex: 1 1 0;
  min-width: 0;
  display: grid;
  place-items: center;
  overflow: hidden;
}
.tab-switcher-snapshot {
  width: 100%;
  height: 100%;
  min-height: 0;
  object-fit: cover;
  object-position: top left;
}
.tab-switcher-pane :deep(.favicon),
.tab-switcher-pane :deep(svg) {
  width: 30px;
  height: 30px;
  object-fit: contain;
}
.tab-switcher-title {
  min-width: 0;
  overflow: hidden;
  font-size: 14px;
  line-height: 20px;
  text-align: center;
  text-overflow: ellipsis;
  white-space: nowrap;
}
@media (prefers-reduced-transparency: reduce) {
  .tab-switcher-panel { background: #18181a; backdrop-filter: none; }
}
</style>
