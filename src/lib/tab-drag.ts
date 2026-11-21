import { inject, nextTick, onBeforeUnmount, onMounted, provide, ref, shallowRef, watch, type InjectionKey, type Ref } from "vue";
import { useBrowser } from "../stores/browser";
import { savedPage } from "./bookmarks";
import type { Rect } from "./types";

interface TabDrag {
  id: string;
  key: string;
  source: HTMLElement | null;
  target?: HTMLElement;
  folder: boolean;
  intoFolder?: string;
  index: number;
  dragging: boolean;
}

interface TabDragContext {
  state: Ref<TabDrag | undefined>;
  menuOpen: Ref<boolean>;
}

interface Press {
  row: HTMLElement;
  pointerId: number;
  x: number;
  y: number;
}

const tabDragKey: InjectionKey<TabDragContext> = Symbol("sidebar-tab-drag");
export const useTabDrag = () => inject(tabDragKey);

const dragThreshold = 5;
const splitInset = 24;
const splitTravel = 48;
const scrollEdge = 36;
const maxScrollSpeed = 480;

function previewRect(rect: DOMRect, radius: number): Rect {
  return { x: rect.x, y: rect.y, width: rect.width, height: rect.height, corners: [radius, radius, radius, radius] };
}

function rowKey(row: HTMLElement) {
  return row.dataset.folderId || row.dataset.bookmarkId || row.dataset.tabId;
}

