<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useBrowser } from "../../stores/browser";
import { commandLook, type LookFields } from "../../lib/space-look";
import Icon from "../common/Icon.vue";
import SpaceIdentityEditor from "./SpaceIdentityEditor.vue";
import SpaceLookEditor from "./SpaceLookEditor.vue";

const b = useBrowser();
const form = ref<HTMLElement>();
const themePanel = ref<HTMLElement>();
const picking = ref(false);
const themeOpen = ref(false);
const saving = ref(false);
let existingIds = new Set<string>();

function preview(look: LookFields) {
  if (b.spaceCreation) Object.assign(b.spaceCreation, look);
}
function cancel() {
  b.closeSpaceCreation();
  void nextTick(() => document.querySelector<HTMLElement>(".creation-trigger")?.focus());
}
function create() {
  const draft = b.spaceCreation;
  if (!draft?.name.trim() || saving.value) return;
  existingIds = new Set(b.state.spaces.map((space) => space.id));
  saving.value = true;
  b.send({ type: "spaceCreate", name: draft.name.trim(), icon: draft.icon ?? null, ...commandLook(draft) });
  if (b.error) saving.value = false;
}
watch(() => b.state.spaces, (spaces) => {
  if (saving.value && spaces.some((space) => !existingIds.has(space.id))) b.closeSpaceCreation();
}, { deep: true });
watch(() => b.error, (error) => { if (error) saving.value = false; });
watch(picking, (open) => { if (open) themeOpen.value = false; });
function toggleTheme() {
  picking.value = false;
  themeOpen.value = !themeOpen.value;
}
function key(event: KeyboardEvent) {
  if (event.defaultPrevented || event.key !== "Escape" || event.isComposing || b.creationMenu) return;
  event.preventDefault();
  event.stopPropagation();
  if (themeOpen.value) {
    themeOpen.value = false;
    form.value?.querySelector<HTMLElement>(".space-theme-button")?.focus();
  } else if (picking.value) picking.value = false;
  else cancel();
}
function outside(event: PointerEvent) {
  if (!themeOpen.value) return;
  const target = event.target as HTMLElement;
  if (!themePanel.value?.contains(target) && !target.closest(".space-theme-button")) themeOpen.value = false;
}
onMounted(() => {
  form.value?.querySelector<HTMLInputElement>(".space-name-input input")?.focus();
  window.addEventListener("keydown", key);
  window.addEventListener("pointerdown", outside, true);
});
onBeforeUnmount(() => {
  window.removeEventListener("keydown", key);
  window.removeEventListener("pointerdown", outside, true);
});
</script>

<template>
  <form v-if="b.spaceCreation" ref="form" class="space-create-page" aria-labelledby="space-create-title" :aria-busy="saving" @submit.prevent="create">
    <div class="space-create-scroll">
      <header class="space-create-heading">
        <h1 id="space-create-title">创建 Space</h1>
        <p>用 Space 整理你的标签页与浏览会话。</p>
      </header>
      <div class="space-create-fields">
        <SpaceIdentityEditor v-model:name="b.spaceCreation.name" v-model:icon="b.spaceCreation.icon" v-model:picking="picking" placeholder="Space 名称" />
        <div class="space-create-profile"><span>配置</span><span>默认</span></div>
        <button type="button" class="space-theme-button" :aria-expanded="themeOpen" aria-controls="space-create-theme" @click="toggleTheme">主题…</button>
      </div>
    </div>
    <div class="space-create-actions">
      <p v-if="b.error" role="alert">{{ b.error }}</p>
      <button class="primary-button" type="submit" :disabled="!b.spaceCreation.name.trim() || saving || b.state.spaces.length >= 16">{{ saving ? '正在创建…' : '创建 Space' }}</button>
      <button class="plain-button" type="button" :disabled="saving" @click="cancel">取消</button>
    </div>
    <Teleport to="body">
      <section v-if="themeOpen" ref="themePanel" id="space-create-theme" class="space-editor-panel space-create-theme" role="region" aria-label="Space 主题">
        <header><h2>主题</h2><button type="button" class="icon-button" aria-label="关闭主题" @click="themeOpen = false"><Icon name="close" :size="14" /></button></header>
        <SpaceLookEditor :space="b.spaceCreation" :dark="b.state.theme === 'dark'" @preview="preview" @commit="preview" />
      </section>
    </Teleport>
  </form>
</template>

<style src="../../styles/space-creation.css"></style>
