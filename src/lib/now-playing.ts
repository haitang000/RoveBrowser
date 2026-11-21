// Card selection adapted from Zigzag's NowPlayingChoice, Copyright 2026 Zigao Wang, Apache-2.0.
import { computed, onScopeDispose, reactive, shallowRef, watch } from "vue";
import type { Command, Tab } from "./types";

export type MediaOutcome = "applied" | "blocked" | "unavailable" | "cancelled";
type RGB = [number, number, number];
export interface NowPlayingDetails {
  title: string | null;
  colors: [RGB, RGB, RGB] | null;
  sampling: boolean;
}
type Paused = { phase: "paused"; heardQuiet: boolean };
type Playback = Paused
  | { phase: "pending"; ticket: number; page: string; previous?: Paused; timer: number }
  | { phase: "starting"; timer: number };

const maxCards = 3;
const pageOf = (url: string) => url.split("#", 1)[0];

export function useNowPlaying(
  tabs: () => Tab[],
  audible: (id: string) => boolean,
  selected: () => (string | undefined)[],
  send: (command: Command) => void,
) {
  const actions = reactive(new Map<string, Playback>());
  const hidden = reactive(new Map<string, string>());
  const order = shallowRef<string[]>([]);
  const details = reactive(new Map<string, Omit<NowPlayingDetails, "sampling">>());
  const detailRequests = new Map<string, { page: string; ticket: number; deadline: number }>();
  let serial = 0;

  function clear(id: string) {
    const action = actions.get(id);
    if (action && "timer" in action) window.clearTimeout(action.timer);
    actions.delete(id);
  }
  onScopeDispose(() => { for (const id of actions.keys()) clear(id); });

  const candidates = computed(() => tabs().map((tab) => ({
    tab,
    page: pageOf(tab.url),
    audible: audible(tab.id),
    loaded: !tab.unloaded,
    loading: Boolean(tab.loading),
  })));
  watch(candidates, (current, previous) => {
    const byId = new Map(current.map((item) => [item.tab.id, item]));
    const before = new Map(previous?.map((item) => [item.tab.id, item]));
    for (const [id, page] of hidden) {
      if (byId.get(id)?.page !== page) hidden.delete(id);
    }
    for (const id of new Set([...details.keys(), ...detailRequests.keys()])) {
      const item = byId.get(id);
      const prior = before.get(id);
      if (!item?.loaded || prior?.page !== item.page || (item.loading && !prior.loading)
          || (!item.audible && !actions.has(id))) {
        detailRequests.delete(id);
        details.delete(id);
      }
    }
    for (const [id, action] of actions) {
      const item = byId.get(id);
      const prior = before.get(id);
      if (!item?.loaded || prior?.page !== item.page || (item.loading && !prior.loading)) {
        clear(id);
      } else if (action.phase === "paused") {
        // WebView2 can still report audio for a moment after a DOM pause.
        if (!item.audible) action.heardQuiet = true;
        else if (action.heardQuiet) clear(id);
      } else if (action.phase === "starting" && item.audible) {
        clear(id);
      }
    }
  }, { immediate: true });

  const sounding = computed(() => new Map(candidates.value
    .filter(({ tab, audible, loaded }) => loaded && (audible || actions.has(tab.id)))
    .map(({ tab }) => [tab.id, tab])));
  watch(sounding, (current) => {
    const retained = order.value.filter((id) => current.has(id));
    const known = new Set(retained);
    const added = [...current.values()].reverse()
      .filter(({ id }) => !known.has(id))
      .sort((a, b) => (a.lastActivated ?? 0) - (b.lastActivated ?? 0));
    order.value = [...retained, ...added.map(({ id }) => id)];
  }, { immediate: true });

  const cards = computed(() => {
    const visible = new Set(selected());
    return order.value.flatMap((id) => {
      const tab = sounding.value.get(id);
      return tab && !visible.has(id) && hidden.get(id) !== pageOf(tab.url) ? [tab] : [];
    }).slice(-maxCards).reverse();
  });

  function complete(id: string, ticket: number, outcome: MediaOutcome) {
    const pending = actions.get(id);
    if (pending?.phase !== "pending" || pending.ticket !== ticket) return;
    clear(id);
    const item = candidates.value.find(({ tab }) => tab.id === id);
    if (!item?.loaded || item.page !== pending.page || outcome === "cancelled") return;
    if (outcome === "applied") {
      if (!pending.previous) {
        actions.set(id, { phase: "paused", heardQuiet: !item.audible });
      } else if (!item.audible) {
        // Keep the card in place until the resumed audio is audible.
        actions.set(id, { phase: "starting", timer: window.setTimeout(() => clear(id), 1500) });
      }
    } else if (outcome === "blocked" && pending.previous) {
      actions.set(id, pending.previous);
    }
  }

  function toggle(tab: Tab) {
    const current = actions.get(tab.id);
    if (current?.phase === "pending") return;
    const previous = current?.phase === "paused" ? current : undefined;
    const ticket = ++serial;
    const page = pageOf(tab.url);
    clear(tab.id);
    actions.set(tab.id, {
      phase: "pending", ticket, page, previous,
      timer: window.setTimeout(() => complete(tab.id, ticket, "blocked"), 4000),
    });
    send({ type: "mediaControl", id: tab.id, url: page, action: previous ? "play" : "pause", ticket });
  }

  function requestDetails(tab: Tab) {
    if (!audible(tab.id)) return;
    const now = Date.now();
    const page = pageOf(tab.url);
    const pending = detailRequests.get(tab.id);
    if (pending?.page === page && now < pending.deadline) return;
    const ticket = ++serial;
    detailRequests.set(tab.id, { ticket, page, deadline: now + 4000 });
    send({ type: "nowPlayingDetails", id: tab.id, url: page, ticket });
  }

  function receiveDetails(id: string, ticket: number, incoming?: NowPlayingDetails | null) {
    const pending = detailRequests.get(id);
    if (pending?.ticket !== ticket) return;
    detailRequests.delete(id);
    const item = candidates.value.find(({ tab }) => tab.id === id);
    if (!item?.loaded || item.page !== pending.page || incoming === undefined) return;
    const previous = details.get(id);
    const title = incoming?.title || null;
    const colors = incoming?.sampling ? previous?.colors ?? null : incoming?.colors ?? null;
    if (title === (previous?.title ?? null) && String(colors) === String(previous?.colors ?? null)) return;
    if (title || colors) details.set(id, { title, colors });
    else details.delete(id);
  }

  return {
    cards, toggle, complete, details, requestDetails, receiveDetails,
    hide: (tab: Tab) => hidden.set(tab.id, pageOf(tab.url)),
    paused: (id: string) => {
      const action = actions.get(id);
      return action?.phase === "paused" || (action?.phase === "pending" && Boolean(action.previous));
    },
    pending: (id: string) => actions.get(id)?.phase === "pending",
  };
}
