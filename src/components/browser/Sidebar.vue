<script setup lang="ts">
import { nextTick, ref } from "vue";
import { useBrowser } from "../../stores/browser";
import { host } from "../../lib/address";
import type { Tab } from "../../lib/types";
import { MAX_FAVORITES, sameFavoriteUrl } from "../../lib/favorites";
import Icon from "../common/Icon.vue";
import Favorites from "./Favorites.vue";
import PinnedTabs from "./PinnedTabs.vue";
import TabList from "./TabList.vue";
import WorkspaceSwitcher from "./WorkspaceSwitcher.vue";
const b = useBrowser(),
  menu = ref<{ x: number; y: number; tab: Tab }>();
function showMenu(e: MouseEvent, tab: Tab) {
  menu.value = {
    x: Math.min(e.clientX, innerWidth - 220),
    y: Math.max(32, Math.min(e.clientY, innerHeight - 300)),
    tab,
  };
  b.send({ type: "overlay", visible: true });
  nextTick(() => document.querySelector<HTMLButtonElement>(".context-menu button")?.focus());
}
function closeMenu() {
  menu.value = undefined;
  b.send({ type: "overlay", visible: b.overlay });
}
function act(type: string, extra = {}) {
  if (menu.value) b.send({ type, id: menu.value.tab.id, ...extra });
  closeMenu();
}
</script>
<template>
  <aside class="sidebar" aria-label="浏览器侧栏">
    <div class="sidebar-toolbar">
      <button
        class="icon-button"
        title="折叠侧栏 (Ctrl+B)"
        aria-label="折叠侧栏"
        @click="b.send({ type: 'sidebar' })"
      >
        <Icon name="sidebar" :size="14" /></button
      ><span class="toolbar-spacer" /><button
        class="icon-button"
        title="后退 (Alt+←)"
        aria-label="后退"
        @click="b.send({ type: 'back' })"
      >
        <Icon name="back" :size="14" /></button
      ><button
        class="icon-button"
        title="前进 (Alt+→)"
        aria-label="前进"
        @click="b.send({ type: 'forward' })"
      >
        <Icon name="forward" :size="14" /></button
      ><button
        class="icon-button"
        title="刷新 (Ctrl+R)"
        aria-label="刷新"
        @click="b.send({ type: 'reload' })"
      >
        <Icon name="reload" :size="14" />
      </button>
    </div>
    <button
      class="address-field"
      aria-label="搜索或输入地址"
      title="搜索或输入地址 (Ctrl+L)"
      @click="b.openCommand('edit')"
    >
      <span>{{ host(b.active?.url || "") || "Search or enter address" }}</span
      ><Icon name="settings" :size="13" /></button
    ><Favorites /><Transition name="space" mode="out-in"
      ><section :key="b.state.activeSpace" class="space-list">
        <h1 class="space-name">{{ b.space.name }}</h1>
        <PinnedTabs @menu="showMenu" />
        <div class="section-label">
          <span>Tabs</span
          ><span>{{ b.tabs.filter((t) => !t.pinned).length }}</span>
        </div>
        <button class="new-tab" @click="b.openCommand()">
          <Icon name="plus" :size="16" /><span>New Tab</span><kbd>Ctrl T</kbd>
        </button>
        <TabList
          :tabs="b.tabs.filter((t) => !t.pinned)"
          aria-label="标签页"
          @menu="showMenu"
        /></section></Transition
    ><WorkspaceSwitcher /><Teleport to="body"
      ><div
        v-if="menu"
        class="menu-scrim"
        @pointerdown.self="closeMenu"
        @keydown.esc="closeMenu"
      >
        <div
          class="context-menu"
          role="menu"
          :style="{ left: menu.x + 'px', top: menu.y + 'px' }"
        >
          <button role="menuitem" @click="act('pin')">
            <Icon name="pin" />{{
              menu.tab.pinned ? "取消固定" : "固定标签页"
            }}</button
          ><button
            role="menuitem"
            :disabled="!/^https?:\/\//i.test(menu.tab.url) || b.state.favorites.length >= MAX_FAVORITES || b.state.favorites.some((f) => sameFavoriteUrl(f.url, menu!.tab.url))"
            @click="act('favoriteSave', { title: Array.from(menu.tab.title).slice(0, 80).join(''), url: menu.tab.url })"
          >
            <Icon name="plus" />添加到顶部常用网站
          </button><button
            role="menuitem"
            :disabled="
              !b.active ||
              b.active.id === menu.tab.id ||
              b.active.spaceId !== menu.tab.spaceId
            "
            @click="
              b.send({
                type: 'split',
                id: b.state.activeTab,
                other: menu.tab.id,
              });
              closeMenu();
            "
          >
            <Icon name="split" />在当前网页旁分屏
          </button>
          <button
            role="menuitem"
            :disabled="!menu.tab.url.startsWith('http')"
            @click="
              b.send({ type: 'peekOpen', url: menu.tab.url });
              closeMenu();
            "
          >
            <Icon name="globe" />在 Peek 中预览
          </button>
          <div class="menu-label">移至工作区</div>
          <button
            v-for="s in b.state.spaces.filter(
              (x) => x.id !== menu!.tab.spaceId,
            )"
            :key="s.id"
            role="menuitem"
            @click="act('move', { spaceId: s.id })"
          >
            {{ s.name }}
          </button>
          <hr />
          <button role="menuitem" @click="act('close')">
            <Icon name="close" />关闭标签页
          </button>
        </div>
      </div></Teleport
    >
  </aside>
</template>
