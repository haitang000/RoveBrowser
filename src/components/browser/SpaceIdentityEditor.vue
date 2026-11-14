<script setup lang="ts">
import { computed, ref } from "vue";
import SpaceGlyph from "../common/SpaceGlyph.vue";
import {
  SPACE_EMOJIS,
  cleanIcon,
  clearsEmoji,
  fieldState,
  normalized,
  searchEmoji,
  toggled,
} from "../../lib/space-icon";

const name = defineModel<string>("name", { required: true });
const icon = defineModel<string | undefined>("icon");
const picking = defineModel<boolean>("picking", { default: false });
const fieldId = `space-name-${Math.random().toString(36).slice(2, 8)}`;
const field = ref(cleanIcon(icon.value) ?? "");
const shown = ref<string | undefined>(cleanIcon(icon.value));
const query = ref<string>();
const composing = ref(false);
const results = computed(() => (query.value ? searchEmoji(query.value).slice(0, 64) : []));

function syncField() {
  const emoji = cleanIcon(icon.value);
  field.value = emoji ?? "";
  shown.value = emoji;
  query.value = undefined;
}
function toggle() {
  picking.value = !picking.value;
  if (picking.value) syncField();
}
function choose(next?: string, close = !next) {
  icon.value = next;
  if (close) picking.value = false;
  const emoji = cleanIcon(next);
  field.value = emoji ?? "";
  shown.value = emoji;
  query.value = undefined;
}
function onInput(event: Event) {
  if (composing.value) return;
  const text = (event.target as HTMLInputElement).value;
  const state = fieldState(text, shown.value);
  if (state.type === "empty") {
    const wasEmoji = shown.value !== undefined;
    shown.value = undefined;
    query.value = undefined;
    field.value = "";
    if (wasEmoji && clearsEmoji("", icon.value)) choose(undefined);
    return;
  }
  if (state.type === "emoji") {
    choose(state.emoji);
    return;
  }
  field.value = state.text;
  shown.value = undefined;
  query.value = normalized(state.text) ? state.text : undefined;
}
function onEnter(event: KeyboardEvent) {
  event.preventDefault();
  if (!query.value) return;
  const first = results.value[0];
  if (first) choose(first.emoji);
}
function onEsc(event: KeyboardEvent) {
  if (!query.value) return;
  event.preventDefault();
  event.stopPropagation();
  syncField();
}
</script>
<template>
  <div class="space-identity">
    <label :for="fieldId">工作区名称</label>
    <div class="space-name-input">
      <button
        type="button"
        class="space-icon-trigger"
        :aria-pressed="picking"
        aria-label="选择图标"
        title="选择图标"
        @click="toggle"
      >
        <SpaceGlyph v-if="icon" :icon="icon" :size="14" />
        <svg v-else viewBox="0 0 16 16" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.2" aria-hidden="true">
          <circle cx="8" cy="8" r="5.2" stroke-dasharray="2.2 1.8" />
        </svg>
      </button>
      <input
        :id="fieldId"
        v-model="name"
        aria-label="工作区名称"
        maxlength="40"
        autocomplete="off"
      />
    </div>
    <div v-if="picking" class="icon-picker">
      <div v-if="query" class="icon-grid icon-results" role="listbox" aria-label="表情搜索结果">
        <p v-if="!results.length" class="icon-empty">没有找到表情</p>
        <button
          v-for="entry in results"
          :key="entry.emoji"
          type="button"
          :title="entry.name"
          :aria-label="entry.name"
          @click="choose(entry.emoji)"
        >
          <SpaceGlyph :icon="entry.emoji" :size="20" />
        </button>
      </div>
      <div v-else class="icon-grid" role="group" aria-label="图标">
        <button type="button" title="无图标" aria-label="无图标" :aria-pressed="!icon" @click="choose(undefined)">
          <svg viewBox="0 0 16 16" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.2" aria-hidden="true">
            <circle cx="8" cy="8" r="5.2" stroke-dasharray="2.2 1.8" />
          </svg>
        </button>
        <button
          v-for="item in SPACE_EMOJIS"
          :key="item.emoji"
          type="button"
          :title="item.label"
          :aria-label="item.label"
          :aria-pressed="icon === item.emoji"
          @click="choose(toggled(item.emoji, icon), true)"
        >
          <SpaceGlyph :icon="item.emoji" :size="18" />
        </button>
      </div>
      <input
        :value="field"
        type="text"
        aria-label="搜索或输入表情"
        placeholder="搜索或输入表情"
        autocomplete="off"
        spellcheck="false"
        @input="onInput"
        @compositionstart="composing = true"
        @compositionend="composing = false; onInput($event)"
        @keydown.enter="onEnter"
        @keydown.esc="onEsc"
      />
    </div>
  </div>
</template>
