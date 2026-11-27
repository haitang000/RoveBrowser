<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { host, isHistoryUrl } from "../../lib/address";
import { isSettingsUrl } from "../../lib/settings";
import type { Tab } from "../../lib/types";
import { useBrowser } from "../../stores/browser";
import Icon from "../common/Icon.vue";

const props = defineProps<{ pane?: Tab }>();
const b = useBrowser();
const tab = computed(() => props.pane ?? b.active);
const id = computed(() => tab.value?.id);
const reading = computed(() => Boolean(id.value && b.reader.id === id.value));
const waiting = computed(() => Boolean(id.value && b.reader.waiting === id.value));
const canRead = computed(() => id.value && (reading.value || waiting.value || b.reader.available.includes(id.value)));
const address = computed(() => isSettingsUrl(tab.value?.url) ? b.t("设置")
  : isHistoryUrl(tab.value?.url) ? b.t("历史记录")
  : tab.value?.url.startsWith("chrome-extension:") ? tab.value.title || b.t("扩展")
  : host(tab.value?.url || "") || (props.pane ? b.t("新标签页") : "Search or enter address"));
const extensionsOpen = computed(() => Boolean(
  b.extensionMenuAnchor && !b.extensionMenuAnchor.extensionId && b.extensionMenuAnchor.tabId === id.value,
));
const extensionTrigger = ref<HTMLButtonElement>();
watch(() => b.pinnedExtensions.length, () => {
  if (extensionsOpen.value && extensionTrigger.value) b.placeExtensionMenu(extensionTrigger.value, id.value);
}, { flush: "post" });

function act(type: "back" | "forward" | "reload" | "reader") {
  b.send({ type, id: props.pane?.id });
}
</script>

<template>
  <div :class="pane ? 'pane-header' : 'browser-toolbar'" :data-window-drag="pane ? '' : undefined">
    <nav class="browser-navigation" :aria-label="b.t('网页导航')" data-window-control>
      <button class="icon-button" :title="b.shortcutTitle(b.t('后退'), 'back')" :aria-label="b.t('后退')" @click="act('back')">
        <Icon name="back" class="navigation-arrow navigation-back" :size="14" />
      </button>
      <button class="icon-button" :title="b.shortcutTitle(b.t('前进'), 'forward')" :aria-label="b.t('前进')" @click="act('forward')">
        <Icon name="forward" class="navigation-arrow navigation-forward" :size="14" />
      </button>
      <button class="icon-button" :title="b.shortcutTitle(b.t('刷新'), 'reload')" :aria-label="b.t('刷新')"
        :aria-busy="tab?.loading || undefined" @click="act('reload')">
        <span class="navigation-reload-icons" aria-hidden="true">
          <Icon name="reload" class="navigation-reload" :size="14" />
          <Icon name="spinner" class="reload-spinner" :size="14" />
        </span>
      </button>
    </nav>
    <button class="address-field" :aria-label="b.t('搜索或输入地址')" data-window-control
      :title="pane ? address : b.shortcutTitle(b.t('搜索或输入地址'), 'address')" @click="b.openCommand('edit', pane?.id)">
      <span :key="id" class="address-loading" :class="{ 'is-loading': tab?.loading }" aria-hidden="true" />
      <span class="address-label">{{ address }}</span>
      <Icon v-if="!pane" name="settings" :size="13" />
    </button>
    <div class="toolbar-actions" data-window-control>
      <button v-if="b.codexSharedTab && b.codexSharedTab === id" class="icon-button codex-sharing" type="button"
        :aria-label="b.t('停止与 Codex 共享')" :title="b.t('停止与 Codex 共享')"
        @click="b.send({ type: 'codexStopSharing' })">Codex</button>
      <button v-if="b.native && !b.onboardingOpen" ref="extensionTrigger" type="button" class="icon-button extension-menu-trigger"
        :class="{ chosen: extensionsOpen }" :aria-label="b.t('扩展')" :title="b.t('扩展')"
        aria-haspopup="dialog" :aria-expanded="extensionsOpen"
        @click="b.toggleExtensionMenu($event.currentTarget as HTMLElement, id)">
        <Icon name="extension" :size="15" />
      </button>
      <button v-if="canRead" class="icon-button reader-toggle" :class="{ chosen: reading }"
        :aria-label="reading ? b.t('退出阅读模式') : waiting ? b.t('取消阅读模式') : b.t('进入阅读模式')"
        :title="b.shortcutTitle(reading ? b.t('退出阅读模式') : b.t('阅读模式'), 'reader')"
        :aria-pressed="reading" :aria-busy="waiting || undefined" @click="act('reader')">
        <Icon :name="waiting ? 'spinner' : 'reader'" :size="14" />
      </button>
      <slot />
    </div>
  </div>
</template>

<style scoped>
.icon-button.codex-sharing {
  width: auto;
  padding-inline: 6px;
  background: var(--selected);
  font-size: 10px;
  font-weight: 600;
}
.icon-button.codex-sharing:hover { background: var(--hover); }
</style>
