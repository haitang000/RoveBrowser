<script setup lang="ts">
import { computed, nextTick, ref } from "vue";
import { useBrowser } from "../../stores/browser";
import { liveTab, samePinnedUrl } from "../../lib/bookmarks";
import { sameOrigin } from "../../lib/favicon";
import type { Favorite } from "../../lib/types";
import Favicon from "../common/Favicon.vue";
import Icon from "../common/Icon.vue";
const b = useBrowser();
const menu = ref<{ favorite: Favorite; x: number; y: number }>();
const edit = ref(false), editTitle = ref(""), editUrl = ref("");
const tiles = computed(() => b.state.favorites.filter((f, i, all) => !f.splitWith || !all.slice(0, i).some((p) => p.id === f.splitWith)));
const live = (f: Favorite) => liveTab(b.state, f.id);
const favicon = (f: Favorite) => {
  const tab = live(f);
  return f.favicon || (tab && sameOrigin(tab.url, f.url) ? tab.favicon : undefined);
};
const partner = (f: Favorite) => b.state.favorites.find((p) => p.id === f.splitWith);
function open(f: Favorite, e: MouseEvent) {
  const target = e.currentTarget as HTMLElement;
  const other = partner(f);
  const id = other && e.clientX > target.getBoundingClientRect().left + target.clientWidth / 2 ? other.id : f.id;
  b.send({ type: "bookmarkOpen", id });
}
function showMenu(e: MouseEvent, favorite: Favorite) {
  edit.value = false;
  menu.value = { favorite, x: Math.min(e.clientX, innerWidth - 220), y: Math.max(32, Math.min(e.clientY, innerHeight - 340)) };
  b.send({ type: "overlay", visible: true });
  nextTick(() => document.querySelector<HTMLButtonElement>(".favorite-menu button")?.focus());
}
function closeMenu() { menu.value = undefined; b.send({ type: "overlay", visible: b.overlay }); }
function beginEdit() {
  editTitle.value = menu.value!.favorite.title;
  editUrl.value = menu.value!.favorite.url;
  edit.value = true;
  nextTick(() => document.querySelector<HTMLInputElement>(".favorite-menu input")?.focus());
}
function act(type: string) {
  if (menu.value) b.send({ type, id: menu.value.favorite.id, url: menu.value.favorite.url, spaceId: b.state.activeSpace });
  closeMenu();
}
function drag(e: DragEvent, f: Favorite) {
  e.dataTransfer?.setData("text/era-tab", f.id);
  if (e.dataTransfer) e.dataTransfer.effectAllowed = "move";
  b.dragging = true;
}
function drop(e: DragEvent, before?: string) {
  const id = e.dataTransfer?.getData("text/era-tab");
  if (id && id !== before) b.send({ type: "favoriteTab", id, before });
  b.dragging = false;
}
</script>
<template>
  <div class="favorites" aria-label="常用网站" @dragover.prevent @drop.prevent.self="drop($event)">
    <div v-for="f in tiles" :key="f.id" class="favorite-tile" :data-bookmark-id="f.id" draggable="true" @dragstart="drag($event, f)" @dragend="b.dragging = false" @dragover.prevent @drop.prevent.stop="drop($event, f.id)" @contextmenu.prevent="showMenu($event, f)" @mousedown.middle.prevent @auxclick.middle.prevent.stop="b.send({ type: 'bookmarkUnload', id: f.id })">
    <button class="favorite-open" draggable="true" @dragstart.stop="drag($event, f)"
      :title="b.previewTarget === f.id ? undefined : f.title"
      :aria-describedby="b.previewTarget === f.id ? 'tab-hover-preview' : undefined"
      :aria-label="f.title"
      :class="{
        chosen: !!b.active?.bookmarkId && (b.active.bookmarkId === f.id || b.active.bookmarkId === f.splitWith),
        'favorite-loaded': live(f) || (partner(f) && live(partner(f)!)),
      }"
      @click="open(f, $event)"
    >
      <Favicon :url="f.url" :favicon="favicon(f)" /><Favicon v-if="partner(f)" :url="partner(f)!.url" :favicon="favicon(partner(f)!)" />
    </button>
    <button v-if="live(f) || (partner(f) && live(partner(f)!))" class="favorite-close" :aria-label="`卸载 ${f.title}`" title="卸载网页（保留常用网站）" @click.stop="b.send({ type: 'bookmarkUnload', id: f.id })"><Icon name="close" :size="9" /></button>
    </div>
    <button v-if="!tiles.length" title="打开标签页后，右键添加或拖到此处置顶" aria-label="打开新标签页以添加置顶网站" @click="b.openCommand()">
      <Icon name="plus" />
    </button>
  </div>
  <Teleport to="body">
    <div v-if="menu" class="menu-scrim" @pointerdown.self="closeMenu" @keydown.esc="closeMenu">
      <div class="context-menu favorite-menu" role="menu" :style="{ left: menu.x + 'px', top: menu.y + 'px' }">
        <form v-if="edit" class="bookmark-menu-form" @submit.prevent="b.send({ type: 'bookmarkUpdate', id: menu.favorite.id, title: editTitle, url: editUrl }); closeMenu()">
          <label>名称<input v-model="editTitle" aria-label="置顶网站名称" maxlength="80" required /></label>
          <label>起始网址<input v-model="editUrl" aria-label="置顶网站网址" maxlength="2048" required /></label>
          <button class="primary-button" type="submit">保存修改</button>
        </form>
        <template v-else>
        <button role="menuitem" @click="beginEdit">编辑常用网站</button>
        <template v-if="live(menu.favorite)">
          <button v-if="!samePinnedUrl(live(menu.favorite)!.url, menu.favorite.url)" role="menuitem" @click="act('bookmarkReset')">返回固定网址</button>
          <button role="menuitem" @click="act('bookmarkSeparate')">分离为普通标签</button>
          <button role="menuitem" @click="act('bookmarkUnload')">卸载网页</button>
        </template>
        <button role="menuitem" @click="act('bookmarkPin')">移到当前工作区固定标签</button>
        <button role="menuitem" @click="act('bookmarkUnpin')">移回普通标签</button>
        <hr />
        <button role="menuitem" @click="act('favoriteRemove')">移除常用网站</button>
        </template>
      </div>
    </div>
  </Teleport>
</template>
