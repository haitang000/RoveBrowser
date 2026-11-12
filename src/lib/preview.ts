// Explicit standalone UI preview. Native builds use Rust as the sole state authority.
import type { BrowserState, Command, Tab } from "./types";
import { address, host } from "./address";
import { favorite, MAX_FAVORITES, sameFavoriteUrl } from "./favorites";
import { activateTab, bookmarkCommand, openBookmark, removeBookmark, savedPage, upgradeLibrary } from "./bookmarks";
import { tabsToDismiss } from "./command";
import { sameOrigin } from "./favicon";
import { isSettingsUrl, settingsUrl, settingsFor, validSearchTemplate, validateShortcuts, type SettingsPane, type Settings } from "./settings";
import { nowSeconds, tabIsFaded } from "./tab-meta";
export function initialState(): BrowserState {
  return upgradeLibrary({
    spaces: [
      { id: "personal", name: "Personal", color: "sky" },
      { id: "work", name: "Work", color: "sage" },
      { id: "studio", name: "Studio", color: "lavender" },
    ],
    activeSpace: "personal",
    activeTab: "start",
    theme: "light",
    sidebarCollapsed: false,
    borderless: true,
    settings: settingsFor(),
    closed: [],
    favorites: [
      { id: "favorite-github", title: "GitHub", url: "https://github.com" },
      { id: "favorite-figma", title: "Figma", url: "https://figma.com" },
      { id: "favorite-notion", title: "Notion", url: "https://notion.so" },
      { id: "favorite-claude", title: "Claude", url: "https://claude.ai" },
      { id: "favorite-gmail", title: "Gmail", url: "https://mail.google.com" },
      { id: "favorite-youtube", title: "YouTube", url: "https://youtube.com" },
      { id: "favorite-spotify", title: "Spotify", url: "https://open.spotify.com" },
      { id: "favorite-linear", title: "Linear", url: "https://linear.app" },
    ],
    tabs: [
      {
        id: "github",
        spaceId: "personal",
        title: "GitHub",
        url: "https://github.com",
        pinned: true,
      },
      {
        id: "wiki",
        spaceId: "personal",
        title: "Wikipedia",
        url: "https://en.wikipedia.org",
        pinned: true,
      },
      {
        id: "start",
        spaceId: "personal",
        title: "New Tab",
        url: "era://newtab",
        pinned: false,
      },
    ],
  });
}
/** Records the tab the current space is showing, so switching back restores it. */
function rememberActive(s: BrowserState) {
  const space = s.spaces.find((item) => item.id === s.activeSpace);
  if (space && s.tabs.some((tab) => tab.id === s.activeTab)) space.activeTab = s.activeTab;
}
function mostRecentTab(s: BrowserState, spaceId: string) {
  let best: Tab | undefined;
  for (const tab of s.tabs) {
    if (tab.spaceId === spaceId && (!best || (tab.lastActivated || 0) > (best.lastActivated || 0))) best = tab;
  }
  return best;
}
/** The tab to show when entering a space: the one it was showing when the user
 * left it, else its most recently shown page. A favorite is shared by every
 * space, so its live tab may currently belong to another one. */
function spaceTab(s: BrowserState, spaceId: string) {
  const space = s.spaces.find((item) => item.id === spaceId);
  const remembered = s.tabs.find((tab) => tab.id === space?.activeTab);
  if (remembered && (remembered.spaceId === spaceId || s.favorites.some((f) => f.id === remembered.bookmarkId)))
    return remembered;
  return mostRecentTab(s, spaceId);
}
/** Switches spaces and restores the tab the target space was showing, opening a
 * blank page only when that space has no tabs left. */
