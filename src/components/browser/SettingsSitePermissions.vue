<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from "vue";
import { permissionBehavior, permissionOrigin, permissions, type PermissionDefault, type PermissionKind, type PermissionScope } from "../../lib/permissions";
import { matchesSettingsSearch, settingsSearchWords } from "../../lib/settings-search";
import { useBrowser } from "../../stores/browser";
import Icon from "../common/Icon.vue";
import SearchHighlight from "../common/SearchHighlight.vue";

const props = withDefaults(defineProps<{
  tabId: string;
  kind?: PermissionKind;
  kinds?: readonly PermissionKind[];
  words?: readonly string[];
  initialFilter?: string;
}>(), { kinds: () => [], words: () => [], initialFilter: "" });
const emit = defineEmits<{ open: [kind: PermissionKind]; back: [] }>();
const more = defineModel<boolean>("more", { default: false });
const b = useBrowser();
const overviewGroups = computed(() => [
  props.kinds.filter((kind) => kind !== "screen"),
  props.kinds.filter((kind) => kind === "screen"),
]);
const defaultActions = ["ask", "block"] as const;
const filter = ref(props.initialFilter);
const filterWords = computed(() => settingsSearchWords(filter.value));
const ruleGroups = computed(() => {
  const rules = b.permissionRules.filter(([scope]) => scope.kind === props.kind);
  return [false, true].map((allowed) => {
    const group = rules.filter(([, ruleAllowed]) => ruleAllowed === allowed);
    return {
      allowed,
      title: b.t(allowed ? "允许的网站" : "阻止的网站"),
      filtered: group.filter(([scope]) => matchesSettingsSearch(filterWords.value, scope.requester, scope.page)),
      hasRules: group.length > 0,
    };
  });
});

function changeDefault(kind: PermissionKind, choice: PermissionDefault) {
  b.send({
    type: "settingsUpdate",
    settings: { ...b.settings, permissionDefaults: { ...b.settings.permissionDefaults, [kind]: choice } },
  });
}
function changeRule(scope: PermissionScope, event: Event) {
  const choice = (event.target as HTMLSelectElement).value;
  b.send({ type: "permissionRule", scope, allowed: choice === "default" ? null : choice === "allow" });
}

const dialog = ref<HTMLDialogElement>();
const site = ref("");
const siteError = ref("");
const addAllowed = ref(false);
const addTitle = computed(() => b.t(addAllowed.value ? "添加允许的网站" : "添加阻止的网站"));

function openAdd(allowed: boolean) {
  addAllowed.value = allowed;
  site.value = "";
  siteError.value = "";
  dialog.value?.showModal();
}
function addSite(kind: PermissionKind) {
  const origin = permissionOrigin(site.value);
  if (!origin) {
    siteError.value = b.t("请输入有效的 HTTP 或 HTTPS 网站地址。");
    return;
  }
  if (kind === "screen" && !origin.startsWith("https://")) {
    siteError.value = b.t("屏幕共享的例外需要使用 HTTPS 地址。");
    return;
  }
  const scope = { kind, page: origin, requester: origin };
  b.send({ type: "permissionRule", scope, allowed: addAllowed.value });
  if (!matchesSettingsSearch(filterWords.value, origin)) filter.value = "";
  dialog.value?.close();
}
onBeforeUnmount(() => dialog.value?.close());
</script>