/** Keep sorting local until release; the host only receives the final order. */
export function provideTabDrag(sidebar: Ref<HTMLElement | undefined>) {
  const b = useBrowser();
  const state = shallowRef<TabDrag>();
  const menuOpen = ref(false);
  let pressed: Press | undefined;
  let item: HTMLElement | undefined;
  let itemRadius = 0;
  let pageArea: HTMLElement | null = null;
  let x = 0, y = 0;
  let frame = 0, lastFrame = 0;
  let lastRegions = "";
  let suppressClick = false;
  let pending = false;
  let resetTimer: ReturnType<typeof setTimeout> | undefined;

  function syncNativeRegions() {
    if (!b.native) return;
    const page = pageArea?.getBoundingClientRect();
    const bounds = item?.getBoundingClientRect();
    const guide = b.tabSplitGuide && pageArea?.querySelector<HTMLElement>(".split-drop-target");
    const rect = guide ? previewRect(guide.getBoundingClientRect(), 8) : null;
    const preview = bounds && page && bounds.right > page.left && bounds.left < page.right &&
      bounds.bottom > page.top && bounds.top < page.bottom ? previewRect(bounds, itemRadius) : null;
    const regions = JSON.stringify([rect, preview]);
    if (regions === lastRegions) return;
    lastRegions = regions;
    b.send({ type: "tabDropTarget", rect, preview });
  }

  function stopTracking() {
    const pointer = pressed?.pointerId;
    pressed = undefined;
    if (pointer !== undefined && sidebar.value?.hasPointerCapture(pointer)) {
      sidebar.value.releasePointerCapture(pointer);
    }
    cancelAnimationFrame(frame);
    frame = lastFrame = 0;
    document.removeEventListener("wheel", wheel, true);
    item?.remove();
    item = undefined;
    b.tabSplitGuide = false;
    menuOpen.value = false;
    b.dragging = false;
    if (lastRegions) syncNativeRegions();
    pageArea = null;
  }

  function reset() {
    clearTimeout(resetTimer);
    stopTracking();
    pending = false;
    state.value = undefined;
  }

  function press(event: PointerEvent) {
    if (event.button !== 0 || !event.isPrimary || event.pointerType === "touch") return;
    if (!(event.target instanceof Element) || event.target.closest("button:not(.favorite-open):not(.pin-folder-toggle)")) return;
    const row = event.target.closest<HTMLElement>(".tab-row, .favorite-tile, [data-pin-folder-header]");
    if (!row || !(row.dataset.tabId || row.dataset.bookmarkId || row.dataset.pinFolderHeader)) return;
    reset();
    pressed = { row, pointerId: event.pointerId, x: event.clientX, y: event.clientY };
  }

  function start(press: Press) {
    const { row, pointerId } = press;
    const id = (row.dataset.tabId || row.dataset.bookmarkId || row.dataset.pinFolderHeader)!;
    const key = row.dataset.bookmarkId || id;
    const rect = row.getBoundingClientRect();
    const favorite = row.matches(".favorite-tile");
    const folder = Boolean(row.dataset.pinFolderHeader);
    item = row.cloneNode(true) as HTMLElement;
    item.className = favorite ? "favorites tab-drag-item tab-drag-favorite" : "tab-row tab-drag-item";
    if (row.classList.contains("tab-row-split")) item.classList.add("tab-row-split");
    if (folder) item.classList.add("tab-drag-folder");
    for (const attribute of [...item.attributes]) {
      if (attribute.name !== "class") item.removeAttribute(attribute.name);
    }
    item.setAttribute("aria-hidden", "true");
    item.inert = true;
    Object.assign(item.style, {
      width: `${rect.width}px`, height: `${rect.height}px`,
      left: `${rect.left}px`, top: `${rect.top}px`,
    });
    document.body.append(item);
    itemRadius = parseFloat(getComputedStyle(item).borderTopLeftRadius) || 0;
    pageArea = sidebar.value?.closest(".browser-body")?.querySelector(".page-area") ?? null;
    state.value = { id, key, folder, source: row.closest(".tab-list"), index: 0, dragging: true };
    sidebar.value?.setPointerCapture(pointerId);
    document.addEventListener("wheel", wheel, { capture: true, passive: false });
    suppressClick = true;
    b.dragging = true;
  }

  function rowsIn(list: HTMLElement, key: string) {
    return [...list.children].filter((row): row is HTMLElement =>
      row instanceof HTMLElement && row.matches(":is(.tab-row, .pin-folder):not(.tab-leave-active)") && rowKey(row) !== key);
  }

  function hitTest() {
    return document.elementFromPoint(x, y);
  }

  function updateTarget(hit: Element | null) {
    const drag = state.value;
    if (!drag?.dragging) return;
    const header = hit?.closest<HTMLElement>("[data-pin-folder-header]");
    const folderBounds = header?.getBoundingClientRect();
    const fraction = folderBounds ? (y - folderBounds.top) / folderBounds.height : 0;
    const intoFolder = !drag.folder && fraction > 0.25 && fraction < 0.75 ? header?.dataset.pinFolderHeader : undefined;
    const rootTarget = hit?.closest("[data-pin-root-target]")?.closest(".pinned-area")?.querySelector<HTMLElement>(".pinned-tabs");
    const list = drag.folder ? hit?.closest<HTMLElement>(".pinned-tabs") ?? rootTarget
      : intoFolder ? undefined : hit?.closest<HTMLElement>(".tab-list") ?? rootTarget;
    const target = list && sidebar.value?.contains(list) ? list : undefined;
    // Use layout positions so moving neighbours cannot push the insertion point back and forth.
    const rows = target ? rowsIn(target, drag.key) : [];
    const top = target?.getBoundingClientRect().top ?? 0;
    const next = rows.findIndex((row) => y < top + row.offsetTop + row.offsetHeight / 2);
    const folderIndex = header ? rows.findIndex((row) => row.dataset.folderId === header.dataset.pinFolderHeader) : -1;
    const index = rootTarget ? 0 : folderIndex >= 0 ? folderIndex + Number(fraction >= 0.5) : next < 0 ? rows.length : next;
    if (drag.target !== target || drag.index !== index || drag.intoFolder !== intoFolder) {
      state.value = { ...drag, target, index, intoFolder };
    }

    if (!drag.folder && hit?.closest(".browser-menu-trigger") && sidebar.value?.contains(hit)) menuOpen.value = true;

    const active = b.active;
    const canSplit = !drag.folder && active && active.id !== drag.id && active.bookmarkId !== drag.key && !b.spaceFormOpen && !b.peek;
    const edge = sidebar.value?.getBoundingClientRect().right ?? Infinity;
    // Enter the page decisively from the sidebar. Once shown, keep the guide
    // until the pointer leaves the page so small adjustments near its edge are stable.
    b.tabSplitGuide = Boolean(canSplit && pageArea?.contains(hit) && (
      b.tabSplitGuide || (x >= edge + splitInset && x - (pressed?.x ?? x) >= splitTravel)
    ));
  }

  function scheduleFrame() {
    if (!frame) frame = requestAnimationFrame(render);
  }

  function render(now: number) {
    frame = 0;
    if (!item || !pressed) return;
    item.style.transform = `translate3d(${x - pressed.x}px, ${y - pressed.y}px, 0)`;
    syncNativeRegions();

    const list = hitTest()?.closest<HTMLElement>(".space-list");
    if (!list || !sidebar.value?.contains(list)) { lastFrame = 0; return; }
    const { top, bottom } = list.getBoundingClientRect();
    const distance = Math.max(0, scrollEdge - (y - top)) || -Math.max(0, scrollEdge - (bottom - y));
    if (!distance) { lastFrame = 0; return; }

    const elapsed = lastFrame ? Math.min(32, now - lastFrame) : 16;
    lastFrame = now;
    const before = list.scrollTop;
    list.scrollTop -= Math.sign(distance) * Math.max(1, elapsed * maxScrollSpeed * Math.abs(distance) / scrollEdge / 1000);
    if (list.scrollTop === before) { lastFrame = 0; return; }
    updateTarget(hitTest());
    scheduleFrame();
  }

  function move(event: PointerEvent) {
    if (!pressed || event.pointerId !== pressed.pointerId) return;
    if (!(event.buttons & 1)) { reset(); return; }
    x = event.clientX;
    y = event.clientY;
    if (!item) {
      if (Math.hypot(x - pressed.x, y - pressed.y) < dragThreshold) return;
      if (!pressed.row.isConnected) { reset(); return; }
      start(pressed);
    }
    updateTarget(hitTest());
    scheduleFrame();
  }

  function commitList(drag: TabDrag, target: HTMLElement) {
    const rows = rowsIn(target, drag.key);
    const anchor = rows[drag.index] || rows.at(-1);
    const after = drag.index >= rows.length;
    pending = true;
    state.value = { ...drag, dragging: false };
    stopTracking();
    // The local order stays in place until the native host acknowledges it.
    resetTimer = setTimeout(reset, 600);
    if (target.matches(".pinned-tabs, .pin-folder-tabs")) {
      b.send({ type: "pinPlace", id: drag.id, before: anchor && rowKey(anchor), after,
        folderId: target.dataset.pinFolderList, spaceId: target.closest<HTMLElement>(".space-list")?.dataset.spaceId || b.state.activeSpace });
    } else if (anchor?.dataset.tabId) {
      b.send({ type: "reorder", id: drag.id, before: anchor.dataset.tabId, after });
    } else if (savedPage(b.state, drag.id)) {
      b.send({ type: "bookmarkUnpin", id: drag.id, spaceId: b.state.activeSpace });
    } else {
      void nextTick(reset);
    }
  }

  function release(event: PointerEvent) {
    if (!pressed || event.pointerId !== pressed.pointerId || event.button !== 0) return;
    if (!item) { pressed = undefined; return; }
    x = event.clientX;
    y = event.clientY;
    const hit = hitTest();
    updateTarget(hit);
    const drag = state.value!;
    if (drag.target) { commitList(drag, drag.target); return; }

    if (drag.intoFolder) {
      reset();
      b.send({ type: "pinPlace", id: drag.id, spaceId: b.state.activeSpace, folderId: drag.intoFolder });
      return;
    }

    const space = hit?.closest<HTMLElement>("[data-tab-drop-space]")?.dataset.tabDropSpace;
    const favorites = hit?.closest(".favorites");
    const favorite = hit?.closest<HTMLElement>(".favorite-tile")?.dataset.bookmarkId;
    const active = b.tabSplitGuide ? b.active : undefined;
    reset();
    if (drag.folder) return;
    if (space) {
      b.send({ type: "move", id: drag.id, spaceId: space });
    } else if (favorites && sidebar.value?.contains(favorites) && drag.id !== favorite) {
      b.send({ type: "favoriteTab", id: drag.id, before: favorite });
    } else if (active) {
      // Restore the live page region before the host captures its split entrance.
      void nextTick(() => b.send({ type: "split", id: active.id, other: drag.id }));
    }
  }

  function wheel(event: WheelEvent) {
    if (!item || !event.deltaY || event.ctrlKey || event.metaKey || event.altKey || event.shiftKey) return;
    const list = hitTest()?.closest<HTMLElement>(".space-list, .favorites");
    if (!list || !sidebar.value?.contains(list)) return;
    event.preventDefault();
    const unit = event.deltaMode === WheelEvent.DOM_DELTA_LINE ? 16 :
      event.deltaMode === WheelEvent.DOM_DELTA_PAGE ? list.clientHeight : 1;
    list.scrollTop += event.deltaY * unit;
    updateTarget(hitTest());
  }

  function scrolled(event: Event) {
    if (item && event.target instanceof Node && sidebar.value?.contains(event.target)) {
      updateTarget(hitTest());
      scheduleFrame();
    }
  }

  function cancel(event: PointerEvent) { if (event.pointerId === pressed?.pointerId) reset(); }
  function captureLost(event: PointerEvent) { if (event.target === sidebar.value) cancel(event); }
  function escape(event: KeyboardEvent) {
    if (event.key !== "Escape" || !item) return;
    event.preventDefault();
    event.stopPropagation();
    reset();
  }
  function allowClick() { suppressClick = false; }
  function click(event: MouseEvent) {
    if (!suppressClick || !event.detail) return;
    allowClick();
    event.preventDefault();
    event.stopPropagation();
  }
  function visibilityChanged() { if (document.hidden) reset(); }
  watch([() => b.state.tabs, () => b.state.pins, () => b.state.pinLayouts], () => { if (pending) reset(); });
  watch([() => b.state.activeSpace, () => b.state.sidebarCollapsed, () => b.spaceFormOpen], reset);
  onMounted(() => {
    document.addEventListener("pointerdown", allowClick, true);
    document.addEventListener("pointermove", move);
    document.addEventListener("pointerup", release);
    document.addEventListener("pointercancel", cancel);
    document.addEventListener("scroll", scrolled, true);
    document.addEventListener("keydown", escape, true);
    document.addEventListener("click", click, true);
    document.addEventListener("visibilitychange", visibilityChanged);
    window.addEventListener("blur", reset);
  });
  onBeforeUnmount(() => {
    reset();
    document.removeEventListener("pointerdown", allowClick, true);
    document.removeEventListener("pointermove", move);
    document.removeEventListener("pointerup", release);
    document.removeEventListener("pointercancel", cancel);
    document.removeEventListener("scroll", scrolled, true);
    document.removeEventListener("keydown", escape, true);
    document.removeEventListener("click", click, true);
    document.removeEventListener("visibilitychange", visibilityChanged);
    window.removeEventListener("blur", reset);
  });
  provide(tabDragKey, { state, menuOpen });
  return { press, captureLost };
}
