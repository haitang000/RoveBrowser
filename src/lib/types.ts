export type SpaceColor =
  "sky" | "sage" | "sand" | "rose" | "lavender" | "mint" | "peach" | "graphite";
export interface Space {
  id: string;
  name: string;
  color: SpaceColor;
  /** 0 is solid. Absent means solid. */
  translucency?: number;
  /** Custom wheel hue, 0–1. Absent: the named color. */
  hue?: number;
  /** How much of that hue, 0–1. Absent with a hue means all of it. */
  tint?: number;
  /** 0 is near white, 1 is near black. Absent: follow the app theme. */
  brightness?: number;
  hue2?: number;
  tint2?: number;
  brightness2?: number;
  /** Gradient direction in degrees. 0 is left to right, 90 is top to bottom. */
  angle?: number;
  /** 0 is smooth. Absent means smooth. */
  grain?: number;
  /** One emoji. Absent: the switcher shows its number. */
  icon?: string;
  /** The tab this space was showing when the user left it; restored on switch back. */
  activeTab?: string;
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
  /** The tab's sound is muted (persisted with the session). */
  muted?: boolean;
  /** A rename from the tab context menu; the page title stays in `title`. */
  customTitle?: string;
  /** Unix seconds the tab was last shown, for fading and the Clear button. */
  lastActivated?: number;
}
export type DownloadState =
  | "inProgress"
  | "paused"
  | "finished"
  | "failed"
  | "cancelled";
/** One row of the downloads panel, persisted by the native host. */
export interface DownloadItem {
  id: string;
  url: string;
  path: string;
  filename: string;
  received: number;
  total: number;
  state: DownloadState;
  failure?: string;
  started: number;
  finished?: number;
  /** The host refreshes this from disk: a finished file may be moved already. */
  exists?: boolean;
}
export interface Favorite {
  id: string;
  title: string;
  url: string;
  favicon?: string;
  splitWith?: string;
  spaceId?: string;
}
export interface BrowserState {
  /** Absent in existing profiles: onboarding is already complete. */
  onboarding?: import("./onboarding").OnboardingState;
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
  /** Preferred width in CSS pixels; absent uses the responsive default. */
  sidebarWidth?: number;
  settings?: import("./settings").Settings;
  /** Per-site zoom by normalized host; 100% is stored as absence. */
  siteZoom?: Record<string, number>;
  /** Legacy session preference; the restored framed layout ignores this flag. */
  borderless?: boolean;
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
  const __APP_VERSION__: string;
  interface Window {
    ipc?: { postMessage(message: string): void };
    __ERA_NATIVE__?: boolean;
    __ERA_POPUP__?: boolean;
    __ERA_INITIAL_STATE__?: BrowserState;
    __ERA_RECEIVE__?: (event: {
      type: string;
      importSources?: import("./import").ImportSource[];
      importBusy?: boolean;
      defaultBrowser?: { http: boolean; https: boolean };
      ratio?: number;
      resizing?: boolean;
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
      popup?: Record<string, unknown>;
      count?: number;
      active?: number;
      /** Page zoom factor, and whether a page is playing sound. */
      factor?: number;
      playing?: boolean;
      /** The full downloads list, and the row a just-started download created. */
      items?: DownloadItem[];
    }) => void;
  }
}
