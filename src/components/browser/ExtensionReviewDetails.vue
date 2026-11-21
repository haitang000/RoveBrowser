<script setup lang="ts">
import type { ExtensionReview } from "../../lib/extensions";
import { useBrowser } from "../../stores/browser";
import ExtensionIcon from "./ExtensionIcon.vue";

defineProps<{ review: ExtensionReview }>();
const b = useBrowser();
</script>

<template>
  <div class="extension-review-layout">
    <ExtensionIcon :src="review.icon" />
    <div class="extension-review-details">
      <div>
        <h2>{{ b.t('安装 {name}？', { name: review.name }) }}</h2>
        <p v-if="review.source === 'chromeStore'">{{ b.t('版本 {version} · 已验证 Chrome 网上应用店签名', { version: review.version }) }}</p>
        <p v-else>{{ b.t('版本 {version}。仅安装你信任来源的扩展。', { version: review.version }) }}</p>
      </div>
      <div v-if="review.permissions.length">
        <strong>{{ b.t('声明的权限（含可选权限）') }}</strong>
        <ul><li v-for="permission in review.permissions" :key="permission">{{ permission }}</li></ul>
      </div>
      <div v-if="review.hosts.length">
        <strong>{{ b.t('声明的网站访问范围') }}</strong>
        <ul><li v-for="host in review.hosts" :key="host">{{ host }}</li></ul>
      </div>
      <p v-if="!review.permissions.length && !review.hosts.length">
        {{ b.t('这份清单未声明额外权限或网站访问范围。') }}
      </p>
    </div>
  </div>
</template>

<style scoped>
.extension-review-layout { display: flex; align-items: flex-start; gap: 12px; }
.extension-review-details {
  flex: 1;
  min-width: 0;
  display: grid;
  gap: 12px;
  font-size: 13px;
  overflow-wrap: anywhere;
}
h2 { margin: 0; font-size: 16px; line-height: 1.4; font-weight: 650; }
p { margin: 0; color: var(--quiet); }
h2 + p { margin-top: 4px; }
ul { margin: 4px 0 0; padding-inline-start: 20px; max-height: 135px; overflow-y: auto; }
</style>
