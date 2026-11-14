<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { useBrowser } from "../../stores/browser";
import type { Tab } from "../../lib/types";
import { MAX_FAVORITES } from "../../lib/favorites";
import { savedPage, samePinnedUrl } from "../../lib/bookmarks";
import { tabsToDismiss } from "../../lib/command";
import { tabDisplayTitle } from "../../lib/tab-meta";
import Icon from "../common/Icon.vue";
import SpaceGlyph from "../common/SpaceGlyph.vue";
const b = useBrowser();
const menu = computed(() => b.contextMenu?.tab ? { ...b.contextMenu, tab: b.contextMenu.tab } : undefined);
const edit = ref(false), editTitle = ref(""), editUrl = ref("");
const rename = ref(false), renameTitle = ref("");
const panel = ref<HTMLElement>();
watch(menu, async (value) => {
  edit.value = false;
  rename.value = false;
  if (value) { await nextTick(); panel.value?.querySelector<HTMLButtonElement>("button")?.focus(); }
});
function bookmark() { return menu.value?.tab.bookmarkId ? savedPage(b.state, menu.value.tab.bookmarkId) : undefined; }
/** The live controller record, so mute/zoom/rename labels follow host state. */
const liveTab = computed(() => {
  const current = menu.value?.tab;
  if (!current) return undefined;
  return b.state.tabs.find((tab) =>
    tab.id === current.id ||
    (current.bookmarkId && tab.bookmarkId === current.bookmarkId),
  );
});
const isWeb = computed(() => /^https?:/i.test(menu.value?.tab.url || ""));
const isActive = computed(() =>
  menu.value?.tab.id === b.state.activeTab && !b.peek,
);
const muted = computed(() => Boolean(liveTab.value?.muted ?? menu.value?.tab.muted));
const zoomLabel = computed(() => `${Math.round(b.pageZoom * 100)}%`);
const title = computed(() =>
  liveTab.value
    ? tabDisplayTitle(liveTab.value)
    : menu.value?.tab
      ? tabDisplayTitle(menu.value.tab)
      : "",
);
const canRename = computed(() => Boolean(liveTab.value && !liveTab.value.bookmarkId));
const hasCustomTitle = computed(() => Boolean(liveTab.value?.customTitle));
const othersCount = computed(() =>
  menu.value ? tabsToDismiss(b.state, "others", menu.value.tab.id).length : 0,
);
const belowCount = computed(() =>
  menu.value ? tabsToDismiss(b.state, "below", menu.value.tab.id).length : 0,
);
function zoom(action: "in" | "out" | "reset") { b.zoom(action); }
function mute() { b.toggleMute(liveTab.value || menu.value?.tab); }
function beginEdit() { const f = bookmark(); if (f) { editTitle.value = f.title; editUrl.value = f.url; edit.value = true; nextTick(() => document.querySelector<HTMLInputElement>(".bookmark-menu-form input")?.focus()); } }
function beginRename() {
  if (!liveTab.value) return;
  renameTitle.value = liveTab.value.customTitle || "";
  rename.value = true;
  nextTick(() => document.querySelector<HTMLInputElement>(".tab-rename-form input")?.focus());
}
function submitRename() {
  if (!liveTab.value) return;
  b.send({ type: "renameTab", id: liveTab.value.id, title: renameTitle.value });
  closeMenu();
}
function resetRename() {
  if (!liveTab.value) return;
  b.send({ type: "renameTab", id: liveTab.value.id, title: "" });
  closeMenu();
}
function copyLink() {
  const url = menu.value?.tab.url;
  if (url) {
    b.send({ type: "copyText", text: url });
    b.notify("已复制链接");
  }
  closeMenu();
}
function duplicate() {
  if (liveTab.value) b.send({ type: "duplicate", id: liveTab.value.id });
  closeMenu();
}
function closeMenu() {
  b.contextMenu = undefined;
  edit.value = false;
  rename.value = false;
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
          :style="{ left: menu.x + 'px', top: menu.y + 'px', maxHeight: `calc(100dvh - ${menu.y + 12}px)` }"
        >
          <form v-if="edit" class="bookmark-menu-form" @submit.prevent="b.send({ type: 'bookmarkUpdate', id: bookmark()?.id, title: editTitle, url: editUrl }); closeMenu()">
            <label>名称<input v-model="editTitle" aria-label="固定标签名称" maxlength="80" required /></label>
            <label>起始网址<input v-model="editUrl" aria-label="固定标签网址" maxlength="2048" required /></label>
            <button class="primary-button" type="submit">保存修改</button>
          </form>
          <form v-else-if="rename" class="tab-rename-form" @submit.prevent="submitRename">
            <label>标签名称<input v-model="renameTitle" aria-label="标签名称" maxlength="80" placeholder="留空恢复网页标题" /></label>
            <button class="primary-button" type="submit">保存名称</button>
          </form>
          <template v-else>
          <div class="menu-label menu-tab-title">{{ title }}</div>
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
          <template v-if="canRename">
            <button role="menuitem" @click="beginRename">
              <Icon name="edit" />重命名标签页
            </button>
            <button v-if="hasCustomTitle" role="menuitem" @click="resetRename">
              恢复网页标题
            </button>
          </template>
          <button role="menuitem" :disabled="!canRename" @click="duplicate">
            <Icon name="duplicate" />复制标签页
          </button>
          <button role="menuitem" :disabled="!isWeb" @click="copyLink">
            <Icon name="copy" />复制链接
          </button>
          <template v-if="isWeb && !menu.tab.unloaded">
            <button role="menuitem" @click="mute">
              <Icon :name="muted ? 'speakerMuted' : 'speaker'" />{{
                muted ? "取消静音标签页" : "静音标签页"
              }}
            </button>
          </template>
          <template v-if="isActive && isWeb">
            <div class="menu-label">网页缩放 {{ zoomLabel }}</div>
            <div class="menu-zoom">
              <button
                role="menuitem"
                :disabled="b.pageZoom <= 0.5"
                @click="zoom('out')"
              >
                <Icon name="minus" />缩小
              </button>
              <button
                role="menuitem"
                :disabled="Math.abs(b.pageZoom - 1) < 0.001"
                @click="zoom('reset')"
              >
                100%
              </button>
              <button
                role="menuitem"
                :disabled="b.pageZoom >= 3"
                @click="zoom('in')"
              >
                <Icon name="plus" />放大
              </button>
            </div>
          </template>
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
          <div class="menu-label">关闭</div>
          <button role="menuitem" :disabled="othersCount === 0" @click="act('closeOthers')">
            <Icon name="close" />关闭其他标签页<small v-if="othersCount">{{ othersCount }}</small>
          </button>
          <button role="menuitem" :disabled="belowCount === 0" @click="act('closeBelow')">
            <Icon name="close" />关闭下方标签页<small v-if="belowCount">{{ belowCount }}</small>
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
            <SpaceGlyph v-if="s.icon" :icon="s.icon" :size="14" />{{ s.name }}
          </button>
          <hr />
          <button role="menuitem" @click="menu.tab.bookmarkId ? (b.send({ type: 'bookmarkUnload', id: menu.tab.bookmarkId }), closeMenu()) : act('close')">
            <Icon name="close" />{{ menu.tab.pinned ? '卸载网页' : '关闭标签页' }}
          </button>
          </template>
        </div>
      </div></Teleport>
</template>
