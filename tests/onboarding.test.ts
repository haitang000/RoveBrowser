import { describe, expect, it } from "vitest";
import { initialState, reduce } from "../src/lib/preview";
import { ONBOARDING_SITES, saveOnboardingSites } from "../src/lib/onboarding";

describe("onboarding favorites", () => {
  it("validates the whole selection before changing saved pages", () => {
    const state = initialState();
    const original = structuredClone(state.favorites);
    for (const ids of [["unknown"], ["gmail", "gmail"], ONBOARDING_SITES.map((s) => s.id)]) {
      expect(() => saveOnboardingSites(state, ids)).toThrow();
      expect(state.favorites).toEqual(original);
    }
  });
  it("preserves custom sites, saved identity and live pages when editing choices", () => {
    const state = initialState();
    state.favorites = [{ id: "custom", title: "Docs", url: "https://example.com/docs" }, { id: "gmail", title: "Mail", url: "https://mail.google.com/", favicon: "data:image/png;base64,cached" }];
    saveOnboardingSites(state, ["gmail", "github"]);
    expect(state.favorites.find((f) => f.id === "gmail")?.favicon).toBe("data:image/png;base64,cached");
    const github = state.favorites.find((f) => f.title === "GitHub")!;
    state.tabs.push({ id: "live", bookmarkId: github.id, title: "Working page", url: "https://github.com/issues", spaceId: state.activeSpace, pinned: false });
    saveOnboardingSites(state, ["gmail"]);
    expect(state.favorites.map((f) => f.id)).toEqual(["custom", "gmail"]);
    expect(state.tabs.find((t) => t.id === "live")?.bookmarkId).toBeUndefined();
    expect(state.tabs.find((t) => t.id === "live")?.url).toBe("https://github.com/issues");
  });
  it("ignores stale advances and only commits sites on Continue", () => {
    let state = reduce(initialState(), { type: "onboardingOpen" });
    state = reduce(state, { type: "onboardingAdvance", step: "hello" });
    expect(() => reduce(state, { type: "onboardingAdvance", step: "hello" })).toThrow();
    state = reduce(state, { type: "onboardingAdvance", step: "bring", skip: true });
    state = reduce(state, { type: "onboardingAdvance", step: "spaces" });
    state = reduce(state, { type: "onboardingAdvance", step: "search" });
    const original = structuredClone(state.favorites);
    state = reduce(state, { type: "onboardingAdvance", step: "sites", skip: true, sites: [] });
    expect(state.favorites).toEqual(original);
    expect(state.onboarding).toEqual({ step: "ready", completed: false });
  });
});
