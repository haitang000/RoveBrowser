export const MIN_SIDEBAR_WIDTH = 192;
export const MAX_SIDEBAR_WIDTH = 480;

/** An absent preference keeps the original responsive 240/216px defaults. */
export function normalizeSidebarWidth(width: unknown): number | undefined {
  return typeof width === "number" && Number.isFinite(width)
    ? Math.round(Math.max(MIN_SIDEBAR_WIDTH, Math.min(MAX_SIDEBAR_WIDTH, width)))
    : undefined;
}

export function sidebarWidthLimit(viewport: number): number {
  // Leave a 240px page and its 8px outer inset in the minimum 480px window.
  return Math.min(MAX_SIDEBAR_WIDTH, Math.max(MIN_SIDEBAR_WIDTH, viewport - 248));
}

export function visibleSidebarWidth(preferred: unknown, viewport: number): number {
  const width = normalizeSidebarWidth(preferred) ?? (viewport <= 740 ? 216 : 240);
  return Math.min(width, sidebarWidthLimit(viewport));
}
