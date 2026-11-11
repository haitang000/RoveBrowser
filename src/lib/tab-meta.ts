// Zigzag's tab aging (Tokens.Fade.start/full and Opacity.fadedFloor) plus the
// custom name a tab picks up from its context menu. Shared by the sidebar row,
// the Clear button preview and the tab menus so they cannot drift apart.
import type { Tab } from "./types";

/** Tabs start fading after six hours without being looked at. */
export const TAB_FADE_START = 6 * 60 * 60;
/** ...and reach the faded floor after four days. */
export const TAB_FADE_FULL = 4 * 24 * 60 * 60;
/** How faint a forgotten tab gets: visible, but clearly resting. */
export const TAB_FADE_FLOOR = 0.42;

export function nowSeconds(): number {
  return Date.now() / 1000;
}

/** 1 for recently used tabs, easing down to 0.42 for tabs left alone for days. */
export function tabFreshness(lastActivated?: number, now = nowSeconds()): number {
  if (!lastActivated || !Number.isFinite(lastActivated)) return 1;
  const age = Math.max(0, now - lastActivated);
  if (age <= TAB_FADE_START) return 1;
  const progress = Math.min(1, (age - TAB_FADE_START) / (TAB_FADE_FULL - TAB_FADE_START));
  const eased = 1 - (1 - progress) ** 2;
  return 1 - eased * (1 - TAB_FADE_FLOOR);
}

/** Whether the tab is old enough for Zigzag's "Clear old" step. */
export function tabIsFaded(lastActivated?: number, now = nowSeconds()): boolean {
  return Boolean(lastActivated) && now - (lastActivated as number) > TAB_FADE_START;
}

/** The name shown in the sidebar and menus: a rename wins over the page title. */
export function tabDisplayTitle(tab: Pick<Tab, "title" | "customTitle">): string {
  const custom = tab.customTitle?.trim();
  return custom ? custom : tab.title;
}
