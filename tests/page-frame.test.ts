import { describe, expect, it } from "vitest";
import {
  coverage,
  edgeToEdge,
  isSeeThrough,
  pageArea,
  pageRadii,
  splitGap,
  usesClearGlass,
} from "../src/lib/page-frame";

const frame = {
  width: 1280,
  height: 820,
  sidebar: 240,
  windowRadius: 12,
};

describe("page frame from BrowserRootView", () => {
  it("meets the window edges beside the sidebar when borderless", () => {
    expect(pageArea({ ...frame, sidebarVisible: true, borderless: true })).toEqual({
      x: 240, y: 0, width: 1040, height: 820,
    });
    expect(edgeToEdge({ sidebarVisible: true, borderless: true })).toBe(false);
    expect(pageRadii({ ...frame, sidebarVisible: true, borderless: true })).toEqual([12, 0, 0, 12]);
  });

  it("floats the page by the card inset when borderless is off", () => {
    expect(pageArea({ ...frame, sidebarVisible: true, borderless: false })).toEqual({
      x: 240, y: 8, width: 1032, height: 804,
    });
    expect(pageRadii({ ...frame, sidebarVisible: true, borderless: false })).toEqual([8, 8, 8, 8]);
  });

  it("fills the window with the sidebar hidden, and insets it again when borderless is off", () => {
    expect(edgeToEdge({ sidebarVisible: false, borderless: true })).toBe(true);
    expect(pageArea({ ...frame, sidebarVisible: false, borderless: true })).toEqual({
      x: 0, y: 0, width: 1280, height: 820,
    });
    expect(pageRadii({ ...frame, sidebarVisible: false, borderless: true })).toEqual([0, 0, 0, 0]);
    expect(pageArea({ ...frame, sidebarVisible: false, borderless: false })).toEqual({
      x: 8, y: 8, width: 1264, height: 804,
    });
  });

  it("uses a hairline between split pages only when the page is edge to edge", () => {
    expect(splitGap({ sidebarVisible: true, borderless: true })).toBe(8);
    expect(splitGap({ sidebarVisible: false, borderless: true })).toBe(1);
    expect(splitGap({ sidebarVisible: false, borderless: false })).toBe(8);
  });

  it("squares attached corners when the window itself is square", () => {
    expect(pageRadii({ ...frame, windowRadius: 0, sidebarVisible: true, borderless: true })).toEqual([0, 0, 0, 0]);
  });
});

describe("glass coverage from SpaceInfo", () => {
  it("stays solid until see-through is perceptible", () => {
    expect(isSeeThrough(0)).toBe(false);
    expect(isSeeThrough(0.005)).toBe(false);
    expect(isSeeThrough(0.006)).toBe(true);
    expect(coverage(0, true)).toBe(1);
  });

  it("thins the wash to a half for frosted glass and further for clear glass", () => {
    expect(coverage(1, false)).toBeCloseTo(0.5);
    expect(coverage(1, true)).toBeCloseTo(0.25);
    expect(coverage(0.5, false)).toBeCloseTo(0.75);
    expect(coverage(0.5, true)).toBeCloseTo(0.625);
    expect(coverage(2, false)).toBeCloseTo(0.5);
  });

  it("treats an unset glass style as clear, and frosted when clear glass is unavailable", () => {
    expect(usesClearGlass(undefined, true)).toBe(true);
    expect(usesClearGlass(false, true)).toBe(false);
    expect(usesClearGlass(undefined, false)).toBe(false);
  });
});
