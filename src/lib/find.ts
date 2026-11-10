// Find-in-page helpers, adapted from Zigzag Window/FindBar.swift
// (Copyright 2026 Zigao Wang, Apache-2.0), reduced to the browser-rendered part:
// WebView2 reports the match count, so no script is injected to count matches.
export const FIND_QUERY_MAX = 256;

/** One line, trimmed to the cap the host enforces, so both sides agree. */
export function normalizeFindQuery(value: string): string {
  return value.replace(/[\r\n]+/g, " ").slice(0, FIND_QUERY_MAX);
}

/**
 * Zigzag's "3 of 12" counter, drawn as "3 / 12".
 *
 * WebView2's `ActiveMatchIndex` is 1-based (measured against a page with three
 * matches: the first is 1, the last 3), unlike most of the COM surface. Keep
 * the raw value and clamp it for display only.
 */
export function findMatchLabel(
  count: number,
  active: number,
  hasQuery: boolean,
): string {
  if (!hasQuery) return "";
  if (count <= 0) return "无结果";
  const place = Math.min(Math.max(active, 1), count);
  return `${place} / ${count}`;
}
