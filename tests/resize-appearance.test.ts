import { describe, expect, it } from "vitest";
import { luminance } from "../src/lib/space-color";
import { resizeAppearance } from "../src/lib/resize-appearance";

const hex = (rgb: number[]) => "#" + rgb.map(c => c.toString(16).padStart(2, "0")).join("");

describe("resize surround", () => {
  it("follows Space colors with distinct gradient stops and moderate brightness", () => {
    const sky = resizeAppearance({ color: "sky" }, "light", false);
    const sage = resizeAppearance({ color: "sage" }, "light", false);
    const night = resizeAppearance({ color: "sky" }, "dark", false);
    expect(sky.start[2]).toBeGreaterThan(sky.start[0]);
    expect(sage.start[1]).toBeGreaterThan(sage.start[0]);
    expect(sky.start).not.toEqual(sage.start);
    for (const stop of ["start", "end"] as const) {
      const day = luminance(hex(sky[stop]));
      const dark = luminance(hex(night[stop]));
      expect(day).toBeGreaterThan(0.12);
      expect(dark).toBeLessThan(0.55);
      expect(dark).toBeGreaterThan(day);
    }
    expect(Math.abs(luminance(hex(sky.start)) - luminance(hex(sky.end)))).toBeGreaterThan(0.05);
    expect(Math.abs(luminance(hex(night.start)) - luminance(hex(night.end)))).toBeGreaterThan(0.05);
    const neutral = resizeAppearance({ color: "graphite" }, "light", false);
    expect(Math.max(...neutral.start) - Math.min(...neutral.start)).toBeLessThan(2);
  });
  it("preserves two custom hues and direction, using the visible Space theme", () => {
    const custom = { color: "sky" as const, hue: 0, hue2: 0.34, brightness: 0.85, brightness2: 0.8, angle: 185 };
    const colors = resizeAppearance(custom, "light", true);
    expect(colors.dark).toBe(true);
    expect(colors.start[0]).toBeGreaterThan(colors.start[1]);
    expect(colors.end[1]).toBeGreaterThan(colors.end[0]);
    expect(colors.angle).toBe(275);
    expect(colors.reduceMotion).toBe(true);
    const neutralSecond = resizeAppearance({ ...custom, tint2: 0 }, "light", false);
    expect(Math.max(...neutralSecond.end) - Math.min(...neutralSecond.end)).toBeLessThan(2);
  });
});
