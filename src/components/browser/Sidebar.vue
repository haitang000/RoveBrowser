<script setup lang="ts">
import { ref } from "vue";
import { useBrowser } from "../../stores/browser";
import type { Tab } from "../../lib/types";
import Icon from "../common/Icon.vue";
import SpaceGlyph from "../common/SpaceGlyph.vue";
import Favorites from "./Favorites.vue";
import TabMenu from "./TabMenu.vue";
import SpaceDeck from "./SpaceDeck.vue";
import WorkspaceSwitcher from "./WorkspaceSwitcher.vue";
import DownloadsPanel from "./DownloadsPanel.vue";
const b = useBrowser();
const spaceDeck = ref<InstanceType<typeof SpaceDeck>>();
const swipeShares = ref<Record<string, number> | null>(null);
function showMenu(e: MouseEvent, tab: Tab) {
  b.contextMenu = {
    x: Math.min(e.clientX, innerWidth - 220),
    y: Math.max(32, Math.min(e.clientY, innerHeight - 420)),
    tab,
  };
}
</script>
<template>
  <aside class="sidebar" aria-label="浏览器侧栏" @wheel="spaceDeck?.wheel($event)">
    <div class="sidebar-header">
      <h1 class="space-name" :class="{compact: !b.settings.largeSpaceNames}" :title="b.space.name">
        <SpaceGlyph v-if="b.space.icon" :icon="b.space.icon" :size="b.settings.largeSpaceNames ? 22 : 13" />
        <span>{{ b.space.name }}</span>
      </h1>
    </div>
    <Favorites />
    <SpaceDeck ref="spaceDeck" :blocked="b.overlay || Boolean(b.peek) || Boolean(b.contextMenu)" @menu="showMenu" @progress="swipeShares = $event" />
    <WorkspaceSwitcher :swipe-shares="swipeShares" /><DownloadsPanel /><TabMenu v-if="!b.native" />
  </aside>
</template>
