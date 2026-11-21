<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { CHROME_STORE_URL, MAX_PINNED_EXTENSIONS, type ExtensionItem } from "../../lib/extensions";
import { useBrowser } from "../../stores/browser";
import Icon from "../common/Icon.vue";
import ExtensionIcon from "./ExtensionIcon.vue";

const b = useBrowser();
const selected = ref<ExtensionItem>();
const bubble = ref<HTMLElement>();
const panel = ref<HTMLElement>();
const content = ref<HTMLElement>();
const openMenu = computed(() => Boolean(b.extensionMenuAnchor && !b.extensions.popup && !b.quitOpen));
const anchor = ref(b.extensionMenuAnchor);
const viewport = ref({ width: innerWidth, height: innerHeight });
const reducedMotion = matchMedia("(prefers-reduced-motion: reduce)");
const entering = ref(false);
const revealingPage = ref(false);
let pagePresented = false;
let pageAnimation: Animation | undefined;
const panelRadius = 20;
const position = computed(() => {
  const trigger = anchor.value;
  const margin = 10;
  const pageSize = b.extensionView?.size;
  const showingPage = selected.value && b.extensionView?.phase === "ready";
  const panelWidth = showingPage ? Math.max(176, (pageSize?.[0] ?? 458) + 2) : 344;
  const panelHeight = showingPage ? Math.max((pageSize?.[1] ?? 470) + 50, 86) : 144;
  const width = Math.max(0, Math.min(panelWidth, viewport.value.width - margin * 2));
  const top = Math.max(32, Math.min((trigger?.y ?? 24) + (trigger?.height ?? 0) + 8, viewport.value.height / 3));
  const triggerCenter = trigger ? trigger.x + trigger.width / 2 : width / 2;
  const left = Math.max(margin, Math.min(triggerCenter - width + 30, viewport.value.width - width - margin));
  const tip = Math.max(22, Math.min(triggerCenter - left, width - 22));
  return {
    width: width + "px",
    left: left + "px",
    top: top + "px",
    height: selected.value ? panelHeight + "px" : undefined,
    maxHeight: Math.max(0, viewport.value.height - top - margin) + "px",
    "--bubble-tip": tip + "px",
    "--extension-bubble-radius": panelRadius + "px",
  };
});

function bounds() {
  const card = bubble.value;
  const shell = panel.value;
  const slot = content.value;
  if (!card || !shell || !slot || !slot.clientWidth || !slot.clientHeight) return;
  // A CSS transform only moves the painted bubble. The child WebView uses its layout bounds.
  return {
    x: card.offsetLeft + shell.clientLeft + slot.offsetLeft,
    y: card.offsetTop + shell.clientTop + slot.offsetTop + slot.clientTop,
    width: slot.clientWidth,
    height: slot.clientHeight,
    corners: [0, 0, panelRadius - 1, panelRadius - 1],
  };
}

