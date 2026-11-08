<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { useBrowser } from "../../stores/browser";
import type { Tab } from "../../lib/types";
import { MAX_FAVORITES } from "../../lib/favorites";
import { savedPage, samePinnedUrl } from "../../lib/bookmarks";
import Icon from "../common/Icon.vue";
const b = useBrowser();
const menu = computed(() => b.contextMenu?.tab ? { ...b.contextMenu, tab: b.contextMenu.tab } : undefined);
const edit = ref(false), editTitle = ref(""), editUrl = ref("");
const panel = ref<HTMLElement>();
watch(menu, async (value) => {
  edit.value = false;
  if (value) { await nextTick(); panel.value?.querySelector<HTMLButtonElement>("button")?.focus(); }
});
function bookmark() { return menu.value?.tab.bookmarkId ? savedPage(b.state, menu.value.tab.bookmarkId) : undefined; }
function beginEdit() { const f = bookmark(); if (f) { editTitle.value = f.title; editUrl.value = f.url; edit.value = true; nextTick(() => document.querySelector<HTMLInputElement>(".bookmark-menu-form input")?.focus()); } }
function closeMenu() {
  b.contextMenu = undefined;
  edit.value = false;
}
function act(type: string, extra = {}) {
  if (menu.value) b.send({ type, id: menu.value.tab.id, ...extra });
  closeMenu();
}
</script>
<template>
<Teleport to="body"
      ><div
        v-if="menu"
        class="menu-scrim"
        @pointerdown.self="closeMenu"
        @keydown.esc="closeMenu"
      >
        <div
          ref="panel"
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
      </div></Teleport>
</template>
