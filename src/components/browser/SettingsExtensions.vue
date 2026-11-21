<script setup lang="ts">
import { nextTick, onMounted, ref, watch } from "vue";
import { CHROME_STORE_URL, type ExtensionItem } from "../../lib/extensions";
import type { Command } from "../../lib/types";
import { useBrowser } from "../../stores/browser";

const b = useBrowser();
const removal = ref<ExtensionItem | null>(null);
const choose = ref<HTMLButtonElement>();
const cancelReview = ref<HTMLButtonElement>();
const cancelRemoval = ref<HTMLButtonElement>();
const storeInput = ref("");

function run(command: Command) {
  if (!b.native || b.extensions.busy) return;
  b.extensions = { ...b.extensions, busy: true, error: null };
  b.send(command);
}

function review(ticket: string, install: boolean) {
  run({ type: "extensionsReview", ticket, install });
}

function remove(id: string) {
  removal.value = null;
  run({ type: "extensionsRemove", id });
}

watch(() => b.extensions.review, async (current, previous) => {
  if (current?.ticket === previous?.ticket) return;
  await nextTick();
  if (current) cancelReview.value?.focus();
  else choose.value?.focus();
});
watch(removal, async (item) => {
  if (item) {
    await nextTick();
    cancelRemoval.value?.focus();
  }
});

onMounted(() => {
  if (b.extensions.busy) b.send({ type: "extensionsList" });
  else run({ type: "extensionsList" });
});
</script>

<template>
  <section class="settings-group extension-settings" :aria-label="b.t('浏览器扩展')" :aria-busy="b.extensions.busy">
    <div class="setting-row extension-intro">
      <div class="setting-description">
        <strong>{{ b.t('Chrome 网上应用店') }}</strong>
        <p>{{ b.t('粘贴扩展详情链接或 ID。浏览详情页时，也可以从浏览器菜单安装。') }}</p>
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
          :disabled="!b.native || b.extensions.busy || !!b.extensions.review || !!removal"
          required
        />
        <button class="setting-button" :disabled="!b.native || b.extensions.busy || !!b.extensions.review || !!removal || !storeInput.trim()">
          {{ b.t('查看权限') }}
        </button>
      </form>
    </div>
    <div class="setting-row extension-intro">
      <div class="setting-description">
        <strong>{{ b.t('本地扩展') }}</strong>
        <p>{{ b.t('选择包含 manifest.json 的 Manifest V3 扩展目录。扩展会作用于所有工作区中的匹配网页。') }}</p>
        <p>{{ b.t('扩展工具栏弹窗和自动更新暂不可用。') }}</p>
        <p v-if="!b.native">{{ b.t('请在 Windows 桌面应用中管理扩展。') }}</p>
      </div>
      <button
        ref="choose"
        class="setting-button"
        :disabled="!b.native || b.extensions.busy || !!b.extensions.review || !!removal"
        @click="run({ type: 'extensionsChoose' })"
      >{{ b.t('加载本地扩展…') }}</button>
    </div>

    <p v-if="b.extensions.error" class="settings-feedback" role="alert">{{ b.extensions.error }}</p>
    <p v-else-if="b.extensions.busy" class="settings-feedback" role="status">{{ b.t('正在处理扩展…') }}</p>

    <section
      v-if="b.extensions.review"
      class="extension-review"
      :aria-label="b.t('确认安装扩展')"
      @keydown.esc.stop="review(b.extensions.review.ticket, false)"
    >
      <div class="setting-description">
        <strong>{{ b.t('安装 {name}？', { name: b.extensions.review.name }) }}</strong>
        <p v-if="b.extensions.review.source === 'chromeStore'">{{ b.t('版本 {version} · 已验证 Chrome 网上应用店签名', { version: b.extensions.review.version }) }}</p>
        <p v-else>{{ b.t('版本 {version}。仅安装你信任来源的扩展。', { version: b.extensions.review.version }) }}</p>
      </div>
      <div v-if="b.extensions.review.permissions.length" class="extension-request">
        <strong>{{ b.t('声明的权限（含可选权限）') }}</strong>
        <ul><li v-for="permission in b.extensions.review.permissions" :key="permission">{{ permission }}</li></ul>
      </div>
      <div v-if="b.extensions.review.hosts.length" class="extension-request">
        <strong>{{ b.t('声明的网站访问范围') }}</strong>
        <ul><li v-for="host in b.extensions.review.hosts" :key="host">{{ host }}</li></ul>
      </div>
      <p v-if="!b.extensions.review.permissions.length && !b.extensions.review.hosts.length">
        {{ b.t('这份清单未声明额外权限或网站访问范围。') }}
      </p>
      <div class="extension-actions">
        <button
          ref="cancelReview"
          class="setting-button"
          :disabled="b.extensions.busy"
          @click="review(b.extensions.review.ticket, false)"
        >{{ b.t('取消') }}</button>
        <button
          class="primary-button"
          :disabled="b.extensions.busy"
          @click="review(b.extensions.review.ticket, true)"
        >{{ b.t('安装扩展') }}</button>
      </div>
    </section>

    <div v-if="b.native && b.extensions.items" class="extension-list">
      <h3>{{ b.t('已安装') }}</h3>
      <div v-if="!b.extensions.items.length" class="setting-row">
        <div class="setting-description"><p>{{ b.t('还没有安装扩展。') }}</p></div>
      </div>
      <div v-for="item in b.extensions.items" :key="item.id" class="setting-row extension-row">
        <div class="setting-description">
          <strong>{{ item.name }}</strong>
          <p>{{ b.t('版本 {version}', { version: item.version }) }} · {{ item.enabled ? b.t('已启用') : b.t('已停用') }}<template v-if="item.source === 'chromeStore'"> · {{ b.t('Chrome 网上应用店') }}</template></p>
        </div>
        <div class="extension-actions">
          <button
            class="setting-toggle"
            role="switch"
            :aria-label="b.t('启用 {name}', { name: item.name })"
            :aria-checked="item.enabled"
            :disabled="b.extensions.busy || !!b.extensions.review || !!removal"
            @click="run({ type: 'extensionsEnable', id: item.id, enabled: !item.enabled })"
          ><span /></button>
          <button
            class="setting-button"
            :aria-label="b.t('卸载 {name}', { name: item.name })"
            :disabled="b.extensions.busy || !!b.extensions.review || !!removal"
            @click="removal = item"
          >{{ b.t('卸载…') }}</button>
        </div>
      </div>
      <section v-if="removal" class="extension-review" :aria-label="b.t('确认卸载扩展')" @keydown.esc.stop="removal = null">
        <div class="setting-description">
          <strong>{{ b.t('卸载 {name}？', { name: removal.name }) }}</strong>
          <p>{{ b.t('扩展及其应用内副本会被移除。') }}</p>
        </div>
        <div class="extension-actions">
          <button ref="cancelRemoval" class="setting-button" @click="removal = null">{{ b.t('取消') }}</button>
          <button class="primary-button" @click="remove(removal.id)">{{ b.t('确认卸载') }}</button>
        </div>
      </section>
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
.extension-row .setting-description {
  flex: 1 1 180px;
  overflow-wrap: anywhere;
}
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
.extension-review > p,
.extension-request {
  margin: 0;
  color: var(--quiet);
  font-size: 12px;
}
.extension-request ul {
  margin: 4px 0 0;
  padding-inline-start: 20px;
  max-height: 135px;
  overflow-y: auto;
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