<template>
  <section v-if="!kind" class="settings-group permission-overview" :aria-label="b.t('网站权限')">
    <h3 :class="{ 'settings-result-heading': words.length }">
      <span v-if="words.length" class="settings-result-pane">
        <SearchHighlight :text="b.t('隐私')" :words="words" />
        <span class="settings-result-separator" aria-hidden="true">›</span>
      </span>
      <SearchHighlight :text="b.t('网站权限')" :words="words" />
    </h3>
    <template v-for="(group, index) in overviewGroups" :key="index">
      <template v-if="group.length">
        <button v-if="index && !words.length" type="button" class="permission-disclosure"
          :aria-expanded="more" :aria-controls="`more-permissions-${tabId}`" @click="more = !more">
          {{ b.t('更多权限') }}
          <Icon :name="more ? 'chevronUp' : 'chevronDown'" :size="16" />
        </button>
        <ul v-show="!index || more || words.length" class="permission-list" :id="index ? `more-permissions-${tabId}` : undefined">
          <li v-for="entry in group" :key="entry">
            <button type="button" :id="`permission-${entry}-${tabId}`" class="permission-entry"
              :aria-label="b.t(permissions[entry].name)" :aria-describedby="`permission-summary-${entry}-${tabId}`"
              @click="emit('open', entry)">
              <span class="permission-entry-icon"><Icon :name="permissions[entry].icon" :size="18" /></span>
              <span class="setting-description">
                <strong><SearchHighlight :text="b.t(permissions[entry].name)" :words="words" /></strong>
                <span :id="`permission-summary-${entry}-${tabId}`" class="permission-summary">
                  <SearchHighlight :text="b.t(permissionBehavior[entry][b.settings.permissionDefaults[entry]])" :words="words" />
                </span>
              </span>
              <Icon name="forward" :size="14" />
            </button>
          </li>
        </ul>
      </template>
    </template>
  </section>

  <section v-else class="settings-group permission-detail" :aria-labelledby="`permission-heading-${tabId}`">
    <header class="permission-detail-header">
      <div class="permission-detail-title">
        <button type="button" class="icon-button permission-back" :aria-label="b.t('返回网站权限')" @click="emit('back')">
          <Icon name="arrow" :size="18" />
        </button>
        <h3 :id="`permission-heading-${tabId}`" tabindex="-1">{{ b.t(permissions[kind].name) }}</h3>
      </div>
      <label class="settings-search permission-filter">
        <Icon name="filter" :size="15" />
        <input v-model="filter" type="search" :placeholder="b.t('筛选网站')" :aria-label="b.t('筛选网站')" @keydown.esc.stop="filter = ''" />
      </label>
    </header>

    <fieldset class="permission-defaults" :aria-describedby="`permission-default-help-${tabId}`">
      <legend>{{ b.t('默认行为') }}</legend>
      <p :id="`permission-default-help-${tabId}`" class="permission-help">{{ b.t('已保存的网站权限优先。') }}</p>
      <label v-for="choice in defaultActions" :key="choice" class="permission-choice">
        <input type="radio" :name="`permission-default-${tabId}`" :value="choice"
          :checked="b.settings.permissionDefaults[kind] === choice" @change="changeDefault(kind, choice)" />
        <Icon :name="choice === 'ask' ? permissions[kind].icon : 'blocked'" :size="17" />
        <span>{{ b.t(permissionBehavior[kind][choice]) }}</span>
      </label>
    </fieldset>

    <section class="permission-exceptions" :aria-labelledby="`permission-exceptions-${tabId}`">
      <h4 :id="`permission-exceptions-${tabId}`">{{ b.t('自定义行为') }}</h4>
      <div v-for="group in ruleGroups" :key="String(group.allowed)" class="permission-rule-group"
        role="group" :aria-label="group.title">
        <div class="permission-rule-heading">
          <h5>{{ group.title }}</h5>
          <button type="button" class="setting-button permission-add" @click="openAdd(group.allowed)">
            <Icon name="plus" :size="13" />{{ b.t('添加') }}
          </button>
        </div>
        <p v-if="!group.filtered.length" class="permission-rule-empty">
          {{ b.t(group.hasRules ? '没有匹配的网站。' : '尚未添加网站。') }}
        </p>
        <ul v-else class="permission-list permission-rule-list">
          <li v-for="([scope, allowed], index) in group.filtered" :key="JSON.stringify(scope)" class="permission-rule">
            <span class="permission-rule-icon"><Icon name="globe" :size="16" /></span>
            <div class="setting-description">
              <strong><bdi><SearchHighlight :text="scope.requester" :words="filterWords" /></bdi></strong>
              <p v-if="scope.page !== scope.requester" :id="`permission-embed-${tabId}-${group.allowed}-${index}`">
                {{ b.t('嵌入在') }} <bdi><SearchHighlight :text="scope.page" :words="filterWords" /></bdi>
              </p>
            </div>
            <select class="setting-select" :value="allowed ? 'allow' : 'block'"
              :aria-label="b.t('更改 {website} 的权限', { website: scope.requester })"
              :aria-describedby="scope.page !== scope.requester ? `permission-embed-${tabId}-${group.allowed}-${index}` : undefined"
              @change="changeRule(scope, $event)">
              <option value="allow">{{ b.t('允许') }}</option>
              <option value="block">{{ b.t('阻止') }}</option>
              <option value="default">{{ b.t('使用默认设置') }}</option>
            </select>
          </li>
        </ul>
      </div>
    </section>

    <dialog ref="dialog" class="settings-space-editor permission-dialog" :aria-label="addTitle" @keydown.esc.stop>
      <h2>{{ addTitle }}</h2>
      <form @submit.prevent="addSite(kind)">
        <label>
          {{ b.t('网站地址') }}
          <input v-model="site" type="text" inputmode="url" placeholder="https://example.com" maxlength="2048"
            autocomplete="off" spellcheck="false" autofocus :aria-invalid="siteError ? 'true' : undefined"
            :aria-describedby="siteError ? `permission-site-error-${tabId}` : undefined" @input="siteError = ''" />
        </label>
        <p v-if="siteError" :id="`permission-site-error-${tabId}`" class="permission-site-error" role="alert">{{ siteError }}</p>
        <footer>
          <button type="button" class="setting-button" @click="dialog?.close()">{{ b.t('取消') }}</button>
          <button type="submit" class="primary-button">{{ b.t('添加') }}</button>
        </footer>
      </form>
    </dialog>
  </section>
</template>
<style src="../../styles/settings-permissions.css"></style>
