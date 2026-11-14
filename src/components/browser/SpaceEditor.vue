<script setup lang="ts">
// Floating space editor, from Zigzag/Sidebar/SpaceEditor.swift (SpaceEditorPanel).
// Copyright 2026 Zigao Wang, Apache-2.0. Modified for Rove: no profile row.
import { onBeforeUnmount, onMounted, ref } from "vue";
import { useBrowser } from "../../stores/browser";
import SpaceIdentityEditor from "./SpaceIdentityEditor.vue";
import SpaceLookEditor from "./SpaceLookEditor.vue";
import { commandLook, type LookFields } from "../../lib/space-look";
const b = useBrowser();
const panel = ref<HTMLElement>();
const name = ref(b.space.name);
const icon = ref(b.space.icon);
const picking = ref(false);
const look = ref<LookFields>({ color: b.space.color });

function send(next = look.value) {
  look.value = next;
  b.previewSpace(b.space.id, next);
  b.send({
    type: "spaceUpdate",
    id: b.space.id,
    name: name.value.trim() || b.space.name,
    icon: icon.value ?? null,
    ...commandLook({ ...next, color: b.space.color }),
  });
}
function onPointerDown(event: PointerEvent) {
  const target = event.target as HTMLElement;
  if (panel.value?.contains(target)) return;
  if (target.closest?.(".space-name, .space-key")) return;
  b.closeSpaceEditor();
}
function onKey(event: KeyboardEvent) {
  if (event.key !== "Escape" || event.isComposing) return;
  event.preventDefault();
  b.closeSpaceEditor();
}
onMounted(() => {
  name.value = b.space.name;
  icon.value = b.space.icon;
  window.addEventListener("pointerdown", onPointerDown, true);
  window.addEventListener("keydown", onKey, true);
});
onBeforeUnmount(() => {
  window.removeEventListener("pointerdown", onPointerDown, true);
  window.removeEventListener("keydown", onKey, true);
  b.spacePreview = undefined;
});
</script>
<template>
  <section
    ref="panel"
    class="space-editor-panel"
    role="dialog"
    aria-modal="false"
    aria-label="编辑空间"
    @keydown.esc.stop="b.closeSpaceEditor()"
  >
    <SpaceIdentityEditor v-model:name="name" v-model:icon="icon" v-model:picking="picking" @update:name="send()" @update:icon="send()" />
    <SpaceLookEditor
      v-show="!picking"
      :space="b.space"
      :dark="b.state.theme === 'dark'"
      @preview="b.previewSpace(b.space.id, $event)"
      @commit="send"
    />
  </section>
</template>
<style src="../../styles/space-editor.css"></style>
