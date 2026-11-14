// Space look editing, from Zigzag/Sidebar/SpaceEditor.swift and App/Library.swift
// (SpaceInfo). Copyright 2026 Zigao Wang, Apache-2.0. Modified for Rove:
// no per-space profile, and Windows has one frosted glass so the clear/frosted
// switch is omitted. The named color stays the color Use Default Colors restores.

import type { Space, SpaceColor } from "./types";
import { clamp01, hueDegrees, paint, readableTheme } from "./space-color";
import { coverage, isSeeThrough } from "./page-frame";

export const WHEEL_HUE: Partial<Record<SpaceColor, number>> = {
  sky: 0.585,
  sage: 0.29,
  sand: 0.11,
  rose: 0.935,
  lavender: 0.765,
  mint: 0.46,
  peach: 0.055,
};

export interface LookFields {
  color: SpaceColor;
  hue?: number;
  tint?: number;
  brightness?: number;
  hue2?: number;
  tint2?: number;
  brightness2?: number;
  angle?: number;
  translucency?: number;
  grain?: number;
}

export interface LookDraft extends LookFields {
  colorChosen: boolean;
  selected: 0 | 1;
}

export interface Spot {
  hue: number;
  amount: number;
}

export interface SpaceLook {
  name: string;
  hue: number;
  tint: number;
  depth: number;
  hue2?: number;
  depth2: number;
  angle: number;
  seeThrough: number;
  grain: number;
}

function fade(
  name: string,
  hue: number,
  depth: number,
  hue2: number,
  depth2: number,
  extra: { tint?: number; grain?: number; seeThrough?: number } = {},
): SpaceLook {
  return {
    name,
    hue,
    tint: extra.tint ?? 1,
    depth,
    hue2,
    depth2,
    angle: 90,
    seeThrough: extra.seeThrough ?? 0,
    grain: extra.grain ?? 0,
  };
}

/** The eighteen material presets, in Zigzag's order. */
export const PRESETS: SpaceLook[] = [
  { name: "白色", hue: 0, tint: 0, depth: 0, depth2: 0, angle: 90, seeThrough: 0, grain: 0 },
  { name: "黑色", hue: 0, tint: 0, depth: 1, depth2: 0, angle: 90, seeThrough: 0, grain: 0 },
  { name: "玻璃", hue: 0, tint: 0, depth: 0.02, depth2: 0, angle: 90, seeThrough: 0.7, grain: 0 },
  { name: "烟雾", hue: 0, tint: 0, depth: 0.9, depth2: 0, angle: 90, seeThrough: 0.6, grain: 0 },
  fade("炭灰", 0.08, 0.84, 0.08, 0.94, { tint: 0.12, grain: 0.3 }),
  fade("浓缩", 0.07, 0.8, 0.07, 0.93, { tint: 0.45 }),
  fade("深红", 0.99, 0.78, 0.99, 0.92, { tint: 0.5 }),
  fade("松绿", 0.42, 0.8, 0.42, 0.93, { tint: 0.45 }),
  fade("石板", 0.62, 0.76, 0.62, 0.9, { tint: 0.35 }),
  { name: "纸", hue: 0.11, tint: 0.55, depth: 0.1, depth2: 0, angle: 90, seeThrough: 0, grain: 0.5 },
  fade("燕麦", 0.1, 0.02, 0.1, 0.26, { tint: 0.45 }),
  fade("黄油", 0.13, 0.02, 0.13, 0.26),
  fade("陶土", 0.075, 0.06, 0.06, 0.36, { tint: 0.55 }),
  fade("玫瑰", 0.97, 0.04, 0.97, 0.32, { tint: 0.55 }),
  fade("抹茶", 0.12, 0.02, 0.27, 0.32),
  fade("薄荷", 0.46, 0.03, 0.46, 0.32, { tint: 0.55 }),
  fade("天空", 0.6, 0.04, 0.6, 0.34, { tint: 0.6 }),
  fade("霜", 0.6, 0.02, 0.6, 0.18, { tint: 0.3, seeThrough: 0.5 }),
];

