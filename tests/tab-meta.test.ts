import { describe, expect, it } from "vitest";
import {
  TAB_FADE_FLOOR,
  TAB_FADE_FULL,
  TAB_FADE_START,
  tabDisplayTitle,
  tabFreshness,
  tabIsFaded,
} from "../src/lib/tab-meta";

describe("tab aging", () => {
  const now = 10_000_000;
  it("keeps fresh tabs at full strength and eases forgotten ones to the floor", () => {
    expect(tabFreshness(now - 60, now)).toBe(1);
    expect(tabFreshness(now - TAB_FADE_START, now)).toBe(1);
    expect(tabFreshness(undefined, now)).toBe(1);
    const halfway = now - (TAB_FADE_START + TAB_FADE_FULL) / 2;
    const faded = tabFreshness(halfway, now);
    expect(faded).toBeGreaterThan(TAB_FADE_FLOOR);
    expect(faded).toBeLessThan(1);
    expect(tabFreshness(now - TAB_FADE_FULL, now)).toBeCloseTo(TAB_FADE_FLOOR, 5);
    expect(tabFreshness(now - TAB_FADE_FULL * 10, now)).toBeCloseTo(TAB_FADE_FLOOR, 5);
  });
  it("marks only tabs past the fade start as old enough to clear", () => {
    expect(tabIsFaded(undefined, now)).toBe(false);
    expect(tabIsFaded(now - TAB_FADE_START + 1, now)).toBe(false);
    expect(tabIsFaded(now - TAB_FADE_START - 1, now)).toBe(true);
  });
  it("shows a custom rename in place of the page title", () => {
    expect(tabDisplayTitle({ title: "Page", customTitle: " 我的标签 " })).toBe("我的标签");
    expect(tabDisplayTitle({ title: "Page" })).toBe("Page");
    expect(tabDisplayTitle({ title: "Page", customTitle: "   " })).toBe("Page");
  });
});
