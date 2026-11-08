// Command ranking, site search and inline completion adapted from Zigzag
// CommandBarModel.swift, SiteSearch.swift and InlineCompletion.swift.
// Copyright 2026 Zigao Wang, Apache-2.0. Modified for Rove: only commands the
// Windows app can run, selectable search engines, and a reduced public-suffix list.
import type { BrowserState, HistoryEntry, Tab } from "./types";
import { isSettingsUrl, normalizeSettingsUrl, searchName } from "./settings";

export const COMMAND_LIMIT = 8;
const LOCAL_TLDS = new Set(["local", "lan", "internal", "test", "localhost", "home", "arpa", "corp", "intranet"]);
const KNOWN_TLDS = new Set(
  "com org net io app dev ai so page xyz me co uk de fr jp cn kr tw hk edu gov info blog wiki news media tv gg sh to cc ly fm im is it nl se no fi es pt br au nz ca us ru in id sg my th vn ph za ie ch at be pl cz hu ro ua il ae sa tr mx ar cl pe dk example".split(" "),
);
const MULTI_SUFFIX = new Set(["co.uk", "com.au", "com.cn", "co.jp", "com.br", "co.kr", "com.tw", "com.hk", "com.sg", "co.nz", "com.mx", "co.za", "com.tr", "co.in"]);

export interface SiteSearch {
  name: string;
  keys: string[];
  template: string;
  home: string;
}
export interface CompletionSite { url: string; score: number; }
export interface CompletionMatch { text: string; url: string; }
export type CommandKind =
  | { type: "open"; url: string }
  | { type: "search"; query: string }
  | { type: "searchOn"; url: string }
  | { type: "switchTo"; id: string }
  | { type: "bookmark"; id: string; favorite: boolean }
  | { type: "history"; url: string }
  | { type: "suggestion"; query: string }
  | { type: "action"; id: string }
  | { type: "note" };
export interface CommandRow {
  kind: CommandKind;
  title: string;
  detail: string;
  icon: string;
  action: string;
  key: string;
}
interface Action { id: string; title: string; keywords: string[]; icon: string; }
export interface CommandContext {
  hasPage: boolean;
  isPinned: boolean;
  isFavorite: boolean;
  inSplit: boolean;
  splitPartner?: { id: string; title: string };
  canMove: boolean;
  hasOtherTabs: boolean;
  hasTabsBelow: boolean;
  canReopen: boolean;
  spaces: { id: string; name: string }[];
  currentSpaceId?: string;
}

const SITES: SiteSearch[] = [
  site("YouTube", ["youtube", "youtube.com", "yt"], "https://www.youtube.com/results?search_query=%s", "https://www.youtube.com"),
  site("GitHub", ["github", "github.com", "gh"], "https://github.com/search?q=%s&type=repositories", "https://github.com"),
  site("AvoidXray", ["avoidxray", "avoidxray.com", "axr"], "https://avoidxray.com/search?q=%s", "https://avoidxray.com"),
  site("Google", ["google", "google.com"], "https://www.google.com/search?q=%s", "https://www.google.com"),
  site("Wikipedia", ["wikipedia", "wikipedia.org", "wiki"], "https://en.wikipedia.org/w/index.php?search=%s", "https://en.wikipedia.org"),
  site("Claude", ["claude", "claude.ai"], "https://claude.ai/new?q=%s", "https://claude.ai"),
  site("ChatGPT", ["chatgpt", "chatgpt.com", "gpt"], "https://chatgpt.com/?q=%s", "https://chatgpt.com"),
  site("Perplexity", ["perplexity", "perplexity.ai"], "https://www.perplexity.ai/search?q=%s", "https://www.perplexity.ai"),
  site("Reddit", ["reddit", "reddit.com"], "https://www.reddit.com/search/?q=%s", "https://www.reddit.com"),
  site("X", ["x.com", "twitter", "twitter.com"], "https://x.com/search?q=%s", "https://x.com"),
  site("Google Maps", ["maps", "google maps", "maps.google.com"], "https://www.google.com/maps/search/%s", "https://www.google.com/maps"),
  site("Google Images", ["images", "google images"], "https://www.google.com/search?tbm=isch&q=%s", "https://images.google.com"),
  site("Amazon", ["amazon", "amazon.com"], "https://www.amazon.com/s?k=%s", "https://www.amazon.com"),
  site("Stack Overflow", ["stackoverflow", "stack overflow", "stackoverflow.com", "so"], "https://stackoverflow.com/search?q=%s", "https://stackoverflow.com"),
  site("MDN", ["mdn", "developer.mozilla.org"], "https://developer.mozilla.org/en-US/search?q=%s", "https://developer.mozilla.org"),
  site("npm", ["npm", "npmjs.com"], "https://www.npmjs.com/search?q=%s", "https://www.npmjs.com"),
  site("Apple Developer", ["apple developer", "developer.apple.com"], "https://developer.apple.com/search/?q=%s", "https://developer.apple.com"),
  site("Spotify", ["spotify", "open.spotify.com"], "https://open.spotify.com/search/%s", "https://open.spotify.com"),
  site("IMDb", ["imdb", "imdb.com"], "https://www.imdb.com/find/?q=%s", "https://www.imdb.com"),
  site("Bilibili", ["bilibili", "bilibili.com", "bili"], "https://search.bilibili.com/all?keyword=%s", "https://www.bilibili.com"),
  site("DuckDuckGo", ["duckduckgo", "ddg", "duckduckgo.com"], "https://duckduckgo.com/?q=%s", "https://duckduckgo.com"),
];
function site(name: string, keys: string[], template: string, home: string): SiteSearch {
  return { name, keys, template, home };
}