export function lookOf(space: LookFields): LookFields {
  return {
    color: space.color,
    hue: space.hue,
    tint: space.tint,
    brightness: space.brightness,
    hue2: space.hue2,
    tint2: space.tint2,
    brightness2: space.brightness2,
    angle: space.angle,
    translucency: space.translucency,
    grain: space.grain,
  };
}

export function baseHue(space: LookFields): number {
  return space.hue ?? WHEEL_HUE[space.color] ?? 0.6;
}

export function isNeutral(space: LookFields): boolean {
  return (space.hue == null && WHEEL_HUE[space.color] == null) || (space.tint ?? 1) < 0.005;
}

export function tintAmount(space: LookFields): number {
  return isNeutral(space) ? 0 : clamp01(space.tint ?? 1);
}

export function tintAmount2(space: LookFields): number {
  return clamp01(space.tint2 ?? 1);
}

export function seeThroughAmount(space: LookFields): number {
  const value = space.translucency;
  if (value == null || !Number.isFinite(value)) return 0;
  return clamp01(value);
}

/** Where a space that still follows the app theme sits on Shade, so the knob doesn't jump. */
export function startingDepth(space: LookFields, dark: boolean): number {
  if (isNeutral(space)) return dark ? 0.9 : 0.06;
  return dark ? 0.86 : 0.06 + 0.2 * 0.4;
}

export function depthAt(draft: LookFields, index: 0 | 1, dark: boolean): number {
  const follows = startingDepth(draft, dark);
  return index === 0 ? (draft.brightness ?? follows) : (draft.brightness2 ?? draft.brightness ?? follows);
}

export function openDraft(space: LookFields, dark: boolean): LookDraft {
  const chosen = space.brightness != null;
  const draft: LookDraft = { ...lookOf(space), colorChosen: chosen, selected: 0 };
  if (draft.brightness == null) draft.brightness = startingDepth(draft, dark);
  return draft;
}

/** What to save. An untouched space keeps following the app theme. */
export function committed(draft: LookDraft, original: LookFields): LookFields {
  const next = lookOf(draft);
  if (!draft.colorChosen) {
    next.brightness = original.brightness;
    next.brightness2 = original.brightness2;
  }
  return next;
}

export function spotsOf(draft: LookFields, dark: boolean): { spots: Spot[]; depths: number[] } {
  const spots = [{ hue: baseHue(draft), amount: tintAmount(draft) }];
  const depths = [depthAt(draft, 0, dark)];
  if (draft.hue2 != null) {
    spots.push({ hue: draft.hue2, amount: tintAmount2(draft) });
    depths.push(depthAt(draft, 1, dark));
  }
  return { spots, depths };
}

function omitEmpty<T extends LookFields>(space: T): T {
  const next = { ...space };
  if ((next.translucency ?? 0) <= 0.005) delete next.translucency;
  if ((next.grain ?? 0) <= 0.01) delete next.grain;
  if (next.hue2 == null) {
    delete next.hue2;
    delete next.tint2;
    delete next.brightness2;
    delete next.angle;
  }
  return next;
}

/** A preset replaces the whole look and leaves the space's default named color in place. */
export function applyPreset(space: LookFields, look: SpaceLook): LookFields {
  const next: LookFields = { color: space.color };
  if (look.tint < 0.005) next.tint = 0;
  else {
    next.hue = look.hue;
    if (look.tint <= 0.995) next.tint = look.tint;
  }
  next.brightness = look.depth;
  if (look.hue2 != null) {
    next.hue2 = look.hue2;
    next.brightness2 = look.depth2;
    if (look.tint <= 0.995) next.tint2 = look.tint;
    next.angle = look.angle;
  }
  if (look.seeThrough > 0) next.translucency = look.seeThrough;
  if (look.grain > 0) next.grain = look.grain;
  return next;
}

