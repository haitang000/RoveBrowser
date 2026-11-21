<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { useBrowser } from "../../stores/browser";
import Icon from "../common/Icon.vue";

const b = useBrowser();
const menu = computed(() => b.contextMenu?.pinFolder ? b.contextMenu : undefined);
const target = computed(() => menu.value?.pinFolder);
const folder = computed(() => {
  const value = target.value;
  return value && b.state.pinLayouts?.[value.spaceId]?.folders.find((folder) => folder.id === value.id);
});
const editing = ref(false);
const name = ref("");
const panel = ref<HTMLElement>();
watch(target, async (value) => {
  editing.value = Boolean(value && !value.id);
  name.value = "";
  if (!value) return;
  await nextTick();
  panel.value?.querySelector<HTMLElement>(editing.value ? "input" : "button")?.focus();
});

function close() { b.contextMenu = undefined; }
function act(type: string, extra = {}) {
  if (target.value) b.send({ type, ...target.value, ...extra });
  close();
}
async function rename() {
  name.value = folder.value?.name ?? "";
  editing.value = true;
  await nextTick();
  panel.value?.querySelector("input")?.select();
}
function save() {
  if (!name.value.trim()) return;
  act(target.value?.id ? "pinFolderRename" : "pinFolderCreate", { name: name.value });
}
</script>

<template>
  <Teleport to="body">
    <div v-if="menu && (!target?.id || folder)" class="menu-scrim" @pointerdown.self="close" @keydown.esc.stop.prevent="close">
      <div ref="panel" class="context-menu" :style="{ left: `${menu.x}px`, top: `${menu.y}px`, maxHeight: `calc(100dvh - ${menu.y + 12}px)` }"
        :role="editing ? 'dialog' : 'menu'" :aria-label="editing ? (folder ? '重命名文件夹' : '新建文件夹') : folder?.name">
        <form v-if="editing" class="bookmark-menu-form" @submit.prevent="save">
          <label>文件夹名称<input v-model="name" maxlength="80" required /></label>
          <button type="submit" class="primary-button" :disabled="!name.trim()">{{ folder ? '保存名称' : '创建文件夹' }}</button>
        </form>
        <template v-else>
          <div class="menu-label menu-tab-title">{{ folder?.name }}</div>
          <button type="button" role="menuitem" @click="rename"><Icon name="edit" />重命名文件夹</button>
          <button type="button" role="menuitem" @click="act('pinFolderRemove')"><Icon name="folder" />解散文件夹</button>
        </template>
      </div>
    </div>
  </Teleport>
</template>
