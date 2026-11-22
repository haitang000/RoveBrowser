// Session grouping adapted from Zigzag's HistoryClusters.swift (Copyright 2026
// Zigao Wang, Apache-2.0). Times use the browser's local calendar.
import type { HistoryEntry } from "./types";
import { host } from "./address";

export type HistoryVisit = HistoryEntry & { visited: number };
export type HistoryCursor = Pick<HistoryVisit, "url" | "visited">;
export interface HistoryPageData {
  entries: HistoryVisit[];
  next: HistoryCursor | null;
}
export interface HistoryReply { requestId: string; page?: HistoryPageData; ok?: boolean }
export type HistoryEdit =
  | { action: "remove"; urls: string[] }
  | { action: "clear"; since: number | null }
  | { action: "undo" };
export type HistoryRange = "hour" | "today" | "yesterday" | "all";
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

export function historyRangeStart(range: HistoryRange, now = new Date()): number | null {
  if (range === "all") return null;
  if (range === "hour") return Math.floor(now.getTime() / 1000) - 3600;
  return new Date(now.getFullYear(), now.getMonth(), now.getDate() - Number(range === "yesterday")).getTime() / 1000;
}
