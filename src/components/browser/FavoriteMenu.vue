<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { useBrowser } from "../../stores/browser";
import { liveTab, samePinnedUrl } from "../../lib/bookmarks";
import ContextMenuSurface from "./ContextMenuSurface.vue";
const b = useBrowser();
const menu = computed(() => b.contextMenu?.favorite ? { ...b.contextMenu, favorite: b.contextMenu.favorite } : undefined);
const edit = ref(false), editTitle = ref(""), editUrl = ref("");
const panel = ref<InstanceType<typeof ContextMenuSurface>>();
watch(menu, async (value) => {
  edit.value = false;
  if (value) { await nextTick(); panel.value?.panel?.querySelector<HTMLButtonElement>("button")?.focus({ preventScroll: true }); }
});
const live = (f: import("../../lib/types").Favorite) => liveTab(b.state, f.id);
function closeMenu() { b.contextMenu = undefined; edit.value = false; }
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

</script>
<template>
<Teleport to="body">
    <div v-if="menu" class="menu-scrim" @pointerdown.self="closeMenu" @keydown.esc="closeMenu">
      <ContextMenuSurface ref="panel" class="favorite-menu" role="menu" :anchor="menu">
        <form v-if="edit" class="bookmark-menu-form" @submit.prevent="b.send({ type: 'bookmarkUpdate', id: menu.favorite.id, title: editTitle, url: editUrl }); closeMenu()">
          <label>{{ b.t('名称') }}<input v-model="editTitle" :aria-label="b.t('置顶网站名称')" maxlength="80" required /></label>
          <label>{{ b.t('起始网址') }}<input v-model="editUrl" :aria-label="b.t('置顶网站网址')" maxlength="2048" required /></label>
          <button class="primary-button" type="submit">{{ b.t('保存修改') }}</button>
        </form>
        <template v-else>
        <button role="menuitem" @click="beginEdit">{{ b.t('编辑常用网站') }}</button>
        <template v-if="live(menu.favorite)">
          <button v-if="!samePinnedUrl(live(menu.favorite)!.url, menu.favorite.url)" role="menuitem" @click="act('bookmarkReset')">{{ b.t('返回固定网址') }}</button>
          <button role="menuitem" @click="act('bookmarkSeparate')">{{ b.t('分离为普通标签') }}</button>
          <button role="menuitem" @click="act('bookmarkUnload')">{{ b.t('卸载网页') }}</button>
        </template>
        <button role="menuitem" @click="act('bookmarkPin')">{{ b.t('移到当前工作区固定标签') }}</button>
        <button role="menuitem" @click="act('bookmarkUnpin')">{{ b.t('移回普通标签') }}</button>
        <hr />
        <button role="menuitem" @click="act('favoriteRemove')">{{ b.t('移除常用网站') }}</button>
        </template>
      </ContextMenuSurface>
    </div>
  </Teleport>
</template>
