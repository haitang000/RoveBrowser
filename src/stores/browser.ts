import { computed, ref, watch } from "vue";
import { defineStore } from "pinia";
import { initialState, reduce } from "../lib/preview";
import type {
  Command,
  DownloadItem,
  HistoryEntry,
  Tab,
  Rect,
  Favorite,
  Point,
  PeekPhase,
} from "../lib/types";
import { address, host } from "../lib/address";
import { normalizeFindQuery } from "../lib/find";
import { nowSeconds } from "../lib/tab-meta";
import { activeDownloadCount, isActiveDownload } from "../lib/downloads";
import { settingsFor, shortcutFor, shortcutText, type SettingsPane } from "../lib/settings";
import { liveTab, savedPage, upgradeLibrary } from "../lib/bookmarks";
export const useBrowser = defineStore("browser", () => {
  const native = Boolean(window.__ERA_NATIVE__ && window.ipc);
  const popupWindow = Boolean(window.__ERA_POPUP__);
  const contextMenu = ref<{ x: number; y: number; tab?: Tab; favorite?: Favorite }>();
  const popupArea = ref<Rect>({ x: 240, y: 32, width: 1032, height: 780 });
  const splitResize = ref<{ ratio: number; resizing: boolean }>();
  const hoverPreview = ref<{ id: string; frame: Rect; opacity: number; phase: string; closing: boolean; ticket: number; instant: boolean }>();
  const previewAnimationDone = ref(0);
  const state = ref(upgradeLibrary((native && window.__ERA_INITIAL_STATE__) || initialState())),
    history = ref<HistoryEntry[]>([]),
    suggestions = ref<string[]>([]),
    recent = ref<string[]>([]),
    error = ref("");
  const commandOpen = ref(false),
    thumbnails = ref<Record<string, string>>({}),
    previewTarget = ref<string>(),
    previewHiddenTicket = ref(0),
    previewDismissals = ref(0),
    previewPresentedTicket = ref(0),
    previewFailures = ref<Record<string, boolean>>({}),
    peek = ref<Tab | null>(null),
    peekBackground = ref<Record<number, string>>({}),
    peekBackgroundReady = ref(true),
    peekOrigin = ref<Point | null>(null),
    peekPhase = ref<PeekPhase>("open"),
    peekTicket = ref(0),
    peekMoving = ref(0),
    peekSurface = ref<{ ticket: number; image: string | null } | null>(null),
    commandMode = ref<"new" | "edit">("new"),
    deskOpen = ref(false),
    settingsOpen = ref(false),
    dragging = ref(false),
    glassBlocked = ref(false),
    findOpen = ref(false),
    findQuery = ref(""),
    findCount = ref(0),
    findActive = ref(0),
    findTicket = ref(0),
    findFocusTicket = ref(0),
    pageZoom = ref(1),
    audio = ref<Record<string, boolean>>({}),
    zoomNotice = ref<{ ticket: number; factor: number } | null>(null),
    actionToast = ref<{ ticket: number; message: string; action?: string } | null>(null),
    /** Whether an undoable close batch is recent enough for the sidebar Undo. */
    undoAvailable = ref(false),
    downloads = ref<DownloadItem[]>([]),
    downloadsOpen = ref(false),
    downloadsFocus = ref<string | undefined>(undefined),
    /** Clock for tab fading and the Clear button's stale check. */
    now = ref(nowSeconds());
  let zoomNoticeTimer: ReturnType<typeof setTimeout> | undefined;
  let zoomNoticeTicket = 0;
  let actionToastTimer: ReturnType<typeof setTimeout> | undefined;
  let actionToastTicket = 0;
  let undoTimer: ReturnType<typeof setTimeout> | undefined;
  type PreviewUndo = { entries: { index: number; tab: Tab }[]; active: string; blankIds: string[] };
  let previewUndo: PreviewUndo | null = null;
  // Aging is displayed in minutes, so a minute tick is enough; the popup layer
  // never renders tabs and does not need its own interval.
  if (!popupWindow) setInterval(() => { now.value = nowSeconds(); }, 60_000);
  const active = computed(() =>
    state.value.tabs.find((t) => t.id === state.value.activeTab),
  );
  const space = computed(
    () =>
      state.value.spaces.find((s) => s.id === state.value.activeSpace) ||
      state.value.spaces[0],
  );
  const tabs = computed(() =>
    state.value.tabs.filter((t) => t.spaceId === state.value.activeSpace && !state.value.favorites.some((f) => f.id === t.bookmarkId)),
  );
  const pinnedTabs = computed<Tab[]>(() => (state.value.pins || [])
    .filter((f) => f.spaceId === state.value.activeSpace)
    .map((f) => ({ ...(liveTab(state.value, f.id) || { id: f.id, spaceId: f.spaceId!, url: f.url, unloaded: true }), title: f.title, favicon: liveTab(state.value, f.id)?.favicon || f.favicon, pinned: true, bookmarkId: f.id })));
  const activeBookmark = computed(() => active.value?.bookmarkId ? savedPage(state.value, active.value.bookmarkId) : undefined);
  const activeDownloads = computed(() => activeDownloadCount(downloads.value));
  const settings = computed(() => settingsFor(state.value.settings));
  function shortcutLabel(id: string) {
    const binding = shortcutFor(id, settings.value);
    return binding ? shortcutText(binding) : "";
  }
  function shortcutTitle(title: string, id: string) {
    const label = shortcutLabel(id);
    return label ? `${title} (${label})` : title;
  }
  const dataStatus = ref("");
  function openSettings(pane?: SettingsPane) {
    commandOpen.value = false; settingsOpen.value = false; deskOpen.value = false;
    send({ type: "settingsOpen", ...(pane ? { pane } : {}) });
  }
  const overlay = computed(
    () =>
      commandOpen.value ||
      deskOpen.value ||
      settingsOpen.value ||
      Boolean(contextMenu.value) ||
      dragging.value,
  );
  function send(c: Command) {
    if (
      ![
        "layout",
        "peekLayout",
        "peekMotionStart",
        "peekMotionComplete",
        "history",
        "suggest",
        "overlay",
        "thumbnails",
        "tabPreview",
        "tabPreviewRefresh",
        "windowChrome",
        "popupState",
        "popupLayer",
        "popupReady",
      ].includes(c.type)
    )
      error.value = "";
    try {
      if (native) window.ipc!.postMessage(JSON.stringify(c));
      else if (c.type === "tabPreview") {
        if (!c.rect) previewHiddenTicket.value = Number(c.ticket);
      } else if (c.type.startsWith("tabPreview")) {
        // Standalone UI previews never create or capture real pages.
      } else if (c.type === "peekOpen") {
        const url = address(String(c.url));
        if (!url.startsWith("http")) throw Error("Peek 仅支持网页");
        peek.value = {
          id: crypto.randomUUID(),
          spaceId: state.value.activeSpace,
          title: host(url),
          url,
          pinned: false,
        };
        peekBackground.value = {};
        peekBackgroundReady.value = true;
        peekOrigin.value = (c.origin as Point) || null;
        peekPhase.value = "opening";
        peekTicket.value = 1;
        peekMoving.value = 0;
        peekSurface.value = null;
      } else if (
        c.type === "peekClose" ||
        (c.type === "close" && !c.id && peek.value)
      ) {
        if (peek.value && peekPhase.value !== "closing") {
          peekPhase.value = "closing";
          peekTicket.value++;
          peekMoving.value = 0;
          peekSurface.value = {
            ticket: peekTicket.value,
            image: peekSurface.value?.image || null,
          };
        }
      } else if (c.type === "peekMotionStart") {
        if (c.id === peek.value?.id && c.ticket === peekTicket.value)
          peekMoving.value = peekTicket.value;
      } else if (c.type === "peekMotionComplete") {
        if (c.id === peek.value?.id && c.ticket === peekTicket.value) {
          if (peekPhase.value === "opening") peekPhase.value = "open";
          else if (peekPhase.value === "closing") {
            peek.value = null;
            peekBackground.value = {};
          }
        }
      } else if (c.type === "peekPromote") {
        if (peek.value)
          state.value = reduce(JSON.parse(JSON.stringify(state.value)), {
            type: "new",
            url: peek.value.url,
          });
        peek.value = null;
        peekBackground.value = {};
      } else if (c.type === "peekBackdropFailed") {
        if (c.id === peek.value?.id && c.ticket === peekTicket.value) {
          peek.value = null;
          peekBackground.value = {};
          error.value = "无法解码 Peek 背景，请重试";
        }
      } else if (c.type === "navigate" && peek.value) {
        const url = address(String(c.url));
        if (!url.startsWith("http")) throw Error("Peek 仅支持网页");
        peek.value = { ...peek.value, url, title: host(url) };
      } else {
        const batchClose = ["clearStep", "closeOthers", "closeBelow"].includes(c.type);
        const beforeTabs = batchClose
          ? state.value.tabs.map((tab) => ({ ...tab }))
          : [];
        const beforeActive = state.value.activeTab;
        state.value = reduce(
          structuredClone(JSON.parse(JSON.stringify(state.value))),
          c,
        );
        // The native host confirms batch closes with an action toast; the
        // standalone preview mirrors both the message and a real Undo batch.
        if (batchClose && beforeTabs.length) {
          const alive = new Set(state.value.tabs.map((tab) => tab.id));
          const removed = beforeTabs.filter((tab) => !alive.has(tab.id));
          if (removed.length) {
            previewUndo = {
              entries: removed.map((tab) => ({
                index: beforeTabs.findIndex((item) => item.id === tab.id),
                tab,
              })),
              active: beforeActive,
              blankIds: state.value.tabs
                .filter((tab) => !beforeTabs.some((item) => item.id === tab.id))
                .map((tab) => tab.id),
            };
            armUndoWindow();
            notify(`已关闭 ${removed.length} 个标签`);
          }
        }
        if (
          [
            "new",
            "activate",
            "bookmarkOpen",
            "space",
            "spaceCreate",
            "restore",
            "move",
            "close",
          ].includes(c.type)
        ) {
          peek.value = null;
          peekBackground.value = {};
        }
      }
    } catch (e) {
      error.value = String(e);
    }
  }
  function openCommand(mode: "new" | "edit" = "new") {
    error.value = "";
    commandMode.value = mode;
    commandOpen.value = true;
  }
  function cycle(reverse = false) {
    const list = [
      ...state.value.favorites.flatMap((f) => liveTab(state.value, f.id) || []),
      ...pinnedTabs.value.filter((t) => !t.unloaded),
      ...tabs.value.filter((t) => !t.pinned),
    ];
    const i = list.findIndex((t) => t.id === state.value.activeTab);
    const t = list[(i + (reverse ? -1 : 1) + list.length) % list.length];
    if (t) send({ type: "activate", id: t.id });
  }
  function resetFind() {
    findOpen.value = false;
    findQuery.value = "";
    findCount.value = 0;
    findActive.value = 0;
  }
  /** ⌘F/Ctrl+F. The host owns the WebView2 find session; in the standalone
   * preview there is no page, so the bar only demonstrates its own layout. */
  function openFind() {
    if (native) send({ type: "findOpen" });
    else {
      resetFind();
      findOpen.value = true;
      findTicket.value++;
    }
  }
  function closeFind() {
    resetFind();
    if (native) send({ type: "findClose" });
  }
  function setFindQuery(value: string) {
    findQuery.value = normalizeFindQuery(value);
    if (native) send({ type: "findStart", query: findQuery.value });
    else {
      findCount.value = 0;
      findActive.value = 0;
    }
  }
  function findStep(backwards = false) {
    if (native) send({ type: "findNext", backwards });
  }
  /** Zigzag's View › Zoom In / Zoom Out / Actual Size, on the current page. */
  function zoom(action: "in" | "out" | "reset") {
    if (native) send({ type: "zoom", action });
  }
  /** Mute or unmute a tab's sound; the host owns the WebView2 controller. */
  function toggleMute(tab?: Tab | null) {
    if (!tab) return;
    if (native) {
      send({ type: "mute", id: tab.bookmarkId || tab.id, muted: !tab.muted });
    } else {
      const live = state.value.tabs.find((item) => item.id === tab.id);
      if (live) live.muted = !live.muted;
    }
  }
  function showZoomNotice(factor: number) {
    zoomNotice.value = { ticket: ++zoomNoticeTicket, factor };
    clearTimeout(zoomNoticeTimer);
    zoomNoticeTimer = setTimeout(() => {
      zoomNotice.value = null;
    }, 1200);
  }
  /** Bottom action toast: Clear/Close batch confirms and the Copy-link hint. */
  function notify(message: string, action?: string, duration = 2400) {
    actionToast.value = { ticket: ++actionToastTicket, message, action };
    clearTimeout(actionToastTimer);
    actionToastTimer = setTimeout(() => {
      actionToast.value = null;
    }, duration);
  }
  function dismissToast() {
    actionToast.value = null;
    clearTimeout(actionToastTimer);
  }
  /** Keeps the sidebar's Clear turned into Undo for the same window as the toast. */
  function armUndoWindow() {
    undoAvailable.value = true;
    clearTimeout(undoTimer);
    undoTimer = setTimeout(() => {
      undoAvailable.value = false;
    }, 4000);
  }
  function undoBatch() {
    clearTimeout(undoTimer);
    undoAvailable.value = false;
    if (native) {
      send({ type: "undoBatch" });
      return;
    }
    const batch = previewUndo;
    previewUndo = null;
    if (!batch) return;
    try {
      state.value = reduce(
        structuredClone(JSON.parse(JSON.stringify(state.value))),
        {
          type: "restoreBatch",
          entries: batch.entries,
          active: batch.active,
          blankIds: batch.blankIds,
        },
      );
      notify(`已恢复 ${batch.entries.length} 个标签`);
    } catch (e) {
      error.value = String(e);
    }
  }
  function toggleDownloads(open?: boolean) {
    const next = open ?? !downloadsOpen.value;
    // The panel lives in the sidebar lane; opening it with the sidebar
    // collapsed would otherwise reveal nothing.
    if (next && state.value.sidebarCollapsed) send({ type: "sidebar" });
    downloadsOpen.value = next;
  }
  function closeDownloads() {
    downloadsOpen.value = false;
  }
  /** Runs a panel action; the standalone preview mutates its own list so the
   * panel stays usable without the native host. */
  function downloadAction(action: string, id?: string) {
    if (native) {
      send({ type: "downloadAction", id, action });
      return;
    }
    const items = downloads.value;
    if (action === "clearFinished") {
      downloads.value = items.filter(isActiveDownload);
      return;
    }
    if (action === "openFolder") return;
    if (!id || !items.some((item) => item.id === id)) return;
    const change = (patch: Partial<DownloadItem>) => {
      downloads.value = items.map((item) => (item.id === id ? { ...item, ...patch } : item));
    };
    const now = Math.floor(Date.now() / 1000);
    if (action === "pause") change({ state: "paused" });
    else if (action === "resume") change({ state: "inProgress" });
    else if (action === "cancel") change({ state: "cancelled", finished: now, exists: false });
    else if (action === "remove") downloads.value = items.filter((item) => item.id !== id);
  }
  // The page accelerator needs to know whether Esc belongs to the panel. The
  // popup layer never renders it, so only the main UI reports.
  watch(downloadsOpen, (open) => {
    if (native && !popupWindow) send({ type: "downloadsPanel", open });
  });
  window.__ERA_RECEIVE__ = (e) => {
    if (e.type === "splitResize" && typeof e.ratio === "number" && Number.isFinite(e.ratio))
      splitResize.value = { ratio: e.ratio, resizing: Boolean(e.resizing) };
    if (e.type === "tabPreviewPresented" && e.ticket !== undefined) previewPresentedTicket.value = e.ticket;
    if (e.type === "tabPreviewUnavailable" && e.id) previewFailures.value[e.id] = true;
    if (e.type === "tabPreviewHidden" && e.ticket !== undefined) previewHiddenTicket.value = e.ticket;
    if (e.type === "tabPreviewDismissed") previewDismissals.value++;
    if (e.type === "popupState" && e.popup) receivePopup(e.popup);
    if (e.type === "thumbnails" && e.images) thumbnails.value = e.images;
    if (e.type === "thumbnail" && e.id) {
      const allowed = new Set(e.kept || Object.keys(thumbnails.value));
      const next = Object.fromEntries(
        Object.entries(thumbnails.value).filter(([id]) => allowed.has(id)),
      );
      if (e.image) { next[e.id] = e.image; delete previewFailures.value[e.id]; }
      else delete next[e.id];
      thumbnails.value = next;
    }
    if (e.type === "peek") {
      if (peek.value?.id !== e.peek?.id) {
        // Replace the session and its cached images in one Vue update. Reopening
        // must not briefly clear a backdrop already covering live native windows.
        peekBackground.value = Object.fromEntries(
          (e.backgrounds || []).flatMap((image, pane) =>
            image ? [[pane, image]] : [],
          ),
        );
        peekBackgroundReady.value = e.backgroundReady ?? !native;
        peekSurface.value = null;
        peekMoving.value = 0;
      }
      peekPhase.value = e.phase || "open";
      peekTicket.value = e.ticket || 0;
      peekOrigin.value = e.origin || null;
      peek.value = e.peek ?? null;
    }
    if (
      e.type === "peekSurface" &&
      e.id === peek.value?.id &&
      e.ticket === peekTicket.value
    )
      peekSurface.value = { ticket: e.ticket, image: e.surface || null };
    if (
      e.type === "peekMoving" &&
      e.id === peek.value?.id &&
      e.ticket === peekTicket.value
    )
      peekMoving.value = e.ticket;
    if (
      e.type === "peekBackground" &&
      e.id === peek.value?.id &&
      (e.pane === 0 || e.pane === 1) &&
      e.background
    ) {
      peekBackground.value[e.pane] = e.background;
      if (e.backgroundReady) peekBackgroundReady.value = true;
    }
    if (e.type === "state" && e.state) {
      const sources = (s: typeof state.value) => new Map([...s.tabs, ...(s.pins || []), ...s.favorites].map(t => [t.id, t.url]));
      const previous = sources(state.value);
      state.value = upgradeLibrary(e.state);
      const current = sources(state.value);
      thumbnails.value = Object.fromEntries(
        Object.entries(thumbnails.value).filter(([id]) => current.has(id) && current.get(id) === previous.get(id)),
      );
      // Sound is runtime state; a closed tab must not leave a stale speaker.
      const live = new Set(state.value.tabs.map((tab) => tab.id));
      audio.value = Object.fromEntries(
        Object.entries(audio.value).filter(([id, playing]) => playing && live.has(id)),
      );
    }
    if (e.type === "history" && e.entries) history.value = e.entries;
    if (e.type === "suggestions" && e.ticket === suggestTicket && e.suggestions && settings.value.searchSuggestions) suggestions.value = e.suggestions;
    if (e.type === "dataCleared") { dataStatus.value = e.message || "已清除"; if (e.action === "history") history.value = []; }
    if (e.type === "error") error.value = e.message || "操作失败";
    if (e.type === "glassUnavailable") glassBlocked.value = true;
    if (e.type === "findOpen") {
      resetFind();
      findOpen.value = true;
      findTicket.value++;
    }
    if (e.type === "findFocus") findFocusTicket.value++;
    if (e.type === "findClosed") resetFind();
    if (e.type === "findMatches") {
      findCount.value = typeof e.count === "number" ? e.count : 0;
      findActive.value = typeof e.active === "number" ? e.active : 0;
    }
    if (e.type === "zoom" && typeof e.factor === "number" && Number.isFinite(e.factor))
      pageZoom.value = e.factor;
    if (e.type === "zoomNotice" && typeof e.factor === "number" && Number.isFinite(e.factor)) {
      pageZoom.value = e.factor;
      showZoomNotice(e.factor);
    }
    if (e.type === "audio" && e.id && typeof e.playing === "boolean")
      audio.value = { ...audio.value, [e.id]: e.playing };
    if (e.type === "downloads" && e.items) downloads.value = e.items;
    if (e.type === "downloadStarted" && e.id) {
      if (state.value.sidebarCollapsed) send({ type: "sidebar" });
      downloadsOpen.value = true;
      downloadsFocus.value = e.id;
    }
    if (e.type === "toast" && e.message) {
      notify(e.message, e.action, e.action === "undoBatch" ? 4000 : 2400);
      if (e.action === "undoBatch") armUndoWindow();
    }
    if (e.type === "shortcut") {
      if (e.action === "address") openCommand("edit");
      if (e.action === "new") openCommand();
      if (e.action === "desk") deskOpen.value = !deskOpen.value;
      if (e.action === "settings") openSettings();
      if (e.action === "downloads") toggleDownloads();
      if (e.action === "downloadsEscape") closeDownloads();
    }
  };
  let suggestTicket = 0;
  watch(() => [settings.value.searchSuggestions, settings.value.searchEngine], () => { suggestTicket++; suggestions.value = []; });
  watch(() => state.value.activeTab, (id) => {
    if (!id) return;
    recent.value = [id, ...recent.value.filter((item) => item !== id)].slice(0, 24);
  }, { immediate: true });
  // The host ends the find session on any tab/space transition as well; the UI
  // half hides the bar here so the floating layer and the page agree at once.
  watch(() => state.value.activeTab, () => {
    if (!findOpen.value) return;
    if (!native || popupWindow) closeFind();
    else resetFind();
  });
  function suggest(query: string) {
    suggestions.value = [];
    const ticket = ++suggestTicket;
    if (settings.value.searchSuggestions && settings.value.searchEngine === "google") send({ type: "suggest", query, ticket });
    return ticket;
  }
  // Only transient UI state crosses the trusted popup bridge. Each renderer keeps
  // its own DOM; native browser state continues to come from the host.
  let receivingPopup = false;
  const popupFields = { commandOpen, commandMode, deskOpen, settingsOpen, contextMenu, popupArea, hoverPreview, previewTarget, previewAnimationDone, error };
  function receivePopup(patch: Record<string, unknown>) {
    receivingPopup = true;
    for (const [key, value] of Object.entries(patch)) {
      if (key in popupFields) (popupFields[key as keyof typeof popupFields] as { value: unknown }).value = value ?? undefined;
    }
    receivingPopup = false;
  }
  for (const [key, value] of Object.entries(popupFields)) {
    watch(value, (next) => {
      if (!native || receivingPopup) return;
      // Cached favicons are already in native browser state and can exceed the
      // IPC message limit. Menus only need the saved item/controller identity.
      const patch = key === "contextMenu" && next
        ? JSON.parse(JSON.stringify(next, (name, value) => name === "favicon" ? undefined : value))
        : next ?? null;
      send({ type: "popupState", popup: { [key]: patch } });
    }, { deep: true, flush: "sync" });
  }
  send({ type: popupWindow ? "popupReady" : "ready" });
  return {
    native,
    popupWindow,
    contextMenu,
    popupArea,
    splitResize,
    hoverPreview,
    previewAnimationDone,
    thumbnails,
    previewTarget,
    previewHiddenTicket,
    previewDismissals,
    previewPresentedTicket,
    previewFailures,
    peek,
    peekBackground,
    peekBackgroundReady,
    peekOrigin,
    peekPhase,
    peekTicket,
    peekMoving,
    peekSurface,
    state,
    settings,
    shortcutLabel,
    shortcutTitle,
    dataStatus,
    openSettings,
    history,
    suggestions,
    recent,
    error,
    glassBlocked,
    findOpen,
    findQuery,
    findCount,
    findActive,
    findTicket,
    findFocusTicket,
    openFind,
    closeFind,
    setFindQuery,
    findStep,
    pageZoom,
    audio,
    zoomNotice,
    zoom,
    toggleMute,
    actionToast,
    notify,
    dismissToast,
    undoAvailable,
    undoBatch,
    downloads,
    downloadsOpen,
    downloadsFocus,
    activeDownloads,
    toggleDownloads,
    closeDownloads,
    downloadAction,
    now,
    active,
    space,
    tabs,
    pinnedTabs,
    activeBookmark,
    commandOpen,
    commandMode,
    deskOpen,
    settingsOpen,
    dragging,
    overlay,
    send,
    openCommand,
    cycle,
    suggest,
  };
});