export function siteSearchUrl(target: SiteSearch, query: string): string {
  return target.template.replace("%s", encodeURIComponent(query).replace(/[!'()*]/g, (c) => `%${c.charCodeAt(0).toString(16).toUpperCase()}`));
}
export function matchSite(typed: string): SiteSearch | undefined {
  const text = typed.trim().toLowerCase();
  if (text.length < 2) return;
  const bare = text.startsWith("www.") ? text.slice(4) : text;
  const exact = SITES.find((item) => item.keys.includes(bare));
  if (exact) return exact;
  const spaced = bare.includes(" ") && SITES.some((item) => item.keys.some((key) => key.startsWith(bare) && key.includes(" ")));
  if (bare.length < 3 || (bare.includes(" ") && !spaced)) return;
  return SITES.find((item) => item.keys.some((key) => key.startsWith(bare)));
}

export function urlFrom(text: string): string | undefined {
  const trimmed = text.trim();
  if (isSettingsUrl(trimmed)) return normalizeSettingsUrl(trimmed);
  if (!trimmed || /\s/.test(trimmed)) return;
  const lower = trimmed.toLowerCase();
  if (lower.startsWith("http://") || lower.startsWith("https://")) {
    try { const url = new URL(trimmed); return url.protocol === "http:" || url.protocol === "https:" ? url.href : undefined; } catch { return; }
  }
  const hostEnd = lower.search(/[/?#]/);
  const hostPart = hostEnd < 0 ? lower : lower.slice(0, hostEnd);
  if (hostPart.includes("@")) return;
  const host = hostPart.split(":")[0] || hostPart;
  if (host === "localhost" || /^\d{1,3}(\.\d{1,3}){3}$/.test(host)) return `http://${trimmed}`;
  const labels = host.split(".");
  const tld = labels.at(-1) || "";
  if (labels.length < 2 || labels.some((label) => !label)) return;
  if (LOCAL_TLDS.has(tld)) return `http://${trimmed}`;
  if (!KNOWN_TLDS.has(tld) && !tld.startsWith("xn--")) return;
  return `https://${trimmed}`;
}
export function displayUrl(value: string): string {
  let text = value;
  for (const prefix of ["https://", "http://"]) if (text.startsWith(prefix)) text = text.slice(prefix.length);
  if (text.startsWith("www.")) text = text.slice(4);
  if (text.endsWith("/")) text = text.slice(0, -1);
  return text;
}
export function suggestionQuery(text: string): string | undefined {
  const trimmed = text.trim();
  return trimmed.length >= 2 && !urlFrom(trimmed) ? trimmed : undefined;
}
export function parseSuggestions(data: string): string[] {
  try {
    const json = JSON.parse(data) as unknown;
    if (Array.isArray(json) && Array.isArray(json[1])) return json[1].filter((item): item is string => typeof item === "string");
    if (Array.isArray(json)) return json.flatMap((item) => item && typeof item === "object" && "phrase" in item && typeof item.phrase === "string" ? [item.phrase] : []);
  } catch { /* remote suggestions are optional */ }
  return [];
}

const GLOBAL_ACTIONS: Action[] = [
  { id: "settings", title: "Settings", keywords: ["settings", "preferences", "options"], icon: "settings" },
];
function wordsOf(text: string): string[] {
  return text.toLowerCase().split(" ").filter(Boolean);
}
function titleWords(title: string): string[] {
  return title.toLowerCase().split(/[^\p{L}\p{N}]+/u).filter(Boolean);
}
function isNamed(action: Action, words: string[]): boolean {
  const title = titleWords(action.title);
  return words.length > 0 && words.every((word) => title.some((part) => part.startsWith(word)));
}
export function contextActions(c: CommandContext): Action[] {
  const list: Action[] = [];
  if (c.hasPage && !c.isFavorite) {
    list.push(c.isPinned
      ? { id: "unpin", title: "Unpin Tab", keywords: ["unpin", "pin", "tab", "remove"], icon: "pin" }
      : { id: "pin", title: "Pin Tab", keywords: ["pin", "tab", "keep", "bookmark"], icon: "pin" });
    list.push({ id: "favorite", title: "Add to Favorites", keywords: ["add", "favorites", "favourites", "star", "bookmark"], icon: "plus" });
  }
  if (c.inSplit) list.push({ id: "separate", title: "Separate Tabs", keywords: ["separate", "tabs", "split", "unsplit", "view", "close"], icon: "split" });
  else if (c.splitPartner) list.push({ id: "split", title: "Open in Split View", keywords: ["open", "split", "view", "side", "two"], icon: "split" });
  if (c.hasOtherTabs) list.push({ id: "close-others", title: "Close Other Tabs", keywords: ["close", "other", "tabs", "clean", "tidy"], icon: "close" });
  if (c.hasTabsBelow) list.push({ id: "close-below", title: "Close Tabs Below", keywords: ["close", "tabs", "below", "under"], icon: "close" });
  if (c.canReopen) list.push({ id: "reopen", title: "Reopen Closed Tab", keywords: ["reopen", "closed", "tab", "undo", "restore", "back"], icon: "history" });
  list.push({ id: "new-space", title: "New Space", keywords: ["new", "space", "create", "add"], icon: "plus" });
  for (const space of c.spaces) if (space.id !== c.currentSpaceId) {
    const name = wordsOf(space.name);
    list.push({ id: `space:${space.id}`, title: `Switch to ${space.name}`, keywords: ["switch", "go", "space", ...name], icon: "arrow" });
    if (c.canMove) list.push({ id: `move:${space.id}`, title: `Move to ${space.name}`, keywords: ["move", "tab", "space", "send", ...name], icon: "arrow" });
  }
  return list;
}
function matchingActions(query: string, context: CommandContext): Action[] {
  const words = wordsOf(query);
  if (!words.length) return [];
  const candidates = [...(query.trim().length >= 3 ? contextActions(context) : []), ...GLOBAL_ACTIONS];
  return candidates.filter((action) => words.every((word) => action.keywords.some((key) => key.startsWith(word))));
}

function findLastIndex<T>(items: T[], match: (item: T) => boolean): number {
  for (let index = items.length - 1; index >= 0; index -= 1) if (match(items[index])) return index;
  return -1;
}
function partnerId(state: BrowserState, id: string): string | undefined {
  const tab = state.tabs.find((item) => item.id === id);
  return tab?.splitWith || state.tabs.find((item) => item.splitWith === id)?.id;
}
function regularIds(state: BrowserState): string[] {
  return state.tabs.filter((tab) => tab.spaceId === state.activeSpace && !tab.bookmarkId).map((tab) => tab.id);
}
export function tabsToDismiss(state: BrowserState, mode: "others" | "below"): string[] {
  const active = state.tabs.find((tab) => tab.id === state.activeTab);
  const pages = regularIds(state);
  const partner = active ? partnerId(state, active.id) : undefined;
  if (!active || active.bookmarkId) return mode === "others" ? pages.filter((id) => id !== partner) : pages;
  if (mode === "others") return pages.filter((id) => id !== active.id && id !== partner);
  const last = findLastIndex(pages, (id) => id === active.id || id === partner);
  return pages.slice(last + 1);
}
export function commandContext(state: BrowserState, recent: string[]): CommandContext {
  const active = state.tabs.find((tab) => tab.id === state.activeTab);
  const saved = active?.bookmarkId ? [...(state.pins || []), ...state.favorites].find((item) => item.id === active.bookmarkId) : undefined;
  const favorite = Boolean(saved && state.favorites.some((item) => item.id === saved.id));
  const pages = regularIds(state);
  const partner = active ? partnerId(state, active.id) : undefined;
  const last = active ? findLastIndex(pages, (id) => id === active.id || id === partner) : -1;
  const splitPartner = active && active.url && active.url !== "era://newtab" && !partner
    ? recent.map((id) => state.tabs.find((tab) => tab.id === id)).find((tab) => tab && tab.id !== active.id && tab.spaceId === active.spaceId && tab.url && tab.url !== "era://newtab" && !tab.splitWith && !state.tabs.some((item) => item.splitWith === tab.id))
    : undefined;
  return {
    hasPage: Boolean(active?.url && active.url !== "era://newtab"),
    isPinned: Boolean(saved && !favorite),
    isFavorite: favorite,
    inSplit: Boolean(partner),
    splitPartner: splitPartner ? { id: splitPartner.id, title: splitPartner.title } : undefined,
    canMove: Boolean(active && !active.bookmarkId && active.url && active.url !== "era://newtab"),
    hasOtherTabs: active?.bookmarkId ? pages.length > 0 : pages.some((id) => id !== active?.id && id !== partner),
    hasTabsBelow: active?.bookmarkId ? pages.length > 0 : last >= 0 && pages.length > last + 1,
    canReopen: state.closed.length > 0,
    spaces: state.spaces.map((space) => ({ id: space.id, name: space.name })),
    currentSpaceId: state.activeSpace,
  };
}

function containsWords(words: string[], ...haystacks: string[]): boolean {
  const joined = haystacks.join(" ").toLowerCase();
  return words.every((word) => joined.includes(word));
}
function row(kind: CommandKind, title: string, detail: string, icon: string, action: string, key: string): CommandRow {
  return { kind, title, detail, icon, action, key };
}
export function rankCommands(text: string, input: {
  state: BrowserState;
  recent: string[];
  history: HistoryEntry[];
  suggestions: string[];
  editingCurrent?: boolean;
}): CommandRow[] {
  const query = text.trim();
  const context = commandContext(input.state, input.recent);
  const results: CommandRow[] = [];
  const seen = new Set<string>();
  const add = (item: CommandRow) => { if (!seen.has(item.key)) { seen.add(item.key); results.push(item); } };
  if (!query) {
    const tabs = input.recent.map((id) => input.state.tabs.find((tab) => tab.id === id)).filter((tab): tab is Tab => Boolean(tab));
    for (const tab of tabs) {
      if (results.length >= 6 || tab.id === input.state.activeTab || !tab.url || tab.url === "era://newtab") continue;
      add(row({ type: "switchTo", id: tab.id }, tab.title, displayUrl(tab.url), "globe", "Switch to Tab", normalizeKey(tab.url)));
    }
    return results;
  }
  if (input.editingCurrent) return [];
  const words = wordsOf(query);
  const matched = matchingActions(query, context);
  const named = matched.filter((action) => isNamed(action, words));
  const addActions = (list: Action[]) => {
    for (const action of list) add(row(
      { type: "action", id: action.id },
      action.title,
      action.id === "split" && context.splitPartner ? `with ${context.splitPartner.title}` : "",
      action.icon,
      "Run",
      `action:${action.id}`,
    ));
  };
  addActions(named);
  const address = urlFrom(query);
  if (address) add(row({ type: "open", url: address }, displayUrl(address), "", "globe", "Open", normalizeKey(address)));
  else add(row({ type: "search", query }, query, searchName(input.state.settings), "search", "Search", `search:${query}`));
  addActions(matched.filter((action) => !isNamed(action, words)));
  let tabCount = 0;
  const ordered = [...input.recent.map((id) => input.state.tabs.find((tab) => tab.id === id)), ...input.state.tabs].filter((tab): tab is Tab => Boolean(tab));
  for (const tab of ordered) {
    if (tabCount >= 3 || !tab.url || tab.url === "era://newtab" || !containsWords(words, tab.title, tab.url)) continue;
    const key = normalizeKey(tab.url);
    if (seen.has(key)) continue;
    add(row({ type: "switchTo", id: tab.id }, tab.title, displayUrl(tab.url), "globe", "Switch to Tab", key));
    tabCount++;
  }
  let marks = 0;
  const favorites = new Set(input.state.favorites.map((item) => item.id));
  for (const mark of [...input.state.favorites, ...(input.state.pins || [])]) {
    if (marks >= 3 || !containsWords(words, mark.title, mark.url)) continue;
    const key = normalizeKey(mark.url);
    if (seen.has(key)) continue;
    add(row({ type: "bookmark", id: mark.id, favorite: favorites.has(mark.id) }, mark.title, displayUrl(mark.url), "globe", favorites.has(mark.id) ? "Open Favorite" : "Open Pinned Tab", key));
    marks++;
  }
  let historyCount = 0;
  for (const entry of input.history) {
    if (historyCount >= 4 || seen.has(normalizeKey(entry.url)) || isSearchPage(entry.url)) continue;
    add(row({ type: "history", url: entry.url }, entry.title || displayUrl(entry.url), displayUrl(entry.url), "history", "Open", normalizeKey(entry.url)));
    historyCount++;
  }
  if (!address) for (const suggestion of input.suggestions.slice(0, 4)) {
    if (suggestion.toLowerCase() === query.toLowerCase()) continue;
    add(row({ type: "suggestion", query: suggestion }, suggestion, "", "search", "Search", `search:${suggestion}`));
  }
  return results.slice(0, COMMAND_LIMIT);
}
export function scopedResults(target: SiteSearch, query: string, suggestions: string[], history: HistoryEntry[]): CommandRow[] {
  const text = query.trim();
  if (!text) {
    const domain = target.home.replace(/^https?:\/\//, "").split(".").slice(-2).join(".");
    const rows = [row({ type: "open", url: target.home }, target.name, displayUrl(target.home), "globe", "Open", normalizeKey(target.home))];
    const seen = new Set(rows.map((item) => item.key));
    for (const entry of history) {
      if (rows.length >= 6) break;
      let host = "";
      try { host = new URL(entry.url).hostname; } catch { continue; }
      const key = normalizeKey(entry.url);
      if (!host.endsWith(domain) || new URL(entry.url).pathname.length <= 1 || isSearchPage(entry.url) || seen.has(key)) continue;
      rows.push(row({ type: "history", url: entry.url }, entry.title || displayUrl(entry.url), displayUrl(entry.url), "history", "Open", key));
      seen.add(key);
    }
    return rows;
  }
  return [text, ...suggestions.filter((item) => item.toLowerCase() !== text.toLowerCase()).slice(0, 5)]
    .map((item) => row({ type: "searchOn", url: siteSearchUrl(target, item) }, item, `Search ${target.name}`, "search", "Search", `search:${item}`));
}
function isSearchPage(value: string): boolean {
  try { const url = new URL(value); return url.pathname === "/search" || url.hostname.startsWith("www.google.") && url.pathname === "/search"; } catch { return false; }
}
function normalizeKey(value: string): string {
  try {
    const url = new URL(value);
    url.hash = "";
    return url.href.replace(/\/$/, "");
  } catch { return value; }
}

export function looksLikeHost(text: string): boolean {
  return /^[\p{L}\p{N}][\p{L}\p{N}.:-]*$/u.test(text);
}
export function completionAllowed(text: string, shown: string, typed: string, deleting: boolean, caretAtEnd: boolean, composing: boolean): boolean {
  const deletion = deleting || (text === typed && shown !== typed);
  return caretAtEnd && !composing && looksLikeHost(text) && !deletion;
}
function normalizedHost(value: string): string {
  try { return new URL(value).hostname.replace(/^www\./, ""); } catch { return ""; }
}
function completionAddress(value: string): string {
  const url = new URL(value);
  let text = normalizedHost(value);
  if (url.port) text += `:${url.port}`;
  if (url.pathname.length > 1) text += url.pathname.replace(/\/$/, "");
  return text;
}
function completionTarget(value: string, wholePage: boolean): string | undefined {
  let url: URL;
  try { url = new URL(value); } catch { return; }
  if (url.protocol !== "http:" && url.protocol !== "https:" || !url.hostname) return;
  const next = new URL(`${url.protocol}//${url.host}`);
  if (wholePage && !url.search && !url.hash && url.pathname.length > 1) next.pathname = url.pathname;
  return next.href;
}
function shortenedHosts(host: string): string[] {
  const labels = host.split(".");
  const suffix = labels.length >= 3 && MULTI_SUFFIX.has(labels.slice(-2).join(".")) ? 3 : 2;
  if (labels.length <= suffix) return [];
  const site = labels.slice(-suffix).join(".");
  if (site === host) return [];
  const hosts: string[] = [];
  const rest = [...labels];
  while (rest.length > suffix) { rest.shift(); hosts.push(rest.join(".")); }
  return hosts;
}
export function mergeSites(sites: CompletionSite[]): CompletionSite[] {
  const merged = new Map<string, CompletionSite>();
  const order: string[] = [];
  for (const site of sites) {
    let address = "";
    try { address = completionAddress(site.url); } catch { continue; }
    const key = address.split("/")[0];
    const existing = merged.get(key);
    if (existing) {
      const longer = completionAddress(existing.url).length >= address.length ? existing.url : site.url;
      merged.set(key, { url: longer, score: existing.score + site.score });
    } else { merged.set(key, site); order.push(key); }
  }
  return order.flatMap((key) => merged.get(key) || []);
}
export function bestCompletion(typed: string, sites: CompletionSite[]): CompletionMatch | undefined {
  const lower = typed.toLowerCase();
  const www = lower.startsWith("www.");
  const bare = www ? lower.slice(4) : lower;
  if (!bare || !looksLikeHost(lower)) return;
  let best: { site: CompletionSite; address: string; whole: boolean } | undefined;
  let exact: CompletionSite | undefined;
  const consider = (site: CompletionSite, address: string, whole: boolean) => {
    if (best) {
      if (best.whole && !whole) return;
      if (best.whole === whole && (site.score < best.site.score || (site.score === best.site.score && (address.length > best.address.length || (address.length === best.address.length && address >= best.address))))) return;
    }
    best = { site, address, whole };
  };
  for (const site of sites) {
    let address = "";
    try { address = completionAddress(site.url); } catch { continue; }
    const host = address.split("/")[0];
    if (host === bare && (!exact || site.score > exact.score)) exact = site;
    if (host.startsWith(bare)) consider(site, address, true);
    else {
      const hosts = shortenedHosts(normalizedHost(site.url));
      const short = hosts.find((item, index) => item.startsWith(bare) && hosts.slice(index + 1).every((later) => !later.startsWith(bare)));
      if (!short) continue;
      const source = new URL(site.url);
      const url = `${source.protocol}//${www ? "www." : ""}${short}${source.port ? `:${source.port}` : ""}/`;
      consider({ url, score: site.score }, short + (source.port ? `:${source.port}` : ""), false);
    }
  }
  if (exact) {
    const front = completionTarget(exact.url, false);
    if (front) return { text: `${www ? "www." : ""}${bare}`, url: front };
  }
  if (!best) return;
  return { text: `${www ? "www." : ""}${best.address}`, url: best.site.url };
}
export function nextCompletion(previous: CompletionMatch | undefined, found: CompletionMatch | undefined, typed: string): CompletionMatch | undefined {
  if (found && found.text.length === typed.length) return found;
  if (previous && previous.text.toLowerCase().startsWith(typed.toLowerCase())) return previous;
  return found;
}
export function completionFor(typed: string, history: HistoryEntry[], kept: string[]): CompletionMatch | undefined {
  const bare = typed.toLowerCase().replace(/^www\./, "");
  if (!bare || !looksLikeHost(typed)) return;
  const fromHistory = history.flatMap((entry, index) => {
    const target = completionTarget(entry.url, true);
    return target && normalizedHost(entry.url).includes(bare) ? [{ url: target, score: history.length - index }] : [];
  });
  const remembered = kept.flatMap((url) => normalizedHost(url).includes(bare) && completionTarget(url, false) ? [{ url: completionTarget(url, false)!, score: 1 }] : []);
  return bestCompletion(typed, mergeSites([...fromHistory, ...remembered]));
}
