// Session grouping adapted from Zigzag's HistoryClusters.swift (Copyright 2026
// Zigao Wang, Apache-2.0). Times use the browser's local calendar.
import type { HistoryEntry } from "./types";
import { host } from "./address";

export type HistoryVisit = HistoryEntry & { visited: number; visitCount: number };
export type HistoryCursor = Pick<HistoryVisit, "url" | "visited" | "visitCount">;
export type HistorySort = "date" | "site" | "popular" | "last";
export type HistoryPeriod = "today" | "week" | "month";
export interface HistoryFilters {
  period: HistoryPeriod | null;
  sort: HistorySort;
  source: "history" | "closed";
}
export interface HistoryQuery {
  query: string;
  before?: HistoryCursor;
  since?: number;
  until?: number;
  sort?: HistorySort;
  urls?: string[];
}
export interface HistoryPageData {
  entries: HistoryVisit[];
  next: HistoryCursor | null;
}
export interface HistoryReply { requestId: string; page?: HistoryPageData; ok?: boolean }
export type HistoryEdit =
  | { action: "remove"; urls: string[] }
  | { action: "clear"; since: number | null }
  | { action: "undo" };
export interface HistoryCluster { host: string; items: HistoryVisit[] }
export interface HistorySession { start: Date; clusters: HistoryCluster[]; count: number }

export function historyPeriod(date: Date): string {
  const hour = date.getHours();
  if (hour < 5) return "凌晨";
  if (hour < 12) return "上午";
  if (hour < 17) return "下午";
  if (hour < 21) return "傍晚";
  return "夜间";
}

export function groupHistory(entries: HistoryVisit[]): HistorySession[] {
  const sessions: HistorySession[] = [];
  let previous: Date | undefined;
  for (const entry of entries) {
    const site = host(entry.url);
    const date = new Date(entry.visited * 1000);
    if (!site || !Number.isFinite(date.getTime())) continue;
    let session = sessions.at(-1);
    if (!session || !previous || previous.getTime() - date.getTime() > 40 * 60_000
      || previous.toDateString() !== date.toDateString() || historyPeriod(previous) !== historyPeriod(date)) {
      session = { start: date, clusters: [], count: 0 };
      sessions.push(session);
    }
    const cluster = session.clusters.at(-1);
    if (cluster?.host === site) cluster.items.push(entry);
    else session.clusters.push({ host: site, items: [entry] });
    session.start = date;
    session.count++;
    previous = date;
  }
  return sessions;
}

export function historyBounds(period: HistoryPeriod | null, now = new Date()): Pick<HistoryQuery, "since" | "until"> {
  if (!period) return {};
  const year = now.getFullYear(), month = now.getMonth(), day = now.getDate();
  const monday = day - (now.getDay() + 6) % 7;
  const start = new Date(year, month, period === "month" ? 1 : period === "week" ? monday : day);
  const end = period === "month" ? new Date(year, month + 1, 1)
    : new Date(year, month, period === "week" ? monday + 7 : day + 1);
  return { since: start.getTime() / 1000, until: end.getTime() / 1000 };
}

export function historyMatches({ title, url }: HistoryEntry, words: string[]) {
  const name = title.toLowerCase(), address = url.toLowerCase();
  return words.every((word) => name.includes(word) || address.includes(word));
}

export function compareHistory(a: HistoryCursor, b: HistoryCursor, sort: HistorySort) {
  if (sort === "site") {
    const left = host(a.url), right = host(b.url);
    if (left !== right) return left < right ? -1 : 1;
  }
  if (sort === "popular" && a.visitCount !== b.visitCount) return b.visitCount - a.visitCount;
  return b.visited - a.visited || Number(b.url > a.url) - Number(b.url < a.url);
}

export function groupHistoryBySite(entries: HistoryVisit[]): HistoryCluster[] {
  const groups: HistoryCluster[] = [];
  for (const entry of entries) {
    const site = host(entry.url);
    const previous = groups.at(-1);
    if (previous?.host === site) previous.items.push(entry);
    else groups.push({ host: site, items: [entry] });
  }
  return groups;
}
