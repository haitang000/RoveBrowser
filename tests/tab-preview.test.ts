import { describe, expect, it } from "vitest";
import { initialState } from "../src/lib/preview";
import { previewFrame, previewPages } from "../src/lib/tab-preview";

describe("hover preview groups", () => {
  it("excludes both halves of the active split even when only one has a partner pointer", () => {
    const state = initialState();
    state.tabs.find((t) => t.id === "start")!.splitWith = "wiki";
    expect(previewPages(state, "pin-wiki")).toEqual([]);
    expect(previewPages(state, "start")).toEqual([]);
    expect(previewPages(state, "pin-github")).toHaveLength(1);
  });
  it("resolves an unloaded saved split without creating tabs, and keeps its two captions", () => {
    const state = initialState();
    state.pins![0].splitWith = state.pins![1].id;
    state.pins![1].splitWith = state.pins![0].id;
    state.tabs = state.tabs.filter((t) => t.id !== "wiki");
    const pages = previewPages(state, "pin-wiki");
    expect(pages.map((p) => p.key)).toEqual(["pin-github", "pin-wiki"]);
    expect(pages[0].tabId).toBe("github");
    expect(pages[1].tabId).toBeUndefined();
    expect(state.tabs).toHaveLength(2);
  });
  it("uses a saved title and the live page's current URL", () => {
    const state = initialState();
    state.tabs.find((t) => t.id === "wiki")!.url += "/wiki/Browser";
    expect(previewPages(state, "pin-wiki")[0]).toMatchObject({ title: "Wikipedia", url: "https://en.wikipedia.org/wiki/Browser" });
    state.activeSpace = "work";
    expect(previewPages(state, "pin-wiki")).toEqual([]);
    expect(previewPages(state, "favorite-figma")).toHaveLength(1);
    expect(previewPages(state, "deleted")).toEqual([]);
  });
});

describe("preview placement", () => {
  const sidebar = { x: 0, y: 32, width: 240, height: 788 };
  const page = { x: 240, y: 32, width: 1040, height: 788 };
  it("fits a narrow Windows window and a row near the bottom", () => {
    const frame = previewFrame({ x: 6, y: 760, width: 228, height: 30 }, sidebar, page, { width: 480, height: 820 })!;
    expect(frame.width).toBe(216);
    expect(frame.x + frame.width).toBeLessThanOrEqual(480 - 16);
    expect(frame.y + frame.height).toBeLessThanOrEqual(820 - 16);
    expect(frame.y).toBeGreaterThanOrEqual(48);
  });
  it("caps portrait previews and avoids covering the titlebar", () => {
    const frame = previewFrame({ x: 6, y: 60, width: 50, height: 36 }, sidebar, { ...page, width: 100 }, { width: 1280, height: 820 })!;
    expect(frame).toMatchObject({ x: 248, y: 48, width: 320, height: 336 });
    expect(previewFrame(page, sidebar, page, { width: 320, height: 400 })).toBeUndefined();
  });
});
