import { computed, ref, watch } from "vue";
import { defineStore } from "pinia";
import { initialState, reduce } from "../lib/preview";
import type {
  Command,
  HistoryEntry,
  Tab,
  Rect,
  Favorite,
  Point,
  PeekPhase,
} from "../lib/types";
import { address, host } from "../lib/address";
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
    glassBlocked = ref(false);
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
        state.value = reduce(
          structuredClone(JSON.parse(JSON.stringify(state.value))),
          c,
        );
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
    }
    if (e.type === "history" && e.entries) history.value = e.entries;
    if (e.type === "suggestions" && e.ticket === suggestTicket && e.suggestions && settings.value.searchSuggestions) suggestions.value = e.suggestions;
    if (e.type === "dataCleared") { dataStatus.value = e.message || "已清除"; if (e.action === "history") history.value = []; }
    if (e.type === "error") error.value = e.message || "操作失败";
    if (e.type === "glassUnavailable") glassBlocked.value = true;
    if (e.type === "shortcut") {
      if (e.action === "address") openCommand("edit");
      if (e.action === "new") openCommand();
      if (e.action === "desk") deskOpen.value = !deskOpen.value;
      if (e.action === "settings") openSettings();
    }
  };
  let suggestTicket = 0;
  watch(() => [settings.value.searchSuggestions, settings.value.searchEngine], () => { suggestTicket++; suggestions.value = []; });
  watch(() => state.value.activeTab, (id) => {
    if (!id) return;
    recent.value = [id, ...recent.value.filter((item) => item !== id)].slice(0, 24);
  }, { immediate: true });
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
