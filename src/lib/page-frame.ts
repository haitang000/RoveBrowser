// Page frame and glass coverage adapted from Zigzag BrowserRootView.swift,
// PageCardView.swift and Library.swift (SpaceInfo). Copyright 2026 Zigao Wang,
// Apache-2.0. Modified for Rove: the attached corner follows this window's
// 12px radius, and the caption strip is laid out by the shell.

export const CARD_INSET = 8;
export const CARD_RADIUS = 8;
export const SPLIT_HAIRLINE = 1;

export interface PageFrame {
  width: number;
  height: number;
  sidebar: number;
  sidebarVisible: boolean;
  borderless: boolean;
  windowRadius: number;
}

export function edgeToEdge(input: Pick<PageFrame, "sidebarVisible" | "borderless">): boolean {
  return !input.sidebarVisible && input.borderless;
}

/** The page card in window coordinates, before a caption strip is reserved inside it. */
export function pageArea(input: PageFrame): { x: number; y: number; width: number; height: number } {
  const inset = CARD_INSET;
  if (edgeToEdge(input)) return { x: 0, y: 0, width: input.width, height: input.height };
  if (input.sidebarVisible) {
    if (input.borderless) {
      return { x: input.sidebar, y: 0, width: Math.max(0, input.width - input.sidebar), height: input.height };
    }
    return {
      x: input.sidebar,
      y: inset,
      width: Math.max(0, input.width - input.sidebar - inset),
      height: Math.max(0, input.height - inset * 2),
    };
  }
  return {
    x: inset,
    y: inset,
    width: Math.max(0, input.width - inset * 2),
    height: Math.max(0, input.height - inset * 2),
  };
}

/** Clockwise from the top left. Window corners stay square; the window rounds them. */
export function pageRadii(input: PageFrame): [number, number, number, number] {
  if (edgeToEdge(input)) return [0, 0, 0, 0];
  if (input.sidebarVisible && input.borderless) {
    const radius = Math.max(0, input.windowRadius);
    return [radius, 0, 0, radius];
  }
  return [CARD_RADIUS, CARD_RADIUS, CARD_RADIUS, CARD_RADIUS];
}

export function splitGap(input: Pick<PageFrame, "sidebarVisible" | "borderless">): number {
  return edgeToEdge(input) ? SPLIT_HAIRLINE : CARD_INSET;
}

export function seeThrough(value: number | null | undefined): number {
  if (value == null || !Number.isFinite(value)) return 0;
  return Math.min(1, Math.max(0, value));
}

export function isSeeThrough(value: number | null | undefined): boolean {
  return seeThrough(value) > 0.005;
}

/** The single clear appearance keeps a thin color wash over the softened backdrop. */
export function coverage(value: number | null | undefined): number {
  return 1 - 0.75 * seeThrough(value);
}
