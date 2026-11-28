<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from "vue";
import { CHROME_STORE_URL } from "../../lib/extensions";
import type { Command } from "../../lib/types";
import { useBrowser } from "../../stores/browser";
import ExtensionIcon from "./ExtensionIcon.vue";
import ExtensionReviewDetails from "./ExtensionReviewDetails.vue";

const b = useBrowser();
const choose = ref<HTMLButtonElement>();
const reviewPanel = ref<HTMLElement>();
const cancelReview = ref<HTMLButtonElement>();
const storeInput = ref("");
const inlineReview = computed(() => b.extensions.popup ? null : b.extensions.review);
const unavailable = computed(() => !b.native || b.extensions.busy || b.extensions.popup || !!b.extensions.review);

function run(command: Command) {
  if (!b.native || b.extensions.busy || b.extensions.popup) return;
  b.extensions = { ...b.extensions, busy: true, error: null };
  b.send(command);
}

function review(ticket: string, install: boolean) {
  run({ type: "extensionsReview", ticket, install });
}

watch(inlineReview, async (current, previous) => {
  if (current?.ticket === previous?.ticket) return;
  await nextTick();
  if (current) {
    reviewPanel.value?.scrollIntoView({ block: "start" });
    cancelReview.value?.focus({ preventScroll: true });
  } else choose.value?.focus();
});
onMounted(() => {
  if (b.extensions.busy) b.send({ type: "extensionsList" });
  else run({ type: "extensionsList" });
});
</script>

<template>
  <section class="settings-group extension-settings" :aria-label="b.t('浏览器扩展')" :aria-busy="b.extensions.busy">
    <div v-if="!inlineReview" class="setting-row extension-intro">
      <div class="setting-description">
        <strong>{{ b.t('Chrome 网上应用店') }}</strong>
      </div>
      <button class="setting-button" @click="b.send({ type: 'new', url: CHROME_STORE_URL })">{{ b.t('浏览商店') }}</button>
      <form class="setting-input-form extension-store-form" @submit.prevent="run({ type: 'extensionsChromeStore', input: storeInput.trim() })">
        <input
          v-model="storeInput"
          :aria-label="b.t('商店扩展链接或 ID')"
          placeholder="https://chromewebstore.google.com/detail/…"
          maxlength="4096"
          autocomplete="off"
          spellcheck="false"
          :disabled="unavailable"
          required
        />
        <button class="setting-button" :disabled="unavailable || !storeInput.trim()">
          {{ b.t('查看权限') }}
        </button>
      </form>
    </div>
    <div v-if="!inlineReview" class="setting-row extension-intro">
      <div class="setting-description">
        <strong>{{ b.t('本地扩展') }}</strong>
        <p>{{ b.t('选择包含 manifest.json 的 Manifest V3 扩展目录。扩展会作用于所有工作区中的匹配网页。') }}</p>
        <p>{{ b.t('扩展自动更新暂不可用。') }}</p>
        <p v-if="!b.native">{{ b.t('请在 Windows 桌面应用中管理扩展。') }}</p>
      </div>
      <button
        ref="choose"
        class="setting-button"
        :disabled="unavailable"
        @click="run({ type: 'extensionsChoose' })"
      >{{ b.t('加载本地扩展…') }}</button>
    </div>

    <p v-if="!b.extensions.popup && b.extensions.error" class="settings-feedback" role="alert">{{ b.extensions.error }}</p>
    <p v-else-if="!b.extensions.popup && b.extensions.busy" class="settings-feedback" role="status">{{ b.t('正在处理扩展…') }}</p>

    <section
      v-if="inlineReview"
      ref="reviewPanel"
      class="extension-review"
      :aria-label="b.t('确认安装扩展')"
      @keydown.esc.stop="review(inlineReview.ticket, false)"
    >
      <ExtensionReviewDetails :review="inlineReview" />
      <div class="extension-actions">
        <button
          ref="cancelReview"
          class="setting-button"
          :disabled="b.extensions.busy"
          @click="review(inlineReview.ticket, false)"
        >{{ b.t('取消') }}</button>
        <button
          class="primary-button"
          :disabled="b.extensions.busy"
          @click="review(inlineReview.ticket, true)"
        >{{ b.t('安装扩展') }}</button>
      </div>
    </section>

    <div v-if="b.native && b.extensions.items" class="extension-list">
      <h3>{{ b.t('已安装') }}</h3>
      <div v-if="!b.extensions.items.length" class="setting-row">
        <div class="setting-description"><p>{{ b.t('还没有安装扩展。') }}</p></div>
      </div>
      <div v-for="item in b.extensions.items" :key="item.id" class="setting-row extension-row">
        <div class="extension-identity">
          <ExtensionIcon :src="item.icon" />
          <div class="setting-description">
            <strong>{{ item.name }}</strong>
            <p>{{ b.t('版本 {version}', { version: item.version }) }} · {{ item.enabled ? b.t('已启用') : b.t('已停用') }}<template v-if="item.source === 'chromeStore'"> · {{ b.t('Chrome 网上应用店') }}</template></p>
          </div>
        </div>
        <div class="extension-actions">
          <button
            class="setting-toggle"
            role="switch"
            :aria-label="b.t('启用 {name}', { name: item.name })"
            :aria-checked="item.enabled"
            :disabled="unavailable"
            @click="run({ type: 'extensionsEnable', id: item.id, enabled: !item.enabled })"
          ><span /></button>
          <button
            class="setting-button"
            :aria-label="b.t('卸载 {name}', { name: item.name })"
            :disabled="unavailable"
            @click="run({ type: 'extensionsRequestRemoval', id: item.id })"
          >{{ b.t('卸载…') }}</button>
        </div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.extension-intro,
.extension-row {
  flex-wrap: wrap;
  gap: 12px;
}
.extension-intro .setting-description,
.extension-identity {
  flex: 1 1 180px;
  overflow-wrap: anywhere;
}
.extension-identity { display: flex; align-items: center; min-width: 0; gap: 10px; }
.extension-review {
  display: grid;
  gap: 12px;
  margin: 8px 16px 16px;
  padding: 14px;
  border: 1px solid var(--panel-edge);
  border-radius: 12px;
  background: var(--panel);
  overflow-wrap: anywhere;
}
.extension-store-form {
  display: flex;
  flex: 1 0 100%;
  flex-wrap: wrap;
  gap: 8px;
  max-width: none;
}
.extension-store-form input {
  flex: 1 1 14rem;
  min-width: 0;
  width: auto;
}
.extension-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 10px;
}
.extension-actions .primary-button {
  font-size: 12px;
}
</style>
