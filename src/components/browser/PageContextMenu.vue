<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref } from "vue";
import type { ContextMenuAnchor, PageMenuItem, PageContextMenu } from "../../lib/context-menu";
import { useBrowser } from "../../stores/browser";
import Icon from "../common/Icon.vue";
import ContextMenuSurface from "./ContextMenuSurface.vue";

const props = defineProps<{ menu: PageContextMenu }>();
const b = useBrowser();
const scrim = ref<HTMLElement>();
type MenuLevel = { index: number; anchor: ContextMenuAnchor; items: PageMenuItem[]; label: string };
const submenus = ref<MenuLevel[]>([]);
let hoverTimer: ReturnType<typeof setTimeout>;

function parseLabel(label: string) {
  const mnemonic = /^(?:[^&]|&&)*?&([^&])/su.exec(label);
  const start = mnemonic ? mnemonic[0].length - mnemonic[1]!.length - 1 : label.length;
  return {
    before: label.slice(0, start).replaceAll("&&", "&"),
    key: mnemonic?.[1],
    after: label.slice(mnemonic ? start + mnemonic[1]!.length + 1 : start).replaceAll("&&", "&"),
  };
}
const levels = computed(() => [
  { index: -1, anchor: props.menu.anchor, items: props.menu.items, label: b.t("网页右键菜单") },
  ...submenus.value,
].map((level) => ({
  ...level,
  rows: level.items.map((item) => ({ item, label: "label" in item ? parseLabel(item.label) : null })),
})));

function close(command: number | null = null) {
  b.pageContextMenu = undefined;
  b.send({ type: "pageContextMenuResult", ticket: props.menu.ticket, command });
}
function buttons(level: number) {
  return Array.from(scrim.value?.querySelectorAll<HTMLButtonElement>(`[data-menu-level="${level}"] button:not(:disabled)`) ?? []);
}
function cancelHover() { clearTimeout(hoverTimer); }
onBeforeUnmount(cancelHover);

function trim(level: number) {
  cancelHover();
  if (submenus.value.length > level) submenus.value = submenus.value.slice(0, level);
}
function openSubmenu(level: number, index: number, item: PageMenuItem, row: HTMLElement, focus: boolean) {
  cancelHover();
  if (item.kind !== "submenu" || !item.enabled || !item.items.length) {
    trim(level);
    return;
  }
  if (submenus.value[level]?.index !== index) {
    const { left, right } = row.closest(".context-menu")!.getBoundingClientRect();
    const y = row.getBoundingClientRect().top - 6;
    const label = parseLabel(item.label);
    submenus.value = [...submenus.value.slice(0, level), {
      index, anchor: { x: right, y, side: { left, right } }, items: item.items,
      label: `${label.before}${label.key ?? ""}${label.after}`,
    }];
  }
  if (focus) nextTick(() => buttons(level + 1)[0]?.focus({ preventScroll: true }));
}
function hover(event: PointerEvent, level: number, index: number, item: PageMenuItem) {
  if (event.pointerType !== "mouse") return;
  cancelHover();
  const row = event.currentTarget as HTMLElement;
  hoverTimer = setTimeout(() => openSubmenu(level, index, item, row, false), 160);
}
function pick(event: MouseEvent, level: number, index: number, item: PageMenuItem) {
  if (item.kind === "command") close(item.id);
  else openSubmenu(level, index, item, event.currentTarget as HTMLElement, true);
}
function key(event: KeyboardEvent, level: number) {
  if (event.isComposing) return;
  const rows = buttons(level);
  const row = (event.target as HTMLElement).closest<HTMLButtonElement>("button");
  const active = row ? rows.indexOf(row) : -1;
  const index = Number(row?.dataset.menuRow);
  const item = levels.value[level]?.items[index];
  switch (event.key) {
    case "Tab":
      close();
      break;
    case "Escape":
    case "ArrowLeft":
      if (level) {
        const parentIndex = submenus.value[level - 1]?.index;
        trim(level - 1);
        buttons(level - 1).find((button) => button.dataset.menuRow === String(parentIndex))?.focus();
      } else if (event.key === "Escape") close();
      break;
    case "ArrowRight":
      if (row && item?.kind === "submenu") openSubmenu(level, index, item, row, true);
      break;
    case "ArrowDown":
    case "ArrowUp":
    case "Home":
    case "End": {
      trim(level);
      const next = event.key === "Home" ? 0 : event.key === "End" ? rows.length - 1
        : event.key === "ArrowDown" ? (active + 1) % rows.length
        : (active <= 0 ? rows.length - 1 : active - 1);
      rows[next]?.focus();
      break;
    }
    default: {
      if (event.key.length !== 1 || event.ctrlKey || event.metaKey) return;
      const matches = rows.filter((button) => button.dataset.mnemonic?.toLowerCase() === event.key.toLowerCase());
      if (matches.length === 1) matches[0]?.click();
      else if (matches.length) matches[(matches.indexOf(row!) + 1) % matches.length]?.focus();
      else return;
    }
  }
  event.preventDefault();
}
</script>

<template>
  <Teleport to="body">
    <div ref="scrim" class="menu-scrim" :class="{ 'page-context-fullscreen': menu.fullscreen }"
      @pointerdown.self="close()" @contextmenu.prevent>
      <ContextMenuSurface v-for="(level, depth) in levels" :key="`${depth}:${level.index}`" :anchor="level.anchor"
        class="page-context-menu" role="menu" :aria-label="level.label" :data-menu-level="depth" tabindex="-1"
        @keydown.stop="key($event, depth)" @pointerenter="cancelHover" @scroll="trim(depth)">
        <template v-for="({ item, label }, index) in level.rows" :key="index">
          <hr v-if="item.kind === 'separator'" />
          <button v-else :role="item.kind === 'command' && item.checked !== null ? (item.radio ? 'menuitemradio' : 'menuitemcheckbox') : 'menuitem'"
            type="button" tabindex="-1" :data-menu-row="index" :data-mnemonic="label?.key" :disabled="!item.enabled"
            :aria-haspopup="item.kind === 'submenu' ? 'menu' : undefined"
            :aria-expanded="item.kind === 'submenu' ? submenus[depth]?.index === index : undefined"
            :aria-checked="item.kind === 'command' ? item.checked ?? undefined : undefined"
            @pointerenter="hover($event, depth, index, item)" @click="pick($event, depth, index, item)">
            <Icon v-if="item.kind === 'command' && item.checked" name="check" :size="14" />
            <span class="page-menu-label">{{ label?.before }}<u v-if="label?.key">{{ label.key }}</u>{{ label?.after }}</span>
            <kbd v-if="item.kind === 'command' && item.shortcut" aria-hidden="true">{{ item.shortcut }}</kbd>
            <Icon v-if="item.kind === 'submenu'" class="page-menu-chevron" name="chevronDown" :size="14" />
          </button>
        </template>
      </ContextMenuSurface>
    </div>
  </Teleport>
</template>
