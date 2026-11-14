import type { Favorite, HistoryEntry } from "./types";

export type WidgetKind = "clock" | "github" | "calendar" | "notes";
export interface NewTabWidget {
  id: string;
  kind: WidgetKind;
  size: "small" | "wide";
  text?: string;
  username?: string;
}
export interface NewTabSettings {
  displayName: string;
  widgets: NewTabWidget[];
}
export const NEW_TAB_STORAGE_KEY = "rove.new-tab.v1";
export const MAX_WIDGETS = 8;
export const WIDGET_CATALOG: { kind: WidgetKind; title: string; description: string; size: NewTabWidget["size"] }[] = [
  { kind: "github", title: "GitHub 贡献图", description: "每一天的小小进步。", size: "wide" },
  { kind: "clock", title: "时钟", description: "留一点时间，给当下。", size: "wide" },
  { kind: "calendar", title: "日历", description: "今天，以及接下来的日子。", size: "small" },
  { kind: "notes", title: "便签", description: "记下脑海中的下一件事。", size: "small" },
];
export function defaultNewTab(): NewTabSettings {
  return { displayName: "Explorer", widgets: [{ id: "clock", kind: "clock", size: "wide" }] };
}
/** Bound persisted data, while preserving an intentionally empty layout. */
export function newTabSettings(value?: unknown): NewTabSettings {
  if (!value || typeof value !== "object") return defaultNewTab();
  const raw = value as Partial<NewTabSettings>;
  const seen = new Set<string>();
  return {
    displayName: typeof raw.displayName === "string" ? raw.displayName.trim().slice(0, 40) || "Explorer" : "Explorer",
    widgets: Array.isArray(raw.widgets) ? raw.widgets.filter((item) => {
      if (!item || typeof item.id !== "string" || !item.id || item.id.length > 80 || seen.has(item.id) || !WIDGET_CATALOG.some((entry) => entry.kind === item.kind)) return false;
      seen.add(item.id);
      return true;
    }).slice(0, MAX_WIDGETS).map((item) => ({
      id: item.id, kind: item.kind, size: item.size === "wide" ? "wide" : "small",
      ...(typeof item.text === "string" ? { text: item.text.slice(0, 10_000) } : {}),
      ...(typeof item.username === "string" && validGithubUsername(item.username) ? { username: item.username.trim() } : {}),
    })) : defaultNewTab().widgets,
  };
}
export function validGithubUsername(value: string) {
  return /^[a-z\d](?:[a-z\d]|-(?=[a-z\d])){0,38}$/i.test(value.trim());
}
export function greeting(date: Date) {
  const hour = date.getHours();
  return hour < 12 ? "Good morning," : hour < 18 ? "Good afternoon," : "Good evening,";
}
export function localDate(date: Date) {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
}
const fallbackSites: HistoryEntry[] = [
  { title: "GitHub", url: "https://github.com/" },
  { title: "ChatGPT", url: "https://chatgpt.com/" },
  { title: "Figma", url: "https://figma.com/" },
  { title: "Notion", url: "https://notion.so/" },
  { title: "YouTube", url: "https://youtube.com/" },
];
export function newTabSuggestions(history: HistoryEntry[], favorites: Favorite[], pins: Favorite[] = []) {
  const seen = new Set<string>();
  const result: (HistoryEntry & { favicon?: string })[] = [];
  for (const site of [...history, ...favorites, ...pins, ...fallbackSites]) {
    try {
      const url = new URL(site.url);
      if (!/^https?:$/.test(url.protocol) || url.username || url.password) continue;
      const key = url.hostname.replace(/^www\./, "");
      if (seen.has(key)) continue;
      seen.add(key);
      result.push({ ...site, title: site.title || url.hostname, url: url.href });
      if (result.length === 5) break;
    } catch { /* Internal and malformed URLs do not belong in suggestions. */ }
  }
  return result;
}
export function calendarDays(month: Date): (number | null)[] {
  const first = new Date(month.getFullYear(), month.getMonth(), 1);
  const count = new Date(month.getFullYear(), month.getMonth() + 1, 0).getDate();
  const offset = (first.getDay() + 6) % 7;
  return [...Array<null>(offset).fill(null), ...Array.from({ length: count }, (_, i) => i + 1)];
}
