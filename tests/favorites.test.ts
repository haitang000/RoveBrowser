import { describe, expect, it } from "vitest";
import { favorite, favoriteUrl } from "../src/lib/favorites";
import { initialState, reduce } from "../src/lib/preview";

describe("top favorites", () => {
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
