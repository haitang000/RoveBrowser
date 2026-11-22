import type { Point, Rect } from "./types";

export type ContextMenuAnchor = Point & { side?: { left: number; right: number } };

export type PageMenuItem = { kind: "separator" }
  | { kind: "submenu"; label: string; enabled: boolean; items: PageMenuItem[] }
  | { kind: "command"; id: number; label: string; enabled: boolean; shortcut: string; checked: boolean | null; radio: boolean };

export interface PageContextMenu {
  ticket: number;
  anchor: Point;
  items: PageMenuItem[];
  fullscreen: boolean;
}

export interface ContextMenuBackdrop {
  layer: number;
  rect: Rect;
  image: string;
}
