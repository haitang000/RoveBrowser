<script setup lang="ts">
import { nextTick, ref } from "vue";
import { useBrowser } from "../../stores/browser";
import type { Tab } from "../../lib/types";
import { MAX_FAVORITES } from "../../lib/favorites";
import { savedPage, samePinnedUrl } from "../../lib/bookmarks";
import Icon from "../common/Icon.vue";
import Favorites from "./Favorites.vue";
import SpaceDeck from "./SpaceDeck.vue";
import WorkspaceSwitcher from "./WorkspaceSwitcher.vue";
const b = useBrowser(),
  menu = ref<{ x: number; y: number; tab: Tab }>();
const edit = ref(false), editTitle = ref(""), editUrl = ref("");
const spaceDeck = ref<InstanceType<typeof SpaceDeck>>();
const swipeShares = ref<Record<string, number> | null>(null);
function bookmark() { return menu.value?.tab.bookmarkId ? savedPage(b.state, menu.value.tab.bookmarkId) : undefined; }
function beginEdit() { const f = bookmark(); if (f) { editTitle.value = f.title; editUrl.value = f.url; edit.value = true; nextTick(() => document.querySelector<HTMLInputElement>(".bookmark-menu-form input")?.focus()); } }
function showMenu(e: MouseEvent, tab: Tab) {
  menu.value = {
    x: Math.min(e.clientX, innerWidth - 220),
    y: Math.max(32, Math.min(e.clientY, innerHeight - 420)),
    tab,
  };
  b.send({ type: "overlay", visible: true });
  nextTick(() => document.querySelector<HTMLButtonElement>(".context-menu button")?.focus());
}
function closeMenu() {
  menu.value = undefined;
  edit.value = false;
  b.send({ type: "overlay", visible: b.overlay });
}
function act(type: string, extra = {}) {
  if (menu.value) b.send({ type, id: menu.value.tab.id, ...extra });
  closeMenu();
}
</script>
<template>
  <aside class="sidebar" aria-label="浏览器侧栏" @wheel="spaceDeck?.wheel($event)">
    <div class="sidebar-header">
      <h1 class="space-name" :title="b.space.name">{{ b.space.name }}</h1>
    </div>
    <Favorites />
    <SpaceDeck ref="spaceDeck" :blocked="b.overlay || Boolean(b.peek) || Boolean(menu)" @menu="showMenu" @progress="swipeShares = $event" />
    <WorkspaceSwitcher :swipe-shares="swipeShares" /><Teleport to="body"
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
          <form v-if="edit" class="bookmark-menu-form" @submit.prevent="b.send({ type: 'bookmarkUpdate', id: bookmark()?.id, title: editTitle, url: editUrl }); closeMenu()">
            <label>名称<input v-model="editTitle" aria-label="固定标签名称" maxlength="80" required /></label>
            <label>起始网址<input v-model="editUrl" aria-label="固定标签网址" maxlength="2048" required /></label>
            <button class="primary-button" type="submit">保存修改</button>
          </form>
          <template v-else>
          <button role="menuitem" @click="act('pin')">
            <Icon name="pin" />{{
              menu.tab.pinned ? "取消固定" : "固定标签页"
            }}</button
          ><button
            role="menuitem"
            :disabled="!/^https?:\/\//i.test(menu.tab.url) || b.state.favorites.length >= MAX_FAVORITES"
            @click="act('favoriteTab')"
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
          <template v-if="bookmark()">
            <button role="menuitem" @click="beginEdit">编辑固定标签</button>
            <button v-if="!menu.tab.unloaded && !samePinnedUrl(menu.tab.url, bookmark()!.url)" role="menuitem" @click="b.send({ type: 'bookmarkReset', id: bookmark()!.id }); closeMenu()">返回固定网址</button>
            <button v-if="!menu.tab.unloaded" role="menuitem" @click="b.send({ type: 'bookmarkSeparate', id: bookmark()!.id }); closeMenu()">分离为普通标签</button>
            <button v-if="!menu.tab.unloaded && !samePinnedUrl(menu.tab.url, bookmark()!.url)" role="menuitem" @click="b.send({ type: 'bookmarkUpdate', id: bookmark()!.id, title: bookmark()!.title, url: menu.tab.url }); closeMenu()">将当前网址设为固定网址</button>
          </template>
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
          <button role="menuitem" @click="menu.tab.bookmarkId ? (b.send({ type: 'bookmarkUnload', id: menu.tab.bookmarkId }), closeMenu()) : act('close')">
            <Icon name="close" />{{ menu.tab.pinned ? '卸载网页' : '关闭标签页' }}
          </button>
          </template>
        </div>
      </div></Teleport
    >
  </aside>
</template>
