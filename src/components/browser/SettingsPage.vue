<script setup lang="ts">
// Zigzag SettingsView/FormGroup/FormRow port, Copyright 2026 Zigao Wang,
// Apache-2.0. AppKit controls adapted to accessible Vue controls.
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import roveIcon from "../../../native/assets/icon.png";
import googleIcon from "../../assets/onboarding/google.png";
import bingIcon from "../../assets/onboarding/bing.svg";
import duckDuckGoIcon from "../../assets/onboarding/duckDuckGo.png";
import { useBrowser } from "../../stores/browser";
import { INTERFACE_LANGUAGES } from "../../lib/language";
import {
  BROWSING_DATA_OPTIONS,
  SETTINGS_PANES,
  PAGE_ZOOM_STEPS,
  READ_ALOUD_RATES,
  SEARCH_ENGINES,
  SHORTCUTS,
  SIDEBAR_DENSITIES,
  TRACKING_PROTECTION_LEVELS,
  settingsPane,
  shortcutFor,
  shortcutText,
  shortcutFromEvent,
  validateShortcuts,
  validSearchTemplate,
  type BrowsingDataKind,
  type SettingsPane,
  type Settings,
} from "../../lib/settings";
import type { Space } from "../../lib/types";
import Icon from "../common/Icon.vue";
import SpaceGlyph from "../common/SpaceGlyph.vue";
import SearchHighlight from "../common/SearchHighlight.vue";
import { matchesSettingsSearch, settingsSearchWords } from "../../lib/settings-search";
import SpaceIdentityEditor from "./SpaceIdentityEditor.vue";
import SpaceLookEditor from "./SpaceLookEditor.vue";
import { commandLook, committed, frameOf, lookOf, openDraft, type LookFields } from "../../lib/space-look";
import { paneDirection } from "../../lib/page-motion";
import { permissionKinds, permissions, type PermissionKind, type PermissionScope } from "../../lib/permissions";
import SettingsAbout from "./SettingsAbout.vue";
import SettingsExtensions from "./SettingsExtensions.vue";
const props = defineProps<{ tabId: string; url: string }>();
const b = useBrowser(),
  query = ref(""),
  scroll = ref<HTMLElement>(),
  recording = ref("");
const notice = ref(""),
  custom = ref(b.settings.customSearchTemplate),
  folder = ref(b.settings.downloadFolder);
let focusCustomSearch = false;
const editing = ref<string>(),
  spaceName = ref(""),
  spaceIcon = ref<string>(),
  pickingIcon = ref(false),
  spaceLook = ref<LookFields>({ color: "sky" });
