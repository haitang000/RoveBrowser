// Explicit standalone UI preview. Native builds use Rust as the sole state authority.
import type { BrowserState, Command, Tab } from "./types";
import { address, host } from "./address";
import { favorite, MAX_FAVORITES, sameFavoriteUrl } from "./favorites";
export function initialState(): BrowserState {
  return {
    spaces: [
      { id: "personal", name: "Personal", color: "sky" },
      { id: "work", name: "Work", color: "sage" },
      { id: "studio", name: "Studio", color: "lavender" },
    ],
    activeSpace: "personal",
    activeTab: "start",
    theme: "light",
    sidebarCollapsed: false,
    closed: [],
    favorites: [
      { title: "GitHub", url: "https://github.com" },
      { title: "Figma", url: "https://figma.com" },
      { title: "Notion", url: "https://notion.so" },
      { title: "Claude", url: "https://claude.ai" },
      { title: "Gmail", url: "https://mail.google.com" },
      { title: "YouTube", url: "https://youtube.com" },
      { title: "Spotify", url: "https://open.spotify.com" },
      { title: "Linear", url: "https://linear.app" },
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
  };
}
export function reduce(s: BrowserState, c: Command): BrowserState {
  const tab = () => s.tabs.find((t) => t.id === String(c.id || s.activeTab));
  const activate = (t?: Tab) => {
    if (t) {
      s.activeTab = t.id;
      s.activeSpace = t.spaceId;
    }
  };
  switch (c.type) {
    case "favoriteSave": {
      const next = favorite(String(c.title || ""), String(c.url || ""));
      const index = c.previousUrl === undefined ? -1 :
        s.favorites.findIndex((f) => f.url === c.previousUrl);
      if (c.previousUrl !== undefined && index < 0) throw Error("该常用网站已被移除");
      if (s.favorites.some((f, i) => i !== index && sameFavoriteUrl(f.url, next.url)))
        throw Error("该网址已在顶部常用网站中");
      if (index >= 0) s.favorites[index] = next;
      else {
        if (s.favorites.length >= MAX_FAVORITES) throw Error("最多添加 32 个常用网站");
        s.favorites.push(next);
      }
      break;
    }
    case "favoriteRemove":
      s.favorites = s.favorites.filter((f) => f.url !== c.url);
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
      activate(tab());
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
        s.closed.push({ ...t });
        s.closed = s.closed.slice(-30);
        s.tabs = s.tabs.filter((x) => x.id !== t.id);
        s.tabs.forEach((x) => {
          if (x.splitWith === t.id) delete x.splitWith;
        });
        if (t.id === s.activeTab)
          activate(s.tabs.filter((x) => x.spaceId === s.activeSpace).at(-1));
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
        s.tabs.push(t);
        activate(t);
      }
      break;
    }
    case "pin": {
      const t = tab();
      if (t) t.pinned = !t.pinned;
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
    case "reorder": {
      const a = s.tabs.findIndex((t) => t.id === c.id),
        b = s.tabs.findIndex((t) => t.id === c.before);
      if (a >= 0 && b >= 0 && s.tabs[a].spaceId === s.tabs[b].spaceId) {
        const [t] = s.tabs.splice(a, 1);
        s.tabs.splice(
          s.tabs.findIndex((x) => x.id === c.before),
          0,
          t,
        );
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
      }
      break;
    }
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
  }
  return { ...s };
}
