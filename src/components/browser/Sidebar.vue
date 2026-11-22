<script setup lang="ts">
import { computed, ref } from "vue";
import { useBrowser } from "../../stores/browser";
import { host, isHistoryUrl } from "../../lib/address";
import { isSettingsUrl } from "../../lib/settings";
import type { Tab } from "../../lib/types";
import Icon from "../common/Icon.vue";
import SpaceGlyph from "../common/SpaceGlyph.vue";
import Favorites from "./Favorites.vue";
import NowPlaying from "./NowPlaying.vue";
import TabMenu from "./TabMenu.vue";
import PinFolderMenu from "./PinFolderMenu.vue";
import SpaceDeck from "./SpaceDeck.vue";
import WorkspaceSwitcher from "./WorkspaceSwitcher.vue";
import SpaceCreatePage from "./SpaceCreatePage.vue";
import SpaceEditor from "./SpaceEditor.vue";
import { provideTabDrag } from "../../lib/tab-drag";
const b = useBrowser();
const splitAddress = computed(() => isSettingsUrl(b.active?.url) ? b.t("设置")
  : isHistoryUrl(b.active?.url) ? b.t("历史记录")
  : b.active?.url.startsWith("chrome-extension:") ? b.active.title || b.t("扩展")
  : host(b.active?.url || "") || b.t("搜索或输入地址"));
const sidebar = ref<HTMLElement>();
const tabDrag = provideTabDrag(sidebar);
const spaceDeck = ref<InstanceType<typeof SpaceDeck>>();
function showMenu(e: MouseEvent, tab: Tab) {
  b.contextMenu = {
    x: e.clientX,
    y: e.clientY,
    tab,
  };
}
</script>
<template>
  <aside ref="sidebar" class="sidebar" :data-sidebar-density="b.settings.sidebarDensity" :aria-label="b.t('浏览器侧栏')" @wheel="spaceDeck?.wheel($event)"
    @pointerdown="tabDrag.press" @lostpointercapture="tabDrag.captureLost">
    <template v-if="b.splitPartner">
      <div class="split-sidebar-top" data-window-drag data-chrome-motion="sidebar-toolbar">
        <button class="icon-button" :aria-label="b.t('收起侧栏')" :title="b.shortcutTitle(b.t('收起侧栏'), 'sidebar')"
          data-window-control @click="b.send({ type: 'sidebar' })">
          <Icon name="sidebar" :size="14" />
        </button>
      </div>
      <button class="address-field split-sidebar-address" :aria-label="b.t('搜索或输入地址')" data-chrome-motion="sidebar-address"
        :title="b.shortcutTitle(b.t('搜索或输入地址'), 'address')" @click="b.openCommand('edit', b.active?.id)">
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
    <SpaceDeck ref="spaceDeck" :blocked="b.overlay || Boolean(b.peek) || Boolean(b.contextMenu)" @menu="showMenu" />
    <NowPlaying />
    </template>
    <WorkspaceSwitcher /><TabMenu v-if="!b.native" /><PinFolderMenu v-if="!b.native" />
  </aside>
</template>
