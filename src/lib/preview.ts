// Explicit standalone UI preview. Native builds use Rust as the sole state authority.
import type { BrowserState, Command, Tab } from "./types";
import { address, host } from "./address";
import { favorite, MAX_FAVORITES, sameFavoriteUrl } from "./favorites";
import { activateTab, bookmarkCommand, openBookmark, removeBookmark, savedPage, upgradeLibrary } from "./bookmarks";
import { tabsToDismiss } from "./command";
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
export function reduce(s: BrowserState, c: Command): BrowserState {
  upgradeLibrary(s);
  const tab = () => s.tabs.find((t) => t.id === String(c.id || s.activeTab));
  const activate = (t?: Tab) => {
    activateTab(s, t);
  };
  if (bookmarkCommand(s, c, (id) => { Object.assign(s, reduce(s, { type: "close", id })); })) return { ...s };
  switch (c.type) {
    case "favoriteSave": {
      const next = favorite(String(c.title || ""), String(c.url || ""));
      const index = c.previousUrl === undefined ? -1 :
        s.favorites.findIndex((f) => f.url === c.previousUrl);
      if (c.previousUrl !== undefined && index < 0) throw Error("该常用网站已被移除");
      if (s.favorites.some((f, i) => i !== index && sameFavoriteUrl(f.url, next.url)))
        throw Error("该网址已在顶部常用网站中");
      if (index >= 0) s.favorites[index] = { ...s.favorites[index], ...next };
      else {
        if (s.favorites.length >= MAX_FAVORITES) throw Error("最多添加 32 个常用网站");
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
      const url = address(String(c.url || ""));
      const t: Tab = {
        id: crypto.randomUUID(),
        spaceId: s.activeSpace,
        title: host(url) || "New Tab",
        url,
        pinned: false,
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
        t.url = address(String(c.url));
        t.title = host(t.url) || "New Tab";
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
        s.activeSpace = String(c.id);
        s.activeTab = s.tabs.find((t) => t.spaceId === c.id)?.id || "";
        if (!s.activeTab) return reduce(s, { type: "new" });
      }
      break;
    }
    case "spaceCreate": {
      const id = crypto.randomUUID();
      s.spaces.push({
        id,
        name: String(c.name).slice(0, 40) || "Untitled",
        color: (c.color as any) || "sage",
      });
      return reduce(s, { type: "space", id });
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
      for (const id of tabsToDismiss(s, c.type === "closeOthers" ? "others" : "below")) {
        const closing = s.tabs.find((item) => item.id === id);
        if (!closing || closing.bookmarkId) continue;
        s.closed.push({ ...closing, splitWith: undefined });
        s.tabs = s.tabs.filter((item) => item.id !== id);
        s.tabs.forEach((item) => { if (item.splitWith === id) delete item.splitWith; });
      }
      s.closed = s.closed.slice(-30);
      break;
    }
  }
  return { ...s };
}
