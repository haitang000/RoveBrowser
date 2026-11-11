// Adapted from Zigzag Sidebar/TabPreview.swift (Apache-2.0).
// Copyright 2026 Zigao Wang; modified for Vue, CSS pixels and WebView2.
import type { BrowserState, Rect } from "./types";
import { liveTab, savedPage } from "./bookmarks";
import { isSettingsUrl } from "./settings";
import { tabDisplayTitle } from "./tab-meta";

export const TAB_PREVIEW = { delay: 600, hideDelay: 120, refresh: 1000, width: 320, caption: 48, enter: 160, leave: 120, reduced: 80, ease: "cubic-bezier(0.23, 1, 0.32, 1)" };
export interface PreviewPage {
  key: string;
  tabId?: string;
  title: string;
  url: string;
  favicon?: string;
}

/** Resolve saved items without opening them. Either half of the visible split is excluded. */
export function previewPages(state: BrowserState, id: string): PreviewPage[] {
  const saved = savedPage(state, id);
  const tab = state.tabs.find((t) => t.id === id);
  const bookmark = saved || (tab?.bookmarkId ? savedPage(state, tab.bookmarkId) : undefined);
  let pages: PreviewPage[];
  if (bookmark) {
    const marks = [...(state.pins || []), ...state.favorites].filter((f) => f.id === bookmark.id || f.id === bookmark.splitWith);
    if (bookmark.spaceId && bookmark.spaceId !== state.activeSpace) return [];
    pages = marks.map((f) => {
      const live = liveTab(state, f.id);
      return { key: f.id, tabId: live?.id, title: f.title, url: live?.url || f.url, favicon: live?.favicon || f.favicon };
    });
  } else if (tab && tab.spaceId === state.activeSpace) {
    pages = state.tabs.filter((t) => t.id === tab.id || t.id === tab.splitWith || t.splitWith === tab.id)
      .map((t) => ({ key: t.id, tabId: t.id, title: tabDisplayTitle(t), url: t.url, favicon: t.favicon }));
  } else return [];
  // Splitting two existing saved items does not yet change their library groups.
  // Still show the runtime pair, including a one-way splitWith relationship.
  if (pages.length === 1 && pages[0].tabId) {
    const live = state.tabs.find((t) => t.id === pages[0].tabId)!;
    const partner = state.tabs.find((t) => t.id === live.splitWith || t.splitWith === live.id);
    if (partner) {
      const mark = partner.bookmarkId ? savedPage(state, partner.bookmarkId) : undefined;
      pages.push({ key: mark?.id || partner.id, tabId: partner.id, title: mark?.title || tabDisplayTitle(partner), url: partner.url, favicon: partner.favicon });
    }
  }
  const active = state.tabs.find((t) => t.id === state.activeTab);
  return pages.some((p) => isSettingsUrl(p.url) || p.tabId && (p.tabId === active?.id || p.tabId === active?.splitWith)) ? [] : pages;
}

export function previewFrame(row: Rect, sidebar: Rect, page: Rect, viewport: { width: number; height: number }): Rect | undefined {
  const x = Math.round(sidebar.x + sidebar.width + 8);
  const width = Math.min(TAB_PREVIEW.width, viewport.width - x - 16);
  const availableHeight = viewport.height - 64;
  if (width < 160 || availableHeight < 100) return;
  const aspect = page.height > 0 ? Math.min(page.height / Math.max(page.width, 1), 0.9) : 0.62;
  const height = Math.min(Math.round(width * aspect) + TAB_PREVIEW.caption, availableHeight);
  const y = Math.round(Math.max(48, Math.min(row.y + row.height / 2 - height / 2, viewport.height - height - 16)));
  return { x, y, width, height, corners: [14, 14, 14, 14] };
}
