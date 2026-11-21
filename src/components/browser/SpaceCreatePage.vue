<script setup lang="ts">
import { nextTick, ref, watch } from "vue";
import { useBrowser } from "../../stores/browser";
import { commandLook, type LookFields } from "../../lib/space-look";
import SpaceForm from "./SpaceForm.vue";

const b = useBrowser();
const saving = ref(false);
let existingIds = new Set<string>();

function preview(look: LookFields) {
  if (b.spaceCreation) Object.assign(b.spaceCreation, look);
}
function cancel() {
  b.closeSpaceCreation();
  void nextTick(() => document.querySelector<HTMLElement>(".browser-menu-trigger")?.focus());
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
</script>

<template>
  <SpaceForm
    v-if="b.spaceCreation"
    v-model:name="b.spaceCreation.name"
    v-model:icon="b.spaceCreation.icon"
    :look="b.spaceCreation"
    :saving="saving"
    @submit="create"
    @dismiss="cancel"
    @preview="preview"
    @commit="preview"
  >
    <button class="primary-button" type="submit" :disabled="!b.spaceCreation.name.trim() || saving || b.state.spaces.length >= 16">{{ saving ? b.t('正在创建…') : b.t('创建 Space') }}</button>
    <button class="plain-button" type="button" :disabled="saving" @click="cancel">{{ b.t('取消') }}</button>
  </SpaceForm>
</template>
