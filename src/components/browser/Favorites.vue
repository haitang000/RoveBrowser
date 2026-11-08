<script setup lang="ts">
import { useBrowser } from "../../stores/browser";
import Favicon from "../common/Favicon.vue";
import Icon from "../common/Icon.vue";
const b = useBrowser();
function open(url: string) {
  const tab = b.state.tabs.find(
    (t) =>
      t.spaceId === b.state.activeSpace &&
      t.url.replace(/\/$/, "") === url.replace(/\/$/, ""),
  );
  b.send(tab ? { type: "activate", id: tab.id } : { type: "new", url });
}
</script>
<template>
  <div class="favorites-heading">
    <span>常用网站</span>
    <button class="icon-button" title="管理顶部常用网站" aria-label="管理顶部常用网站" @click="b.favoritesOpen = true">
      <Icon name="settings" :size="13" />
    </button>
  </div>
  <div class="favorites" aria-label="常用网站">
    <button
      v-for="f in b.state.favorites"
      :key="f.url"
      :title="f.title"
      :aria-label="f.title"
      :class="{
        chosen: b.active?.url.replace(/\/$/, '') === f.url.replace(/\/$/, ''),
      }"
      @click="open(f.url)"
      @contextmenu.prevent="b.favoritesOpen = true"
    >
      <Favicon :url="f.url" />
    </button>
    <button v-if="!b.state.favorites.length" title="添加常用网站" aria-label="添加常用网站" @click="b.favoritesOpen = true">
      <Icon name="plus" />
    </button>
  </div>
</template>
