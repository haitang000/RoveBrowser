import type { BrowserState } from "./types";

export function splitRatio(state: BrowserState, left?: string, right?: string): number {
  const saved = state.splitRatios?.find(pair =>
    (pair.left === left && pair.right === right) || (pair.left === right && pair.right === left),
  );
  if (!saved || !Number.isFinite(saved.ratio)) return 0.5;
  return saved.left === left ? saved.ratio : 1 - saved.ratio;
}

export function clampSplitRatio(ratio: number, width: number): number {
  const minimum = Math.min(0.5, Math.max(0.2, 260 / Math.max(1, width)));
  return Math.min(1 - minimum, Math.max(minimum, ratio));
}
