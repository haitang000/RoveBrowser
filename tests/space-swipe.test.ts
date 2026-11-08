import { describe, expect, test } from "vitest";
import { settleFrames, settlePosition, swipeLanding, swipeOffset, swipeTravel, swipeVelocity } from "../src/lib/space-swipe";

describe("Zigzag SpaceSwipe port", () => {
  const middle = { width: 240, hasLeft: true, hasRight: true };
  test("tracks one to one toward a neighbor, clamped to one sidebar width", () => {
    expect(swipeOffset(-72, middle)).toBe(-72);
    expect(swipeOffset(500, middle)).toBe(240);
    expect(swipeOffset(-500, middle)).toBe(-240);
  });
  test("edges resist progressively and invert without a discontinuity", () => {
    for (const travel of [1, 40, 200, 1000, -1, -40, -200, -1000]) {
      const edge = { ...middle, hasLeft: false, hasRight: false };
      const offset = swipeOffset(travel, edge);
      expect(Math.abs(offset)).toBeLessThan(Math.min(Math.abs(travel), 120));
      expect(swipeTravel(offset, edge)).toBeCloseTo(travel);
    }
    expect(swipeOffset(100, { ...middle, width: 0 })).toBe(0);
    expect(swipeTravel(100, { ...middle, width: 0 })).toBe(0);
  });
  test("35 percent lands; a short flick lands only in the direction already pulled", () => {
    expect(swipeLanding(-83, 0, 240)).toBe(0);
    expect(swipeLanding(-84, 0, 240)).toBe(1);
    expect(swipeLanding(84, 0, 240)).toBe(-1);
    expect(swipeLanding(-12, -300, 240)).toBe(1);
    expect(swipeLanding(-120, 300, 240)).toBe(0);
    expect(swipeLanding(0, 800, 240)).toBe(0);
    expect(swipeLanding(20, 0, 0)).toBe(0);
  });
  test("release velocity uses the last 80ms and excludes the first sample's distance", () => {
    const samples = [{ time: 0, dx: -100 }, { time: 0.1, dx: -999 }, { time: 0.12, dx: -10 }, { time: 0.14, dx: -10 }];
    expect(swipeVelocity(samples, 0.14)).toBeCloseTo(-500);
    expect(swipeVelocity(samples, 0.3)).toBe(0);
    expect(swipeVelocity([{ time: 1, dx: 100 }], 1)).toBe(0);
  });
  test("critical settle preserves initial velocity and caps fast inward flicks before overshoot", () => {
    const step = 0.000001;
    for (const from of [-200, 200]) {
      const velocity = -Math.sign(from) * 300;
      expect(settlePosition(from, velocity, 0)).toBe(from);
      expect((settlePosition(from, velocity, step) - from) / step).toBeCloseTo(velocity, 0);
      for (let t = 0; t < 0.5; t += 0.005) {
        const position = settlePosition(from, -Math.sign(from) * 50000, t);
        expect(position * from).toBeGreaterThanOrEqual(0);
        expect(Math.abs(position)).toBeLessThanOrEqual(Math.abs(from));
      }
    }
    const { frames, duration } = settleFrames(100, -500);
    expect(duration).toBeLessThan(500);
    expect(frames[0]?.transform).toBe("translateX(100px)");
    expect(frames.at(-1)?.transform).toBe("translateX(0px)");
  });
});
