// Adapted from Zigzag Import/Onboarding.swift, Copyright 2026 Zigao Wang,
// Apache-2.0. Rove branding and Windows capability descriptions.
import type { BrowserState } from "./types";
import { sameFavoriteUrl, MAX_FAVORITES } from "./favorites";

export const ONBOARDING_STEPS = ["hello", "bring", "sites", "ads", "ready"] as const;
export type OnboardingStep = typeof ONBOARDING_STEPS[number];
export interface OnboardingState { step: OnboardingStep; completed: boolean; imports?: import("./import").ImportReport[] }
export const ONBOARDING_SITES = [
  { id: "gmail", title: "Gmail", url: "https://mail.google.com", color: "#b84037", glyph: "M" },
  { id: "calendar", title: "Calendar", url: "https://calendar.google.com", color: "#3476ca", glyph: "31" },
  { id: "youtube", title: "YouTube", url: "https://www.youtube.com", color: "#e43e3e", glyph: "▶" },
  { id: "drive", title: "Drive", url: "https://drive.google.com", color: "#27855c", glyph: "△" },
  { id: "notion", title: "Notion", url: "https://www.notion.so", color: "#252525", glyph: "N" },
  { id: "chatgpt", title: "ChatGPT", url: "https://chatgpt.com", color: "#27846d", glyph: "✳" },
  { id: "claude", title: "Claude", url: "https://claude.ai", color: "#b96c4e", glyph: "✷" },
  { id: "spotify", title: "Spotify", url: "https://open.spotify.com", color: "#23834a", glyph: "≋" },
  { id: "slack", title: "Slack", url: "https://app.slack.com", color: "#813c78", glyph: "#" },
  { id: "figma", title: "Figma", url: "https://www.figma.com", color: "#854bc1", glyph: "F" },
  { id: "github", title: "GitHub", url: "https://github.com", color: "#343b43", glyph: "GH" },
  { id: "discord", title: "Discord", url: "https://discord.com/app", color: "#5865bf", glyph: "D" },
] as const;

export function selectedSiteIds(state: BrowserState) {
  return ONBOARDING_SITES.filter((site) => state.favorites.some((f) => sameFavoriteUrl(f.url, site.url))).map((site) => site.id as string);
}
export function otherFavorites(state: BrowserState) {
  return state.favorites.filter((f) => !ONBOARDING_SITES.some((site) => sameFavoriteUrl(f.url, site.url)));
}
export function saveOnboardingSites(state: BrowserState, ids: string[]) {
  const unique = new Set(ids);
  if (unique.size !== ids.length || ids.some((id) => !ONBOARDING_SITES.some((site) => site.id === id))) throw Error("无效的常用网站选项");
  if (otherFavorites(state).length + ids.length > MAX_FAVORITES) throw Error("最多置顶 8 个网站");
  const removed = state.favorites.filter((f) => ONBOARDING_SITES.some((site) => sameFavoriteUrl(f.url, site.url) && !unique.has(site.id)));
  // Removing a saved favorite keeps an already open page as an ordinary tab.
  for (const tab of state.tabs) if (removed.some((f) => f.id === tab.bookmarkId)) { delete tab.bookmarkId; tab.pinned = false; }
  state.favorites = state.favorites.filter((f) => !removed.includes(f));
  for (const f of state.favorites) if (removed.some((item) => item.id === f.splitWith)) delete f.splitWith;
  for (const site of ONBOARDING_SITES) if (unique.has(site.id) && !state.favorites.some((f) => sameFavoriteUrl(f.url, site.url))) {
    state.favorites.push({ id: crypto.randomUUID(), title: site.title, url: site.url });
  }
}

/** The standalone preview opts in, so existing UI fixtures remain useful. */
export const ONBOARDING_PREVIEW_KEY = "rove.onboarding.preview.v1";
export function onboardingPreview(state: BrowserState): BrowserState {
  if (!new URLSearchParams(location.search).has("onboarding")) return state;
  try {
    const saved = JSON.parse(localStorage.getItem(ONBOARDING_PREVIEW_KEY) || "null") as BrowserState | null;
    if (saved && Array.isArray(saved.spaces) && saved.spaces.length && Array.isArray(saved.tabs) && Array.isArray(saved.favorites) && saved.onboarding && ONBOARDING_STEPS.includes(saved.onboarding.step)) return saved;
  } catch { /* A blocked or damaged preview cache starts a fresh preview. */ }
  return { ...state, favorites: [], pins: [], tabs: state.tabs.filter((t) => t.url === "era://newtab"), onboarding: { step: "hello", completed: false } };
}
