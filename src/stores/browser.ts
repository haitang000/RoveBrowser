import { computed, ref } from "vue";
import { defineStore } from "pinia";
import { initialState, reduce } from "../lib/preview";
import type {
  Command,
  HistoryEntry,
  Tab,
  Point,
  PeekPhase,
} from "../lib/types";
import { address, host } from "../lib/address";
import { liveTab, savedPage, upgradeLibrary } from "../lib/bookmarks";
export const useBrowser = defineStore("browser", () => {
  const native = Boolean(window.__ERA_NATIVE__ && window.ipc);
  const state = ref(upgradeLibrary((native && window.__ERA_INITIAL_STATE__) || initialState())),
    history = ref<HistoryEntry[]>([]),
    error = ref("");
  const commandOpen = ref(false),
    commandBackdrop = ref<{ ticket: number; images: (string | null)[]; ready: boolean; shown: boolean } | null>(null),
    thumbnails = ref<Record<string, string>>({}),
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
    favoritesOpen = ref(false),
    favoriteEditId = ref<string>(),
    dragging = ref(false);
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
    .map((f) => ({ ...(liveTab(state.value, f.id) || { id: f.id, spaceId: f.spaceId!, url: f.url, unloaded: true }), title: f.title, pinned: true, bookmarkId: f.id })));
  const activeBookmark = computed(() => active.value?.bookmarkId ? savedPage(state.value, active.value.bookmarkId) : undefined);
  const overlay = computed(
    () =>
      commandOpen.value ||
      deskOpen.value ||
      settingsOpen.value ||
      favoritesOpen.value ||
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
        "overlay",
        "thumbnails",
        "windowChrome",
        "commandBackdropReady",
        "commandBackdropFailed",
      ].includes(c.type)
    )
      error.value = "";
    try {
      if (native) window.ipc!.postMessage(JSON.stringify(c));
      else if (c.type === "peekOpen") {
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
    if (e.type === "commandBackdrop") {
      commandBackdrop.value = e.backgrounds && e.ticket !== undefined
        ? { ticket: e.ticket, images: e.backgrounds, ready: Boolean(e.backgroundReady), shown: false }
        : null;
    }
    if (e.type === "commandPresented" && commandBackdrop.value?.ticket === e.ticket)
      commandBackdrop.value!.shown = true;
    if (e.type === "commandDismissed") commandOpen.value = false;
    if (e.type === "thumbnails" && e.images) thumbnails.value = e.images;
    if (e.type === "thumbnail" && e.id) {
      const allowed = new Set(e.kept || Object.keys(thumbnails.value));
      const next = Object.fromEntries(
        Object.entries(thumbnails.value).filter(([id]) => allowed.has(id)),
      );
      if (e.image) next[e.id] = e.image;
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
      state.value = upgradeLibrary(e.state);
      const ids = new Set(e.state.tabs.map((t) => t.id));
      thumbnails.value = Object.fromEntries(
        Object.entries(thumbnails.value).filter(([id]) => ids.has(id)),
      );
    }
    if (e.type === "history" && e.entries) history.value = e.entries;
    if (e.type === "error") error.value = e.message || "操作失败";
    if (e.type === "shortcut") {
      if (e.action === "address") openCommand("edit");
      if (e.action === "new") openCommand();
      if (e.action === "desk") deskOpen.value = !deskOpen.value;
    }
  };
  send({ type: "ready" });
  return {
    native,
    thumbnails,
    peek,
    peekBackground,
    peekBackgroundReady,
    peekOrigin,
    peekPhase,
    peekTicket,
    peekMoving,
    peekSurface,
    state,
    history,
    error,
    active,
    space,
    tabs,
    pinnedTabs,
    activeBookmark,
    commandOpen,
    commandBackdrop,
    commandMode,
    deskOpen,
    settingsOpen,
    favoritesOpen,
    favoriteEditId,
    dragging,
    overlay,
    send,
    openCommand,
    cycle,
  };
});
