import type { Tab } from "./types";

export interface TabSwitcherState {
  ids: string[];
  selected: string;
  visible: boolean;
}

export type TabSwitcherInput =
  | { action: "step"; reverse: boolean }
  | { action: "commit"; id?: string }
  | { action: "cancel" };

export function tabSwitcherIds(tabs: Tab[], current: string, recent: string[]): string[] {
  const byId = new Map(tabs.map((tab) => [tab.id, tab]));
  const ordered = [...tabs].sort((a, b) => (b.lastActivated ?? 0) - (a.lastActivated ?? 0));
  const seen = new Set<string>();
  return [current, ...recent, ...ordered.map((tab) => tab.id)].filter((id) => {
    const tab = byId.get(id);
    if (!tab || seen.has(id)) return false;
    seen.add(id);
    if (tab.splitWith) seen.add(tab.splitWith);
    return true;
  });
}
