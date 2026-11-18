<script setup lang="ts">
import { computed, ref } from "vue";
import { useBrowser } from "../../stores/browser";
import { host } from "../../lib/address";
import { isSettingsUrl } from "../../lib/settings";
import type { Tab } from "../../lib/types";
import Icon from "../common/Icon.vue";
import SpaceGlyph from "../common/SpaceGlyph.vue";
import Favorites from "./Favorites.vue";
import TabMenu from "./TabMenu.vue";
import SpaceDeck from "./SpaceDeck.vue";
import WorkspaceSwitcher from "./WorkspaceSwitcher.vue";
import SpaceCreatePage from "./SpaceCreatePage.vue";
import SpaceEditor from "./SpaceEditor.vue";
import { provideTabDrag } from "../../lib/tab-drag";
const b = useBrowser();
const splitAddress = computed(() => isSettingsUrl(b.active?.url) ? "设置" : host(b.active?.url || "") || "搜索或输入地址");
const sidebar = ref<HTMLElement>();
const tabDrag = provideTabDrag(sidebar);
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
  <aside ref="sidebar" class="sidebar" aria-label="浏览器侧栏" @wheel="spaceDeck?.wheel($event)" @dragstart.capture="tabDrag.start">
    <template v-if="b.splitPartner">
      <div class="split-sidebar-top" data-window-drag data-chrome-motion="sidebar-toolbar">
        <button class="icon-button" aria-label="收起侧栏" :title="b.shortcutTitle('收起侧栏', 'sidebar')"
          data-window-control @click="b.send({ type: 'sidebar' })">
          <Icon name="sidebar" :size="14" />
        </button>
      </div>
      <button class="address-field split-sidebar-address" aria-label="搜索或输入地址" data-chrome-motion="sidebar-address"
        :title="b.shortcutTitle('搜索或输入地址', 'address')" @click="b.openCommand('edit', b.active?.id)">
        <span>{{ splitAddress }}</span>
        <Icon name="settings" :size="13" />
      </button>
    </template>
    <SpaceCreatePage v-if="b.spaceCreation" />
    <SpaceEditor v-else-if="b.spaceEditor" :key="b.spaceEditor" />
    <template v-else>
    <div class="sidebar-header">
      <h1 class="space-name editable" :class="{compact: !b.settings.largeSpaceNames}" :title="`${b.space.name} · 编辑空间`" role="button" tabindex="0" @click="b.toggleSpaceEditor(b.space.id)" @keydown.enter.prevent="b.toggleSpaceEditor(b.space.id)">
        <SpaceGlyph v-if="b.space.icon" :icon="b.space.icon" :size="b.settings.largeSpaceNames ? 22 : 13" />
        <span>{{ b.space.name }}</span>
      </h1>
    </div>
    <Favorites />
    <SpaceDeck ref="spaceDeck" :blocked="b.overlay || Boolean(b.peek) || Boolean(b.contextMenu)" @menu="showMenu" @progress="swipeShares = $event" />
    </template>
    <WorkspaceSwitcher :swipe-shares="swipeShares" /><TabMenu v-if="!b.native" />
  </aside>
</template>
