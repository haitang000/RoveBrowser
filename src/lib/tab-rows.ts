import type { Tab } from "./types";

export interface TabRow {
  tab: Tab;
  panes?: [Tab, Tab];
}

export function sidebarTabRows(pins: Tab[], tabs: Tab[], favorites: Tab[], activeId: string) {
  const visible = [...pins, ...tabs];
  const pages = [...visible, ...favorites];
  const byId = new Map(pages.map((tab) => [tab.id, tab]));
  const visibleIds = new Set(visible.map((tab) => tab.id));
  const claimed = new Set<string>();
  const groups = new Map<string, [Tab, Tab]>();

  // A live split can be one-way. Start with the active tab's outgoing link
  // so the displayed pair matches the actual panes if older links overlap.
  for (const primary of [byId.get(activeId), ...pages]) {
    const secondary = primary && byId.get(primary.splitWith ?? "");
    if (!primary || !secondary || primary === secondary ||
        Boolean(primary.unloaded) !== Boolean(secondary.unloaded) ||
        claimed.has(primary.id) || claimed.has(secondary.id)) continue;

    const candidates = [primary, secondary].filter((tab) => visibleIds.has(tab.id));
    if (!candidates.length) continue;
    // Use a stable row for reciprocal links even when the primary pane changes.
    if (secondary.splitWith === primary.id &&
        (secondary.bookmarkId ?? secondary.id) < (primary.bookmarkId ?? primary.id)) candidates.reverse();
    const anchor = candidates.find((tab) => tab.pinned) ?? candidates[0];
    groups.set(anchor.id, [primary, secondary]);
    claimed.add(primary.id);
    claimed.add(secondary.id);
  }

  const rows = (list: Tab[]): TabRow[] => list.flatMap((tab) => {
    const panes = groups.get(tab.id);
    return panes || !claimed.has(tab.id) ? [{ tab, panes }] : [];
  });
  return { pins: rows(pins), tabs: rows(tabs) };
}
