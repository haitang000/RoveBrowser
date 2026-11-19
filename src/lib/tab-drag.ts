import { inject, nextTick, onBeforeUnmount, onMounted, provide, shallowRef, watch, type InjectionKey, type Ref } from "vue";
import { useBrowser } from "../stores/browser";
import { savedPage } from "./bookmarks";
import type { Tab } from "./types";

interface TabDrag {
  id: string;
  key: string;
  source?: HTMLElement;
  target?: HTMLElement;
  index: number;
  dragging: boolean;
}

interface TabDragContext {
  state: Ref<TabDrag | undefined>;
  over(event: DragEvent, tabs: Tab[]): void;
  drop(event: DragEvent, tabs: Tab[]): void;
}

const tabDragKey: InjectionKey<TabDragContext> = Symbol("sidebar-tab-drag");
export const useTabDrag = () => inject(tabDragKey);
const keyOf = (tab: Tab) => tab.bookmarkId || tab.id;

/** Keep sorting local until release; the host only receives the final order. */
export function provideTabDrag(sidebar: Ref<HTMLElement | undefined>) {
  const b = useBrowser();
  const state = shallowRef<TabDrag>();
  let item: HTMLElement | undefined;
  let nativeImage: HTMLCanvasElement | undefined;
  let itemFrame = 0;
  let originX = 0, originY = 0, deltaX = 0, deltaY = 0;
  let pending = false;
  let resetTimer: ReturnType<typeof setTimeout> | undefined;

  function removeItem() {
    cancelAnimationFrame(itemFrame);
    itemFrame = 0;
    item?.remove();
    nativeImage?.remove();
    item = nativeImage = undefined;
  }

  function follow(event: DragEvent) {
    if (!item || !state.value?.dragging) return;
    // Chromium's final source drag event can report (0, 0). The document's
    // dragover coordinates stay in the UI viewport, including over drop zones.
    if (event.type === "drag" && !event.clientX && !event.clientY) return;
    item.hidden = false;
    deltaX = event.clientX - originX;
    deltaY = event.clientY - originY;
    if (!itemFrame) itemFrame = requestAnimationFrame(() => {
      itemFrame = 0;
      if (item) item.style.transform = `translate3d(${deltaX}px, ${deltaY}px, 0)`;
    });
  }

  function reset() {
    clearTimeout(resetTimer);
    removeItem();
    pending = false;
    state.value = undefined;
    b.dragging = false;
  }

  function start(event: DragEvent) {
    if (!(event.target instanceof Element) || !event.dataTransfer) return;
    const row = event.target.closest<HTMLElement>(".tab-row, .favorite-tile");
    const id = row?.dataset.tabId || row?.dataset.bookmarkId;
    if (!row || !id) return;
    reset();
    const key = row.dataset.bookmarkId || id;
    state.value = { id, key, source: row.closest<HTMLElement>(".tab-list") || undefined, index: 0, dragging: true };
    event.dataTransfer.setData("text/era-tab", id);
    event.dataTransfer.effectAllowed = "move";
    b.dragging = true;

    if (!row.matches(".tab-row")) return;
    const rect = row.getBoundingClientRect();
    item = row.cloneNode(true) as HTMLElement;
    item.className = "tab-row tab-drag-item";
    if (row.classList.contains("tab-row-split")) item.classList.add("tab-row-split");
    for (const attribute of [...item.attributes]) {
      if (attribute.name !== "class") item.removeAttribute(attribute.name);
    }
    item.setAttribute("aria-hidden", "true");
    item.inert = true;
    Object.assign(item.style, {
      width: `${rect.width}px`, height: `${rect.height}px`,
      left: `${rect.left}px`, top: `${rect.top}px`,
    });
    originX = event.clientX;
    originY = event.clientY;
    deltaX = deltaY = 0;
    // Draw the full, opaque row in the UI instead of handing it to the OS,
    // which applies translucency to native drag images regardless of its CSS.
    document.body.append(item);
    nativeImage = document.createElement("canvas");
    nativeImage.width = nativeImage.height = 1;
    nativeImage.style.cssText = "position:fixed;left:0;top:0;pointer-events:none";
    nativeImage.setAttribute("aria-hidden", "true");
    document.body.append(nativeImage);
    event.dataTransfer.setDragImage(nativeImage, 0, 0);
  }

  function over(event: DragEvent, tabs: Tab[]) {
    if (!event.dataTransfer?.types.includes("text/era-tab")) return;
    event.preventDefault();
    event.dataTransfer.dropEffect = "move";
    const target = event.currentTarget as HTMLElement;
    const drag = state.value;
    if (!drag) return;
    const remaining = tabs.filter((tab) => keyOf(tab) !== drag.key);
    const top = target.getBoundingClientRect().top;
    // Use layout positions, not the animated presentation positions. The empty
    // slot then provides a dead zone rather than oscillating under the pointer.
    const rows = [...target.children].filter((row): row is HTMLElement =>
      row instanceof HTMLElement && row.matches(".tab-row:not(.tab-drag-source):not(.tab-leave-active)"));
    const next = rows.findIndex((row) => event.clientY < top + row.offsetTop + row.offsetHeight / 2);
    const index = next < 0 ? remaining.length : Math.min(next, remaining.length);
    if (drag.target !== target || drag.index !== index) state.value = { ...drag, target, index };
  }

  function drop(event: DragEvent, tabs: Tab[]) {
    const id = event.dataTransfer?.getData("text/era-tab");
    if (!id) return;
    event.preventDefault();
    // A quick release may follow dragenter before Chromium emits dragover.
    over(event, tabs);
    const target = event.currentTarget as HTMLElement;
    const drag = state.value;
    const key = drag?.key || b.state.tabs.find((tab) => tab.id === id)?.bookmarkId || id;
    const remaining = tabs.filter((tab) => keyOf(tab) !== key);
    const hit = (event.target as Element).closest<HTMLElement>(".tab-row");
    const hitIndex = remaining.findIndex((tab) => tab.id === hit?.dataset.tabId);
    const index = drag?.target === target ? drag.index : hitIndex < 0 ? remaining.length : hitIndex;
    const before = remaining[index] || remaining.at(-1);
    pending = true;
    if (drag) state.value = { ...drag, dragging: false };
    removeItem();
    b.dragging = false;
    // The local order stays in place until the native host acknowledges it.
    resetTimer = setTimeout(reset, 600);
    if (before) {
      b.send({ type: "reorder", id, before: before.id, after: index >= remaining.length });
    } else if (target.classList.contains("pinned-tabs")) {
      b.send({ type: "bookmarkPin", id, spaceId: target.closest<HTMLElement>(".space-list")?.dataset.spaceId || b.state.activeSpace });
    } else if (savedPage(b.state, id)) {
      b.send({ type: "bookmarkUnpin", id, spaceId: b.state.activeSpace });
    } else {
      void nextTick(reset);
    }
  }

  function leaveLists(event: DragEvent) {
    follow(event);
    const drag = state.value;
    if (!drag?.dragging || !(event.target instanceof Element)) return;
    const list = event.target.closest(".tab-list");
    if ((!list || !sidebar.value?.contains(list)) && drag.target) state.value = { ...drag, target: undefined };
  }

  function end() {
    removeItem();
    if (!pending) reset();
  }
  function leaveWindow(event: DragEvent) {
    if (!event.relatedTarget && event.target === document.documentElement) {
      if (item) item.hidden = true;
      const drag = state.value;
      if (drag?.dragging && drag.target) state.value = { ...drag, target: undefined };
    }
  }
  function visibilityChanged() { if (document.hidden) reset(); }
  // A drop on the page can merge the source row into a split before dragend.
  watch(() => b.dragging, (dragging) => { if (!dragging && !pending && state.value) reset(); });
  watch(() => b.state.tabs, () => { if (pending) reset(); });
  watch([() => b.state.activeSpace, () => b.state.sidebarCollapsed], reset);
  onMounted(() => {
    document.addEventListener("dragover", leaveLists, true);
    document.addEventListener("drag", follow, true);
    document.addEventListener("dragend", end, true);
    document.addEventListener("dragleave", leaveWindow);
    document.addEventListener("visibilitychange", visibilityChanged);
    window.addEventListener("blur", reset);
  });
  onBeforeUnmount(() => {
    reset();
    document.removeEventListener("dragover", leaveLists, true);
    document.removeEventListener("drag", follow, true);
    document.removeEventListener("dragend", end, true);
    document.removeEventListener("dragleave", leaveWindow);
    document.removeEventListener("visibilitychange", visibilityChanged);
    window.removeEventListener("blur", reset);
  });
  provide(tabDragKey, { state, over, drop });
  return { start };
}