export function lookMatches(look: SpaceLook, space: LookFields): boolean {
  if (Math.abs((space.grain ?? 0) - look.grain) >= 0.01) return false;
  if (Math.abs(seeThroughAmount(space) - look.seeThrough) >= 0.01) return false;
  if (space.brightness == null || Math.abs(space.brightness - look.depth) >= 0.005) return false;
  if (look.tint < 0.005) {
    if (!isNeutral(space)) return false;
  } else if (
    isNeutral(space) ||
    space.hue == null ||
    Math.abs(space.hue - look.hue) >= 0.002 ||
    Math.abs(tintAmount(space) - look.tint) >= 0.01
  ) {
    return false;
  }
  if (look.hue2 == null) return space.hue2 == null;
  if (space.hue2 == null || Math.abs(space.hue2 - look.hue2) >= 0.002) return false;
  if (Math.abs((space.brightness2 ?? space.brightness) - look.depth2) >= 0.005) return false;
  return Math.abs(tintAmount2(space) - Math.min(look.tint, 1)) < 0.01;
}

export function hasDefaultColors(space: LookFields): boolean {
  return (
    space.hue == null &&
    space.brightness == null &&
    space.hue2 == null &&
    (space.tint ?? 1) > 0.99 &&
    seeThroughAmount(space) <= 0.005 &&
    (space.grain ?? 0) <= 0.01
  );
}

/** Both colors move together, keeping the step between them. Option moves only the selected one. */
export function shadeTo(draft: LookDraft, value: number, onlySelected: boolean, dark: boolean): LookDraft {
  const next: LookDraft = { ...draft, colorChosen: true };
  if (draft.hue2 == null || onlySelected) {
    if (draft.selected === 0) next.brightness = clamp01(value);
    else next.brightness2 = clamp01(value);
    return next;
  }
  const first = depthAt(draft, 0, dark);
  const second = depthAt(draft, 1, dark);
  const delta = Math.min(Math.max(value - depthAt(draft, draft.selected, dark), -Math.min(first, second)), 1 - Math.max(first, second));
  next.brightness = first + delta;
  next.brightness2 = second + delta;
  return next;
}

export function moveSpot(draft: LookDraft, index: 0 | 1, spot: Spot): LookDraft {
  const amount = spot.amount < 0.07 ? 0 : spot.amount;
  const tint = amount > 0.995 ? undefined : amount;
  const next: LookDraft = { ...draft, colorChosen: true, selected: index };
  if (index === 0) {
    next.hue = spot.hue;
    next.tint = tint;
  } else {
    next.hue2 = spot.hue;
    next.tint2 = tint;
  }
  return next;
}

/** A second color, the first's neighbor and a step deeper, or none. */
export function toggleSecond(draft: LookDraft, dark: boolean): LookDraft {
  const next: LookDraft = { ...draft, colorChosen: true };
  if (draft.hue2 == null) {
    next.hue2 = isNeutral(draft) ? 0.6 : (baseHue(draft) + 0.22) % 1;
    next.tint2 = isNeutral(draft) ? undefined : draft.tint;
    const first = depthAt(draft, 0, dark);
    next.brightness2 = first < 0.6 ? first + 0.16 : first - 0.16;
    next.angle = draft.angle ?? 90;
    next.selected = 1;
  } else {
    delete next.hue2;
    delete next.tint2;
    delete next.brightness2;
    next.selected = 0;
  }
  return next;
}

export function useDefaultColors(color: SpaceColor, dark: boolean): LookDraft {
  return openDraft({ color }, dark);
}

export function spotAt(x: number, y: number, width: number, height: number, keeping: number): Spot {
  const cx = width / 2;
  const cy = height / 2;
  const radius = Math.min(width, height) / 2 - 14;
  const dx = x - cx;
  const dy = y - cy;
  const amount = Math.min(Math.hypot(dx, dy) / radius, 1);
  if (amount <= 0.03) return { hue: keeping, amount };
  let h = (Math.atan2(dy, dx) + Math.PI / 2) / (2 * Math.PI);
  if (h < 0) h += 1;
  return { hue: h, amount };
}

export function spotPoint(spot: Spot, width: number, height: number): { x: number; y: number } {
  const cx = width / 2;
  const cy = height / 2;
  const radius = Math.min(width, height) / 2 - 14;
  const a = spot.hue * 2 * Math.PI - Math.PI / 2;
  const r = radius * spot.amount;
  return { x: cx + Math.cos(a) * r, y: cy + Math.sin(a) * r };
}

