<script setup lang="ts">
import { ref, watch } from "vue";
import { useBrowser } from "../../stores/browser";
import Icon from "../common/Icon.vue";
import SpaceIdentityEditor from "./SpaceIdentityEditor.vue";
import SpaceLookEditor from "./SpaceLookEditor.vue";
import { commandLook, committed, lookOf, openDraft, type LookFields } from "../../lib/space-look";
const b = useBrowser(),
  name = ref(""),
  icon = ref<string>(),
  pickingIcon = ref(false),
  look = ref<LookFields>({ color: "sky" });
watch(
  () => b.settingsOpen,
  (open) => {
    if (open) {
      name.value = b.space.name;
      icon.value = b.space.icon;
      pickingIcon.value = false;
      look.value = committed(openDraft(b.space, b.state.theme === "dark"), lookOf(b.space));
    }
  },
);
function publish(create = false, next = look.value) {
  look.value = next;
  b.send({
    type: create ? "spaceCreate" : "spaceUpdate",
    id: b.space.id,
    name: name.value,
    icon: icon.value ?? null,
    ...commandLook({ ...next, color: b.space.color }),
  });
}
function save(create = false) {
  publish(create);
  b.settingsOpen = false;
}
</script>
<template>
  <div
    v-if="b.settingsOpen"
    class="settings-scrim"
    @pointerdown.self="b.settingsOpen = false"
    @keydown.esc="b.settingsOpen = false"
  >
    <section
      class="settings-panel"
      role="dialog"
      aria-modal="true"
      aria-label="工作区与外观"
    >
      <header>
        <h2>工作区与外观</h2>
        <button
          class="icon-button"
          aria-label="关闭设置"
          @click="b.settingsOpen = false"
        >
          <Icon name="close" />
        </button>
      </header>
      <SpaceIdentityEditor v-model:name="name" v-model:icon="icon" v-model:picking="pickingIcon" />
      <SpaceLookEditor
        v-if="b.settingsOpen"
        v-show="!pickingIcon"
        :space="b.space"
        :dark="b.state.theme === 'dark'"
        @preview="publish(false, $event)"
        @commit="publish(false, $event)"
      />
      <div class="settings-actions">
        <button class="plain-button" @click="save(true)">新建工作区</button
        ><button class="primary-button" @click="save()">保存</button>
      </div>
      <p class="settings-note">清透在「效果」里。背景带轻微模糊，界面和网页保持清晰。</p>
      <hr />
      <button class="theme-button" @click="b.send({ type: 'theme' })">
        <Icon :name="b.state.theme === 'light' ? 'moon' : 'sun'" />{{
          b.state.theme === "light" ? "切换深色模式" : "切换浅色模式"
        }}
      </button>
      <p class="settings-note">Rove · Windows / WebView2</p>
    </section>
  </div>
</template>
