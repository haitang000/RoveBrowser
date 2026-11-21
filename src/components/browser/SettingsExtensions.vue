<script setup lang="ts">
import { nextTick, onMounted, ref, watch } from "vue";
import type { ExtensionItem } from "../../lib/extensions";
import type { Command } from "../../lib/types";
import { useBrowser } from "../../stores/browser";

const b = useBrowser();
const removal = ref<ExtensionItem | null>(null);
const choose = ref<HTMLButtonElement>();
const cancelReview = ref<HTMLButtonElement>();
const cancelRemoval = ref<HTMLButtonElement>();

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

onMounted(() => run({ type: "extensionsList" }));
</script>

<template>
  <section class="settings-group extension-settings" aria-label="浏览器扩展" :aria-busy="b.extensions.busy">
    <div class="setting-row extension-intro">
      <div class="setting-description">
        <strong>本地扩展</strong>
        <p>选择包含 manifest.json 的 Manifest V3 扩展目录。扩展会作用于所有工作区中的匹配网页。</p>
        <p>扩展工具栏弹窗和商店安装暂不可用。</p>
        <p v-if="!b.native">请在 Windows 桌面应用中管理扩展。</p>
      </div>
      <button
        ref="choose"
        class="setting-button"
        :disabled="!b.native || b.extensions.busy || !!b.extensions.review || !!removal"
        @click="run({ type: 'extensionsChoose' })"
      >加载本地扩展…</button>
    </div>

    <p v-if="b.extensions.error" class="settings-feedback" role="alert">{{ b.extensions.error }}</p>
    <p v-else-if="b.extensions.busy" class="settings-feedback" role="status">正在处理扩展…</p>

    <section
      v-if="b.extensions.review"
      class="extension-review"
      aria-label="确认安装扩展"
      @keydown.esc.stop="review(b.extensions.review.ticket, false)"
    >
      <div class="setting-description">
        <strong>安装 {{ b.extensions.review.name }}？</strong>
        <p>版本 {{ b.extensions.review.version }}。仅安装你信任来源的扩展。</p>
      </div>
      <div v-if="b.extensions.review.permissions.length" class="extension-request">
        <strong>声明的权限（含可选权限）</strong>
        <ul><li v-for="permission in b.extensions.review.permissions" :key="permission">{{ permission }}</li></ul>
      </div>
      <div v-if="b.extensions.review.hosts.length" class="extension-request">
        <strong>声明的网站访问范围</strong>
        <ul><li v-for="host in b.extensions.review.hosts" :key="host">{{ host }}</li></ul>
      </div>
      <p v-if="!b.extensions.review.permissions.length && !b.extensions.review.hosts.length">
        这份清单未声明额外权限或网站访问范围。
      </p>
      <div class="extension-actions">
        <button
          ref="cancelReview"
          class="setting-button"
          :disabled="b.extensions.busy"
          @click="review(b.extensions.review.ticket, false)"
        >取消</button>
        <button
          class="primary-button"
          :disabled="b.extensions.busy"
          @click="review(b.extensions.review.ticket, true)"
        >安装扩展</button>
      </div>
    </section>

    <div v-if="b.native && b.extensions.items" class="extension-list">
      <h3>已安装</h3>
      <div v-if="!b.extensions.items.length" class="setting-row">
        <div class="setting-description"><p>还没有安装扩展。</p></div>
      </div>
      <div v-for="item in b.extensions.items" :key="item.id" class="setting-row extension-row">
        <div class="setting-description">
          <strong>{{ item.name }}</strong>
          <p>版本 {{ item.version }} · {{ item.enabled ? "已启用" : "已停用" }}</p>
        </div>
        <div class="extension-actions">
          <button
            class="setting-toggle"
            role="switch"
            :aria-label="`启用 ${item.name}`"
            :aria-checked="item.enabled"
            :disabled="b.extensions.busy || !!b.extensions.review || !!removal"
            @click="run({ type: 'extensionsEnable', id: item.id, enabled: !item.enabled })"
          ><span /></button>
          <button
            class="setting-button"
            :aria-label="`卸载 ${item.name}`"
            :disabled="b.extensions.busy || !!b.extensions.review || !!removal"
            @click="removal = item"
          >卸载…</button>
        </div>
      </div>
      <section v-if="removal" class="extension-review" aria-label="确认卸载扩展" @keydown.esc.stop="removal = null">
        <div class="setting-description">
          <strong>卸载 {{ removal.name }}？</strong>
          <p>扩展及其应用内副本会被移除。</p>
        </div>
        <div class="extension-actions">
          <button ref="cancelRemoval" class="setting-button" @click="removal = null">取消</button>
          <button class="primary-button" @click="remove(removal.id)">确认卸载</button>
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
