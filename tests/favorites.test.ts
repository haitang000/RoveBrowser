import { describe, expect, it } from "vitest";
import { favorite, favoriteUrl } from "../src/lib/favorites";
import { initialState, reduce } from "../src/lib/preview";
import { upgradeLibrary } from "../src/lib/bookmarks";

describe("top favorites", () => {
  it("caps all addition paths at eight without partial changes, including split pairs", () => {
    let s = initialState();
    expect(() => reduce(s, { type: "favoriteSave", title: "Ninth", url: "https://extra.example" })).toThrow("8");
    const before = structuredClone(s);
    expect(() => reduce(s, { type: "favoriteTab", id: "wiki" })).toThrow("8");
    expect(s).toEqual(before);
    s.favorites.pop();
    s = reduce(s, { type: "new", url: "https://example.com/a" });
    const a = s.activeTab;
    s = reduce(s, { type: "new", url: "https://example.com/b" });
    s = reduce(s, { type: "split", id: a, other: s.activeTab });
    const split = structuredClone(s);
    expect(() => reduce(s, { type: "favoriteTab", id: a })).toThrow("8");
    expect(s).toEqual(split);
    s.favorites.pop();
    s = reduce(s, { type: "favoriteTab", id: a });
    expect(s.favorites).toHaveLength(8);
    s = reduce(s, { type: "favoriteTab", id: s.favorites.at(-1)!.id, before: s.favorites[0].id });
    expect(s.favorites).toHaveLength(8);
  });

  it("preserves legacy overflow as workspace pins without splitting saved pairs", () => {
    const s = initialState();
    const overflow = { id: "overflow", title: "Extra", url: "https://extra.example", splitWith: s.favorites[7].id };
    s.favorites[7].splitWith = overflow.id;
    s.favorites.push(overflow);
    upgradeLibrary(s);
    expect(s.favorites).toHaveLength(7);
    expect(s.pins!.slice(-2).map((f) => [f.id, f.spaceId])).toEqual([["favorite-linear", "personal"], ["overflow", "personal"]]);
    const count = s.pins!.length;
    upgradeLibrary(s);
    expect(s.pins).toHaveLength(count);
  });

  it("keeps cached icons through unloads and same-origin edits and clears them for another site", () => {
    let s = initialState();
    const f = s.favorites[0];
    f.favicon = "data:image/png;base64,cached";
    s = reduce(s, { type: "bookmarkOpen", id: f.id });
    expect(s.tabs.find((t) => t.id === s.activeTab)?.favicon).toBe(f.favicon);
    s = reduce(s, { type: "bookmarkUnload", id: f.id });
    expect(s.favorites[0].favicon).toBe(f.favicon);
    s = reduce(s, { type: "bookmarkUpdate", id: f.id, title: "GitHub", url: "https://github.com/explore" });
    expect(s.favorites[0].favicon).toBe(f.favicon);
    s = reduce(s, { type: "bookmarkUpdate", id: f.id, title: "Other", url: "https://example.com" });
    expect(s.favorites[0].favicon).toBeUndefined();
  });
  it("normalizes addresses without turning invalid input into a search", () => {
    expect(favorite("  文档  ", "example.com/docs")).toEqual({ title: "文档", url: "https://example.com/docs" });
    expect(favorite("", "http://localhost:8080/").title).toBe("localhost");
    for (const url of ["", "search terms", "javascript:alert(1)", "file:///C:/Windows", "era://newtab", "https://user:pass@example.com"])
      expect(() => favoriteUrl(url)).toThrow();
  });

  it("edits in place, orders favorites, and allows removing the entire default list", () => {
    let s = initialState();
    const previousUrl = s.favorites[0].url;
    s = reduce(s, { type: "favoriteSave", title: "文档", url: "example.com", previousUrl });
    expect(s.favorites[0]).toMatchObject({ id: "favorite-github", title: "文档", url: "https://example.com/" });
    s = reduce(s, { type: "favoriteMove", url: "https://example.com/", offset: 1 });
    expect(s.favorites[1].title).toBe("文档");
    for (const { url } of [...s.favorites]) s = reduce(s, { type: "favoriteRemove", url });
    expect(s.favorites).toEqual([]);
    s = reduce(s, { type: "favoriteSave", title: "", url: "https://example.com/" });
    expect(s.favorites).toMatchObject([{ title: "example.com", url: "https://example.com/" }]);
  });

  it("rejects canonical duplicates and stale edits without changing the list", () => {
    const s = initialState();
    const before = structuredClone(s.favorites);
    expect(() => reduce(s, { type: "favoriteSave", title: "Duplicate", url: "https://GITHUB.com:443/" })).toThrow("已在");
    expect(() => reduce(s, { type: "favoriteSave", title: "Changed", url: "example.com", previousUrl: "missing" })).toThrow("已被移除");
    expect(s.favorites).toEqual(before);
  });
});
