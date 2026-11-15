import { clamp01, paint } from "./space-color";
import { baseHue, frameOf, tintAmount, tintAmount2, type LookFields } from "./space-look";

const rgb = (hex: string) => [1, 3, 5].map(i => Number.parseInt(hex.slice(i, i + 2), 16));

/** Reuse the Space color wheel, both gradient stops and its effective theme. */
export function resizeAppearance(space: LookFields, theme: "light" | "dark", reduceMotion: boolean) {
  const dark = (frameOf(space, theme, false).theme ?? theme) === "dark";
  // Keep the hue/tint and selected shade while guaranteeing a contrasting
  // surround for the lifted page in either appearance.
  const depth = (shade: number | undefined, second: boolean) => {
    const variation = (clamp01(shade ?? 0.5) - 0.5) * 0.1;
    return dark ? 0.22 + (second ? -0.10 : 0) + variation : 0.72 + (second ? 0.12 : 0) + variation;
  };
  return {
    start: rgb(paint(baseHue(space), depth(space.brightness, false), tintAmount(space))),
    end: rgb(paint(space.hue2 ?? baseHue(space), depth(space.brightness2 ?? space.brightness, true),
      space.hue2 == null ? tintAmount(space) : tintAmount2(space))),
    angle: ((space.angle ?? 45) + 90) % 360,
    dark,
    reduceMotion,
  };
}
