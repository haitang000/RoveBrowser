<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { PIN_FOLDER_COLORS, type PinFolderColor } from "../../lib/pin-folders";
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
const color = ref<PinFolderColor>("default");
const panel = ref<HTMLElement>();
const nameInput = ref<HTMLInputElement>();
watch(target, async (value) => {
  editing.value = Boolean(value && !value.id);
  name.value = "";
  color.value = "default";
  if (!value) return;
  await nextTick();
  if (editing.value) nameInput.value?.focus();
  else panel.value?.querySelector("button")?.focus();
});

function close() { b.contextMenu = undefined; }
function act(type: string, extra = {}) {
  if (target.value) b.send({ type, ...target.value, ...extra });
  close();
}
async function editFolder() {
  name.value = folder.value?.name ?? "";
  color.value = folder.value?.color ?? "default";
  editing.value = true;
  await nextTick();
  nameInput.value?.focus();
  nameInput.value?.select();
}
function save() {
  if (!name.value.trim()) return;
  act(target.value?.id ? "pinFolderRename" : "pinFolderCreate", { name: name.value, color: color.value });
}
</script>

<template>
  <Teleport to="body">
    <div v-if="menu && (!target?.id || folder)" class="menu-scrim" @pointerdown.self="close" @keydown.esc.stop.prevent="close">
      <form v-if="editing" class="pin-folder-editor" role="dialog" aria-labelledby="pin-folder-editor-title"
        :style="{ '--anchor-x': `${menu.x}px`, '--anchor-y': `${menu.y}px` }" @submit.prevent="save">
        <h2 id="pin-folder-editor-title">{{ folder ? '编辑文件夹' : '新建文件夹' }}</h2>
        <label class="folder-name-field">
          文件夹名称
          <input ref="nameInput" v-model="name" type="text" placeholder="输入名称" maxlength="80" required />
        </label>
        <fieldset class="folder-color-field">
          <legend>颜色</legend>
          <div class="folder-color-palette">
            <label v-for="(option, value) in PIN_FOLDER_COLORS" :key="value" :title="option.label" class="folder-color-option">
              <input v-model="color" type="radio" name="folder-color" :value="value" :aria-label="option.label" />
              <span class="folder-color-bubble" :style="{ '--swatch': option.fill }"></span>
            </label>
          </div>
        </fieldset>
        <div class="folder-editor-actions">
          <button type="button" class="plain-button" @click="close">取消</button>
          <button type="submit" class="primary-button" :disabled="!name.trim()">{{ folder ? '保存修改' : '创建文件夹' }}</button>
        </div>
      </form>
      <div v-else ref="panel" class="context-menu" role="menu" :aria-label="folder?.name"
        :style="{ left: `clamp(8px, ${menu.x}px, calc(100dvw - 218px))`, top: `clamp(calc(var(--titlebar-block) + 4px), ${menu.y}px, calc(100dvh - 132px))` }">
        <div class="menu-label menu-tab-title">{{ folder?.name }}</div>
        <button type="button" role="menuitem" @click="editFolder"><Icon name="edit" />编辑文件夹</button>
        <button type="button" role="menuitem" @click="act('pinFolderRemove')"><Icon name="folder" />解散文件夹</button>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.pin-folder-editor {
  --editor-width: min(366px, calc(100dvw - 16px));
  --editor-height: 226px;
  --editor-left: clamp(8px, calc(var(--anchor-x) + 24px - var(--editor-width)), calc(100dvw - var(--editor-width) - 8px));
  --editor-top: clamp(calc(var(--titlebar-block) + 8px), var(--anchor-y), calc(100dvh - var(--editor-height) - 8px));
  position: fixed;
  left: var(--editor-left);
  top: var(--editor-top);
  display: grid;
  gap: 12px;
  width: var(--editor-width);
  max-height: calc(100dvh - var(--editor-top) - 8px);
  overflow: auto;
  padding: 14px 15px 12px;
  border: 1px solid var(--panel-edge);
  border-radius: 14px;
  background: var(--panel);
  color: var(--ink);
  box-shadow: 0 10px 28px var(--panel-shadow), 0 2px 6px var(--shadow);
  transform-origin:
    clamp(14px, calc(var(--anchor-x) + 12px - var(--editor-left)), calc(var(--editor-width) - 14px))
    clamp(0px, calc(var(--anchor-y) - var(--editor-top)), calc(var(--editor-height) - 14px));
  animation: folder-editor-pop 300ms both;
}
.pin-folder-editor h2 {
  margin: 0;
  padding-bottom: 10px;
  border-bottom: 1px solid var(--edge);
  font-size: 15px;
  font-weight: 650;
}
.folder-name-field {
  display: grid;
  gap: 6px;
  font-size: 12px;
  font-weight: 600;
}
.folder-name-field input {
  width: 100%;
  height: 34px;
  padding: 0 12px;
  border: 1px solid var(--edge);
  border-radius: 18px;
  background: var(--surface);
  color: var(--ink);
  font-size: 13px;
  font-weight: 400;
}
.folder-color-field {
  min-width: 0;
  margin: 0;
  padding: 0;
  border: 0;
}
.folder-color-field legend {
  padding: 0;
  font-size: 12px;
  font-weight: 600;
}
.folder-color-palette {
  display: grid;
  grid-template-columns: repeat(10, minmax(0, 1fr));
  margin: 3px -4px 0;
}
.folder-color-option {
  position: relative;
  display: grid;
  place-items: center;
  height: 34px;
  cursor: pointer;
}
.folder-color-option input {
  position: absolute;
  width: 1px;
  height: 1px;
  opacity: 0;
}
.folder-color-bubble {
  width: 20px;
  height: 20px;
  border: 1px solid color-mix(in srgb, var(--swatch) 83%, var(--ink));
  border-radius: 50%;
  background: var(--swatch);
  transition: transform 180ms ease-out;
}
.folder-color-option input:checked + .folder-color-bubble {
  outline: 2px solid var(--ink);
  outline-offset: 2px;
  animation: folder-color-pop 280ms both;
}
.folder-color-option input:focus-visible + .folder-color-bubble {
  box-shadow: 0 0 0 6px var(--chosen);
}
@media (hover: hover) and (pointer: fine) {
  .folder-color-option:hover .folder-color-bubble { transform: scale(1.1); }
}
.folder-color-option:active .folder-color-bubble { transform: scale(.86); }
.folder-editor-actions { display: flex; flex-wrap: wrap; justify-content: flex-end; gap: 8px; }
.folder-editor-actions button { height: 30px; border-radius: 8px; font-size: 12px; }
.folder-editor-actions .plain-button:hover { background: var(--hover); }
.folder-editor-actions .primary-button:disabled { opacity: .42; cursor: default; }

