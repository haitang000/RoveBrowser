<script setup lang="ts">
import { computed, nextTick, ref } from "vue";
import { useBrowser } from "../../stores/browser";
import { liveTab, samePinnedUrl } from "../../lib/bookmarks";
import type { Favorite } from "../../lib/types";
import Favicon from "../common/Favicon.vue";
import Icon from "../common/Icon.vue";
const b = useBrowser();
const menu = ref<{ favorite: Favorite; x: number; y: number }>();
const tiles = computed(() => b.state.favorites.filter((f, i, all) => !f.splitWith || !all.slice(0, i).some((p) => p.id === f.splitWith)));
const live = (f: Favorite) => liveTab(b.state, f.id);
const partner = (f: Favorite) => b.state.favorites.find((p) => p.id === f.splitWith);
function open(f: Favorite, e: MouseEvent) {
  const target = e.currentTarget as HTMLElement;
  const other = partner(f);
  const id = other && e.clientX > target.getBoundingClientRect().left + target.clientWidth / 2 ? other.id : f.id;
  b.send({ type: "bookmarkOpen", id });
}
function showMenu(e: MouseEvent, favorite: Favorite) {
  menu.value = { favorite, x: Math.min(e.clientX, innerWidth - 220), y: Math.max(32, Math.min(e.clientY, innerHeight - 340)) };
  b.send({ type: "overlay", visible: true });
  nextTick(() => document.querySelector<HTMLButtonElement>(".favorite-menu button")?.focus());
}
function closeMenu() { menu.value = undefined; b.send({ type: "overlay", visible: b.overlay }); }
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
  <div class="favorites-heading">
    <span>常用网站</span>
    <button class="icon-button" title="管理顶部常用网站" aria-label="管理顶部常用网站" @click="b.favoriteEditId = undefined; b.favoritesOpen = true">
      <Icon name="settings" :size="13" />
    </button>
  </div>
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
      <Favicon :url="f.url" :favicon="live(f)?.favicon" /><Favicon v-if="partner(f)" :url="partner(f)!.url" :favicon="live(partner(f)!)?.favicon" />
    </button>
    <button v-if="live(f) || (partner(f) && live(partner(f)!))" class="favorite-close" :aria-label="`卸载 ${f.title}`" title="卸载网页（保留常用网站）" @click.stop="b.send({ type: 'bookmarkUnload', id: f.id })"><Icon name="close" :size="9" /></button>
    </div>
    <button v-if="!tiles.length" title="添加常用网站" aria-label="添加常用网站" @click="b.favoritesOpen = true">
      <Icon name="plus" />
    </button>
  </div>
  <Teleport to="body">
    <div v-if="menu" class="menu-scrim" @pointerdown.self="closeMenu" @keydown.esc="closeMenu">
      <div class="context-menu favorite-menu" role="menu" :style="{ left: menu.x + 'px', top: menu.y + 'px' }">
        <button role="menuitem" @click="b.favoriteEditId = menu.favorite.id; b.favoritesOpen = true; closeMenu()">编辑常用网站</button>
        <template v-if="live(menu.favorite)">
          <button v-if="!samePinnedUrl(live(menu.favorite)!.url, menu.favorite.url)" role="menuitem" @click="act('bookmarkReset')">返回固定网址</button>
          <button role="menuitem" @click="act('bookmarkSeparate')">分离为普通标签</button>
          <button role="menuitem" @click="act('bookmarkUnload')">卸载网页</button>
        </template>
        <button role="menuitem" @click="act('bookmarkPin')">移到当前工作区固定标签</button>
        <button role="menuitem" @click="act('bookmarkUnpin')">移回普通标签</button>
        <hr />
        <button role="menuitem" @click="act('favoriteRemove')">移除常用网站</button>
      </div>
    </div>
  </Teleport>
</template>
