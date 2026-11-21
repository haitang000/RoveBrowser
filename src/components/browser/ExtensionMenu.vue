<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { CHROME_STORE_URL, type ExtensionItem } from "../../lib/extensions";
import { useBrowser } from "../../stores/browser";
import Icon from "../common/Icon.vue";
import ExtensionIcon from "./ExtensionIcon.vue";

const b = useBrowser();
const selected = ref<ExtensionItem>();
const panel = ref<HTMLElement>();
const content = ref<HTMLElement>();
const viewport = ref({ width: innerWidth, height: innerHeight });
const position = computed(() => {
  const anchor = b.extensionMenuAnchor;
  const margin = 10;
  const pageSize = b.extensionView?.size;
  const showingPage = selected.value && b.extensionView?.phase === "ready";
  const panelWidth = showingPage ? Math.max(176, (pageSize?.[0] ?? 458) + 2) : 344;
  const panelHeight = showingPage ? Math.max((pageSize?.[1] ?? 470) + 50, 86) : 144;
  const width = Math.max(0, Math.min(panelWidth, viewport.value.width - margin * 2));
  const top = Math.max(32, Math.min((anchor?.y ?? 24) + (anchor?.height ?? 0) + 8, viewport.value.height / 3));
  const left = Math.max(margin, Math.min(
    (anchor?.x ?? width) + (anchor?.width ?? 0) - width,
    viewport.value.width - width - margin,
  ));
  return {
    width: width + "px",
    left: left + "px",
    top: top + "px",
    height: selected.value ? panelHeight + "px" : undefined,
    maxHeight: Math.max(0, viewport.value.height - top - margin) + "px",
  };
});

function bounds() {
  const r = content.value?.getBoundingClientRect();
  return r && r.width > 0 && r.height > 0
    ? { x: r.x, y: r.y, width: r.width, height: r.height }
    : undefined;
}

async function open(item: ExtensionItem) {
  if (!item.enabled || !item.hasPage || selected.value) return;
  b.extensionView = undefined;
  selected.value = item;
  await nextTick();
  const rect = bounds();
  if (rect && b.extensionMenuAnchor && selected.value === item) {
    b.send({ type: "extensionViewOpen", id: item.id, rect });
  } else if (selected.value === item) {
    selected.value = undefined;
  }
}

function resize() {
  viewport.value = { width: innerWidth, height: innerHeight };
}

watch(position, () => {
  const rect = bounds();
  if (rect && b.extensionMenuAnchor && b.extensionView?.phase === "ready") {
    b.send({ type: "extensionViewBounds", rect });
  }
}, { flush: "post" });

function close() {
  b.extensionMenuAnchor = undefined;
}

async function back() {
  b.send({ type: "extensionViewClose" });
  selected.value = undefined;
  b.extensionView = undefined;
  await nextTick();
  panel.value?.focus();
}

function unavailable(item: ExtensionItem) {
  if (!item.enabled) return b.t("已停用");
  return item.hasPage ? "" : b.t("此扩展没有可打开的页面");
}

function tileLabel(item: ExtensionItem) {
  const reason = unavailable(item);
  return reason ? item.name + " · " + reason : item.name;
}

onMounted(() => {
  b.send({ type: "extensionsList" });
  window.addEventListener("resize", resize);
});
onBeforeUnmount(() => window.removeEventListener("resize", resize));
</script>

