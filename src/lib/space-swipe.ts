/* Ported from Zigzag Sidebar/SidebarView.swift (SpaceSwipe) and Design/Tokens.swift.
 * Copyright 2026 Zigao Wang, Apache-2.0. Modified for CSS pixels and Web Animations.
 * Source baseline: 5338e99f58aa9a75a0696d5d96ec74353c3b3c7d. */
export const SPACE_SWIPE = {
  axisLock: 4,
  landingShare: 0.35,
  flickSpeed: 300,
  velocityWindow: 0.08,
  wheelTurnGap: 0.25,
  edgeGive: 0.5,
  edgeStiffness: 0.55,
  settleResponse: 0.15,
  switchDuration: 220,
  backgroundDuration: 200,
  reducedDuration: 200,
  ease: "cubic-bezier(.23,1,.32,1)",
  // DOM WheelEvent has no NSEvent phase/momentumPhase. Release on a clear
  // momentum decay, with a short silence fallback for streams without a tail.
  releaseDelay: 32,
  maxReleaseDelay: 64,
} as const;

export interface SwipeGeometry { width: number; hasLeft: boolean; hasRight: boolean }
export interface SwipeSample { time: number; dx: number }

/** Positive offsets move the list right, revealing the space to its left. */
export function swipeOffset(travel: number, g: SwipeGeometry): number {
  if (g.width <= 0) return 0;
  if (travel > 0 ? g.hasLeft : g.hasRight)
    return Math.max(-g.width, Math.min(g.width, travel));
  const limit = g.width * SPACE_SWIPE.edgeGive;
  const pulled = (1 - 1 / (Math.abs(travel) * SPACE_SWIPE.edgeStiffness / limit + 1)) * limit;
  return travel < 0 ? -pulled : pulled;
}

/** Invert the rubber band to pick up a settling list at its presentation position. */
export function swipeTravel(offset: number, g: SwipeGeometry): number {
  if (g.width <= 0) return 0;
  if (offset > 0 ? g.hasLeft : g.hasRight) return offset;
  const limit = g.width * SPACE_SWIPE.edgeGive;
  const share = Math.min(Math.abs(offset) / limit, 0.95);
  const travel = (1 / (1 - share) - 1) * limit / SPACE_SWIPE.edgeStiffness;
  return offset < 0 ? -travel : travel;
}

export function swipeLanding(offset: number, velocity: number, width: number): -1 | 0 | 1 {
  if (!offset || width <= 0) return 0;
  const toward = offset > 0 ? -1 : 1;
  if (Math.abs(velocity) >= SPACE_SWIPE.flickSpeed)
    return (velocity > 0) === (offset > 0) ? toward : 0;
  return Math.abs(offset) >= width * SPACE_SWIPE.landingShare ? toward : 0;
}

export function swipeVelocity(samples: SwipeSample[], now: number): number {
  const recent = samples.filter((s) => now - s.time <= SPACE_SWIPE.velocityWindow);
  const first = recent[0];
  if (!first || recent.length < 2 || now <= first.time) return 0;
  return recent.slice(1).reduce((sum, s) => sum + s.dx, 0) / (now - first.time);
}

/** Two packet intervals cover a missed frame without pausing a fast touchpad
 * for a fixed 80ms. Slower devices get a longer, bounded release window. */
export function swipeReleaseDelay(samples: SwipeSample[]): number {
  const recent = samples.slice(-5);
  const intervals = recent.slice(1).map((s, i) => (s.time - recent[i]!.time) * 1000)
    .filter((interval) => interval > 0).sort((a, b) => a - b);
  const cadence = intervals[Math.floor(intervals.length / 2)] || 0;
  return Math.max(SPACE_SWIPE.releaseDelay, Math.min(SPACE_SWIPE.maxReleaseDelay, cadence * 2));
}

/** The spring inherits the list's velocity, including edge resistance, rather
 * than the much faster raw finger travel outside the first/last space. */
export function swipeOffsetVelocity(travel: number, velocity: number, g: SwipeGeometry): number {
  if (g.width <= 0) return 0;
  if (travel > 0 ? g.hasLeft : g.hasRight) return Math.abs(travel) < g.width ? velocity : 0;
  const limit = g.width * SPACE_SWIPE.edgeGive;
  const resistance = 1 + Math.abs(travel) * SPACE_SWIPE.edgeStiffness / limit;
  return velocity * SPACE_SWIPE.edgeStiffness / (resistance * resistance);
}

/** Compare two short speed windows instead of requiring perfectly monotonic
 * deltas: touchpad inertia often contains small rebounds and quantized steps.
 * Normalize by packet spacing so a change in event frequency is not a release. */
export function swipeCoasting(samples: SwipeSample[]): boolean {
  if (samples.length < 5) return false;
  const tail = samples.slice(-5);
  const speeds: number[] = [];
  for (let i = 1; i < tail.length; i++) {
    const previous = tail[i - 1]!, current = tail[i]!;
    const elapsed = current.time - previous.time;
    if (current.dx * previous.dx <= 0 || elapsed <= 0 || elapsed > SPACE_SWIPE.velocityWindow) return false;
    speeds.push(Math.abs(current.dx) / elapsed);
  }
  const before = (speeds[0]! + speeds[1]!) / 2;
  const after = (speeds[2]! + speeds[3]!) / 2;
  return after <= before * 0.75 && speeds[2]! <= before * 0.9 && speeds[3]! <= before * 0.8;
}

/** Ignore tail jitter, but let a distinct new push take over a finished swipe. */
export function swipeNewImpulse(dx: number, previous: number): boolean {
  return Math.abs(dx) > Math.abs(previous) * 2
    && Math.abs(dx) - Math.abs(previous) >= SPACE_SWIPE.axisLock * 2;
}

/** Analytic critically damped spring. Cap inward speed at its natural frequency,
 * as Tokens.Motion.slide does, so the handoff cannot overshoot its destination. */
export function settlePosition(from: number, velocity: number, time: number): number {
  if (!from) return 0;
  const natural = 2 * Math.PI / SPACE_SWIPE.settleResponse;
  const speed = -from * Math.min(-velocity / from, natural);
  return (from + (speed + natural * from) * time) * Math.exp(-natural * time);
}

/** Sample once, then let the compositor run the spring without per-frame Vue work. */
export function settleFrames(from: number, velocity: number) {
  const frames: Keyframe[] = [];
  const duration = 400;
  for (let ms = 0; ms < duration; ms += 1000 / 120)
    frames.push({ offset: ms / duration, transform: `translateX(${settlePosition(from, velocity, ms / 1000)}px)` });
  frames.push({ offset: 1, transform: "translateX(0px)" });
  return { frames, duration };
}