export interface PaintedFrame {
  custom: boolean;
  /** Opaque first color, for chrome sampling and accents. */
  start: string;
  end?: string;
  /** CSS linear-gradient angle. Zigzag's 0° is left to right. */
  cssAngle: number;
  grain: number;
  theme?: "light" | "dark";
  hue: number;
  neutralGlass: boolean;
  /** The shell's background, alpha included when see-through. */
  background: string;
}

function mix(color: string, alpha: number): string {
  if (alpha >= 0.999) return color;
  return `color-mix(in srgb, ${color} ${Math.round(alpha * 1000) / 10}%, transparent)`;
}

export function frameOf(space: LookFields, theme: "light" | "dark", glass: boolean): PaintedFrame {
  const through = glass && isSeeThrough(space.translucency);
  const alpha = through ? coverage(space.translucency) : 1;
  const custom = space.brightness != null || space.hue != null || space.hue2 != null || (space.tint ?? 1) < 0.995;
  if (!custom) {
    return {
      custom: false,
      start: "",
      cssAngle: 135,
      grain: 0,
      hue: hueDegrees(baseHue(space)),
      neutralGlass: through && space.color === "graphite",
      background: "",
    };
  }
  const depth = space.brightness ?? startingDepth(space, theme === "dark");
  const start = paint(baseHue(space), depth, tintAmount(space));
  const end =
    space.hue2 == null ? undefined : paint(space.hue2, space.brightness2 ?? depth, tintAmount2(space));
  const cssAngle = (space.angle ?? 45) + 90;
  const background = end
    ? `linear-gradient(${cssAngle}deg, ${mix(start, alpha)}, ${mix(end, alpha)})`
    : mix(start, alpha);
  const mode = space.brightness == null ? undefined : readableTheme(end ? [start, end] : [start]);
  return {
    custom: true,
    start,
    end,
    cssAngle,
    grain: clamp01(space.grain ?? 0),
    theme: mode,
    hue: hueDegrees(baseHue(space)),
    neutralGlass: false,
    background,
  };
}

export function swatchBackground(look: SpaceLook): string {
  const space = applyPreset({ color: "graphite" }, look);
  const start = paint(baseHue(space), space.brightness ?? look.depth, tintAmount(space));
  const end = space.hue2 == null ? start : paint(space.hue2, space.brightness2 ?? look.depth2, tintAmount2(space));
  const alpha = 1 - 0.45 * look.seeThrough;
  const fill = `linear-gradient(160deg, ${mix(start, alpha)}, ${mix(end, alpha)})`;
  if (look.seeThrough <= 0.005) return fill;
  return `${fill}, repeating-conic-gradient(#f4f4f4 0% 25%, #d5d5d5 0% 50%)`;
}

export function commandLook(look: LookFields): Record<string, unknown> {
  const clean = omitEmpty(look);
  return {
    color: clean.color,
    translucency: clean.translucency ?? 0,
    hue: clean.hue ?? null,
    tint: clean.tint ?? null,
    brightness: clean.brightness ?? null,
    hue2: clean.hue2 ?? null,
    tint2: clean.tint2 ?? null,
    brightness2: clean.brightness2 ?? null,
    angle: clean.angle ?? null,
    grain: clean.grain ?? null,
  };
}

export function paintKeys(command: Record<string, unknown>, space: Space) {
  const keys = ["hue", "tint", "brightness", "hue2", "tint2", "brightness2", "angle", "grain"] as const;
  for (const key of keys) {
    if (!Object.prototype.hasOwnProperty.call(command, key)) continue;
    const raw = command[key];
    if (raw == null || raw === "") {
      delete space[key];
      continue;
    }
    const value = Number(raw);
    if (!Number.isFinite(value)) continue;
    if (key === "angle") space.angle = ((value % 360) + 360) % 360;
    else if (key === "grain") {
      if (value > 0.01) space.grain = clamp01(value);
      else delete space.grain;
    } else if ((key === "tint" || key === "tint2") && value > 0.995) delete space[key];
    else space[key] = clamp01(value);
  }
}
