import { describe, expect, it } from "vitest";
import {
  SHADOW_BLEED,
  shadowBleed,
  surfaceRegion,
} from "../src/lib/popup-layer";

describe("floating layer regions", () => {
  it("widens shadowed surfaces and leaves plain ones at their box", () => {
    expect(shadowBleed("find-bar")).toBe(SHADOW_BLEED);
    expect(shadowBleed("tab-hover-preview visible")).toBe(SHADOW_BLEED);
    expect(shadowBleed("error-toast")).toBe(SHADOW_BLEED);
    expect(shadowBleed("command-scrim")).toBe(0);
    expect(shadowBleed("find-bar-surface")).toBe(0);
    expect(shadowBleed("")).toBe(0);
  });

  it("gives the find bar region room for its shadow", () => {
    // A 320 x 40 pill at the page's top-right; 20px bleed on every side.
    expect(surfaceRegion({ x: 939, y: 45, width: 320, height: 40 }, 20)).toEqual(
      { x: 919, y: 25, width: 360, height: 80 },
    );
    // Clamped at the window edges like the native callback expected.
    expect(surfaceRegion({ x: 4, y: 4, width: 100, height: 40 }, 20)).toEqual({
      x: 0,
      y: 0,
      width: 140,
      height: 80,
    });
  });
});
