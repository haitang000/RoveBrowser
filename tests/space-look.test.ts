import { describe, expect, it } from "vitest";
import { luminance, paint } from "../src/lib/space-color";
import { initialState, reduce } from "../src/lib/preview";
import {
  PRESETS,
  applyPreset,
  committed,
  frameOf,
  hasDefaultColors,
  lookMatches,
  openDraft,
  shadeTo,
  spotAt,
  toggleSecond,
  useDefaultColors,
} from "../src/lib/space-look";

describe("space color", () => {
  it("paints white at the light end and black at the dark end", () => {
    expect(luminance(paint(0.6, 0, 0))).toBeGreaterThan(0.9);
    expect(luminance(paint(0.6, 1, 0))).toBeLessThan(0.02);
  });

  it("keeps one depth equally light across hues", () => {
    const yellow = luminance(paint(0.16, 0.4, 1));
    const blue = luminance(paint(0.6, 0.4, 1));
    expect(Math.abs(yellow - blue)).toBeLessThan(0.02);
  });
});

describe("space look", () => {
  const sky = { color: "sky" as const };

  it("leaves an untouched space on its named color", () => {
    const draft = openDraft(sky, false);
    expect(draft.brightness).toBeGreaterThan(0);
    expect(committed(draft, sky).brightness).toBeUndefined();
    expect(hasDefaultColors(committed(draft, sky))).toBe(true);
    expect(frameOf(sky, "light", false).custom).toBe(false);
  });

  it("moves both gradient stops together unless only the selected one should move", () => {
    let draft = toggleSecond(openDraft(sky, false), false);
    draft = { ...draft, brightness: 0.2, brightness2: 0.4, selected: 0 };
    const both = shadeTo(draft, 0.3, false, false);
    expect(both.brightness).toBeCloseTo(0.3);
    expect(both.brightness2).toBeCloseTo(0.5);
    const one = shadeTo(draft, 0.5, true, false);
    expect(one.brightness).toBeCloseTo(0.5);
    expect(one.brightness2).toBe(0.4);
  });

  it("adds a neighbor as the second color and a preset round-trips", () => {
    const second = toggleSecond(openDraft(sky, false), false);
    expect(second.hue2).toBeCloseTo((0.585 + 0.22) % 1);
    expect(second.selected).toBe(1);
    const mint = PRESETS.find((look) => look.name === "薄荷")!;
    const applied = applyPreset(sky, mint);
    expect(lookMatches(mint, applied)).toBe(true);
    expect(applied.color).toBe("sky");
    expect(frameOf(applied, "light", true).custom).toBe(true);
    expect(frameOf(applied, "light", true).background).toContain("linear-gradient");
    const restored = useDefaultColors("sky", false);
    expect(hasDefaultColors(committed(restored, sky))).toBe(true);
  });

  it("keeps the hue when the pointer sits in the middle of the wheel", () => {
    expect(spotAt(100, 100, 200, 200, 0.4).hue).toBe(0.4);
    expect(spotAt(100, 20, 200, 200, 0.4).amount).toBeGreaterThan(0.5);
  });

  it("persists a custom look and clears it", () => {
    const created = reduce(initialState(), {
      type: "spaceUpdate",
      id: "personal",
      name: "Personal",
      color: "sky",
      translucency: 0.4,
      hue: 0.2,
      tint: 0.5,
      brightness: 0.3,
      hue2: 0.55,
      brightness2: 0.46,
      angle: 90,
      grain: 0.2,
    });
    expect(created.spaces[0]).toMatchObject({
      hue: 0.2,
      tint: 0.5,
      brightness: 0.3,
      hue2: 0.55,
      brightness2: 0.46,
      angle: 90,
      grain: 0.2,
      translucency: 0.4,
    });
    const cleared = reduce(created, {
      type: "spaceUpdate",
      id: "personal",
      name: "Personal",
      color: "sky",
      translucency: 0,
      hue: null,
      tint: null,
      brightness: null,
      hue2: null,
      tint2: null,
      brightness2: null,
      angle: null,
      grain: null,
    });
    expect(cleared.spaces[0]).not.toHaveProperty("hue");
    expect(cleared.spaces[0]).not.toHaveProperty("grain");
    expect(cleared.spaces[0]?.translucency).toBeUndefined();
  });
});
