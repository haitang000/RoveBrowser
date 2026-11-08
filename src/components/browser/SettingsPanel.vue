<script setup lang="ts">
import { ref, watch } from "vue";
import { useBrowser } from "../../stores/browser";
import type { SpaceColor } from "../../lib/types";
import Icon from "../common/Icon.vue";
const b = useBrowser(),
  name = ref(""),
  color = ref<SpaceColor>("sky");
const colors: SpaceColor[] = [
  "sky",
  "sage",
  "sand",
  "rose",
  "lavender",
  "mint",
  "peach",
  "graphite",
];
watch(
  () => b.settingsOpen,
  (open) => {
    if (open) {
      name.value = b.space.name;
      color.value = b.space.color;
    }
  },
);
function save(create = false) {
  b.send({
    type: create ? "spaceCreate" : "spaceUpdate",
    id: b.space.id,
    name: name.value,
    color: color.value,
  });
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
      <label
        >工作区名称<input v-model="name" aria-label="工作区名称" maxlength="40"
      /></label>
      <div class="swatches" aria-label="工作区颜色">
        <button
          v-for="c in colors"
          :key="c"
          :data-color="c"
          :class="{ chosen: c === color }"
          :aria-label="c"
          :aria-pressed="c === color"
          :title="c"
          @click="color = c"
        >
          <Icon v-if="c === color" name="check" />
        </button>
      </div>
      <div class="settings-actions">
        <button class="plain-button" @click="save(true)">新建工作区</button
        ><button class="primary-button" @click="save()">保存</button>
      </div>
      <hr />
      <button class="theme-button" @click="b.favoritesOpen = true; b.settingsOpen = false">
        <Icon name="pin" />管理顶部常用网站
      </button>
      <button class="theme-button" @click="b.send({ type: 'theme' })">
        <Icon :name="b.state.theme === 'light' ? 'moon' : 'sun'" />{{
          b.state.theme === "light" ? "切换深色模式" : "切换浅色模式"
        }}
      </button>
      <p class="settings-note">Rove · Windows / WebView2</p>
    </section>
  </div>
</template>
