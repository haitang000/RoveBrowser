export type SpaceColor =
  "sky" | "sage" | "sand" | "rose" | "lavender" | "mint" | "peach" | "graphite";
export interface Space {
  id: string;
  name: string;
  color: SpaceColor;
}
export interface Tab {
  id: string;
  spaceId: string;
  title: string;
  url: string;
  pinned: boolean;
  bookmarkId?: string;
  unloaded?: boolean;
  loading?: boolean;
  favicon?: string;
  splitWith?: string;
}
export interface Favorite {
  id: string;
  title: string;
  url: string;
  splitWith?: string;
  spaceId?: string;
}
export interface BrowserState {
  spaces: Space[];
  tabs: Tab[];
  activeSpace: string;
  activeTab: string;
  favorites: Favorite[];
  pins?: Favorite[];
  libraryVersion?: number;
  closed: Tab[];
  theme: "light" | "dark";
  sidebarCollapsed: boolean;
}
export interface HistoryEntry {
  title: string;
  url: string;
}
export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
  corners?: [number, number, number, number];
}
export interface Layout {
  primary: Rect;
  secondary?: Rect;
}
export interface Point {
  x: number;
  y: number;
}
export type PeekPhase = "opening" | "open" | "closing";
export type Command = { type: string; [key: string]: unknown };
declare global {
  interface Window {
    ipc?: { postMessage(message: string): void };
    __ERA_NATIVE__?: boolean;
    __ERA_INITIAL_STATE__?: BrowserState;
    __ERA_RECEIVE__?: (event: {
      type: string;
      state?: BrowserState;
      entries?: HistoryEntry[];
      suggestions?: string[];
      message?: string;
      action?: string;
      peek?: Tab | null;
      id?: string;
      background?: string;
      backgrounds?: (string | null)[] | null;
      backgroundReady?: boolean;
      pane?: number;
      phase?: PeekPhase;
      ticket?: number;
      origin?: Point | null;
      surface?: string | null;
      images?: Record<string, string>;
      image?: string | null;
      kept?: string[];
    }) => void;
  }
}
