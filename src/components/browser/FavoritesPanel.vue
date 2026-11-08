<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, watch } from "vue";
import { useBrowser } from "../../stores/browser";
import type { Favorite } from "../../lib/types";
import { favorite, MAX_FAVORITES, sameFavoriteUrl } from "../../lib/favorites";
import Icon from "../common/Icon.vue";
import Favicon from "../common/Favicon.vue";

const b = useBrowser();
const panel = ref<HTMLElement>();
const title = ref("");
const url = ref("");
const previousUrl = ref<string>();
const error = ref("");
let returnFocus: HTMLElement | null = null;

function reset() {
  title.value = "";
  url.value = "";
  previousUrl.value = undefined;
  error.value = "";
}
function edit(f: Favorite) {
  title.value = f.title;
  url.value = f.url;
  previousUrl.value = f.url;
  error.value = "";
  panel.value?.querySelector<HTMLInputElement>("input")?.focus();
}
function save() {
  try {
    const next = favorite(title.value, url.value);
    if (b.state.favorites.some((f) => f.url !== previousUrl.value && sameFavoriteUrl(f.url, next.url)))
      throw Error("该网址已在顶部常用网站中");
    b.send({ type: "favoriteSave", ...next, previousUrl: previousUrl.value });
    reset();
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  }
}
function remove(f: Favorite) {
  b.send({ type: "favoriteRemove", url: f.url });
  if (previousUrl.value === f.url) reset();
}
function key(e: KeyboardEvent) {
  if (e.key === "Escape") {
    e.stopPropagation();
    b.favoritesOpen = false;
  }
  if (e.key === "Tab") {
    const controls = panel.value?.querySelectorAll<HTMLElement>("button:not(:disabled), input");
    if (!controls?.length) return;
    const first = controls[0], last = controls[controls.length - 1];
    if (e.shiftKey && document.activeElement === first) { e.preventDefault(); last.focus(); }
    else if (!e.shiftKey && document.activeElement === last) { e.preventDefault(); first.focus(); }
  }
}
watch(() => b.favoritesOpen, async (open) => {
  if (open) {
    returnFocus = document.activeElement as HTMLElement | null;
    window.addEventListener("keydown", key);
    reset();
    const selected = b.state.favorites.find((f) => f.id === b.favoriteEditId);
    if (selected) edit(selected);
    await nextTick();
    panel.value?.querySelector<HTMLButtonElement>("button")?.focus();
  } else {
    window.removeEventListener("keydown", key);
    await nextTick();
    if (returnFocus?.isConnected) returnFocus.focus();
    else document.querySelector<HTMLButtonElement>(".favorites-heading button")?.focus();
  }
});
onBeforeUnmount(() => window.removeEventListener("keydown", key));
</script>

<template>
  <div v-if="b.favoritesOpen" class="settings-scrim" @pointerdown.self="b.favoritesOpen = false">
    <section ref="panel" class="settings-panel favorites-panel" role="dialog" aria-modal="true" aria-label="顶部常用网站">
      <header>
        <h2>顶部常用网站</h2>
        <button class="icon-button" aria-label="关闭常用网站设置" @click="b.favoritesOpen = false"><Icon name="close" /></button>
      </header>
      <p class="favorites-description">在侧栏顶部一键打开，所有工作区共用。修改会自动保存。</p>
      <ol class="favorite-editor-list" aria-label="常用网站列表">
        <li v-for="(f, i) in b.state.favorites" :key="f.id" :class="{ editing: previousUrl === f.url }">
          <Favicon :url="f.url" />
          <button class="favorite-edit" :aria-label="`编辑 ${f.title}`" @click="edit(f)">
            <span>{{ f.title }}</span><small>{{ f.url }}</small>
          </button>
          <button class="icon-button" :aria-label="`上移 ${f.title}`" title="上移" :disabled="i === 0" @click="b.send({ type: 'favoriteMove', url: f.url, offset: -1 })"><Icon name="back" :size="13" class="favorite-up" /></button>
          <button class="icon-button" :aria-label="`下移 ${f.title}`" title="下移" :disabled="i === b.state.favorites.length - 1" @click="b.send({ type: 'favoriteMove', url: f.url, offset: 1 })"><Icon name="forward" :size="13" class="favorite-up" /></button>
          <button class="icon-button" :aria-label="`移除 ${f.title}`" title="移除" @click="remove(f)"><Icon name="close" :size="13" /></button>
        </li>
      </ol>
      <p v-if="!b.state.favorites.length" class="favorites-description">还没有常用网站，在下方添加一个。</p>
      <form class="favorite-form" @submit.prevent="save">
        <h3>{{ previousUrl === undefined ? "添加网站" : "编辑网站" }}</h3>
        <label>名称<input v-model="title" aria-label="网站名称" maxlength="80" placeholder="留空时使用域名" /></label>
        <label>网址<input v-model="url" aria-label="网站网址" maxlength="2048" placeholder="example.com" required spellcheck="false" /></label>
        <p v-if="error" class="favorite-error" role="alert">{{ error }}</p>
        <div class="favorite-form-actions">
          <span>{{ b.state.favorites.length }} / {{ MAX_FAVORITES }}</span>
          <button v-if="previousUrl !== undefined" type="button" class="plain-button" @click="reset">取消编辑</button>
          <button type="submit" class="primary-button" :disabled="previousUrl === undefined && b.state.favorites.length >= MAX_FAVORITES">{{ previousUrl === undefined ? "添加网站" : "保存修改" }}</button>
        </div>
      </form>
    </section>
  </div>
</template>
