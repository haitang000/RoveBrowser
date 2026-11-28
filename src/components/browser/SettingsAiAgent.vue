<script setup lang="ts">
import { reactive } from "vue";
import { CODEX_ALLOW_ALL, CODEX_SITE_LISTS, codexSiteDomain, type CodexAccessChange, type CodexSiteList } from "../../lib/codex-access";
import { useBrowser } from "../../stores/browser";
import Icon from "../common/Icon.vue";

defineProps<{ tabId: string }>();
const b = useBrowser();
const drafts = reactive({ allowlist: { input: "", error: "" }, blocklist: { input: "", error: "" } });

function update(change: CodexAccessChange) {
  b.send({ type: "codexAccessUpdate", change });
}

function addSite(list: CodexSiteList) {
  const draft = drafts[list];
  const site = codexSiteDomain(draft.input);
  if (!site) {
    draft.error = "请输入有效的域名或 HTTP(S) 网址。";
    return;
  }
  if (b.settings.codexAccess[list].includes(site)) {
    draft.error = "此网站已在列表中。";
    return;
  }
  update({ kind: "site", list, site, add: true });
  draft.input = "";
  draft.error = "";
}
</script>

<template>
  <section class="settings-group" :aria-label="b.t('网站访问')">
    <h3>{{ b.t('网站访问') }}</h3>
    <div class="setting-row">
      <div class="setting-description">
        <label :for="`codex-allow-all-${tabId}`">{{ b.t(CODEX_ALLOW_ALL) }}</label>
        <p :id="`codex-allow-all-help-${tabId}`">{{ b.t('开启后，Codex 可访问当前标签页中的网站，黑名单仍然有效。关闭时，白名单网站自动授权，其他网站可从浏览器菜单手动共享。') }}</p>
      </div>
      <button :id="`codex-allow-all-${tabId}`" type="button" class="setting-toggle" role="switch"
        :aria-label="b.t(CODEX_ALLOW_ALL)" :aria-checked="b.settings.codexAccess.allowAll"
        :aria-describedby="`codex-allow-all-help-${tabId}`"
        @click="update({ kind: 'allowAll', enabled: !b.settings.codexAccess.allowAll })"><span /></button>
    </div>
  </section>

  <section v-for="[list, title] in CODEX_SITE_LISTS" :key="list" class="settings-group codex-site-list" :aria-label="b.t(title)">
    <h3>{{ b.t(title) }}</h3>
    <p :id="`codex-${list}-help-${tabId}`" class="codex-site-help">{{ b.t(list === 'allowlist'
      ? '自动授权 Codex 访问这些域名及其子域名。'
      : '始终禁止 Codex 访问这些域名及其子域名。黑名单优先于其他授权。') }}</p>
    <form class="setting-input-form codex-site-form" @submit.prevent="addSite(list)">
      <input v-model="drafts[list].input" type="text" placeholder="example.com" autocomplete="off" spellcheck="false"
        :aria-label="b.t('网站地址')" :aria-describedby="`codex-${list}-help-${tabId}`"
        :aria-invalid="!!drafts[list].error" @input="drafts[list].error = ''" />
      <button type="submit" class="setting-button" :disabled="!drafts[list].input.trim()">{{ b.t('添加') }}</button>
    </form>
    <p v-if="drafts[list].error" class="settings-feedback codex-site-error" role="alert">{{ b.t(drafts[list].error) }}</p>
    <ul v-if="b.settings.codexAccess[list].length" class="codex-site-rules">
      <li v-for="site in b.settings.codexAccess[list]" :key="site" class="setting-row">
        <span dir="ltr">{{ site }}</span>
        <button type="button" class="icon-button" :aria-label="b.t('移除 {title}', { title: site })"
          :title="b.t('移除 {title}', { title: site })" @click="update({ kind: 'site', list, site, add: false })">
          <Icon name="trash" :size="15" />
        </button>
      </li>
    </ul>
  </section>
</template>

<style scoped>
.codex-site-list { padding-bottom: 14px; }
.codex-site-help { color: var(--quiet); font-size: 13px; line-height: 1.55; margin: 4px 0 12px; }
.codex-site-form { width: min(100%, 380px); max-width: 100%; }
.codex-site-form button:disabled { opacity: .55; cursor: default; }
.codex-site-error { margin: 8px 0 0; }
.codex-site-rules { list-style: none; margin: 12px 0 0; padding: 0; }
.codex-site-rules .setting-row { flex-direction: row; align-items: center; min-height: 40px; padding: 4px 0; border-top: 1px solid var(--separator); }
.codex-site-rules span { font-size: 14px; overflow-wrap: anywhere; }
.codex-site-rules button { flex: none; }
</style>
