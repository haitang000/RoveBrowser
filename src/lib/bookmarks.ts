// Port of Zigzag Library / TabManager behavior (Apache-2.0). The native model is authoritative.
import type { BrowserState, Command, Favorite, Tab } from "./types";
import { favoriteUrl, MAX_FAVORITES } from "./favorites";
import { sameOrigin } from "./favicon";

export function samePinnedUrl(a: string, b: string) {
  const key = (input: string) => {
    const u = new URL(input);
    u.hash = "";
    if (u.protocol === "http:") u.protocol = "https:";
    u.hostname = u.hostname.replace(/^www\./, "");
    return `${u.origin}${u.pathname.replace(/\/+$/, "")}${u.search}`;
  };
  try { return key(a) === key(b); } catch { return a === b; }
}

export function savedPage(s: BrowserState, id: string) {
  return [...(s.pins || []), ...s.favorites].find((f) => f.id === id);
}
export function bookmarkId(s: BrowserState, id: string) {
  return savedPage(s, id)?.id || s.tabs.find((t) => t.id === id)?.bookmarkId;
}
export function bookmarkTitle(title: string, input: string) {
  const url = new URL(input);
  if (url.pathname.replace(/\//g, "") || url.search) return title;
  const host = url.hostname.replace(/^www\./, "").split(".")[0].toLowerCase();
  for (const separator of [" · ", " | ", " - ", " — ", " – ", ": "]) {
    const parts = title.split(separator);
    if (parts.length > 1 && host)
      for (const part of [parts[0], parts.at(-1)!])
        if (part.toLowerCase().replaceAll(" ", "").includes(host)) return part;
  }
  return title;
}
export function upgradeLibrary(s: BrowserState) {
  s.pins ||= [];
  s.favorites.forEach((f) => { f.id ||= crypto.randomUUID(); });
  if (!s.libraryVersion) {
    for (const t of s.tabs.filter((t) => t.pinned)) {
      t.bookmarkId ||= `pin-${t.id}`;
      s.pins.push({ id: t.bookmarkId, title: t.title, url: t.url, favicon: t.favicon, spaceId: t.spaceId });
    }
    for (const t of s.tabs) {
      const other = s.tabs.find((x) => x.id === t.splitWith);
      if (t.bookmarkId && other?.bookmarkId) {
        savedPage(s, t.bookmarkId)!.splitWith = other.bookmarkId;
        savedPage(s, other.bookmarkId)!.splitWith = t.bookmarkId;
      }
    }
    s.libraryVersion = 1;
  }
  const overflow = new Set(s.favorites.slice(MAX_FAVORITES).map((f) => f.id));
  for (const f of s.favorites) if (overflow.has(f.id) && f.splitWith) overflow.add(f.splitWith);
  s.pins.push(...s.favorites.filter((f) => overflow.has(f.id)).map((f) => ({ ...f, spaceId: s.activeSpace })));
  s.favorites = s.favorites.filter((f) => !overflow.has(f.id));
  for (const t of s.tabs) {
    const pin = s.pins.find((f) => f.id === t.bookmarkId);
    if (pin) { t.pinned = true; t.spaceId = pin.spaceId!; }
  }
  return s;
}
export function group(s: BrowserState, id: string) {
  const partner = savedPage(s, id)?.splitWith;
  return [...(s.pins || []), ...s.favorites].filter((f) => f.id === id || f.id === partner);
}
export function liveTab(s: BrowserState, id: string) { return s.tabs.find((t) => t.bookmarkId === id); }
export function activateTab(s: BrowserState, t?: Tab) {
  if (!t) return;
  if (s.favorites.some((f) => f.id === t.bookmarkId)) {
    t.spaceId = s.activeSpace;
    const other = s.tabs.find((x) => x.id === t.splitWith || x.splitWith === t.id);
    if (other) other.spaceId = s.activeSpace;
  }
  s.activeTab = t.id; s.activeSpace = t.spaceId;
}
export function openBookmark(s: BrowserState, id: string) {
  const marks = group(s, id);
  if (!marks.length) throw Error("固定项不存在");
  const missing = marks.filter((f) => !liveTab(s, f.id)).length;
  if (s.tabs.length + missing > 256) throw Error("已达到 256 个标签页上限");
  const tabs = marks.map((f) => {
    let t = liveTab(s, f.id);
    if (!t) {
      t = { id: crypto.randomUUID(), spaceId: f.spaceId || s.activeSpace, title: f.title, url: f.url, favicon: f.favicon, pinned: !!f.spaceId, bookmarkId: f.id };
      s.tabs.push(t);
    }
    return t;
  });
  if (tabs.length === 2) { tabs[0].splitWith = tabs[1].id; tabs[1].splitWith = tabs[0].id; }
  activateTab(s, liveTab(s, id));
}
export function removeBookmark(s: BrowserState, id: string) {
  const ids = group(s, id).map((f) => f.id);
  s.pins = s.pins!.filter((f) => !ids.includes(f.id));
  s.favorites = s.favorites.filter((f) => !ids.includes(f.id));
  for (const t of s.tabs) if (t.bookmarkId && ids.includes(t.bookmarkId)) {
    t.bookmarkId = undefined; t.pinned = false; t.spaceId = s.activeSpace;
  }
}
function keep(s: BrowserState, id: string, favorite: boolean, space?: string) {
  const saved = bookmarkId(s, id);
  let marks: Favorite[];
  if (saved) {
    marks = group(s, saved);
    if (favorite && s.favorites.length + marks.filter((f) => f.spaceId).length > MAX_FAVORITES) throw Error("最多置顶 8 个网站");
    s.pins = s.pins!.filter((f) => !marks.includes(f)); s.favorites = s.favorites.filter((f) => !marks.includes(f));
  } else {
    const tab = s.tabs.find((t) => t.id === id);
    if (!tab) throw Error("标签页不存在");
    const other = s.tabs.find((t) => t.id === tab.splitWith || t.splitWith === tab.id);
    const tabs = other && !other.bookmarkId ? [tab, other] : [tab];
    for (const t of tabs) favoriteUrl(t.url);
    if (favorite && s.favorites.length + tabs.length > MAX_FAVORITES) throw Error("最多置顶 8 个网站");
    marks = tabs.map((t) => ({ id: crypto.randomUUID(), title: bookmarkTitle(t.title, t.url), url: t.url, favicon: t.favicon, spaceId: space || t.spaceId }));
    if (marks.length === 2) { marks[0].splitWith = marks[1].id; marks[1].splitWith = marks[0].id; }
    tabs.forEach((t, i) => { t.bookmarkId = marks[i].id; });
  }
  marks.forEach((f) => {
    f.spaceId = favorite ? undefined : space || f.spaceId || s.activeSpace;
    const t = liveTab(s, f.id);
    if (t) { t.pinned = !favorite; t.spaceId = f.spaceId || s.activeSpace; }
  });
  const live = marks.flatMap((f) => liveTab(s, f.id) || []).map((t) => t.id);
  s.tabs.forEach((t) => { if (t.splitWith && live.includes(t.id) !== live.includes(t.splitWith)) t.splitWith = undefined; });
  (favorite ? s.favorites : s.pins!).push(...marks);
  return marks[0].id;
}
function unpin(s: BrowserState, id: string, space?: string) {
  const mark = bookmarkId(s, id);
  if (!mark) return;
  const target = space || savedPage(s, mark)?.spaceId || s.activeSpace;
  if (!liveTab(s, mark)) openBookmark(s, mark);
  const tabs = group(s, mark).flatMap((f) => liveTab(s, f.id) || []);
  removeBookmark(s, mark);
  tabs.forEach((t) => { t.spaceId = target; });
  const active = s.tabs.find((t) => t.id === s.activeTab);
  if (active) s.activeSpace = active.spaceId;
}
function moveBefore(items: Favorite[], id: string, before: string) {
  if (id === before || !items.some((f) => f.id === before)) return;
  const partner = items.find((f) => f.id === id)?.splitWith;
  const targetPartner = items.find((f) => f.id === before)?.splitWith;
  if (partner === before) return;
  const moving = items.filter((f) => f.id === id || f.id === partner);
  for (const item of moving) items.splice(items.indexOf(item), 1);
  items.splice(items.findIndex((f) => f.id === before || f.id === targetPartner), 0, ...moving);
}
export function bookmarkCommand(s: BrowserState, c: Command, close: (id: string) => void): boolean {
  const id = String(c.id || s.activeTab);
  switch (c.type) {
    case "favoriteMove": {
      const groups: Favorite[][] = [];
      for (const f of s.favorites) {
        const own = groups.find((g) => g.some((p) => p.splitWith === f.id));
        if (own) own.push(f); else groups.push([f]);
      }
      const at = groups.findIndex((g) => g.some((f) => f.url === c.url)), offset = Number(c.offset), to = at + offset;
      if ((offset === -1 || offset === 1) && at >= 0 && to >= 0 && to < groups.length) { [groups[at], groups[to]] = [groups[to], groups[at]]; s.favorites = groups.flat(); }
      break;
    }
    case "split": {
      const resolve = (value: unknown) => {
        const id = String(value); if (savedPage(s, id)) { openBookmark(s, id); return liveTab(s, id)!; } return s.tabs.find((t) => t.id === id);
      };
      const a = resolve(c.id), b = resolve(c.other);
      if (a && b && a.id !== b.id && a.spaceId === b.spaceId) { a.splitWith = b.id; activateTab(s, a); }
      break;
    }
    case "unsplit": {
      const active = s.tabs.find((t) => t.id === s.activeTab);
      const other = active?.splitWith;
      s.tabs.forEach((t) => { if (t.id === s.activeTab || t.id === other || t.splitWith === s.activeTab) t.splitWith = undefined; });
      break;
    }
    case "bookmarkOpen": openBookmark(s, id); break;
    case "bookmarkUnload": group(s, id).flatMap((f) => liveTab(s, f.id) || []).forEach((t) => close(t.id)); break;
    case "bookmarkReset": { const t = liveTab(s, id), f = savedPage(s, id); if (t && f) { t.url = f.url; t.favicon = f.favicon; } break; }
    case "bookmarkSeparate": {
      const t = liveTab(s, id);
      if (t) { t.bookmarkId = undefined; t.pinned = false; t.spaceId = s.activeSpace; t.splitWith = undefined;
        s.tabs.forEach((x) => { if (x.splitWith === t.id) x.splitWith = undefined; }); activateTab(s, t); }
      break;
    }
    case "bookmarkUpdate": {
      const url = favoriteUrl(String(c.url)), title = String(c.title).trim();
      if (!title || Array.from(title).length > 80) throw Error("请输入 1–80 个字符的网站名称");
      const f = savedPage(s, id); if (f) Object.assign(f, { title, url, favicon: sameOrigin(f.url, url) ? f.favicon : undefined }); break;
    }
    case "bookmarkUnpin": unpin(s, id, c.spaceId as string | undefined); break;
    case "bookmarkPin": keep(s, id, false, c.spaceId as string | undefined); break;
    case "favoriteTab": { const mark = keep(s, id, true); if (c.before) moveBefore(s.favorites, mark, String(c.before)); break; }
    case "pin": if (bookmarkId(s, id)) unpin(s, id); else keep(s, id, false); break;
    case "move": if (!bookmarkId(s, id)) return false; keep(s, id, false, String(c.spaceId)); activateTab(s, s.tabs.find((t) => t.id === s.activeTab)); break;
    case "reorder": {
      const target = bookmarkId(s, String(c.before));
      if (target && s.pins!.some((f) => f.id === target)) {
        const mark = keep(s, id, false, savedPage(s, target)!.spaceId); moveBefore(s.pins!, mark, target);
      } else {
        const mark = bookmarkId(s, id);
        if (mark) openBookmark(s, mark);
        const source = mark ? liveTab(s, mark)!.id : id;
        if (mark) unpin(s, id, s.activeSpace);
        if (source !== c.before) { const index = s.tabs.findIndex((t) => t.id === source); if (index >= 0 && s.tabs.some((t) => t.id === c.before)) { const [t] = s.tabs.splice(index, 1); s.tabs.splice(s.tabs.findIndex((t) => t.id === c.before), 0, t); } }
      }
      break;
    }
    default: return false;
  }
  return true;
}
