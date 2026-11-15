import { computed, effect, reactive } from "vue";
import { expect, it } from "vitest";
import { applySnapshot, expandSnapshot } from "../src/lib/state-snapshot";

it("a tab change updates its consumers without rerunning unrelated effects", () => {
  const state = reactive({ activeTab: "a", tabs: [{ id: "a", title: "First" }, { id: "b", title: "Second" }], settings: { reduceMotion: false }, spaces: [{ id: "home", name: "Home" }] });
  const first = state.tabs[0], second = state.tabs[1], spaces = state.spaces, settings = state.settings;
  let settingRuns = 0, spaceRuns = 0, title = "";
  effect(() => { settingRuns++; void state.settings.reduceMotion; });
  effect(() => { spaceRuns++; void state.spaces[0].name; });
  const active = computed(() => state.tabs.find(tab => tab.id === state.activeTab));
  effect(() => { title = active.value!.title; });
  applySnapshot(state, { ...JSON.parse(JSON.stringify(state)), activeTab: "b" });
  expect(title).toBe("Second");
  expect(state.tabs[0]).toBe(first);
  expect(state.tabs[1]).toBe(second);
  expect(state.spaces).toBe(spaces);
  expect(state.settings).toBe(settings);
  expect([settingRuns, spaceRuns]).toEqual([1, 1]);
  const incoming = JSON.parse(JSON.stringify(state));
  incoming.tabs[1].title = "Updated";
  applySnapshot(state, incoming);
  expect(title).toBe("Updated");
  expect(state.tabs[0]).toBe(first);
  expect([settingRuns, spaceRuns]).toEqual([1, 1]);
});

it("reordering, removal and nested preference changes retain the correct identity", () => {
  const state = reactive<{ tabs: { id: string; icon?: string }[]; newTab: { widgets: { id: string; text?: string }[] }; optional?: string }>({ tabs: [{ id: "a", icon: "old" }, { id: "b" }], newTab: { widgets: [{ id: "notes", text: "old" }] }, optional: "remove" });
  const second = state.tabs[1];
  applySnapshot(state, { tabs: [{ id: "b" }, { id: "a" }], newTab: { widgets: [{ id: "notes", text: "new" }] } });
  expect(state.tabs[0]).toBe(second);
  expect(state.tabs[1]).toEqual({ id: "a" });
  expect(state).not.toHaveProperty("optional");
  expect(state.newTab.widgets[0].text).toBe("new");
  applySnapshot(state, { tabs: [], newTab: { widgets: [] } });
  expect(state.tabs).toEqual([]);
  expect(state.newTab.widgets).toEqual([]);
});

it("native patches preserve unchanged rows, replace complete rows, and remove stale fields", () => {
  const before = { activeTab: "a", sidebarWidth: 300, tabs: [{ id: "a", title: "A", favicon: "old" }, { id: "b", title: "B" }], pins: [{ id: "saved" }] };
  const next = expandSnapshot(before, { type: "statePatch", fields: { activeTab: "b" }, removed: ["sidebarWidth"], lists: { tabs: { upsert: [{ id: "a", title: "Changed" } as { id: string }] } } });
  expect(next.tabs).toEqual([{ id: "a", title: "Changed" }, { id: "b", title: "B" }]);
  expect(next.tabs[1]).toBe(before.tabs[1]);
  expect(next.pins).toBe(before.pins);
  expect(next).not.toHaveProperty("sidebarWidth");
  expect(before.tabs[0].favicon).toBe("old");
  const reordered = expandSnapshot(next, { type: "statePatch", fields: {}, removed: [], lists: { tabs: { upsert: [{ id: "new" }], order: ["new", "b"] }, pins: { upsert: [], order: [] } } });
  expect(reordered.tabs.map(row => row.id)).toEqual(["new", "b"]);
  expect(reordered.tabs[1]).toBe(before.tabs[1]);
  expect(reordered.pins).toEqual([]);
});