<template>
  <div class="extension-menu-layer" @click.self="close" @keydown.esc.stop.prevent="close">
    <section ref="panel" class="extension-menu-panel" :style="position"
      role="dialog" :aria-label="selected?.name ?? b.t('扩展')" tabindex="-1">
      <header class="extension-menu-header">
        <button v-if="selected" type="button" class="icon-button" :aria-label="b.t('返回')" @click="back">
          <Icon name="back" :size="15" />
        </button>
        <h2>{{ selected?.name ?? b.t('扩展') }}</h2>
        <button v-if="!selected" type="button" class="extension-manage" @click="close(); b.openSettings('extensions')">
          {{ b.t('管理') }}
        </button>
        <button v-else type="button" class="icon-button" :aria-label="b.t('关闭')" @click="close">
          <Icon name="close" :size="15" />
        </button>
      </header>
      <div v-if="selected" ref="content" class="extension-menu-content">
        <p v-if="b.extensionView?.phase === 'error'" role="alert">
          {{ b.t('无法打开扩展页面') }}<small v-if="b.extensionView.message">{{ b.extensionView.message }}</small>
        </p>
        <p v-else-if="b.extensionView?.phase !== 'ready'" role="status">{{ b.t('正在打开扩展…') }}</p>
      </div>
      <div v-else class="extension-menu-list">
        <p v-if="!b.extensions.items && b.extensions.error" role="alert">{{ b.extensions.error }}</p>
        <p v-else-if="!b.extensions.items" role="status">{{ b.t('正在处理扩展…') }}</p>
        <p v-else-if="!b.extensions.items.length">{{ b.t('还没有安装扩展。') }}</p>
        <div class="extension-menu-grid">
          <button v-for="item in b.extensions.items" :key="item.id" type="button" class="extension-menu-tile"
            :aria-disabled="!item.enabled || !item.hasPage" :aria-label="tileLabel(item)"
            :title="tileLabel(item)" @click="open(item)">
            <ExtensionIcon :src="item.icon" />
            <span>{{ item.name }}</span>
          </button>
          <button type="button" class="extension-menu-tile" :aria-label="b.t('浏览商店')" :title="b.t('浏览商店')"
            @click="close(); b.send({ type: 'new', url: CHROME_STORE_URL })">
            <Icon name="plus" :size="23" />
            <span>{{ b.t('浏览商店') }}</span>
          </button>
        </div>
      </div>
    </section>
  </div>
</template>

<style scoped>
.extension-menu-layer { position: fixed; inset: 0; z-index: 85; }
.extension-menu-panel {
  position: absolute;
  display: flex;
  flex-direction: column;
  min-height: 0;
  overflow: hidden;
  border: 1px solid var(--panel-edge);
  border-radius: var(--radius-panel);
  background: var(--panel);
  color: var(--ink);
  box-shadow: 0 8px 30px var(--panel-shadow);
  outline: none;
}
.extension-menu-header {
  display: flex;
  flex: none;
  align-items: center;
  gap: 8px;
  min-height: 42px;
  padding: 5px 10px 5px 14px;
}
.extension-menu-header h2 {
  flex: 1;
  min-width: 0;
  margin: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 15px;
  font-weight: 650;
}
.extension-manage { padding: 5px 7px; border-radius: 6px; color: var(--accent); font-size: 12px; }
.extension-manage:hover { background: var(--chosen); }
.extension-menu-list { min-height: 0; overflow: auto; padding: 2px 12px 12px; }
.extension-menu-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(76px, 1fr)); gap: 8px; }
.extension-menu-tile {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  min-width: 0;
  min-height: 64px;
  gap: 4px;
  padding: 6px;
  border: 1px solid var(--panel-edge);
  border-radius: 8px;
  background: color-mix(in srgb, var(--ink) 4%, var(--panel));
}
.extension-menu-tile:hover { background: var(--chosen); }
.extension-menu-tile[aria-disabled="true"] { color: var(--quiet); opacity: .62; cursor: default; }
.extension-menu-tile > span:last-child { max-width: 100%; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; font-size: 11px; }
.extension-menu-content {
  display: grid;
  flex: 1;
  place-items: center;
  min-height: 0;
  margin: 0 0 5px;
  border-top: 1px solid var(--panel-edge);
  overflow: auto;
}
.extension-menu-list p, .extension-menu-content p { margin: 12px; color: var(--quiet); font-size: 13px; overflow-wrap: anywhere; }
.extension-menu-content small { display: block; margin-top: 4px; }
</style>
