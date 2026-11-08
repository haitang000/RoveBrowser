import { describe, expect, test } from "vitest";
import { settleFrames, settlePosition, swipeCoasting, swipeLanding, swipeNewImpulse, swipeOffset, swipeOffsetVelocity, swipeReleaseDelay, swipeTravel, swipeVelocity } from "../src/lib/space-swipe";

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
  test("idle release follows event cadence instead of pausing every gesture for 80ms", () => {
    const samples = (interval: number) => Array.from({ length: 5 }, (_, i) => ({ time: i * interval, dx: 5 }));
    expect(swipeReleaseDelay([])).toBe(32);
    expect(swipeReleaseDelay(samples(1 / 120))).toBe(32);
    expect(swipeReleaseDelay(samples(0.02))).toBeCloseTo(40);
    expect(swipeReleaseDelay(samples(0.03))).toBeCloseTo(60);
    expect(swipeReleaseDelay(samples(0.1))).toBe(64);
  });
  test("edge spring velocity matches the derivative of the displayed rubber band", () => {
    const edge = { ...middle, hasLeft: false, hasRight: false };
    const step = 0.000001, velocity = 600;
    for (const travel of [-400, -100, 100, 400]) {
      const derivative = (swipeOffset(travel + velocity * step, edge) - swipeOffset(travel, edge)) / step;
      expect(swipeOffsetVelocity(travel, velocity, edge)).toBeCloseTo(derivative, 2);
      expect(Math.abs(swipeOffsetVelocity(travel, velocity, edge))).toBeLessThan(velocity);
    }
    expect(swipeOffsetVelocity(50, velocity, middle)).toBe(velocity);
    expect(swipeOffsetVelocity(300, velocity, middle)).toBe(0);
    expect(swipeOffsetVelocity(50, velocity, { ...edge, width: 0 })).toBe(0);
  });
  test("detects a sustained inertia decay in either direction, not steady movement or reversal", () => {
    const samples = (deltas: number[]) => deltas.map((dx, i) => ({ time: i * 0.02, dx }));
    expect(swipeCoasting(samples([42, 38, 30, 22, 16]))).toBe(true);
    expect(swipeCoasting(samples([-42, -38, -30, -22, -16]))).toBe(true);
    expect(swipeCoasting(samples([28, 22, 24, 18, 14]))).toBe(true);
    expect(swipeCoasting(samples([-28, -22, -24, -18, -14]))).toBe(true);
    for (const deltas of [[38, 30, 22, 16], [20, 20, 20, 20, 20], [5, 10, 20, 30, 40], [42, 38, 30, -22, -16], [42, 38, 10, 22, 16]])
      expect(swipeCoasting(samples(deltas))).toBe(false);
    // The deltas shrink only because the packet intervals shrink: speed stays constant.
    expect(swipeCoasting([{ time: 0, dx: 40 }, { time: 0.04, dx: 40 }, { time: 0.07, dx: 30 }, { time: 0.09, dx: 20 }, { time: 0.10, dx: 10 }])).toBe(false);
  });
  test("a new push re-arms the swipe while minor inertia jitter stays consumed", () => {
    expect(swipeNewImpulse(14, 1)).toBe(true);
    expect(swipeNewImpulse(-14, -1)).toBe(true);
    for (const [dx, previous] of [[7, 2], [20, 16], [8, 8], [1, 2]])
      expect(swipeNewImpulse(dx!, previous!)).toBe(false);
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