function enterSpace(s: BrowserState, spaceId: string) {
  s.activeSpace = spaceId;
  const target = spaceTab(s, spaceId);
  if (target) activateTab(s, target);
  else reduce(s, { type: "new" });
}
export function reduce(s: BrowserState, c: Command): BrowserState {
  upgradeLibrary(s);
  const tab = () => s.tabs.find((t) => t.id === String(c.id || s.activeTab));
  const activate = (t?: Tab) => {
    activateTab(s, t);
  };
  if (bookmarkCommand(s, c, (id) => { Object.assign(s, reduce(s, { type: "close", id })); })) {
    rememberActive(s);
    return { ...s };
  }
  switch (c.type) {
    case "settingsOpen": {
      const existing = s.tabs.find((t) => t.spaceId === s.activeSpace && isSettingsUrl(t.url)) || s.tabs.find((t) => isSettingsUrl(t.url));
      if (existing) { if (c.pane) existing.url = settingsUrl(c.pane as SettingsPane); activate(existing); }
      else return reduce(s, { type: "new", url: settingsUrl(c.pane as SettingsPane) });
      break;
    }
    case "settingsPane": {
      const t = tab(); if (t && isSettingsUrl(t.url)) t.url = settingsUrl(c.pane as SettingsPane);
      break;
    }
    case "settingsUpdate": {
      const next = settingsFor(c.settings as Settings);
      if (next.customSearchTemplate && !validSearchTemplate(next.customSearchTemplate)) throw Error("搜索地址需要包含 %s，并使用有效的 HTTP 或 HTTPS 网址");
      validateShortcuts(next); s.settings = next; break;
    }
    case "spaceReorder": {
      const index = s.spaces.findIndex((space) => space.id === c.id);
      if (index >= 0 && c.id !== c.before && s.spaces.some((space) => space.id === c.before)) { const [space] = s.spaces.splice(index, 1); s.spaces.splice(s.spaces.findIndex((space) => space.id === c.before), 0, space!); }
      break;
    }
    case "spaceDelete": {
      if (s.spaces.length <= 1) throw Error("至少保留一个工作区");
      const removed = new Set(s.tabs.filter((t) => t.spaceId === c.id && !s.favorites.some((f) => f.id === t.bookmarkId)).map((t) => t.id));
      s.spaces = s.spaces.filter((space) => space.id !== c.id); s.pins = s.pins?.filter((p) => p.spaceId !== c.id);
      if (s.activeSpace === c.id) s.activeSpace = s.spaces[0]!.id;
      s.tabs = s.tabs.filter((t) => !removed.has(t.id));
      s.tabs.forEach((t) => { if (t.spaceId === c.id) t.spaceId = s.activeSpace; if (t.splitWith && removed.has(t.splitWith)) delete t.splitWith; });
      s.closed = s.closed.filter((t) => t.spaceId !== c.id);
      if (!s.tabs.some((t) => t.id === s.activeTab)) {
        // Deleting the current space lands in the first remaining one, which
        // restores the tab it was showing.
        s.activeTab = "";
        enterSpace(s, s.activeSpace);
      }
      break;
    }
    case "favoriteSave": {
      const next = favorite(String(c.title || ""), String(c.url || ""));
      const index = c.previousUrl === undefined ? -1 :
        s.favorites.findIndex((f) => f.url === c.previousUrl);
      if (c.previousUrl !== undefined && index < 0) throw Error("该常用网站已被移除");
      if (s.favorites.some((f, i) => i !== index && sameFavoriteUrl(f.url, next.url)))
        throw Error("该网址已在顶部常用网站中");
      if (index >= 0) s.favorites[index] = { ...s.favorites[index], ...next, favicon: sameOrigin(s.favorites[index].url, next.url) ? s.favorites[index].favicon : undefined };
      else {
        if (s.favorites.length >= MAX_FAVORITES) throw Error("最多置顶 8 个网站");
        s.favorites.push({ id: crypto.randomUUID(), ...next });
      }
      break;
    }
    case "favoriteRemove":
      { const f = s.favorites.find((f) => f.url === c.url); if (f) removeBookmark(s, f.id); }
      break;
    case "favoriteMove": {
      const index = s.favorites.findIndex((f) => f.url === c.url);
      const offset = Number(c.offset);
      const target = index + offset;
      if (index >= 0 && (offset === -1 || offset === 1) && target >= 0 && target < s.favorites.length)
        [s.favorites[index], s.favorites[target]] = [s.favorites[target], s.favorites[index]];
      break;
    }
    case "new": {
      const url = address(String(c.url || ""), s.settings);
      const t: Tab = {
        id: crypto.randomUUID(),
        spaceId: s.activeSpace,
        title: isSettingsUrl(url) ? "设置" : host(url) || "New Tab",
        url,
        pinned: false,
        lastActivated: nowSeconds(),
      };
      s.tabs.push(t);
      activate(t);
      break;
    }
    case "activate":
      if (savedPage(s, String(c.id))) openBookmark(s, String(c.id)); else activate(tab());
      break;
    case "navigate": {
      const t = tab();
      if (t) {
        t.url = address(String(c.url), s.settings);
        t.title = isSettingsUrl(t.url) ? "设置" : host(t.url) || "New Tab";
      }
      break;
    }
    case "close": {
      const t = tab();
      if (t) {
        const partner = t.splitWith || s.tabs.find((x) => x.splitWith === t.id)?.id;
        if (!t.bookmarkId) { s.closed.push({ ...t }); s.closed = s.closed.slice(-30); }
        s.tabs = s.tabs.filter((x) => x.id !== t.id);
        s.tabs.forEach((x) => {
          if (x.splitWith === t.id) delete x.splitWith;
        });
        if (t.id === s.activeTab)
          activate(s.tabs.find((x) => x.id === partner) || s.tabs.filter((x) => x.spaceId === s.activeSpace).at(-1));
        if (!s.tabs.some((x) => x.id === s.activeTab)) {
          s.activeTab = "";
          return reduce(s, { type: "new" });
        }
      }
      break;
    }
    case "restore": {
      const t = s.closed.pop();
      if (t) {
        t.splitWith = undefined;
        t.bookmarkId = undefined;
        t.pinned = false;
        s.tabs.push(t);
        activate(t);
      }
      break;
    }
    case "move": {
      const t = tab();
      if (t && s.spaces.some((x) => x.id === c.spaceId)) {
        t.spaceId = String(c.spaceId);
        s.tabs.forEach((x) => {
          if (x.splitWith === t.id) delete x.splitWith;
        });
        delete t.splitWith;
        if (t.id === s.activeTab) s.activeSpace = t.spaceId;
      }
      break;
    }
    case "space": {
      if (s.spaces.some((x) => x.id === c.id)) {
        // Keep what the space we are leaving is showing, then restore what the
        // target space was showing when it was left.
        rememberActive(s);
        enterSpace(s, String(c.id));
      }
      break;
    }
    case "spaceCreate": {
      if (s.spaces.length >= 16) throw Error("最多创建 16 个工作区");
      const id = crypto.randomUUID();
      s.spaces.push({
        id,
        name: String(c.name).slice(0, 40) || "Untitled",
        color: (c.color as any) || "sage",
        translucency: Number(c.translucency) > .005 ? Math.min(1, Number(c.translucency)) : undefined,
        clearGlass: c.clearGlass === false ? false : undefined,
      });
      if (!c.background) return reduce(s, { type: "space", id });
      break;
    }
    case "spaceUpdate": {
      const space = s.spaces.find((x) => x.id === c.id);
      if (space) {
        space.name = String(c.name).slice(0, 40) || space.name;
        space.color = c.color as any;
        const amount = Number(c.translucency);
        space.translucency = amount > 0.005 ? Math.min(1, amount) : undefined;
        space.clearGlass = c.clearGlass === false ? false : undefined;
      }
      break;
    }
    case "borderless":
      s.borderless = s.borderless === false;
      break;
    case "theme":
      s.theme = s.theme === "light" ? "dark" : "light";
      break;
    case "sidebar":
      s.sidebarCollapsed = !s.sidebarCollapsed;
      break;
    case "split": {
      const t = tab(),
        other = s.tabs.find((x) => x.id === c.other);
      if (t && other && t.id !== other.id && t.spaceId === other.spaceId)
        t.splitWith = other.id;
      break;
    }
    case "unsplit": {
      const t = tab();
      if (t) delete t.splitWith;
      break;
    }
    case "closeOthers":
    case "closeBelow": {
      // The tab menu can target a background tab; the command bar uses the active one.
      const base = c.id ? String(c.id) : undefined;
      for (const id of tabsToDismiss(s, c.type === "closeOthers" ? "others" : "below", base)) {
        const closing = s.tabs.find((item) => item.id === id);
        if (!closing || closing.bookmarkId) continue;
        s.closed.push({ ...closing, splitWith: undefined });
        s.tabs = s.tabs.filter((item) => item.id !== id);
        s.tabs.forEach((item) => { if (item.splitWith === id) delete item.splitWith; });
      }
      s.closed = s.closed.slice(-30);
      if (!s.tabs.some((item) => item.id === s.activeTab)) {
        s.activeTab = "";
        const fallback = s.tabs.filter((item) => item.spaceId === s.activeSpace).at(-1);
        if (fallback) activate(fallback); else return reduce(s, { type: "new" });
      }
      break;
    }
    case "renameTab": {
      const t = tab();
      if (t && !t.bookmarkId) {
        const title = String(c.title ?? "").trim().slice(0, 80);
        if (title) t.customTitle = title; else delete t.customTitle;
      }
      break;
    }
    case "duplicate": {
      const source = tab();
      if (source && !source.bookmarkId) {
        const duplicate: Tab = {
          ...source,
          id: crypto.randomUUID(),
          splitWith: undefined,
          lastActivated: nowSeconds(),
        };
        const index = s.tabs.findIndex((item) => item.id === source.id);
        s.tabs.splice(index + 1, 0, duplicate);
        activate(duplicate);
      }
      break;
    }
    case "clearStep": {
      // The preview mirrors Zigzag's ladder without the native Undo batch.
      const pages = s.tabs.filter((item) => item.spaceId === s.activeSpace && !item.bookmarkId);
      const current = pages.find((item) => item.id === s.activeTab);
      const partner = current?.splitWith || pages.find((item) => item.splitWith === current?.id)?.id;
      const others = pages.filter((item) => item.id !== current?.id && item.id !== partner);
      const now = nowSeconds();
      const faded = others.filter((item) => tabIsFaded(item.lastActivated, now));
      const targets = faded.length ? faded : others.length ? others : current ? [current] : [];
      for (const target of targets) {
        s.closed.push({ ...target, splitWith: undefined });
        s.tabs = s.tabs.filter((item) => item.id !== target.id);
        s.tabs.forEach((item) => { if (item.splitWith === target.id) delete item.splitWith; });
      }
      s.closed = s.closed.slice(-30);
      if (!s.tabs.some((item) => item.id === s.activeTab)) {
        s.activeTab = "";
        const fallback = s.tabs.filter((item) => item.spaceId === s.activeSpace).at(-1);
        if (fallback) activate(fallback); else return reduce(s, { type: "new" });
      }
      break;
    }
    case "restoreBatch": {
      // Preview-only counterpart of the host's Undo: put the batch back at its
      // original indexes, drop the blank tab a clear-all auto-created, then
      // return to the tab that was active before the batch.
      for (const id of (c.blankIds as string[] | undefined) ?? []) {
        s.tabs = s.tabs.filter((tab) => tab.id !== id);
      }
      const entries = (c.entries as { index: number; tab: Tab }[] | undefined) ?? [];
      const restored: Tab[] = [];
      for (const entry of [...entries].sort((a, b) => a.index - b.index)) {
        if (s.tabs.some((tab) => tab.id === entry.tab.id)) continue;
        const tab: Tab = { ...entry.tab, splitWith: undefined, lastActivated: nowSeconds() };
        s.tabs.splice(Math.min(entry.index, s.tabs.length), 0, tab);
        restored.push(tab);
      }
      const previous = s.tabs.find((tab) => tab.id === String(c.active ?? ""));
      const target = previous || restored[0];
      if (target) activate(target);
      break;
    }
  }
  rememberActive(s);
  return { ...s };
}
