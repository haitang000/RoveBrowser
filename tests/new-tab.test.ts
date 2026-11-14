import { describe, expect, it, vi } from "vitest";
import { calendarDays, defaultNewTab, localDate, newTabSettings, newTabSuggestions, validGithubUsername } from "../src/lib/new-tab";
import { githubContributions } from "../src/lib/github-widget";

describe("new tab data", () => {
  it("upgrades old profiles without restoring widgets that were deliberately removed", () => {
    expect(newTabSettings()).toEqual(defaultNewTab());
    expect(newTabSettings({ displayName: "  Ada  ", widgets: [] })).toEqual({ displayName: "Ada", widgets: [] });
    const widget = { id: "a", kind: "notes", size: "small", text: "x".repeat(10_001) };
    const cleaned = newTabSettings({ displayName: "Ada", widgets: [widget, widget, { id: "b", kind: "unknown" }] });
    expect(cleaned.widgets).toHaveLength(1);
    expect(cleaned.widgets[0].text).toHaveLength(10_000);
  });
  it("uses real recent sites, removes duplicate hosts, and rejects internal and credential URLs", () => {
    const result = newTabSuggestions([
      { title: "Internal", url: "era://newtab" },
      { title: "Secret", url: "https://name:pass@example.com" },
      { title: "Recent", url: "https://example.com/docs" },
      { title: "Duplicate", url: "https://www.example.com/" },
    ], [{ id: "saved", title: "Saved", url: "https://github.com" }]);
    expect(result).toHaveLength(5);
    expect(result.slice(0, 2).map((site) => site.title)).toEqual(["Recent", "Saved"]);
    expect(result.filter((site) => site.url.includes("example.com"))).toHaveLength(1);
  });
  it("lays out leap days and Monday-first calendars without converting the local date to UTC", () => {
    const month = calendarDays(new Date(2024, 1, 1));
    expect(month.slice(0, 4)).toEqual([null, null, null, 1]);
    expect(month.at(-1)).toBe(29);
    expect(localDate(new Date(2026, 10, 14, 23, 59))).toBe("2026-11-14");
  });
  it("accepts GitHub usernames and rejects URLs, consecutive hyphens and overlong input", () => {
    for (const username of ["a", "Ada-Lovelace", " octocat "]) expect(validGithubUsername(username)).toBe(true);
    for (const username of ["", "-ada", "ada-", "ada--b", "a".repeat(40), "https://github.com/ada"]) expect(validGithubUsername(username)).toBe(false);
  });
  it("validates network data and reuses public contribution results", async () => {
    const fetcher = vi.fn().mockResolvedValue(new Response(JSON.stringify({ contributions: [
      { date: "2026-11-14", count: 5, level: 4 },
      { date: "2026-11-13", count: 0, level: 0 },
      { date: "invalid", count: -1, level: 9 },
    ] })));
    vi.stubGlobal("fetch", fetcher);
    try {
      const signal = new AbortController().signal;
      const days = await githubContributions("unit-test-user", signal);
      expect(days.map((day) => day.count)).toEqual([0, 5]);
      expect(await githubContributions("UNIT-test-user", signal)).toEqual(days);
      expect(fetcher).toHaveBeenCalledTimes(1);
      await expect(githubContributions("bad/user", signal)).rejects.toThrow("用户名");
    } finally { vi.unstubAllGlobals(); }
  });
});
