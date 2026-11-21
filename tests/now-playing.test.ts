import { effectScope, nextTick, reactive, ref } from "vue";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useNowPlaying } from "../src/lib/now-playing";
import type { Command, Tab } from "../src/lib/types";

const scopes: ReturnType<typeof effectScope>[] = [];
afterEach(() => {
  for (const scope of scopes.splice(0)) scope.stop();
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

function fixture() {
  vi.stubGlobal("window", globalThis);
  const tabs = reactive<Tab[]>(["a", "b", "c", "d", "e"].map((id) => ({
    id, spaceId: "personal", title: id, url: `https://example.com/${id}`, pinned: false,
  })));
  const audio = reactive(new Set<string>());
  const selected = ref<string[]>([]);
  const send = vi.fn<(command: Command) => void>();
  const scope = effectScope();
  scopes.push(scope);
  const controls = scope.run(() => useNowPlaying(() => tabs, (id) => audio.has(id), () => selected.value, send))!;
  return { tabs, audio, selected, controls, send, ids: () => controls.cards.value.map((tab) => tab.id) };
}

describe("Now Playing", () => {
  it("keeps start order and fills the three slots with background tabs", async () => {
    const { audio, ids, selected } = fixture();
    for (const id of ["a", "b", "c", "d", "e"]) {
      audio.add(id);
      await nextTick();
    }
    expect(ids()).toEqual(["e", "d", "c"]);
    selected.value = ["e", "c"];
    expect(ids()).toEqual(["d", "b", "a"]);
    selected.value = [];
    expect(ids()).toEqual(["e", "d", "c"]);
    audio.delete("d");
    await nextTick();
    expect(ids()).toEqual(["e", "c", "b"]);
  });

  it("hides through playback changes and fragment jumps, and resets on a different page", async () => {
    const { tabs, audio, controls, ids } = fixture();
    audio.add("a");
    await nextTick();
    controls.hide(tabs[0]);
    audio.delete("a");
    await nextTick();
    audio.add("a");
    tabs[0].url += "#chapter";
    await nextTick();
    expect(ids()).toEqual([]);
    tabs[0].url = "https://example.com/next";
    await nextTick();
    expect(ids()).toEqual(["a"]);
  });

  it("waits for a successful pause and keeps it through the delayed audio event", async () => {
    const { tabs, audio, controls, send, ids } = fixture();
    audio.add("a");
    await nextTick();
    controls.toggle(tabs[0]);
    controls.toggle(tabs[0]);
    expect(send).toHaveBeenCalledTimes(1);
    expect(controls.pending("a")).toBe(true);
    expect(controls.paused("a")).toBe(false);
    controls.complete("a", 1, "applied");
    expect(controls.paused("a")).toBe(true);
    audio.delete("a");
    await nextTick();
    expect(ids()).toEqual(["a"]);
    controls.toggle(tabs[0]);
    expect(send).toHaveBeenLastCalledWith({ type: "mediaControl", id: "a", url: tabs[0].url, action: "play", ticket: 2 });
    controls.complete("a", 2, "blocked");
    expect(controls.paused("a")).toBe(true);
    audio.add("a");
    await nextTick();
    expect(controls.paused("a")).toBe(false);
  });

  it("drops stale replies after navigation or reload and keeps unavailable media truthful", async () => {
    const { tabs, audio, controls } = fixture();
    audio.add("a");
    await nextTick();
    controls.toggle(tabs[0]);
    controls.complete("a", 1, "unavailable");
    expect(controls.paused("a")).toBe(false);
    controls.toggle(tabs[0]);
    tabs[0].url = "https://example.com/next";
    await nextTick();
    controls.complete("a", 2, "applied");
    expect(controls.paused("a")).toBe(false);
    controls.toggle(tabs[0]);
    controls.complete("a", 3, "applied");
    tabs[0].loading = true;
    await nextTick();
    expect(controls.paused("a")).toBe(false);
  });

  it("retains a resuming card during the audio delay, then frees its slot if playback stays quiet", async () => {
    vi.useFakeTimers();
    const { tabs, audio, controls, ids } = fixture();
    audio.add("a");
    await nextTick();
    controls.toggle(tabs[0]);
    controls.complete("a", 1, "applied");
    audio.delete("a");
    await nextTick();
    controls.toggle(tabs[0]);
    controls.complete("a", 2, "applied");
    expect(ids()).toEqual(["a"]);
    vi.advanceTimersByTime(1500);
    await nextTick();
    expect(ids()).toEqual([]);
    audio.add("b");
    await nextTick();
    audio.add("a");
    await nextTick();
    expect(ids()).toEqual(["a", "b"]);
  });

  it("keeps the previous colors while a new cover loads and drops replies from the old page", async () => {
    const { tabs, audio, controls, send } = fixture();
    const colors: [[number, number, number], [number, number, number], [number, number, number]] =
      [[110, 20, 200], [50, 10, 80], [150, 90, 230]];
    audio.add("a");
    await nextTick();
    controls.requestDetails(tabs[0]);
    controls.requestDetails(tabs[0]);
    expect(send).toHaveBeenCalledTimes(1);
    controls.receiveDetails("a", 1, { title: "First track", colors, sampling: false });
    controls.requestDetails(tabs[0]);
    controls.receiveDetails("a", 2, { title: "Next track", colors: null, sampling: true });
    expect(controls.details.get("a")).toEqual({ title: "Next track", colors });
    controls.requestDetails(tabs[0]);
    tabs[0].url = "https://example.com/different";
    await nextTick();
    controls.receiveDetails("a", 3, { title: "Old page", colors, sampling: false });
    expect(controls.details.get("a")).toBeUndefined();
  });
});
