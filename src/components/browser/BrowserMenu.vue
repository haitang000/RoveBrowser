<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useBrowser } from "../../stores/browser";
import { commandContext } from "../../lib/command";
import Icon from "../common/Icon.vue";
import SpaceGlyph from "../common/SpaceGlyph.vue";

const props = defineProps<{ dropTargetsOnly?: boolean; dropMenuOpen?: boolean }>();
const b = useBrowser();
const panel = ref<HTMLElement>();
const viewport = ref({ width: innerWidth, height: innerHeight });
const open = computed(() => props.dropTargetsOnly ? props.dropMenuOpen : b.browserMenuOpen);
const partner = computed(() => commandContext(b.state, b.recent).splitPartner);
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
function selectSpace(id: string) {
  close(true);
  if (id === b.state.activeSpace) b.toggleSpaceEditor(id);
  else b.send({ type: "space", id });
}
function allowDrop(event: DragEvent) {
  if (!event.dataTransfer?.types.includes("text/era-tab")) return;
  event.preventDefault();
  event.dataTransfer.dropEffect = "move";
}
function moveTab(event: DragEvent, spaceId: string) {
  const id = event.dataTransfer?.getData("text/era-tab");
  if (!id) return;
  event.preventDefault();
  b.send({ type: "move", id, spaceId });
  b.dragging = false;
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
        aria-label="浏览器菜单" :style="position" @keydown="key" @wheel.stop>
        <div class="browser-menu-scroll">
          <template v-if="!dropTargetsOnly">
            <div role="group" aria-label="新建">
              <button role="menuitem" tabindex="-1" @click="b.openCommand()">
                <Icon name="plus" :size="16" /><span>新建标签页</span><kbd v-if="b.shortcutLabel('new')" aria-hidden="true">{{ b.shortcutLabel('new') }}</kbd>
              </button>
              <button role="menuitem" tabindex="-1" :disabled="b.state.spaces.length >= 16"
                :title="b.state.spaces.length >= 16 ? '最多创建 16 个工作区' : undefined" @click="b.openSpaceCreation()">
                <Icon name="spaces" :size="16" /><span>新建 Space</span>
              </button>
              <button role="menuitem" tabindex="-1" :disabled="!partner || Boolean(b.spaceCreation)" @click="split">
                <Icon name="split" :size="16" /><span>新建分屏</span>
              </button>
            </div>
            <hr role="separator" />
            <div role="group" aria-label="工具">
              <button role="menuitem" tabindex="-1" aria-label="下载" aria-haspopup="dialog" aria-keyshortcuts="Control+J"
                :aria-description="b.activeDownloads ? `${b.activeDownloads} 个进行中` : undefined" @click="b.toggleDownloads(true)">
                <Icon name="download" :size="16" /><span>下载</span>
                <small v-if="b.activeDownloads">{{ b.activeDownloads }} 个进行中</small><kbd>Ctrl+J</kbd>
              </button>
              <button role="menuitem" tabindex="-1" @click="b.openSettings()">
                <Icon name="settings" :size="16" /><span>设置</span><kbd v-if="b.shortcutLabel('settings')" aria-hidden="true">{{ b.shortcutLabel('settings') }}</kbd>
              </button>
            </div>
            <hr role="separator" />
          </template>
          <div role="group" :aria-labelledby="dropTargetsOnly ? 'browser-menu-move-label' : 'browser-menu-spaces-label'">
            <div :id="dropTargetsOnly ? 'browser-menu-move-label' : 'browser-menu-spaces-label'" class="browser-menu-label">{{ dropTargetsOnly ? '移动到工作区' : '工作区' }}</div>
            <button v-for="(space, index) in b.state.spaces" :key="space.id" role="menuitem" tabindex="-1"
              :aria-label="`工作区 ${space.name}`" :aria-current="space.id === b.state.activeSpace ? 'true' : undefined"
              :title="space.id === b.state.activeSpace && !dropTargetsOnly ? `编辑 ${space.name}` : space.name"
              @click="!dropTargetsOnly && selectSpace(space.id)" @dragover="allowDrop" @drop="moveTab($event, space.id)">
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
