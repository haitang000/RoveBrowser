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
  let image: HTMLElement | undefined;
  let imageFrame = 0;
  let pending = false;
  let resetTimer: ReturnType<typeof setTimeout> | undefined;

  function removeImage() {
    cancelAnimationFrame(imageFrame);
    image?.remove();
    image = undefined;
  }

  function reset() {
    clearTimeout(resetTimer);
    removeImage();
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
    image = row.cloneNode(true) as HTMLElement;
    image.className = "tab-row tab-drag-image";
    for (const attribute of [...image.attributes]) {
      if (attribute.name !== "class") image.removeAttribute(attribute.name);
    }
    image.setAttribute("aria-hidden", "true");
    image.inert = true;
    Object.assign(image.style, {
      width: `${rect.width}px`, height: `${rect.height}px`,
      left: `${rect.left}px`, top: `${rect.top}px`,
    });
    // A body-level copy escapes both the scrolling list and the space swipe's
    // clipping. Chromium snapshots the whole row, even if its source is clipped.
    document.body.append(image);
    event.dataTransfer.setDragImage(image, event.clientX - rect.left, event.clientY - rect.top);
    imageFrame = requestAnimationFrame(() => { if (image) image.style.left = "-10000px"; });
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
    removeImage();
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
    const drag = state.value;
    if (!drag?.dragging || !(event.target instanceof Element)) return;
    const list = event.target.closest(".tab-list");
    if ((!list || !sidebar.value?.contains(list)) && drag.target) state.value = { ...drag, target: undefined };
  }

  function end() {
    removeImage();
    if (!pending) reset();
  }
  function leaveWindow(event: DragEvent) { if (!event.relatedTarget) leaveLists(event); }
  function visibilityChanged() { if (document.hidden) reset(); }
  watch(() => b.state, () => { if (pending) reset(); });
  watch([() => b.state.activeSpace, () => b.state.sidebarCollapsed], reset);
  onMounted(() => {
    document.addEventListener("dragover", leaveLists, true);
    document.addEventListener("dragend", end, true);
    document.addEventListener("dragleave", leaveWindow);
    document.addEventListener("visibilitychange", visibilityChanged);
    window.addEventListener("blur", reset);
  });
  onBeforeUnmount(() => {
    reset();
    document.removeEventListener("dragover", leaveLists, true);
    document.removeEventListener("dragend", end, true);
    document.removeEventListener("dragleave", leaveWindow);
    document.removeEventListener("visibilitychange", visibilityChanged);
    window.removeEventListener("blur", reset);
  });
  provide(tabDragKey, { state, over, drop });
  return { start };
}
