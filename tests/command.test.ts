import { describe, expect, it } from "vitest";
import { initialState, reduce } from "../src/lib/preview";
import {
  bestCompletion,
  commandContext,
  completionAllowed,
  completionFor,
  contextActions,
  matchSite,
  parseSuggestions,
  rankCommands,
  scopedResults,
  siteSearchUrl,
  suggestionQuery,
  tabsToDismiss,
  urlFrom,
} from "../src/lib/command";

const blank = { state: initialState(), recent: [] as string[], history: [], suggestions: [] as string[] };

describe("command addresses and site search", () => {
  it("opens real hosts and searches everything else", () => {
    expect(urlFrom("example.com")).toBe("https://example.com");
    expect(urlFrom("https://en.wikipedia.org/wiki/Browser")).toMatch(/\/wiki\/Browser$/);
    expect(urlFrom("localhost:8080")).toBe("http://localhost:8080");
    expect(urlFrom("readme.txt")).toBeUndefined();
    expect(urlFrom("node.js")).toBeUndefined();
    expect(urlFrom("javascript:alert(1)")).toBeUndefined();
    expect(urlFrom("user@example.com")).toBeUndefined();
  });
  it("matches a site name and leaves sentences as searches", () => {
    expect(matchSite("you")?.name).toBe("YouTube");
    expect(matchSite("yt")?.name).toBe("YouTube");
    expect(matchSite("gh")?.name).toBe("GitHub");
    expect(matchSite("yo")).toBeUndefined();
    expect(matchSite("youtube cats")).toBeUndefined();
    expect(matchSite("google maps")?.name).toBe("Google Maps");
    expect(siteSearchUrl(matchSite("yt")!, "cats")).toContain("search_query=cats");
    expect(suggestionQuery("cats")).toBe("cats");
    expect(suggestionQuery("example.com")).toBeUndefined();
    expect(parseSuggestions('["cats",["cats musical","cats movie"]]')).toEqual(["cats musical", "cats movie"]);
  });
});

describe("command ranking", () => {
  it("puts a named command ahead of search and a keyword match behind it", () => {
    const named = rankCommands("settings", blank);
    expect(named[0]).toMatchObject({ title: "Settings", action: "Run" });
    expect(named[1]).toMatchObject({ kind: { type: "search" } });
    const keyword = rankCommands("preferences", blank);
    expect(keyword[0]).toMatchObject({ kind: { type: "search", query: "preferences" } });
    expect(keyword.some((row) => row.title === "Settings")).toBe(true);
  });
  it("waits for three letters before offering pin or close", () => {
    const state = initialState();
    state.tabs.push(
      { id: "alpha", spaceId: "personal", title: "Alpha", url: "https://alpha.example/", pinned: false },
      { id: "beta", spaceId: "personal", title: "Beta", url: "https://beta.example/", pinned: false },
    );
    state.activeTab = "alpha";
    const input = { ...blank, state, recent: ["beta", "alpha", "start"] };
    expect(rankCommands("pi", input).some((row) => row.title === "Pin Tab")).toBe(false);
    expect(rankCommands("pin", input)[0]?.title).toBe("Pin Tab");
    expect(rankCommands("close other", input).some((row) => row.kind.type === "action" && row.kind.id === "close-others")).toBe(true);
  });
  it("limits empty input to recent tabs and appends suggestions", () => {
    const state = initialState();
    const recent = rankCommands("", { ...blank, state, recent: ["wiki", "github", "start"] });
    expect(recent.map((row) => row.title)).toEqual(["Wikipedia", "GitHub"]);
    const suggested = rankCommands("cats", { ...blank, suggestions: ["cats musical", "cats"] });
    expect(suggested.at(-1)?.title).toBe("cats musical");
    expect(suggested).toHaveLength(2);
  });
  it("searches inside a site and keeps that site's history when the query is empty", () => {
    const youtube = matchSite("you")!;
    expect(scopedResults(youtube, "cats", ["cats musical"], [])[0]).toMatchObject({ title: "cats", detail: "Search YouTube" });
    const home = scopedResults(youtube, "", [], [{ title: "A video", url: "https://www.youtube.com/watch?v=1" }]);
    expect(home[0]?.title).toBe("YouTube");
    expect(home[1]?.title).toBe("A video");
  });
});

describe("inline completion", () => {
  it("completes the site you use, and stops when text is deleted", () => {
    const match = bestCompletion("wiki", [{ url: "https://en.wikipedia.org/wiki/Type", score: 4 }]);
    expect(match).toEqual({ text: "wikipedia.org", url: "https://wikipedia.org/" });
    expect(completionFor("git", [], ["https://github.com/zigzag", "https://gist.github.com"])?.text).toBe("github.com");
    expect(bestCompletion("github.com", [{ url: "https://github.com/zigzag", score: 3 }])?.text).toBe("github.com");
    expect(completionAllowed("wik", "wikipedia.org", "wi", false, true, false)).toBe(true);
    expect(completionAllowed("wi", "wiki", "wik", true, true, false)).toBe(false);
  });
});

describe("site controls", () => {
  it("offers mute and the zoom ladder for the page and remembers mute state", () => {
    const state = initialState();
    state.tabs.push({
      id: "alpha",
      spaceId: "personal",
      title: "Alpha",
      url: "https://alpha.example/",
      pinned: false,
    });
    state.activeTab = "alpha";
    const actions = contextActions(commandContext(state, []));
    expect(actions.some((action) => action.id === "mute")).toBe(true);
    expect(actions.some((action) => action.id === "zoom-in")).toBe(true);
    expect(actions.some((action) => action.id === "zoom-out")).toBe(true);
    expect(actions.some((action) => action.id === "zoom-reset")).toBe(true);
    expect(commandContext(state, []).muted).toBe(false);
    state.tabs[state.tabs.length - 1]!.muted = true;
    expect(contextActions(commandContext(state, [])).some((action) => action.id === "unmute")).toBe(true);
    expect(commandContext(state, []).muted).toBe(true);
  });
});

describe("closing groups of tabs", () => {  it("closes other and lower regular tabs while keeping pins and the split partner", () => {
    let state = initialState();
    state.tabs.push(
      { id: "alpha", spaceId: "personal", title: "Alpha", url: "https://alpha.example/", pinned: false },
      { id: "beta", spaceId: "personal", title: "Beta", url: "https://beta.example/", pinned: false, splitWith: "gamma" },
      { id: "gamma", spaceId: "personal", title: "Gamma", url: "https://gamma.example/", pinned: false },
    );
    state.activeTab = "beta";
    expect(tabsToDismiss(state, "others").sort()).toEqual(["alpha", "start"]);
    expect(tabsToDismiss(state, "below")).toEqual([]);
    state.activeTab = "alpha";
    expect(commandContext(state, []).hasTabsBelow).toBe(true);
    state = reduce(state, { type: "closeBelow" });
    expect(state.tabs.some((tab) => tab.id === "beta" || tab.id === "gamma")).toBe(false);
    expect(state.tabs.some((tab) => tab.id === "alpha")).toBe(true);
    expect(state.tabs.some((tab) => tab.bookmarkId)).toBe(true);
  });
});
