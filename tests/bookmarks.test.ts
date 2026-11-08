import { expect, it } from "vitest";
import { initialState, reduce } from "../src/lib/preview";
import { bookmarkTitle, liveTab } from "../src/lib/bookmarks";

it("unloading preserves a pin's saved address and title; reopening starts fresh", () => {
  let s = initialState();
  const saved = { ...s.pins![1] };
  s = reduce(s, { type: "activate", id: "wiki" });
  s = reduce(s, { type: "navigate", url: "https://en.wikipedia.org/wiki/Browser" });
  expect(s.pins![1]).toEqual(saved);
  s = reduce(s, { type: "close", id: "wiki" });
  expect(s.closed).toHaveLength(0);
  expect(s.pins).toContainEqual(saved);
  s = reduce(s, { type: "bookmarkOpen", id: saved.id });
  expect(liveTab(s, saved.id)).toMatchObject({ title: saved.title, url: saved.url });
  expect(s.activeTab).not.toBe("wiki");
});

it("promotion, cross-space favorite activation, and unpinning keep live identity", () => {
  let s = reduce(initialState(), { type: "new", url: "https://example.com/docs" });
  s.favorites.pop();
  const id = s.activeTab;
  s = reduce(s, { type: "favoriteTab", id });
  const f = s.favorites.at(-1)!;
  expect(s.tabs.find((t) => t.id === id)?.bookmarkId).toBe(f.id);
  s = reduce(s, { type: "space", id: "work" });
  s = reduce(s, { type: "bookmarkOpen", id: f.id });
  expect(s.activeTab).toBe(id);
  expect(s.activeSpace).toBe("work");
  expect(s.tabs.filter((t) => t.bookmarkId === f.id)).toHaveLength(1);
  s = reduce(s, { type: "bookmarkUnpin", id: f.id });
  expect(s.activeTab).toBe(id);
  expect(s.tabs.find((t) => t.id === id)).toMatchObject({ pinned: false, spaceId: "work" });
  expect(s.tabs.find((t) => t.id === id)?.bookmarkId).toBeUndefined();
});

it("separating a navigated page leaves the saved item intact and reuses the live tab", () => {
  let s = reduce(initialState(), { type: "activate", id: "wiki" });
  const mark = s.tabs.find((t) => t.id === "wiki")!.bookmarkId!;
  s = reduce(s, { type: "navigate", url: "https://en.wikipedia.org/wiki/Browser" });
  s = reduce(s, { type: "bookmarkSeparate", id: mark });
  expect(s.activeTab).toBe("wiki");
  expect(s.tabs.find((t) => t.id === "wiki")?.bookmarkId).toBeUndefined();
  expect(s.pins!.find((f) => f.id === mark)?.url).toBe("https://en.wikipedia.org");
  s = reduce(s, { type: "bookmarkOpen", id: mark });
  expect(s.activeTab).not.toBe("wiki");
  expect(s.tabs.find((t) => t.id === "wiki")?.url).toContain("/wiki/Browser");
});

it("pinning a regular split saves both pages and restores their pair", () => {
  let s = reduce(initialState(), { type: "new", url: "https://example.com/a" });
  const a = s.activeTab;
  s = reduce(s, { type: "new", url: "https://example.com/b" });
  const b = s.activeTab;
  s = reduce(s, { type: "split", id: a, other: b });
  s = reduce(s, { type: "pin", id: a });
  const mark = s.tabs.find((t) => t.id === a)!.bookmarkId!;
  expect(s.pins!.find((f) => f.id === mark)?.splitWith).toBeTruthy();
  s = reduce(s, { type: "bookmarkUnload", id: mark });
  expect(s.tabs.some((t) => t.id === a || t.id === b)).toBe(false);
  s = reduce(s, { type: "bookmarkOpen", id: mark });
  expect(s.tabs.find((t) => t.id === s.activeTab)?.splitWith).toBeTruthy();
});

it("uses Zigzag's home-page title rule without shortening deep-page titles", () => {
  expect(bookmarkTitle("Inbox (3) - Gmail", "https://gmail.com/")).toBe("Gmail");
  expect(bookmarkTitle("Project - GitHub", "https://github.com/team/project")).toBe("Project - GitHub");
});