async function open(item: ExtensionItem) {
  if (!openMenu.value || !item.enabled || !item.hasPage || selected.value) return;
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

function syncPageBounds() {
  if (!openMenu.value || entering.value || revealingPage.value || b.extensionView?.phase !== "ready") return;
  const rect = bounds();
  if (rect) b.send({ type: "extensionViewBounds", rect });
}

function finishEnter() {
  entering.value = false;
  animatePage();
}

function animatePage() {
  const card = bubble.value;
  if (!openMenu.value || entering.value || pagePresented || b.extensionView?.phase !== "ready" || !card) return;
  pagePresented = true;
  if (b.settings.reduceMotion || reducedMotion.matches) {
    syncPageBounds();
    return;
  }
  const animation = card.animate(
    [{ transform: "translateY(-3px) scale(.97)" }, { transform: "none" }],
    { duration: 180, easing: "cubic-bezier(.16, 1.6, .3, 1)" },
  );
  revealingPage.value = true;
  pageAnimation = animation;
  animation.onfinish = () => {
    if (pageAnimation !== animation) return;
    pageAnimation = undefined;
    revealingPage.value = false;
    syncPageBounds();
  };
}

function cancelPageAnimation() {
  pagePresented = false;
  revealingPage.value = false;
  pageAnimation?.cancel();
  pageAnimation = undefined;
}

function beginLeave(layer: Element) {
  (layer as HTMLElement).inert = true;
  entering.value = false;
  cancelPageAnimation();
}

watch(position, () => {
  if (pagePresented) syncPageBounds();
  else animatePage();
}, { flush: "post" });

function finishPageAnimation() {
  if (b.settings.reduceMotion || reducedMotion.matches) pageAnimation?.finish();
}
watch(() => b.settings.reduceMotion, finishPageAnimation);

watch(() => openMenu.value ? b.extensionMenuAnchor : undefined, (request, previous) => {
  if (!request) return;
  anchor.value = request;
  if (previous && previous.tabId === request.tabId && previous.extensionId === request.extensionId) return;
  cancelPageAnimation();
  selected.value = undefined;
  b.extensionView = undefined;
  if (request.extensionId) {
    const item = b.pinnedExtensions.find(item => item.id === request.extensionId);
    if (item) void open(item);
  } else {
    b.send({ type: "extensionsList" });
  }
}, { immediate: true });

function close() {
  b.extensionMenuAnchor = undefined;
}

async function back() {
  cancelPageAnimation();
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

const pinLimitReached = computed(() => (b.state.extensionPins?.length ?? 0) >= MAX_PINNED_EXTENSIONS);

function isPinned(item: ExtensionItem) {
  return b.state.extensionPins?.includes(item.id) ?? false;
}

function pinTitle(item: ExtensionItem) {
  if (isPinned(item)) return b.t("取消固定");
  return pinLimitReached.value ? b.t("最多固定 4 个扩展") : b.t("固定到工具栏");
}

onMounted(() => {
  window.addEventListener("resize", resize);
  reducedMotion.addEventListener("change", finishPageAnimation);
});
onBeforeUnmount(() => {
  cancelPageAnimation();
  window.removeEventListener("resize", resize);
  reducedMotion.removeEventListener("change", finishPageAnimation);
});
</script>

<template>
  <Transition name="bubble" @before-enter="entering = true" @after-enter="finishEnter" @before-leave="beginLeave">
  <div v-if="openMenu" class="extension-menu-layer" @click.self="close" @keydown.esc.stop.prevent="close">
    <div ref="bubble" class="extension-menu-bubble anchored-bubble bubble-below bubble-surface" :style="position">
    <section ref="panel" class="extension-menu-panel"
      role="dialog" :aria-label="selected?.name ?? b.t('扩展')" tabindex="-1">
      <header class="extension-menu-header">
        <button v-if="selected && !anchor?.extensionId" type="button" class="icon-button" :aria-label="b.t('返回')" @click="back">
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
        <p v-else-if="entering || revealingPage || b.extensionView?.phase !== 'ready'" role="status">{{ b.t('正在打开扩展…') }}</p>
      </div>
      <div v-else class="extension-menu-list">
        <p v-if="b.extensions.error" role="alert">{{ b.t(b.extensions.error) }}</p>
        <p v-else-if="!b.extensions.items" role="status">{{ b.t('正在处理扩展…') }}</p>
        <p v-else-if="!b.extensions.items.length">{{ b.t('还没有安装扩展。') }}</p>
        <div class="extension-menu-grid">
          <div v-for="item in b.extensions.items" :key="item.id" class="extension-menu-item">
            <button type="button" class="extension-menu-tile"
              :aria-disabled="!item.enabled || !item.hasPage" :aria-label="tileLabel(item)"
              :title="tileLabel(item)" @click="open(item)">
              <ExtensionIcon :src="item.icon" />
              <span>{{ item.name }}</span>
            </button>
            <button v-if="isPinned(item) || item.enabled" type="button" class="icon-button extension-pin"
              :aria-label="b.t('固定扩展 {name}', { name: item.name })" :aria-pressed="isPinned(item)"
              :title="pinTitle(item)" :disabled="b.extensions.busy || (!isPinned(item) && pinLimitReached)"
              @click="b.send({ type: 'extensionPin', id: item.id, pinned: !isPinned(item) })">
              <Icon name="pin" :size="12" />
            </button>
          </div>
          <button type="button" class="extension-menu-tile" :aria-label="b.t('浏览商店')" :title="b.t('浏览商店')"
            @click="close(); b.send({ type: 'new', url: CHROME_STORE_URL })">
            <Icon name="plus" :size="23" />
            <span>{{ b.t('浏览商店') }}</span>
          </button>
        </div>
      </div>
    </section>
    </div>
  </div>
  </Transition>
</template>

<style scoped>
.extension-menu-layer { position: fixed; inset: 0; z-index: 85; }
.extension-menu-layer.bubble-leave-active { z-index: 0; }
.extension-menu-bubble {
  position: absolute;
  display: flex;
  border-radius: var(--extension-bubble-radius);
}
.extension-menu-panel {
  position: relative;
  display: flex;
  flex: 1;
  min-width: 0;
  flex-direction: column;
  min-height: 0;
  overflow: hidden;
  border: 1px solid var(--panel-edge);
  border-radius: inherit;
  color: var(--ink);
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
.extension-menu-item { position: relative; min-width: 0; }
.extension-menu-item > .extension-menu-tile { width: 100%; }
.extension-menu-item:has(.extension-pin) > .extension-menu-tile { padding-top: 11px; }
.extension-pin {
  position: absolute;
  inset: 3px 3px auto auto;
  width: 20px;
  height: 20px;
  color: var(--quiet);
}
.extension-pin[aria-pressed="true"] { color: var(--accent); background: var(--chosen); }
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
  position: relative;
  display: grid;
  flex: 1;
  place-items: center;
  min-height: 0;
  border-top: 1px solid var(--panel-edge);
  overflow: auto;
}
.extension-menu-list p, .extension-menu-content p { margin: 12px; color: var(--quiet); font-size: 13px; overflow-wrap: anywhere; }
.extension-menu-content small { display: block; margin-top: 4px; }
</style>
