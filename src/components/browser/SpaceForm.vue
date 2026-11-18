<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, useId, watch } from "vue";
import { useBrowser } from "../../stores/browser";
import type { LookFields } from "../../lib/space-look";
import Icon from "../common/Icon.vue";
import SpaceIdentityEditor from "./SpaceIdentityEditor.vue";
import SpaceLookEditor from "./SpaceLookEditor.vue";

const props = defineProps<{ look: LookFields; editing?: boolean; saving?: boolean }>();
const emit = defineEmits<{
  submit: [];
  dismiss: [restoreFocus: boolean];
  preview: [look: LookFields];
  commit: [look: LookFields];
  closeTheme: [];
}>();
const name = defineModel<string>("name", { required: true });
const icon = defineModel<string>("icon");
const b = useBrowser();
const id = useId();
const form = ref<HTMLFormElement>();
const themePanel = ref<HTMLElement>();
const themeButton = ref<HTMLButtonElement>();
const picking = ref(false);
const themeOpen = ref(false);

function closeTheme(restoreFocus = false) {
  if (!themeOpen.value) return;
  themeOpen.value = false;
  emit("closeTheme");
  if (restoreFocus) themeButton.value?.focus();
}
watch(picking, (open) => { if (open) closeTheme(); });
function toggleTheme() {
  if (themeOpen.value) closeTheme();
  else {
    picking.value = false;
    themeOpen.value = true;
  }
}
function key(event: KeyboardEvent) {
  if (event.defaultPrevented || event.key !== "Escape" || event.isComposing || b.creationMenu) return;
  event.preventDefault();
  event.stopPropagation();
  if (themeOpen.value) closeTheme(true);
  else if (picking.value) {
    picking.value = false;
    form.value?.querySelector<HTMLButtonElement>(".space-icon-trigger")?.focus();
  } else if (!props.saving) emit("dismiss", true);
}
function outside(event: PointerEvent) {
  const target = event.target;
  if (!(target instanceof Element) || themePanel.value?.contains(target)) return;
  if (!themeButton.value?.contains(target)) closeTheme();
  if (props.editing && !form.value?.closest(".sidebar")?.contains(target)) emit("dismiss", false);
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
  <form
    ref="form"
    class="space-form"
    :role="editing ? 'dialog' : undefined"
    :aria-label="editing ? '编辑空间' : undefined"
    :aria-labelledby="editing ? undefined : `${id}-title`"
    :aria-busy="saving"
    @submit.prevent="emit('submit')"
  >
    <div class="space-form-scroll">
      <header class="space-form-heading">
        <h1 :id="`${id}-title`">{{ editing ? '编辑 Space' : '创建 Space' }}</h1>
        <p>{{ editing ? '更改当前 Space 的名称、图标与主题。' : '用 Space 整理你的标签页与浏览会话。' }}</p>
      </header>
      <div class="space-form-fields">
        <SpaceIdentityEditor v-model:name="name" v-model:icon="icon" v-model:picking="picking" placeholder="Space 名称" />
        <div class="space-form-profile"><span>配置</span><span>默认</span></div>
        <button ref="themeButton" type="button" class="space-theme-button" :aria-expanded="themeOpen" :aria-controls="`${id}-theme`" @click="toggleTheme">主题…</button>
      </div>
    </div>
    <div class="space-form-actions">
      <p v-if="b.error" role="alert">{{ b.error }}</p>
      <slot />
    </div>
    <Teleport to="body">
      <section v-if="themeOpen" :id="`${id}-theme`" ref="themePanel" class="space-editor-panel space-form-theme" role="region" aria-label="Space 主题">
        <header><h2>主题</h2><button type="button" class="icon-button" aria-label="关闭主题" @click="closeTheme(true)"><Icon name="close" :size="14" /></button></header>
        <SpaceLookEditor :space="look" :dark="b.state.theme === 'dark'" @preview="emit('preview', $event)" @commit="emit('commit', $event)" />
      </section>
    </Teleport>
  </form>
</template>

<style src="../../styles/space-form.css"></style>