const pending = ref<{
  kind: "space" | "update" | "data";
  id?: string;
  title: string;
  detail: string;
}>();
const clearKinds = ref<BrowsingDataKind[]>([]);
const clearLabels = Object.fromEntries(BROWSING_DATA_OPTIONS) as Record<BrowsingDataKind, string>;
const modal = ref<HTMLElement>();
let previousFocus: HTMLElement | null = null;
watch(
  () => !!editing.value || !!pending.value,
  async (open) => {
    if (open) {
      previousFocus = document.activeElement as HTMLElement;
      await nextTick();
      modal.value?.querySelector<HTMLElement>("input,button")?.focus();
    } else {
      await nextTick();
      previousFocus?.focus();
    }
  },
);
function trapFocus(event: KeyboardEvent) {
  const items = modal.value?.querySelectorAll<HTMLElement>(
    'button:not(:disabled), input, select, [tabindex="0"]',
  );
  if (!items?.length) return;
  const first = items[0]!,
    last = items[items.length - 1]!;
  if (event.shiftKey && document.activeElement === first) {
    event.preventDefault();
    last.focus();
  } else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault();
    first.focus();
  }
}
const navigationIds = [
  ["general", "appearance", "spaces", "shortcuts"],
  ["privacy", "passwords", "translation", "accessibility", "about"],
  ["extensions"],
  ["experimental"],
] as const satisfies readonly (readonly SettingsPane[])[];
const navigationGroups = navigationIds.map((ids) =>
  ids.map((id) => SETTINGS_PANES.find((item) => item.id === id)!),
);
const navigationOrder = navigationGroups.flat().map((item) => item.id);
const engineIcons: Partial<Record<Settings["searchEngine"], string>> = {
  google: googleIcon,
  bing: bingIcon,
  duckDuckGo: duckDuckGoIcon,
};
const pane = computed(() => settingsPane(props.url));
const paneMeta = computed(() => SETTINGS_PANES.find((item) => item.id === pane.value)!);
const paneDir = ref<0 | 1 | -1>(0);
let focusSuggestions = false;
watch(pane, (next, prev) => {
  paneDir.value = paneDirection(
    navigationOrder,
    prev,
    next,
  );
  if (scroll.value) scroll.value.scrollTop = 0;
  if (focusSuggestions && next === "general") focusSearchSuggestions();
});
function focusSearchSuggestions() {
  focusSuggestions = false;
  nextTick(() => document.getElementById(`setting-suggestions-${props.tabId}`)?.focus());
}
const searching = computed(() => !!query.value.trim());
const words = computed(() => settingsSearchWords(query.value));
function matches(...text: string[]) {
  return matchesSettingsSearch(words.value, ...text);
}
const visiblePermissionRules = computed(() => b.permissionRules.filter(([scope]) =>
  matches(b.t("网站权限"), b.t(permissions[scope.kind].name), scope.page, scope.requester, "site permissions"),
));
function setPermission(scope: PermissionScope, event: Event) {
  const choice = (event.target as HTMLSelectElement).value;
  b.send({ type: "permissionRule", scope, allowed: choice === "default" ? null : choice === "allow" });
}
type Row = {
  id: string;
  title: string;
  detail: string;
  kind: "toggle" | "select" | "input" | "button" | "status";
  setting?: keyof Settings;
  permission?: PermissionKind;
  forcedOn?: boolean;
  options?: readonly (readonly [string | number, string])[];
  action?: string;
  keywords?: string;
};
type Group = { pane: SettingsPane; title: string; detail?: string; rows: Row[] };
const devtoolsMcpFromEnv = window.__ERA_DEVTOOLS_MCP_FROM_ENV__ === true;
const groups = computed<Group[]>(() => [
  {
    pane: "general",
    title: b.t("搜索"),
    rows: [
      {
        id: "engine",
        title: b.t("搜索引擎"),
        detail: b.t("输入的内容不是网址时，使用此引擎搜索。"),
        kind: "select",
        setting: "searchEngine",
        options: SEARCH_ENGINES.map(([id, name]) => [id, b.t(name)]),
        keywords: "search engine",
      },
      ...(b.settings.searchEngine === "custom"
        ? [
            {
              id: "custom",
              title: b.t("搜索地址"),
              detail:
                b.t("用 %s 表示搜索词，例如 https://example.com/search?q=%s。"),
              kind: "input" as const,
              setting: "customSearchTemplate" as const,
            },
          ]
        : []),
      {
        id: "suggestions",
        title: b.t("搜索建议"),
        detail:
          b.settings.searchEngine === "google"
            ? b.t("输入的搜索词会发送给 Google 获取建议。")
            : b.t("当前引擎不提供远程建议；输入的搜索词仅在搜索时发送。"),
        kind: "toggle",
        setting: "searchSuggestions",
        keywords: "suggestions",
      },
    ],
  },
  {
    pane: "general",
    title: b.t("窗口"),
    rows: [
      {
        id: "default",
        title: b.t("默认浏览器"),
        detail: !b.native ? b.t("请在 Rove 桌面版中设置默认浏览器。") : !b.defaultBrowser ? b.t("在 Windows 设置中选择 Rove 打开网页链接。") : b.defaultBrowser.http && b.defaultBrowser.https ? b.t("Rove 已是 HTTP 和 HTTPS 链接的默认浏览器。") : b.defaultBrowser.http || b.defaultBrowser.https ? b.t("部分网页链接仍由其他浏览器打开，请在 Windows 中将 HTTP 和 HTTPS 都设为 Rove。") : b.t("Rove 尚未设为默认浏览器。在 Windows 设置中选择 Rove。"),
        kind: "button",
        action: b.t("设为默认浏览器"),
        keywords: "default browser",
      },
      {
        id: "language",
        title: b.t("界面语言"),
        detail: "",
        kind: "select",
        setting: "language",
        options: INTERFACE_LANGUAGES,
        keywords: "language locale 界面语言 简体中文 繁體中文 介面語言 English",
      },
      {
        id: "quit",
        title: b.t("退出前询问"),
        detail: b.t("关闭窗口前确认，避免意外关闭所有标签页。"),
        kind: "toggle",
        setting: "askBeforeQuit",
        keywords: "ask before quitting",
      },
    ],
  },
  {
    pane: "appearance",
    title: b.t("主题"),
    rows: [
      {
        id: "theme",
        title: b.t("外观"),
        detail: b.t("选择浏览器界面的浅色或深色外观。"),
        kind: "select",
        options: [
          ["light", b.t("浅色")],
          ["dark", b.t("深色")],
        ],
        keywords: "theme appearance 深色模式 浅色模式",
      },
    ],
  },
  {
    pane: "appearance",
    title: b.t("侧边栏设置"),
    rows: [
      {
        id: "sidebar-density",
        title: b.t("侧边栏紧凑程度"),
        detail: b.t("调整侧边栏元素之间的间距和标签按钮的内边距。"),
        kind: "select",
        setting: "sidebarDensity",
        options: SIDEBAR_DENSITIES.map(([id, label]) => [id, b.t(label)]),
        keywords: "sidebar density spacing padding compact comfortable 侧栏 紧凑程度",
      },
      {
        id: "names",
        title: b.t("大号工作区名称"),
        detail: b.t("在侧栏上方以轻盈的大字号显示名称；关闭后使用普通标题。"),
        kind: "toggle",
        setting: "largeSpaceNames",
        keywords: "large space names",
      },
      {
        id: "favorites",
        title: b.t("每行常用网站"),
        detail: b.t("自动根据侧栏宽度调整；也可固定每行的数量。"),
        kind: "select",
        setting: "favoritesPerRow",
        options: [
          [0, b.t("自动")],
          [2, "2"],
          [3, "3"],
          [4, "4"],
        ],
        keywords: "favorites per row",
      },
    ],
  },
  {
    pane: "accessibility",
    title: b.t("网页缩放"),
    rows: [
      {
        id: "page-zoom",
        title: b.t("默认页面缩放"),
        detail: b.t("未单独设置缩放的网页会使用此比例。"),
        kind: "select",
        setting: "defaultPageZoom",
        options: PAGE_ZOOM_STEPS.map((factor) => [factor, `${Math.round(factor * 100)}%`]),
        keywords: "default page zoom 网页缩放 頁面縮放 網頁縮放",
      },
    ],
  },
  {
    pane: "accessibility",
    title: b.t("语音"),
    rows: [
      {
        id: "read-aloud-enabled",
        title: b.t("文本朗读"),
        detail: b.t("打开后可用快捷键朗读网页，或右键朗读所选文本。"),
        kind: "toggle",
        setting: "readAloudEnabled",
        keywords: "read aloud text to speech tts 朗讀",
      },
      {
        id: "read-aloud-rate",
        title: b.t("朗读语速"),
        detail: b.t("语速在下次开始朗读时生效。"),
        kind: "select",
        setting: "readAloudRate",
        options: READ_ALOUD_RATES.map(([rate, label]) => [rate, b.t(label)]),
        keywords: "read aloud text to speech tts narration speech rate 朗讀 語速",
      },
    ],
  },
  {
    pane: "accessibility",
    title: b.t("动态效果"),
    rows: [
      {
        id: "motion",
        title: b.t("减少动态效果"),
        detail: b.t("使用原地淡入淡出，减少滑动和缩放；同时遵循系统设置。"),
        kind: "toggle",
        setting: "reduceMotion",
        keywords: "reduce motion animation 动画",
      },
    ],
  },
  {
    pane: "general",
    title: b.t("标签页"),
    rows: [
      {
        id: "sleep",
        title: b.t("后台标签休眠"),
        detail:
          b.t("一段时间没有查看的标签页会释放内存，再次打开时回到原来的位置；正在播放声音或加载中的标签页不会休眠。"),
        kind: "toggle",
        setting: "sleepTabs",
        keywords: "tabs sleep asleep memory background suspend 标签 休眠 内存",
      },
    ],
  },
  {
    pane: "general",
    title: b.t("下载"),
    rows: [
      {
        id: "folder",
        title: b.t("保存下载到"),
        detail: b.t("填写已存在的文件夹完整路径；留空使用系统下载文件夹。"),
        kind: "input",
        setting: "downloadFolder",
        keywords: "downloads folder",
      },
    ],
  },
  {
    pane: "general",
    title: b.t("欢迎引导"),
    rows: [
      {
        id: "onboarding",
        title: b.t("重新选择常用网站，或了解首次使用流程。"),
        detail: "",
        kind: "button",
        action: b.t("打开"),
        keywords: "onboarding welcome guide 欢迎引导 欢迎向导",
      },
    ],
  },
  {
    pane: "passwords",
    title: b.t("自动填充"),
    rows: [
      {
        id: "passwords",
        title: b.t("提示保存密码"),
        detail: b.t("由 WebView2 管理网站密码的保存提示。"),
        kind: "toggle",
        setting: "savePasswords",
        keywords: "save passwords",
      },
      {
        id: "autofill",
        title: b.t("自动填充表单"),
        detail: b.t("允许填充表单信息，并在登录页提供已导入密码的账号选择。"),
        kind: "toggle",
        setting: "autofill",
        keywords: "forms autofill",
      },
    ],
  },
  {
    pane: "privacy",
    title: b.t("网络连接"),
    rows: [
      {
        id: "tracking-protection",
        title: b.t("跟踪防护"),
        detail: b.t("限制跨站跟踪。更改后请刷新网页，严格模式可能影响部分网站。"),
        kind: "select",
        setting: "trackingProtection",
        options: TRACKING_PROTECTION_LEVELS.map(([id, label]) => [id, b.t(label)]),
        keywords: "tracking prevention tracker 防追踪 防跟踪",
      },
      {
        id: "webRtcAddressProtection",
        title: b.t("WebRTC 地址保护"),
        detail: b.t("限制 WebRTC 使用未经过代理的 UDP 连接，减少 IP 地址暴露。部分音视频通话可能受影响。") + " " +
          b.t(window.__ERA_WEB_RTC_PREFERENCE_FAILED__
            ? "无法应用 WebRTC 地址保护设置，请重启 Rove 后重试。"
            : "更改后重启 Rove 生效。"),
        kind: "toggle",
        setting: "webRtcAddressProtection",
        keywords: "webrtc ip address protection proxy vpn udp 代理",
      },
      {
        id: "search-privacy",
        title: b.t("搜索时发送的输入"),
        detail: b.t("使用 Google 并开启搜索建议时，提交搜索前输入的内容可能会发送给 Google。"),
        kind: "button",
        action: b.t("转到搜索建议"),
        keywords: "search suggestions Google remote privacy",
      },
    ],
  },
  {
    pane: "privacy",
    title: b.t("浏览数据"),
    rows: [
      {
        id: "remember",
        title: b.t("记录浏览历史"),
        detail:
          b.t("关闭后将停止写入 Rove 的访问历史。已有历史会保留，打开的和最近关闭的标签页仍会随会话保存。"),
        kind: "toggle",
        setting: "rememberHistory",
        keywords: "remember browsing history",
      },
      {
        id: "clear-data",
        title: b.t("清除浏览数据"),
        detail: b.t("选择要清除的浏览历史、缓存的图片和文件、Cookie 和网站数据或下载记录。"),
        kind: "button",
        action: b.t("清除…"),
        keywords: "clear history cache cookies site data storage downloads",
      },
    ],
  },
  {
    pane: "privacy",
    title: b.t("网站权限默认行为"),
    detail: b.t("已保存的网站权限优先。"),
    rows: permissionKinds.map((kind) => ({
      id: `permission-${kind}`,
      title: b.t(permissions[kind].name),
      detail: "",
      kind: "select",
      permission: kind,
      options: [["ask", b.t("询问")], ["block", b.t("阻止")]],
      keywords: `site permissions default ${kind}`,
    })),
  },
  {
    pane: "translation",
    title: b.t("网页翻译"),
    rows: [
      {
        id: "translation",
        title: b.t("翻译网页"),
        detail: b.t("Rove 尚未接入网页翻译服务。"),
        kind: "status",
        action: b.t("暂未提供"),
        keywords: "translation language 翻译 语言",
      },
    ],
  },
  {
    pane: "experimental",
    title: b.t("网页调试"),
    rows: [
      {
        id: "devtoolsMcp",
        title: "Chrome DevTools MCP",
        detail: b.t("允许本机 MCP 客户端通过无需身份验证的端口访问和调试网页。") + " " +
          b.t("启用后，部分网站的人机验证可能失败。") + " " +
          b.t(devtoolsMcpFromEnv
            ? "此接口由 ROVE_DEVTOOLS_MCP 启用。清除该变量并重启 Rove 后可在此更改。"
            : "更改后重启 Rove 生效。"),
        kind: "toggle",
        setting: "devtoolsMcp",
        forcedOn: devtoolsMcpFromEnv,
        keywords: "cdp devtools remote debugging 开发者工具 開發人員工具",
      },
    ],
  },
]);
const visibleGroups = computed(() =>
  groups.value.flatMap((group) => {
    if (!searching.value && group.pane !== pane.value) return [];
    const meta = SETTINGS_PANES.find((p) => p.id === group.pane)!;
    const rows = group.rows.filter(
      (row) =>
        !searching.value ||
        matches(b.t(meta.title), meta.keywords, group.title) ||
        matches(row.title, row.detail, row.keywords || "", row.action || ""),
    );
    return rows.length ? [{ ...group, rows, key: group.rows[0]!.id }] : [];
  }),
);
const visibleSpaces = computed(() =>
  b.state.spaces.filter(
    (space) => !searching.value || matches(b.t("工作区"), b.t("你的工作区"), "spaces", space.name),
  ),
);
const visibleShortcuts = computed(() =>
  SHORTCUTS.map((item) => ({
    id: item.id,
    title: b.t(item.title),
    group: b.t(item.group),
    keys: b.t(shortcutText(shortcutFor(item.id, b.settings))),
  })).filter(
    (item) =>
      !searching.value ||
      matches(
        b.t("快捷键"),
        item.group,
        item.title,
        item.keys,
      ),
  ),
);
const showSpaces = computed(
  () =>
    (searching.value || pane.value === "spaces") &&
    !!visibleSpaces.value.length,
);
const showShortcuts = computed(
  () =>
    (searching.value || pane.value === "shortcuts") &&
    !!visibleShortcuts.value.length,
);
const aboutPane = SETTINGS_PANES.find((item) => item.id === "about")!;
const aboutSummary = computed(() => b.t("当前版本、软件更新和更新通道。"));
const showAboutResult = computed(() =>
  searching.value && matches(b.t(aboutPane.title), aboutSummary.value, aboutPane.keywords),
);
const extensionSummary = computed(() => b.t("加载本地 Chrome 扩展或从 Chrome 网上应用店安装。"));
const showExtensionsResult = computed(() =>
  searching.value && matches(b.t("扩展"), extensionSummary.value, "extension plugin 插件 Manifest V3"),
);
const noResults = computed(
  () =>
    searching.value &&
    !visibleGroups.value.length &&
    !showSpaces.value &&
    !showShortcuts.value &&
    !showAboutResult.value &&
    !showExtensionsResult.value,
);
function pick(id: SettingsPane) {
  query.value = "";
  recording.value = "";
  b.send({ type: "settingsPane", id: props.tabId, pane: id });
}
function update(key: keyof Settings, value: unknown) {
  notice.value = "";
  b.send({ type: "settingsUpdate", settings: { ...b.settings, [key]: value } });
}
function value(row: Row) {
  if (row.forcedOn) return true;
  if (row.permission) return b.settings.permissionDefaults[row.permission];
  return row.id === "theme"
    ? b.state.theme
    : row.setting
      ? b.settings[row.setting]
      : "";
}
function select(row: Row, event: Event) {
  const selected = (event.target as HTMLSelectElement).value;
  if (row.id === "theme") {
    if (selected !== b.state.theme) b.send({ type: "theme" });
  } else if (row.permission) {
    update("permissionDefaults", { ...b.settings.permissionDefaults, [row.permission]: selected });
  } else if (row.setting) {
    const option = row.options?.find(([value]) => String(value) === selected);
    if (row.id === "engine") focusCustomSearch = selected === "custom" && !custom.value;
    if (option) update(row.setting, option[0]);
  }
}
function saveInput(id: string) {
  if (id === "custom") {
    if (custom.value && !validSearchTemplate(custom.value.trim())) {
      notice.value = b.t("搜索地址需要包含 %s，并使用有效的 HTTP 或 HTTPS 网址。");
      return;
    }
    update("customSearchTemplate", custom.value.trim());
  } else update("downloadFolder", folder.value.trim());
}
function action(row: Row) {
  if (row.id === "onboarding") {
    b.openOnboarding();
    return;
  }
  if (row.id === "default") {
    if (!b.native) { notice.value = b.t("请在 Rove 桌面版中设置默认浏览器。"); return; }
    b.send({ type: "systemSettings" });
    return;
  }
  if (row.id === "search-privacy") {
    focusSuggestions = true;
    pick("general");
    if (pane.value === "general") focusSearchSuggestions();
  } else if (row.id === "clear-data") {
    clearKinds.value = [];
    pending.value = { kind: "data", title: b.t("清除浏览数据"), detail: b.t("选择要清除的项目。清理范围为全部时间。") };
  }
}
function editSpace(space?: Space) {
  editing.value = space?.id || "new";
  spaceName.value = space?.name || "";
  spaceIcon.value = space?.icon;
  pickingIcon.value = false;
  const source = space ?? { color: "sky" as const };
  spaceLook.value = committed(openDraft(source, b.state.theme === "dark"), lookOf(source));
}
function saveSpace() {
  if (!spaceName.value.trim()) {
    notice.value = b.t("请输入工作区名称。");
    return;
  }
  b.send({
    type: editing.value === "new" ? "spaceCreate" : "spaceUpdate",
    id: editing.value,
    name: spaceName.value.trim(),
    icon: spaceIcon.value ?? null,
    background: true,
    ...commandLook(spaceLook.value),
  });
  editing.value = undefined;
}
function tileStyle(space: Space) {
  const painted = frameOf(space, b.state.theme, false);
  return painted.custom ? { background: painted.end ? `linear-gradient(160deg, ${painted.start}, ${painted.end})` : painted.start } : undefined;
}
function dropSpace(event: DragEvent, before: string) {
  const id = event.dataTransfer?.getData("text/era-space");
  if (id) b.send({ type: "spaceReorder", id, before });
}
function askToUpdate(offerId: string) {
  const { channel, version } = b.updateStatus;
  const name = b.t(channel === "canary" ? "Canary 构建 {version}" : "版本 {version}", { version: version ?? "" });
  pending.value = {
    kind: "update",
    id: offerId,
    title: b.t("更新并重启 Rove？"),
    detail: b.t("Rove 将下载 {name} 并保存浏览会话，然后关闭窗口、安装更新并重新打开。", { name }),
  };
}
watch(() => b.updateStatus.offerId, (offerId) => {
  if (pending.value?.kind === "update" && pending.value.id !== offerId)
    pending.value = undefined;
});
function confirm() {
  if (!pending.value) return;
  if (pending.value.kind === "space")
    b.send({ type: "spaceDelete", id: pending.value.id });
  else if (pending.value.kind === "update") {
    if (pending.value.id) b.installUpdate(pending.value.id);
  } else if (b.native) {
    b.clearBrowsingData(BROWSING_DATA_OPTIONS.map(([kind]) => kind).filter((kind) => clearKinds.value.includes(kind)));
  } else notice.value = b.t("界面预览不保存真实浏览数据；请在 Windows 应用中清除。");
  pending.value = undefined;
}
function record(event: KeyboardEvent) {
  if (!recording.value) return;
  event.preventDefault();
  event.stopImmediatePropagation();
  if (event.key === "Escape") {
    recording.value = "";
    return;
  }
  const binding =
    event.key === "Backspace" || event.key === "Delete"
      ? null
      : shortcutFromEvent(event);
  if (binding === undefined) return;
  const next = {
    ...b.settings,
    shortcuts: { ...b.settings.shortcuts, [recording.value]: binding },
  };
  try {
    validateShortcuts(next);
    update("shortcuts", next.shortcuts);
    recording.value = "";
  } catch (error) {
    notice.value = String(error).replace(/^Error: /, "");
  }
}
watch(recording, (id) => {
  window.removeEventListener("keydown", record, true);
  if (id) window.addEventListener("keydown", record, true);
});
watch(
  () => props.url,
  () => {
    if (scroll.value) scroll.value.scrollTop = 0;
  },
);
watch(query, () => {
  if (scroll.value) scroll.value.scrollTop = 0;
});
watch(() => b.settings.searchEngine, (engine) => {
  if (!focusCustomSearch) return;
  focusCustomSearch = false;
  if (engine === "custom") {
    // Wait for the picker to finish restoring focus to the select.
    requestAnimationFrame(() => document.getElementById(`custom-${props.tabId}`)?.focus());
  }
});
watch(
  () => b.settings.customSearchTemplate,
  (v) => (custom.value = v),
);
watch(
  () => b.settings.downloadFolder,
  (v) => (folder.value = v),
);
onBeforeUnmount(() => window.removeEventListener("keydown", record, true));
</script>
<template>
  <section class="settings-page" :aria-label="b.t('设置')">
    <div class="settings-layout" :inert="!!editing || !!pending">
      <aside class="settings-navigation">
        <div class="settings-brand">
          <img :src="roveIcon" alt="" width="24" height="24" draggable="false" />
          <h1>{{ b.t('设置') }}</h1>
        </div>
        <nav :aria-label="b.t('设置分类')">
          <ul v-for="items in navigationGroups" :key="items[0]!.id" class="settings-nav-group">
            <li v-for="item in items" :key="item.id">
              <button
                :title="b.t(item.title)"
                :aria-label="b.t(item.title)"
                :aria-current="pane === item.id && !searching ? 'page' : undefined"
                @click="pick(item.id)"
              >
                <Icon :name="item.icon" :size="17" /><span>{{ b.t(item.title) }}</span>
              </button>
            </li>
          </ul>
        </nav>
      </aside>
      <div class="settings-main">
        <header class="settings-page-header">
          <div class="settings-header-inner">
            <label class="settings-search">
              <Icon name="search" :size="15" />
              <input
                v-model="query"
                type="search"
                :placeholder="b.t('查找设置')"
                :aria-label="b.t('查找设置')"
                @keydown.esc.stop="query = ''"
              />
            </label>
            <h2>
              <Icon :name="searching ? 'search' : paneMeta.icon" :size="19" />
              {{ searching ? b.t('搜索结果') : b.t(paneMeta.title) }}
            </h2>
          </div>
        </header>
        <div ref="scroll" class="settings-scroll" :data-page-dir="paneDir">
          <Transition name="settings-pane">
          <div :key="pane" class="settings-content">
            <SettingsAbout v-if="pane === 'about' && !searching" :tab-id="tabId" @confirm-update="askToUpdate" />
            <SettingsExtensions v-if="pane === 'extensions' && !searching" />
            <button v-if="showExtensionsResult" class="about-search-result" @click="pick('extensions')">
              <Icon name="extension" />
              <span class="setting-description">
                <strong><SearchHighlight :text="b.t('扩展')" :words="words" /></strong>
                <span class="about-search-summary"><SearchHighlight :text="extensionSummary" :words="words" /></span>
              </span>
              <Icon name="forward" :size="13" />
            </button>
            <button v-if="showAboutResult" class="about-search-result" @click="pick('about')">
              <Icon name="info" />
              <span class="setting-description">
                <strong><SearchHighlight :text="b.t(aboutPane.title)" :words="words" /></strong>
                <span class="about-search-summary"><SearchHighlight :text="aboutSummary" :words="words" /></span>
              </span>
              <Icon name="forward" :size="13" />
            </button>
            <p
              v-if="(pane !== 'about' || searching) && notice"
              class="settings-feedback"
              role="status"
            >
              {{ notice }}
            </p>
            <section
              v-for="group in visibleGroups"
              :key="group.key"
              class="settings-group"
              :aria-label="group.title"
            >
              <h3 :class="{ 'settings-result-heading': searching }">
                <span v-if="searching" class="settings-result-pane">
                  <SearchHighlight :text="b.t(SETTINGS_PANES.find((p) => p.id === group.pane)!.title)" :words="words" />
                  <span class="settings-result-separator" aria-hidden="true">›</span>
                </span>
                <span><SearchHighlight :text="group.title" :words="words" /></span>
              </h3>
              <p v-if="group.detail" class="settings-group-detail">{{ group.detail }}</p>
              <div v-for="row in group.rows" :key="row.id" class="setting-row" :class="{ 'setting-engine-row': row.id === 'engine' }">
                <div class="setting-description">
                  <label :for="`setting-${row.id}-${tabId}`"><SearchHighlight :text="row.title" :words="words" /></label>
                  <p v-if="row.detail" :id="`setting-${row.id}-${tabId}-detail`"><SearchHighlight :text="row.detail" :words="words" /></p>
                </div>
                <button
                  v-if="row.kind === 'toggle'"
                  :id="`setting-${row.id}-${tabId}`"
                  class="setting-toggle"
                  role="switch"
                  :disabled="row.forcedOn"
                  :aria-checked="!!value(row)"
                  :aria-label="row.title"
                  :aria-describedby="row.detail ? `setting-${row.id}-${tabId}-detail` : undefined"
                  @click="update(row.setting!, !value(row))"
                >
                  <span />
                </button>
                <select
                  v-else-if="row.kind === 'select'"
                  :id="`setting-${row.id}-${tabId}`"
                  class="setting-select"
                  :class="{ 'setting-engine-select': row.id === 'engine' }"
                  :aria-label="row.title"
                  :aria-describedby="row.detail ? `setting-${row.id}-${tabId}-detail` : undefined"
                  :value="value(row)"
                  @change="select(row, $event)"
                >
                  <button v-if="row.id === 'engine'" type="button">
                    <!-- Vue's HTML tag list does not include selectedcontent yet. -->
                    <component :is="'selectedcontent'" />
                  </button>
                  <option v-for="[id, label] in row.options" :key="id" :value="id">
                    <template v-if="row.id === 'engine'">
                      <span class="setting-option-icon" aria-hidden="true">
                        <img v-if="engineIcons[id]" :src="engineIcons[id]" alt="" width="16" height="16" />
                        <Icon v-else name="search" :size="16" />
                      </span>
                      <span class="setting-option-label">{{ label }}</span>
                    </template>
                    <template v-else>{{ label }}</template>
                  </option>
                </select>
                <form
                  v-else-if="row.kind === 'input'"
                  class="setting-input-form"
                  @submit.prevent="saveInput(row.id)"
                >
                  <input
                    v-if="row.id === 'custom'"
                    :id="`custom-${tabId}`"
                    v-model="custom"
                    :aria-label="b.t('搜索地址')"
                    placeholder="https://example.com/search?q=%s"
                    maxlength="2048"
                    @change="saveInput(row.id)"
                  /><input
                    v-else
                    v-model="folder"
                    :aria-label="b.t('下载文件夹')"
                    :placeholder="b.t('系统下载文件夹')"
                    maxlength="1024"
                    @change="saveInput(row.id)"
                  /><button type="submit" class="setting-button">{{ b.t('保存') }}</button>
                </form>
                <button
                  v-else-if="row.kind === 'button'"
                  :id="`setting-${row.id}-${tabId}`"
                  class="setting-button"
                  :disabled="(row.id === 'default' && !b.native) || (row.id === 'clear-data' && b.dataClearPending)"
                  :aria-label="row.id === 'onboarding' ? b.t('打开欢迎引导') : row.id === 'clear-data' ? row.title : row.action"
                  @click="action(row)"
                >
                  <SearchHighlight :text="row.action!" :words="words" />
                </button>
                <span v-else class="setting-status"><SearchHighlight :text="row.action!" :words="words" /></span>
              </div>
              <ul v-if="b.dataClear && group.rows.some((row) => row.id === 'clear-data')" class="settings-clear-results" aria-live="polite">
                <li v-for="result in b.dataClear.results" :key="result.kind">
                  <span>{{ b.t(clearLabels[result.kind]) }}</span>
                  <span>{{ b.t(result.ok === null ? '正在清除…' : result.ok ? '已清除' : '清除失败，请重试') }}</span>
                </li>
              </ul>
            </section>
            <section
              v-if="b.native && (searching ? visiblePermissionRules.length : pane === 'privacy')"
              class="settings-group"
              :aria-label="b.t('网站权限')"
            >
              <h3>{{ b.t('网站权限') }}</h3>
              <div v-if="!visiblePermissionRules.length" class="setting-row">
                <div class="setting-description"><p>{{ b.t('你还没有保存任何网站的权限选择。') }}</p></div>
              </div>
              <div v-for="([scope, allowed], index) in visiblePermissionRules"
                :key="JSON.stringify(scope)" class="setting-row">
                <div class="setting-description permission-description">
                  <label :for="`permission-rule-${tabId}-${index}`">{{ b.t(permissions[scope.kind].name) }} · <bdi>{{ scope.requester }}</bdi></label>
                  <p v-if="scope.page !== scope.requester">{{ b.t('嵌入在') }} <bdi>{{ scope.page }}</bdi></p>
                </div>
                <select :id="`permission-rule-${tabId}-${index}`" class="setting-select" :value="allowed ? 'allow' : 'block'"
                  @change="setPermission(scope, $event)">
                  <option value="default">{{ b.t('使用默认设置') }}</option>
                  <option value="allow">{{ b.t('允许') }}</option>
                  <option value="block">{{ b.t('阻止') }}</option>
                </select>
              </div>
            </section>
            <section
              v-if="showSpaces"
              class="settings-group"
              :aria-label="b.t('你的工作区')"
            >
              <div class="settings-group-heading">
                <h3 :class="{ 'settings-result-heading': searching }">
                  <span v-if="searching" class="settings-result-pane">
                    <SearchHighlight :text="b.t('工作区')" :words="words" />
                    <span class="settings-result-separator" aria-hidden="true">›</span>
                  </span>
                  <span><SearchHighlight :text="b.t('你的工作区')" :words="words" /></span>
                </h3>
                <button
                  class="setting-button"
                  :disabled="b.state.spaces.length >= 16"
                  @click="editSpace()"
                >
                  {{ b.t('新建工作区') }}
                </button>
              </div>
              <div
                v-for="space in visibleSpaces"
                :key="space.id"
                class="setting-row space-setting"
                draggable="true"
                @dragstart="
                  $event.dataTransfer?.setData('text/era-space', space.id)
                "
                @dragover.prevent
                @drop.prevent="dropSpace($event, space.id)"
              >
                <span class="space-color-tile" :data-color="space.color" :style="tileStyle(space)">
                  <SpaceGlyph v-if="space.icon" :icon="space.icon" :size="16" />
                  <Icon v-else name="spaces" :size="18" />
                </span>
                <div class="setting-description">
                  <strong><SearchHighlight :text="space.name" :words="words" /></strong>
                  <p>
                    {{ b.t('{count} 个标签页', { count: b.state.tabs.filter((t) => t.spaceId === space.id && !t.pinned).length }) }} ·
                    {{ b.t('{count} 个固定项', { count: b.state.pins?.filter((p) => p.spaceId === space.id).length ?? 0 }) }}
                  </p>
                </div>
                <div class="setting-inline-actions">
                  <button
                    class="setting-button"
                    :aria-label="b.t('编辑 {name}', { name: space.name })"
                    @click="editSpace(space)"
                  >
                    {{ b.t('编辑') }}</button
                  ><button
                    class="icon-button"
                    :aria-label="b.t('删除 {name}', { name: space.name })"
                    :disabled="b.state.spaces.length <= 1"
                    @click="
                      pending = {
                        kind: 'space',
                        id: space.id,
                        title: b.t('删除“{name}”？', { name: space.name }),
                        detail:
                          b.t('此工作区的标签页与固定项将被移除，共用的常用网站保留。此操作无法撤销。'),
                      }
                    "
                  >
                    <Icon name="trash" />
                  </button>
                </div>
              </div>
              <p class="setting-hint">{{ b.t('拖动工作区调整顺序。') }}</p>
            </section>
            <section
              v-if="showShortcuts"
              class="settings-group"
              :aria-label="b.t('快捷键列表')"
            >
              <div class="settings-group-heading">
                <h3 :class="{ 'settings-result-heading': searching }"><span><SearchHighlight :text="b.t('快捷键')" :words="words" /></span></h3>
                <button
                  class="setting-button"
                  @click="
                    recording = '';
                    update('shortcuts', {});
                  "
                >
                  {{ b.t('恢复默认') }}
                </button>
              </div>
              <p class="setting-hint">
                {{ b.t('点击按键开始录制，Esc 取消，Backspace 清除。Ctrl + 1–9 用于切换工作区。') }}
              </p>
              <div
                v-for="item in visibleShortcuts"
                :key="item.id"
                class="setting-row"
              >
                <div class="setting-description">
                  <strong><SearchHighlight :text="item.title" :words="words" /></strong>
                  <p><SearchHighlight :text="item.group" :words="words" /></p>
                </div>
                <button
                  class="setting-shortcut"
                  :class="{ recording: recording === item.id }"
                  :aria-label="b.t('修改快捷键：{name}', { name: item.title })"
                  @click="
                    notice = '';
                    recording = recording === item.id ? '' : item.id;
                  "
                >
                  <kbd><SearchHighlight
                    :text="recording === item.id ? b.t('请按下组合键…') : item.keys"
                    :words="words"
                  /></kbd>
                </button>
              </div>
            </section>
            <p v-if="noResults" class="settings-empty">
              {{ b.t('没有找到与“{query}”匹配的设置。', { query: query.trim() }) }}
            </p>
          </div>
          </Transition>
        </div>
      </div>
    </div>
    <div
      v-if="editing || pending"
      ref="modal"
      class="settings-local-scrim"
      @pointerdown.self="
        editing = undefined;
        pending = undefined;
      "
      @keydown.tab="trapFocus"
      @keydown.esc.stop="
        editing = undefined;
        pending = undefined;
      "
    >
      <section
        v-if="editing"
        class="settings-space-editor"
        role="dialog"
        aria-modal="true"
        :aria-label="editing === 'new' ? b.t('新建工作区') : b.t('编辑工作区')"
      >
        <header>
          <h2>{{ editing === "new" ? b.t('新建工作区') : b.t('编辑工作区') }}</h2>
          <button
            class="icon-button"
            :aria-label="b.t('关闭编辑')"
            @click="editing = undefined"
          >
            <Icon name="close" />
          </button>
        </header>
        <form @submit.prevent="saveSpace">
          <SpaceIdentityEditor v-model:name="spaceName" v-model:icon="spaceIcon" v-model:picking="pickingIcon" />
          <SpaceLookEditor
            v-show="!pickingIcon"
            :space="spaceLook"
            :dark="b.state.theme === 'dark'"
            @commit="spaceLook = $event"
            @preview="spaceLook = $event"
          />
          <p v-if="notice" role="status">{{ notice }}</p>
          <footer>
            <button
              type="button"
              class="setting-button"
              @click="editing = undefined"
            >
              {{ b.t('取消') }}</button
            ><button class="primary-button">{{ b.t('保存') }}</button>
          </footer>
        </form>
      </section>
      <section
        v-else-if="pending"
        class="settings-space-editor"
        :role="pending.kind === 'data' ? 'dialog' : 'alertdialog'"
        aria-modal="true"
        :aria-label="pending.title"
        :aria-describedby="`settings-confirm-detail-${tabId}`"
      >
        <h2>{{ pending.title }}</h2>
        <p :id="`settings-confirm-detail-${tabId}`">{{ pending.detail }}</p>
        <div v-if="pending.kind === 'data'" class="settings-clear-options">
          <label v-for="[kind, label] in BROWSING_DATA_OPTIONS" :key="kind">
            <input type="checkbox" v-model="clearKinds" :value="kind" />
            <span>{{ b.t(label) }}</span>
          </label>
          <p v-if="clearKinds.includes('siteData')" role="status">{{ b.t('清除 Cookie 和网站数据会退出网站登录。') }}</p>
        </div>
        <footer>
          <button class="setting-button" @click="pending = undefined">
            {{ b.t('取消') }}</button
            ><button class="primary-button" :disabled="pending.kind === 'data' && (!clearKinds.length || b.dataClearPending)" @click="confirm">
            {{ pending.kind === "update" ? b.t('确认更新并重启') : pending.kind === "space" ? b.t('删除工作区') : b.t('确认清除') }}
          </button>
        </footer>
      </section>
    </div>
  </section>
</template>
<style src="../../styles/settings.css"></style>
