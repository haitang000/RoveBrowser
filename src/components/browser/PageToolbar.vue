<script setup lang="ts">
import { computed } from "vue";
import { host } from "../../lib/address";
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
  : host(tab.value?.url || "") || (props.pane ? b.t("新标签页") : "Search or enter address"));

function act(type: "back" | "forward" | "reload" | "reader") {
  b.send({ type, id: props.pane?.id });
}
</script>

<template>
  <div :class="pane ? 'pane-header' : 'browser-toolbar'" :data-window-drag="pane ? '' : undefined">
    <nav class="browser-navigation" :aria-label="b.t('网页导航')" data-window-control>
      <button class="icon-button" :title="b.shortcutTitle(b.t('后退'), 'back')" :aria-label="b.t('后退')" @click="act('back')">
        <Icon name="back" :size="14" />
      </button>
      <button class="icon-button" :title="b.shortcutTitle(b.t('前进'), 'forward')" :aria-label="b.t('前进')" @click="act('forward')">
        <Icon name="forward" :size="14" />
      </button>
      <button class="icon-button" :title="b.shortcutTitle(b.t('刷新'), 'reload')" :aria-label="b.t('刷新')"
        :aria-busy="tab?.loading || undefined" @click="act('reload')">
        <Icon :name="tab?.loading ? 'spinner' : 'reload'" :class="{ 'reload-spinner': tab?.loading }" :size="14" />
      </button>
    </nav>
    <button class="address-field" :aria-label="b.t('搜索或输入地址')" data-window-control
      :title="pane ? address : b.shortcutTitle(b.t('搜索或输入地址'), 'address')" @click="b.openCommand('edit', pane?.id)">
      <span>{{ address }}</span>
      <Icon v-if="!pane" name="settings" :size="13" />
    </button>
    <div class="toolbar-actions" data-window-control>
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