@keyframes folder-editor-pop {
  0% { opacity: 0; transform: translateY(-4px) scale(.94, .89); animation-timing-function: var(--ease-out); }
  58% { opacity: 1; transform: translateY(1px) scale(1.013, 1.027); animation-timing-function: ease-in-out; }
  82% { transform: scale(.997, .995); }
  100% { opacity: 1; transform: none; }
}
@keyframes folder-color-pop {
  0% { scale: .8; animation-timing-function: var(--ease-out); }
  52% { scale: 1.18; animation-timing-function: ease-in-out; }
  78% { scale: .97; }
  100% { scale: 1; }
}
@media (max-width: 332px) {
  .pin-folder-editor { --editor-height: 260px; }
  .folder-color-palette { grid-template-columns: repeat(5, minmax(0, 1fr)); }
}
:global(:root[data-reduce-motion=true]) .pin-folder-editor,
:global(:root[data-reduce-motion=true]) .folder-color-option input:checked + .folder-color-bubble {
  animation-name: none;
}
:global(:root[data-reduce-motion=true]) .folder-color-bubble { transform: none; }
@media (prefers-reduced-motion: reduce) {
  .pin-folder-editor,
  .folder-color-option input:checked + .folder-color-bubble { animation-name: none; }
  .folder-color-option .folder-color-bubble { transform: none; }
}
</style>
