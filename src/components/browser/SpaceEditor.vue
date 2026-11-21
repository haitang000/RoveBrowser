<script setup lang="ts">
// Adapted from Zigzag/Sidebar/SpaceEditor.swift (SpaceEditorPanel).
// Copyright 2026 Zigao Wang, Apache-2.0. Modified for Rove.
import { nextTick, onBeforeUnmount, ref } from "vue";
import { useBrowser } from "../../stores/browser";
import SpaceForm from "./SpaceForm.vue";
import SpaceIdentityEditor from "./SpaceIdentityEditor.vue";
import SpaceLookEditor from "./SpaceLookEditor.vue";
import { commandLook, lookOf, type LookFields } from "../../lib/space-look";
defineProps<{ embedded?: boolean }>();
const b = useBrowser();
const id = b.space.id;
const name = ref(b.space.name);
const icon = ref(b.space.icon);
const picking = ref(false);
const look = ref(lookOf(b.space));
let updatedName = name.value;

function preview(next: LookFields) {
  b.previewSpace(id, next);
}
function send(next = look.value) {
  look.value = next;
  preview(next);
  b.send({
    type: "spaceUpdate",
    id,
    name: updatedName,
    icon: icon.value ?? null,
    ...commandLook(next),
  });
}
function rename(next: string) {
  name.value = next;
  const trimmed = next.trim();
  if (!trimmed || trimmed === updatedName) return;
  updatedName = trimmed;
  send();
}
function setIcon(next?: string) {
  icon.value = next;
  send();
}
function finish(restoreFocus = true) {
  b.closeSpaceEditor();
  if (restoreFocus) void nextTick(() => document.querySelector<HTMLElement>(".sidebar-header .space-name")?.focus());
}
onBeforeUnmount(() => {
  if (b.spacePreview?.id === id) b.spacePreview = undefined;
});
</script>
<template>
  <section v-if="embedded" class="space-editor-panel" role="region" :aria-label="b.t('编辑空间')">
    <SpaceIdentityEditor :name="name" :icon="icon" v-model:picking="picking" @update:name="rename" @update:icon="setIcon" />
    <SpaceLookEditor
      v-show="!picking"
      :space="look"
      :dark="b.state.theme === 'dark'"
      @preview="preview"
      @commit="send"
    />
  </section>
  <SpaceForm
    v-else
    editing
    :name="name"
    :icon="icon"
    :look="look"
    @update:name="rename"
    @update:icon="setIcon"
    @preview="preview"
    @commit="send"
    @close-theme="preview(look)"
    @submit="finish()"
    @dismiss="finish"
  >
    <button class="primary-button" type="submit">{{ b.t('完成') }}</button>
  </SpaceForm>
</template>
