// Direction for a page change. Same tab, a missing neighbor, or a space
// switch stays in place and only fades. A later tab or settings pane comes
// in from the right.
export type PageDirection = -1 | 0 | 1;

export function pageDirection(
  tabs: readonly { id: string }[],
  fromId: string | undefined,
  toId: string | undefined,
  spaceChanged = false,
): PageDirection {
  if (spaceChanged || !fromId || !toId || fromId === toId) return 0;
  const from = tabs.findIndex((tab) => tab.id === fromId);
  const to = tabs.findIndex((tab) => tab.id === toId);
  if (from < 0 || to < 0 || from === to) return 0;
  return to > from ? 1 : -1;
}

export function paneDirection(
  order: readonly string[],
  from: string | undefined,
  to: string | undefined,
): PageDirection {
  if (!from || !to || from === to) return 0;
  const start = order.indexOf(from);
  const end = order.indexOf(to);
  if (start < 0 || end < 0 || start === end) return 0;
  return end > start ? 1 : -1;
}
