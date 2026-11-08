<script setup lang="ts">
import { computed } from "vue";
import { useBrowser } from "../../stores/browser";
import { liveTab } from "../../lib/bookmarks";
import { sameOrigin } from "../../lib/favicon";
import type { Favorite } from "../../lib/types";
import Favicon from "../common/Favicon.vue";
import FavoriteMenu from "./FavoriteMenu.vue";
import Icon from "../common/Icon.vue";
const b = useBrowser();
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
  b.contextMenu = { favorite, x: Math.min(e.clientX, innerWidth - 220), y: Math.max(32, Math.min(e.clientY, innerHeight - 340)) };
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
  <div class="favorites" :style="{gridTemplateColumns: b.settings.favoritesPerRow ? `repeat(${b.settings.favoritesPerRow}, minmax(0, 1fr))` : 'repeat(auto-fit, minmax(56px, 1fr))'}" aria-label="常用网站" @dragover.prevent @drop.prevent.self="drop($event)">
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
  <FavoriteMenu v-if="!b.native" />
</template>
