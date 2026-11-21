<script setup lang="ts">
import { computed } from "vue";
import { useBrowser } from "../../stores/browser";
import { PIN_FOLDER_COLORS, pinFolderById } from "../../lib/pin-folders";
import { useTabDrag } from "../../lib/tab-drag";
import type { PinFolder, Tab } from "../../lib/types";
import type { TabRow } from "../../lib/tab-rows";
import Icon from "../common/Icon.vue";
import TabList from "./TabList.vue";

const props = defineProps<{ spaceId: string; rows: TabRow[]; doomedIds?: string[] }>();
const emit = defineEmits<{ menu: [event: MouseEvent, tab: Tab] }>();
const b = useBrowser();
const drag = useTabDrag();
const layout = computed(() => b.state.pinLayouts?.[props.spaceId]);
const groupedRows = computed(() => {
  const folders = pinFolderById(layout.value);
  const groups = new Map<string | undefined, TabRow[]>();
  for (const row of props.rows) {
    const folder = folders.get(row.tab.bookmarkId ?? row.tab.id);
    const rows = groups.get(folder) ?? [];
    rows.push(row);
    groups.set(folder, rows);
  }
  return groups;
});

function containsActive(folder: PinFolder) {
  return [b.active, b.splitPartner].some((tab) => tab?.bookmarkId && folder.pins.includes(tab.bookmarkId));
}

function toggle(folder: PinFolder) {
  b.send({ type: "pinFolderCollapse", spaceId: props.spaceId, id: folder.id, collapsed: !folder.collapsed });
}

function openMenu(event: MouseEvent, id?: string) {
  const anchor = (event.currentTarget as HTMLElement).getBoundingClientRect();
  const x = event.type === "contextmenu" ? event.clientX : anchor.left;
  const y = event.type === "contextmenu" ? event.clientY : anchor.bottom + 4;
  b.contextMenu = {
    x,
    y,
    pinFolder: { spaceId: props.spaceId, id },
  };
}
</script>

<template>
  <div class="pinned-area">
    <div class="section-label pin-section-heading" data-pin-root-target @contextmenu.prevent="openMenu($event)">
      <span>固定标签</span>
      <button type="button" class="pin-folder-action" aria-label="新建文件夹" title="新建文件夹"
        aria-haspopup="dialog" @click="openMenu($event)">
        <Icon name="plus" :size="14" />
      </button>
    </div>
    <TabList class="pinned-tabs" :rows="groupedRows.get(undefined) ?? []" :folders="layout?.folders"
      :order="layout?.root" :doomed-ids="doomedIds" aria-label="固定标签页" @menu="(e, t) => emit('menu', e, t)">
      <template #folder="{ folder }">
        <div class="pin-folder-header" :class="{
          'pin-folder-active': folder.collapsed && containsActive(folder),
          'pin-folder-drop': drag?.state.value?.intoFolder === folder.id,
        }" :data-pin-folder-header="folder.id" @contextmenu.prevent.stop="openMenu($event, folder.id)">
          <button type="button" class="pin-folder-toggle" :aria-expanded="!folder.collapsed" :title="folder.name"
            :aria-description="folder.collapsed && containsActive(folder) ? '包含当前网页' : undefined" @click="toggle(folder)">
            <Icon name="chevronDown" :size="12" />
            <Icon name="folder" :size="16" class="pin-folder-icon" :data-color="folder.color ?? 'default'"
              :style="{ '--folder-color': PIN_FOLDER_COLORS[folder.color ?? 'default'].fill }" />
            <span>{{ folder.name }}</span>
          </button>
          <button type="button" class="pin-folder-action pin-folder-more" :aria-label="`${folder.name}的文件夹操作`"
            aria-haspopup="menu" title="文件夹操作" @click="openMenu($event, folder.id)">
            <Icon name="more" :size="14" />
          </button>
        </div>
        <TabList v-if="!folder.collapsed" class="pin-folder-tabs" :rows="groupedRows.get(folder.id) ?? []"
          :data-pin-folder-list="folder.id" :aria-label="`${folder.name}内的固定标签`"
          :doomed-ids="doomedIds" @menu="(e, t) => emit('menu', e, t)" />
      </template>
    </TabList>
  </div>
</template>

<style scoped>
.pin-section-heading {
  min-height: 24px;
}
.pin-folder-header {
  display: flex;
  align-items: center;
  min-height: 30px;
  margin-bottom: 2px;
  padding-right: 3px;
  border: 1px solid transparent;
  border-radius: 6px;
  color: var(--ink);
}
.pin-folder-header:hover { background: var(--hover); }
.pin-folder-active { background: var(--selected); border-color: var(--edge); }
.pin-folder-drop { background: var(--selected); border-color: var(--accent); }
.pin-folder-toggle {
  display: flex;
  align-items: center;
  align-self: stretch;
  flex: 1;
  gap: 6px;
  min-width: 0;
  padding: 0 4px;
  font-size: 13px;
  text-align: left;
}
.pin-folder-toggle > svg { flex-shrink: 0; opacity: .75; }
.pin-folder-toggle > .pin-folder-icon:not([data-color="default"]) {
  color: color-mix(in srgb, var(--folder-color) 82%, var(--ink));
  opacity: 1;
}
.pin-folder-toggle[aria-expanded="false"] > svg:first-child { transform: rotate(-90deg); }
.pin-folder-toggle > span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.pin-folder-action {
  display: grid;
  place-items: center;
  flex: 0 0 24px;
  width: 24px;
  height: 22px;
  border-radius: 5px;
  color: var(--muted);
}
.pin-folder-action:hover { background: var(--hover); color: var(--ink); }
.pin-folder-more { opacity: 0; }
.pin-folder-header:hover .pin-folder-more,
.pin-folder-header:focus-within .pin-folder-more { opacity: 1; }
.pin-folder-tabs { margin-inline-start: 11px; padding-inline-start: 7px; border-inline-start: 1px solid var(--edge); }
.pin-folder-tabs:empty { border-inline-start-color: transparent; }
.pin-folder-drag-source > .pin-folder-header { opacity: 0; pointer-events: none; }
</style>
