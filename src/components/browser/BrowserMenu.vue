<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useBrowser } from "../../stores/browser";
import { commandContext } from "../../lib/command";
import { chromeStoreDetailId } from "../../lib/extensions";
import Icon from "../common/Icon.vue";
import SpaceGlyph from "../common/SpaceGlyph.vue";

const props = defineProps<{ dropTargetsOnly?: boolean; dropMenuOpen?: boolean }>();
const b = useBrowser();
const panel = ref<HTMLElement>();
const viewport = ref({ width: innerWidth, height: innerHeight });
const open = computed(() => props.dropTargetsOnly ? props.dropMenuOpen : b.browserMenuOpen);
const partner = computed(() => commandContext(b.state, b.recent).splitPartner);
const shareableTab = computed(() => b.active && /^https?:\/\//i.test(b.active.url) && !b.active.loading ? b.active : undefined);
const position = computed(() => {
  const anchor = b.browserMenuAnchor;
  const width = Math.min(256, viewport.value.width - 24);
  const bottom = Math.max(24, viewport.value.height - (anchor?.y ?? viewport.value.height - 42) + 10);
  return {
    bottom: `${bottom}px`, width: `${width}px`,
    "--menu-max-height": `${Math.max(0, viewport.value.height - bottom - 16)}px`,
    "--bubble-tip": `${Math.max(16, (anchor ? anchor.x + anchor.width / 2 : 26) - 12)}px`,
  };
});
function resize() { viewport.value = { width: innerWidth, height: innerHeight }; }
function close(restoreFocus = false) {
  b.browserMenuOpen = false;
  if (restoreFocus) document.querySelector<HTMLElement>(".browser-menu-trigger")?.focus({ preventScroll: true });
}
function outside(event: PointerEvent) {
  if (!b.browserMenuOpen || !(event.target instanceof Element)) return;
  if (!panel.value?.contains(event.target) && !event.target.closest(".browser-menu-trigger")) close();
}
function items() {
  return Array.from(panel.value?.querySelectorAll<HTMLButtonElement>("[role=menuitem]:not(:disabled)") ?? []);
}
function key(event: KeyboardEvent) {
  if (event.isComposing || !b.browserMenuOpen) return;
  if (event.key === "Escape") {
    event.preventDefault();
    event.stopPropagation();
    close(true);
  } else if (event.key === "Tab") {
    close(true);
  } else if (["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) {
    event.preventDefault();
    const enabled = items();
    const current = enabled.findIndex((item) => item === document.activeElement);
    const next = event.key === "Home" ? 0 : event.key === "End" ? enabled.length - 1
      : event.key === "ArrowDown" ? (current + 1) % enabled.length
      : (current <= 0 ? enabled.length - 1 : current - 1);
    enabled[next]?.focus();
  }
}
function split() {
  const other = partner.value;
  close();
  if (b.active && other) b.send({ type: "split", id: b.active.id, other: other.id });
}
function installStoreExtension() {
  const tab = b.active;
  close();
  if (tab) b.send({ type: "extensionsChromeStoreTab", id: tab.id });
}
function shareWithCodex() {
  close(true);
  if (b.codexSharedTab) b.send({ type: "codexStopSharing" });
  else if (shareableTab.value) b.send({ type: "codexShareTab", id: shareableTab.value.id, url: shareableTab.value.url });
}
function copyCodexConfig() {
  close(true);
  b.send({ type: "codexCopyConfig" });
}
function readAloud(stop = false) {
  close(true);
  if (stop) b.stopReadAloud();
  else b.toggleReadAloud();
}
function selectSpace(id: string) {
  close(true);
  if (id === b.state.activeSpace) b.toggleSpaceEditor(id);
  else b.send({ type: "space", id });
}
watch(open, async (visible) => {
  if (!visible || props.dropTargetsOnly) return;
  await nextTick();
  if (open.value) items()[0]?.focus({ preventScroll: true });
});
onMounted(() => {
  window.addEventListener("resize", resize);
  if (!props.dropTargetsOnly) document.addEventListener("pointerdown", outside, true);
});
onBeforeUnmount(() => {
  window.removeEventListener("resize", resize);
  document.removeEventListener("pointerdown", outside, true);
});
</script>

<template>
  <Transition name="bubble">
    <div v-if="open" class="browser-menu-layer" :class="{ 'browser-menu-drop-layer': dropTargetsOnly }">
      <section ref="panel" :id="dropTargetsOnly ? 'browser-menu-drop-targets' : 'browser-menu'" class="browser-menu anchored-bubble bubble-surface" role="menu"
        :aria-label="b.t('浏览器菜单')" :style="position" @keydown="key" @wheel.stop>
        <div class="browser-menu-scroll">
          <template v-if="!dropTargetsOnly">
            <div role="group" :aria-label="b.t('新建')">
              <button role="menuitem" tabindex="-1" @click="b.openCommand()">
                <Icon name="plus" :size="16" /><span>{{ b.t('新建标签页') }}</span><kbd v-if="b.shortcutLabel('new')" aria-hidden="true">{{ b.shortcutLabel('new') }}</kbd>
              </button>
              <button role="menuitem" tabindex="-1" :disabled="b.state.spaces.length >= 16"
                :title="b.state.spaces.length >= 16 ? b.t('最多创建 16 个工作区') : undefined" @click="b.openSpaceCreation()">
                <Icon name="spaces" :size="16" /><span>{{ b.t('新建 Space') }}</span>
              </button>
              <button role="menuitem" tabindex="-1" :disabled="!partner || Boolean(b.spaceCreation)" @click="split">
                <Icon name="split" :size="16" /><span>{{ b.t('新建分屏') }}</span>
              </button>
            </div>
            <hr role="separator" />
            <div role="group" :aria-label="b.t('网页')">
              <button v-if="b.native && b.settings.readAloudEnabled" role="menuitem" tabindex="-1" :disabled="!b.readAloud && !b.canReadAloud()" @click="readAloud()">
                <Icon :name="b.readAloud?.phase === 'speaking' ? 'pause' : b.readAloud?.phase === 'paused' ? 'play' : 'speaker'" :size="16" />
                <span>{{ b.readAloudLabel() }}</span><kbd v-if="b.shortcutLabel('readAloud')" aria-hidden="true">{{ b.shortcutLabel('readAloud') }}</kbd>
              </button>
              <button v-if="b.readAloud && b.readAloud.phase !== 'loading'" role="menuitem" tabindex="-1" @click="readAloud(true)">
                <Icon name="stop" :size="16" /><span>{{ b.t('停止朗读') }}</span>
              </button>
              <button role="menuitem" tabindex="-1" :disabled="!b.canOutputPage('savePageAs')"
                aria-haspopup="dialog" :aria-keyshortcuts="b.pageOutputShortcut('savePageAs') ? 'Control+S' : undefined"
                @click="b.outputPage('savePageAs')">
                <Icon name="savePage" :size="16" /><span>{{ b.t('另存网页…') }}</span><kbd v-if="b.pageOutputShortcut('savePageAs')" aria-hidden="true">{{ b.pageOutputShortcut('savePageAs') }}</kbd>
              </button>
              <button role="menuitem" tabindex="-1" :disabled="!b.canOutputPage('printPage')"
                aria-haspopup="dialog" :aria-keyshortcuts="b.pageOutputShortcut('printPage') ? 'Control+P' : undefined"
                @click="b.outputPage('printPage')">
                <Icon name="print" :size="16" /><span>{{ b.t('打印…') }}</span><kbd v-if="b.pageOutputShortcut('printPage')" aria-hidden="true">{{ b.pageOutputShortcut('printPage') }}</kbd>
              </button>
            </div>
            <hr role="separator" />
            <div role="group" :aria-label="b.t('工具')">
              <button v-if="b.native && (shareableTab || b.codexSharedTab)" role="menuitem" tabindex="-1"
                :title="b.codexSharedTab ? undefined : b.t('Codex 可读取此网页并点击或填写页面元素')" @click="shareWithCodex">
                <Icon :name="b.codexSharedTab ? 'stop' : 'extension'" :size="16" />
                <span>{{ b.t(b.codexSharedTab ? '停止与 Codex 共享' : '允许 Codex 使用此标签页') }}</span>
              </button>
              <button v-if="b.native" role="menuitem" tabindex="-1" @click="copyCodexConfig">
                <Icon name="copy" :size="16" /><span>{{ b.t('复制 Codex MCP 配置') }}</span>
              </button>
              <button v-if="b.native && chromeStoreDetailId(b.active?.url)" role="menuitem" tabindex="-1" @click="installStoreExtension">
                <Icon name="extension" :size="16" /><span>{{ b.t('安装此商店扩展…') }}</span>
              </button>
              <button role="menuitem" tabindex="-1" :aria-keyshortcuts="b.browserShortcutLabel(72) ? 'Control+H' : undefined" @click="b.openHistory()">
                <Icon name="history" :size="16" /><span>{{ b.t('历史记录') }}</span><kbd v-if="b.browserShortcutLabel(72)" aria-hidden="true">Ctrl+H</kbd>
              </button>
              <button role="menuitem" tabindex="-1" :aria-label="b.t('下载')" aria-haspopup="dialog" aria-keyshortcuts="Control+J"
                :aria-description="b.activeDownloads ? b.t('{count} 个进行中', { count: b.activeDownloads }) : undefined" @click="b.toggleDownloads(true)">
                <Icon name="download" :size="16" /><span>{{ b.t('下载') }}</span>
                <small v-if="b.activeDownloads">{{ b.t('{count} 个进行中', { count: b.activeDownloads }) }}</small><kbd>Ctrl+J</kbd>
              </button>
              <button role="menuitem" tabindex="-1" @click="b.openSettings()">
                <Icon name="settings" :size="16" /><span>{{ b.t('设置') }}</span><kbd v-if="b.shortcutLabel('settings')" aria-hidden="true">{{ b.shortcutLabel('settings') }}</kbd>
              </button>
            </div>
            <hr role="separator" />
          </template>
          <div role="group" :aria-labelledby="dropTargetsOnly ? 'browser-menu-move-label' : 'browser-menu-spaces-label'">
            <div :id="dropTargetsOnly ? 'browser-menu-move-label' : 'browser-menu-spaces-label'" class="browser-menu-label">{{ dropTargetsOnly ? b.t('移动到工作区') : b.t('工作区') }}</div>
            <button v-for="(space, index) in b.state.spaces" :key="space.id" role="menuitem" tabindex="-1"
              :aria-label="b.t('工作区 {name}', { name: space.name })" :aria-current="space.id === b.state.activeSpace ? 'true' : undefined"
              :title="space.id === b.state.activeSpace && !dropTargetsOnly ? b.t('编辑 {name}', { name: space.name }) : space.name"
              :data-tab-drop-space="dropTargetsOnly ? space.id : undefined"
              @click="!dropTargetsOnly && selectSpace(space.id)">
              <SpaceGlyph v-if="space.icon" :icon="space.icon" :size="16" /><Icon v-else name="spaces" :size="16" />
              <span>{{ space.name }}</span>
              <kbd v-if="!dropTargetsOnly && index < 9">Ctrl+{{ index + 1 }}</kbd>
              <Icon v-if="space.id === b.state.activeSpace" name="check" :size="14" />
            </button>
          </div>
        </div>
      </section>
    </div>
  </Transition>
</template>

<style>
.browser-menu-trigger { position: relative; }
.browser-menu-trigger.active { background: var(--selected); }
.browser-menu-layer {
  position: fixed;
  inset: 0;
  z-index: 130;
  pointer-events: none;
}
html[data-popup="true"] .browser-menu-layer:not(.bubble-leave-active) { pointer-events: auto; }
.browser-menu {
  position: absolute;
  left: 12px;
  padding: 0;
  pointer-events: auto;
  user-select: none;
}
.browser-menu-drop-layer .browser-menu { max-width: calc(var(--sidebar-width) - 24px); }
.bubble-leave-active .browser-menu { pointer-events: none; }
.browser-menu-scroll {
  max-height: var(--menu-max-height);
  overflow: auto;
  overscroll-behavior: contain;
  padding: 6px;
  border-radius: inherit;
}
.browser-menu button {
  display: flex;
  align-items: center;
  gap: 9px;
  width: 100%;
  height: 31px;
  padding: 0 8px;
  border-radius: 7px;
  text-align: left;
  font-size: 13px;
}
.browser-menu button:hover:not(:disabled),
.browser-menu button:focus-visible { background: var(--chosen); }
.browser-menu button:focus-visible { outline: 1px solid var(--accent); outline-offset: -1px; }
.browser-menu button:disabled { opacity: .45; cursor: default; }
.browser-menu button > span:not(.space-emoji) {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.browser-menu button > :is(svg, kbd, small, .space-emoji) { flex-shrink: 0; }
.browser-menu button kbd, .browser-menu button small, .browser-menu-label {
  color: var(--quiet);
  font-size: 11px;
  white-space: nowrap;
}
.browser-menu button kbd { font-family: inherit; font-variant-numeric: tabular-nums; }
.browser-menu button[aria-current] { color: var(--ink); font-weight: 600; }
.browser-menu button[aria-current] > svg:last-child { color: var(--accent); }
.browser-menu-label { padding: 4px 8px 2px; }
.browser-menu hr { margin: 4px 3px; border: 0; border-top: 1px solid var(--edge); }
</style>
