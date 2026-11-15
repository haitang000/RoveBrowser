import { describe, expect, it } from "vitest";
import { luminance, paint } from "../src/lib/space-color";
import { baseHue, tintAmount } from "../src/lib/space-look";
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
    const sameHueEnd = rgbDistance(sky.end, paint(baseHue({ color: "sky" }), 0.60, tintAmount({ color: "sky" })));
    expect(sameHueEnd).toBeGreaterThan(20);
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
    for (const stop of [neutral.start, neutral.end]) {
      expect(Math.max(...stop) - Math.min(...stop)).toBeLessThan(2);
    }
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

function rgbDistance(color: number[], other: string) {
  const channels = [1, 3, 5].map(i => Number.parseInt(other.slice(i, i + 2), 16));
  return Math.hypot(...color.map((channel, i) => channel - channels[i]));
}
