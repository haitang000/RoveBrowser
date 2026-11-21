import { describe, expect, it } from "vitest";
import { interfaceLanguage, translate } from "../src/lib/language";
import { english } from "../src/lib/locales/en";
import { traditionalChinese } from "../src/lib/locales/zh-TW";
import { initialState, reduce } from "../src/lib/preview";
import { address } from "../src/lib/address";
import { urlFrom } from "../src/lib/command";
import { upgradeLibrary } from "../src/lib/bookmarks";
import {
  SETTINGS_PANES,
  SEARCH_ENGINES,
  settingsUrl,
  settingsPane,
  isSettingsUrl,
  defaultSettings,
  settingsFor,
  updateChannelDetail,
  searchUrl,
  validSearchTemplate,
  validateShortcuts,
  shortcutFor,
} from "../src/lib/settings";

describe("Zigzag settings port", () => {
  it("keeps Chinese for old or invalid settings and interpolates translated labels", () => {
    expect(settingsFor().language).toBe("zh-CN");
    expect(settingsFor({ language: "en" }).language).toBe("en");
    expect(settingsFor({ language: "zh-TW" }).language).toBe("zh-TW");
    expect(interfaceLanguage("zh-TW")).toBe("zh-TW");
    expect(interfaceLanguage("fr")).toBe("zh-CN");
    expect(translate("zh-TW", "设置")).toBe("設定");
    expect(translate("zh-TW", "标签页")).toBe("分頁");
    expect(translate("zh-TW", "编辑 {name}", { name: "项目 {name}" })).toBe("編輯 项目 {name}");
    expect(translate("en", "编辑 {name}", { name: "项目 {name}" })).toBe("Edit 项目 {name}");
    expect(translate("en", "未知文案")).toBe("未知文案");
    expect(translate("en", "toString")).toBe("toString");
    expect(Object.keys(traditionalChinese).sort()).toEqual(Object.keys(english).sort());
    for (const [source, message] of Object.entries(traditionalChinese)) {
      for (const token of source.match(/\{(?:\w+)\}|%s/g) ?? []) expect(message, source).toContain(token);
      expect(message.split("\n").length, source).toBe(source.split("\n").length);
      expect(message, source).not.toMatch(/设置|标签|下载|默认|软件|网络|文件夹|扩展|简体|窗口|用户|链接|内存|缓存|许可權|小元件|便籤/);
    }
  });
  it("keeps background tab sleeping on by default", () => {
    expect(defaultSettings().sleepTabs).toBe(true);
  });
  it("keeps the update channel on Preview or Canary", () => {
    expect(defaultSettings().updateChannel).toBe("preview");
    expect(updateChannelDetail("preview")).toContain("GitHub");
    expect(settingsFor({ updateChannel: "canary" }).updateChannel).toBe("canary");
    expect(updateChannelDetail("canary")).toContain("每次提交");
    expect(updateChannelDetail("canary")).toContain("windows-installer");
    expect(settingsFor({ updateChannel: "nightly" as "preview" }).updateChannel).toBe("preview");
  });
  it("maps every section to an internal address and preserves it as a tab", () => {
    for (const pane of SETTINGS_PANES)
      expect(settingsPane(settingsUrl(pane.id))).toBe(pane.id);
    expect(settingsUrl()).toBe("rove://settings");
    expect(settingsPane("rove://settings/unknown")).toBe("general");
    expect(isSettingsUrl("https://settings.example.com")).toBe(false);
    expect(isSettingsUrl("era://history")).toBe(false);
    let state = reduce(initialState(), {
      type: "settingsOpen",
      pane: "privacy",
    });
    const id = state.activeTab;
    state = reduce(state, { type: "space", id: "work" });
    state = reduce(state, { type: "settingsOpen" });
    expect(state.activeTab).toBe(id);
    expect(state.tabs.find((t) => t.id === id)?.url).toBe(
      "rove://settings/privacy",
    );
    state = reduce(state, { type: "settingsPane", id, pane: "shortcuts" });
    expect(state.tabs.filter((t) => isSettingsUrl(t.url))).toHaveLength(1);
    expect(state.tabs.find((t) => t.id === id)?.url).toBe(
      "rove://settings/shortcuts",
    );
    const tab = state.tabs.find((t) => t.id === id)!;
    tab.url = "era://settings/shortcuts";
    state.closed.push({
      ...tab,
      id: "legacy-closed-settings",
      url: "era://settings/privacy",
    });
    upgradeLibrary(state);
    expect(tab.url).toBe("rove://settings/shortcuts");
    expect(state.closed.at(-1)?.url).toBe("rove://settings/privacy");
  });
  it("uses the selected engine for typed searches and encodes custom path/query text", () => {
    for (const [engine] of SEARCH_ENGINES.filter(([id]) => id !== "custom")) {
      const settings = { ...defaultSettings(), searchEngine: engine };
      const url = new URL(address("中文 &+#?", settings));
      expect([...url.searchParams.values()][0]).toBe("中文 &+#?");
    }
    const settings = {
      ...defaultSettings(),
      searchEngine: "custom",
      customSearchTemplate: "https://example.com/%s?q=%s",
    };
    expect(searchUrl("a/b&c", settings)).toBe(
      "https://example.com/a%2Fb%26c?q=a%2Fb%26c",
    );
    expect(
      searchUrl("abc", { ...settings, customSearchTemplate: "" }),
    ).toContain("google.com/search");
    for (const value of [
      "javascript:%s",
      "file:///C:/%s",
      "https://example.com",
      "https://user:pass@example.com/?q=%s",
    ])
      expect(validSearchTemplate(value)).toBe(false);
    expect(address("rove://settings/privacy")).toBe("rove://settings/privacy");
    expect(address("era://settings/privacy")).toBe("rove://settings/privacy");
    expect(urlFrom("rove://settings/privacy")).toBe("rove://settings/privacy");
    expect(urlFrom("era://settings/privacy")).toBe("rove://settings/privacy");
  });
  it("rejects duplicate and reserved shortcuts without changing the session", () => {
    const settings = defaultSettings();
    settings.shortcuts.new = shortcutFor("address", settings);
    expect(() => validateShortcuts(settings)).toThrow();
    settings.shortcuts.new = { key: 49, ctrl: true, shift: false, alt: false };
    expect(() => validateShortcuts(settings)).toThrow();
    settings.shortcuts.new = { key: 89, ctrl: true, shift: true, alt: false };
    expect(() => validateShortcuts(settings)).not.toThrow();
    settings.shortcuts.new = null;
    expect(shortcutFor("new", settings)).toBeNull();
  });
  it("creates spaces without leaving settings, reorders, and retains shared favorites on deletion", () => {
    let state = reduce(initialState(), {
      type: "settingsOpen",
      pane: "spaces",
    });
    const id = state.activeTab,
      count = state.favorites.length;
    state = reduce(state, {
      type: "spaceCreate",
      name: "研究",
      color: "mint",
      background: true,
      translucency: 0.5,
      clearGlass: false,
    });
    expect(state.activeTab).toBe(id);
    const space = state.spaces.at(-1)!;
    expect(space).toMatchObject({
      name: "研究",
      translucency: 0.5,
    });
    expect(space).not.toHaveProperty("clearGlass");
    state = reduce(state, {
      type: "spaceUpdate",
      id: space.id,
      name: "研究",
      color: "mint",
      translucency: 0.5,
      icon: "rocket",
    });
    expect(state.spaces.find((item) => item.id === space.id)?.icon).toBeUndefined();
    state = reduce(state, {
      type: "spaceUpdate",
      id: space.id,
      name: "研究",
      color: "mint",
      translucency: 0.5,
      icon: "🚀",
    });
    expect(state.spaces.find((item) => item.id === space.id)?.icon).toBe("🚀");
    state = reduce(state, {
      type: "spaceUpdate",
      id: space.id,
      name: "研究",
      color: "mint",
      translucency: 0.5,
      icon: "⭐",
    });
    expect(state.spaces.find((item) => item.id === space.id)?.icon).toBe("⭐");
    state = reduce(state, {
      type: "spaceUpdate",
      id: space.id,
      name: "研究",
      color: "mint",
      translucency: 0.5,
      icon: null,
    });
    expect(state.spaces.find((item) => item.id === space.id)).not.toHaveProperty("icon");
    state = reduce(state, {
      type: "spaceReorder",
      id: space.id,
      before: "personal",
    });
    expect(state.spaces[0]!.id).toBe(space.id);
    state = reduce(state, { type: "spaceDelete", id: "personal" });
    expect(state.favorites.length).toBe(count);
    expect(state.tabs.some((t) => t.spaceId === "personal")).toBe(false);
    expect(state.pins?.some((p) => p.spaceId === "personal")).toBe(false);
  });
});
