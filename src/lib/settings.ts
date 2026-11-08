// Ported from Zigzag SettingsView, SettingsPage, SearchEngine and Shortcuts.
// Copyright 2026 Zigao Wang. Apache-2.0; adapted for Windows and Vue.
export const SETTINGS_PANES = [
  {
    id: "general",
    title: "通用",
    icon: "settings",
    keywords: "general 搜索 外观 窗口 下载",
  },
  {
    id: "spaces",
    title: "工作区",
    icon: "spaces",
    keywords: "spaces 空间 颜色 玻璃",
  },
  {
    id: "passwords",
    title: "密码",
    icon: "key",
    keywords: "passwords 自动填充",
  },
  {
    id: "privacy",
    title: "隐私",
    icon: "shield",
    keywords: "privacy 历史 cookies 缓存",
  },
  {
    id: "translation",
    title: "翻译",
    icon: "translate",
    keywords: "translation 语言",
  },
  {
    id: "extensions",
    title: "扩展",
    icon: "extension",
    keywords: "extensions 插件",
  },
  {
    id: "shortcuts",
    title: "快捷键",
    icon: "keyboard",
    keywords: "shortcuts keyboard 键盘",
  },
] as const;
export type SettingsPane = (typeof SETTINGS_PANES)[number]["id"];
export const SEARCH_ENGINES = [
  ["google", "Google", "https://www.google.com/search?q=%s"],
  ["duckDuckGo", "DuckDuckGo", "https://duckduckgo.com/?q=%s"],
  ["brave", "Brave Search", "https://search.brave.com/search?q=%s"],
  ["bing", "Bing", "https://www.bing.com/search?q=%s"],
  ["kagi", "Kagi", "https://kagi.com/search?q=%s"],
  ["ecosia", "Ecosia", "https://www.ecosia.org/search?q=%s"],
  ["startpage", "Startpage", "https://www.startpage.com/sp/search?query=%s"],
  ["perplexity", "Perplexity", "https://www.perplexity.ai/search?q=%s"],
  ["yahoo", "Yahoo", "https://search.yahoo.com/search?p=%s"],
  ["baidu", "百度", "https://www.baidu.com/s?wd=%s"],
  ["yandex", "Yandex", "https://yandex.com/search/?text=%s"],
  ["custom", "自定义", ""],
] as const;
export interface Shortcut {
  key: number;
  ctrl: boolean;
  alt: boolean;
  shift: boolean;
}
const combo = (
  key: number,
  ctrl = true,
  shift = false,
  alt = false,
): Shortcut => ({ key, ctrl, shift, alt });
export const SHORTCUTS = [
  { id: "address", title: "编辑地址", group: "浏览", value: combo(76) },
  { id: "new", title: "新建标签页", group: "标签页", value: combo(84) },
  { id: "close", title: "关闭标签页", group: "标签页", value: combo(87) },
  {
    id: "restore",
    title: "重新打开关闭的标签页",
    group: "标签页",
    value: combo(84, true, true),
  },
  { id: "cycle", title: "下一个标签页", group: "标签页", value: combo(9) },
  {
    id: "cyclePrevious",
    title: "上一个标签页",
    group: "标签页",
    value: combo(9, true, true),
  },
  { id: "sidebar", title: "显示或隐藏侧栏", group: "窗口", value: combo(66) },
  {
    id: "desk",
    title: "全部标签页",
    group: "窗口",
    value: combo(220, true, true),
  },
  { id: "settings", title: "打开设置", group: "窗口", value: combo(188) },
  { id: "reload", title: "刷新网页", group: "浏览", value: combo(82) },
  {
    id: "back",
    title: "后退",
    group: "浏览",
    value: combo(37, false, false, true),
  },
  {
    id: "forward",
    title: "前进",
    group: "浏览",
    value: combo(39, false, false, true),
  },
] as const;
export interface Settings {
  searchEngine: string;
  customSearchTemplate: string;
  searchSuggestions: boolean;
  largeSpaceNames: boolean;
  favoritesPerRow: number;
  reduceMotion: boolean;
  askBeforeQuit: boolean;
  savePasswords: boolean;
  autofill: boolean;
  rememberHistory: boolean;
  downloadFolder: string;
  shortcuts: Record<string, Shortcut | null>;
}
export const defaultSettings = (): Settings => ({
  searchEngine: "google",
  customSearchTemplate: "",
  searchSuggestions: true,
  largeSpaceNames: true,
  favoritesPerRow: 4,
  reduceMotion: false,
  askBeforeQuit: true,
  savePasswords: true,
  autofill: true,
  rememberHistory: true,
  downloadFolder: "",
  shortcuts: {},
});
export function settingsFor(value?: Partial<Settings>): Settings {
  return { ...defaultSettings(), ...value };
}
export function isSettingsUrl(value = ""): boolean {
  try {
    const u = new URL(value);
    return (
      (u.protocol === "rove:" || u.protocol === "era:") &&
      u.hostname === "settings" &&
      !u.username &&
      !u.password &&
      !u.port
    );
  } catch {
    return false;
  }
}
export function normalizeSettingsUrl(value: string): string {
  if (!/^(?:rove|era):/i.test(value) || !isSettingsUrl(value)) return value;
  const url = new URL(value);
  url.protocol = "rove:";
  return url.href;
}
export function settingsPane(value: string): SettingsPane {
  const path = isSettingsUrl(value)
    ? new URL(value).pathname.split("/").filter(Boolean)[0]
    : "";
  return SETTINGS_PANES.find((pane) => pane.id === path)?.id || "general";
}
export function settingsUrl(pane: SettingsPane = "general"): string {
  return `rove://settings${pane === "general" ? "" : "/" + pane}`;
}
export function validSearchTemplate(template: string): boolean {
  if (!template.includes("%s") || template.length > 2048) return false;
  try {
    const u = new URL(template.replaceAll("%s", "q"));
    return (
      /^https?:$/.test(u.protocol) && !!u.hostname && !u.username && !u.password
    );
  } catch {
    return false;
  }
}
export function searchUrl(query: string, settings?: Partial<Settings>): string {
  const s = settingsFor(settings);
  const engine =
    SEARCH_ENGINES.find(([id]) => id === s.searchEngine) || SEARCH_ENGINES[0];
  const template =
    engine[0] === "custom" && validSearchTemplate(s.customSearchTemplate)
      ? s.customSearchTemplate
      : engine[2] || SEARCH_ENGINES[0][2];
  return template.replaceAll(
    "%s",
    encodeURIComponent(query).replace(
      /[!'()*]/g,
      (c) => `%${c.charCodeAt(0).toString(16).toUpperCase()}`,
    ),
  );
}
export function searchName(settings?: Partial<Settings>): string {
  return (
    SEARCH_ENGINES.find(([id]) => id === settings?.searchEngine)?.[1] ||
    "Google"
  );
}
export function shortcutFor(id: string, settings: Settings): Shortcut | null {
  return Object.hasOwn(settings.shortcuts, id)
    ? settings.shortcuts[id]!
    : SHORTCUTS.find((s) => s.id === id)?.value || null;
}
export function shortcutText(value: Shortcut | null): string {
  if (!value) return "未设置";
  const names: Record<number, string> = {
    9: "Tab",
    32: "Space",
    33: "PageUp",
    34: "PageDown",
    35: "End",
    36: "Home",
    37: "←",
    38: "↑",
    39: "→",
    40: "↓",
    188: ",",
    190: ".",
    191: "/",
    192: "`",
    219: "[",
    220: "\\",
    221: "]",
    186: ";",
    187: "=",
    189: "-",
    222: "'",
  };
  const key =
    names[value.key] ||
    (value.key >= 112 && value.key <= 123
      ? `F${value.key - 111}`
      : String.fromCharCode(value.key));
  return [value.ctrl && "Ctrl", value.alt && "Alt", value.shift && "Shift", key]
    .filter(Boolean)
    .join(" + ");
}
export function shortcutFromEvent(e: KeyboardEvent): Shortcut | undefined {
  let key = /^(Key[A-Z]|Digit\d)$/.test(e.code)
    ? e.code.charCodeAt(e.code.length - 1)
    : 0;
  const keys: Record<string, number> = {
    Tab: 9,
    Space: 32,
    PageUp: 33,
    PageDown: 34,
    End: 35,
    Home: 36,
    ArrowLeft: 37,
    ArrowUp: 38,
    ArrowRight: 39,
    ArrowDown: 40,
    Comma: 188,
    Period: 190,
    Slash: 191,
    Backquote: 192,
    BracketLeft: 219,
    Backslash: 220,
    BracketRight: 221,
    Semicolon: 186,
    Equal: 187,
    Minus: 189,
    Quote: 222,
  };
  key ||=
    keys[e.code] ||
    (/^F([1-9]|1[0-2])$/.test(e.code) ? 111 + Number(e.code.slice(1)) : 0);
  if (
    !key ||
    e.metaKey ||
    (!e.ctrlKey && !e.altKey && !(key >= 112 && key <= 123)) ||
    e.isComposing ||
    e.getModifierState?.("AltGraph")
  )
    return;
  return { key, ctrl: e.ctrlKey, alt: e.altKey, shift: e.shiftKey };
}
export function sameShortcut(
  a: Shortcut | null | undefined,
  b: Shortcut | null | undefined,
): boolean {
  return (
    !!a &&
    !!b &&
    a.key === b.key &&
    a.ctrl === b.ctrl &&
    a.alt === b.alt &&
    a.shift === b.shift
  );
}
export function validateShortcuts(settings: Settings) {
  const used: Shortcut[] = [];
  for (const item of SHORTCUTS) {
    const value = shortcutFor(item.id, settings);
    if (!value) continue;
    if (
      !shortcutFromValue(value) ||
      (value.ctrl && !value.alt && value.key >= 49 && value.key <= 57) ||
      (value.alt && value.key === 115) ||
      used.some((other) => sameShortcut(value, other))
    )
      throw Error("快捷键重复或被系统／工作区保留");
    used.push(value);
  }
}
function shortcutFromValue(v: Shortcut): boolean {
  return (
    Number.isInteger(v.key) &&
    ((v.key >= 65 && v.key <= 90) ||
      (v.key >= 48 && v.key <= 57) ||
      (v.key >= 112 && v.key <= 123) ||
      [
        9, 32, 33, 34, 35, 36, 37, 38, 39, 40, 186, 187, 188, 189, 190, 191,
        192, 219, 220, 221, 222,
      ].includes(v.key)) &&
    (v.ctrl || v.alt || (v.key >= 112 && v.key <= 123))
  );
}
