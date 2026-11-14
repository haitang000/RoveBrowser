<script setup lang="ts">
// Zigzag SettingsView/FormGroup/FormRow port, Copyright 2026 Zigao Wang,
// Apache-2.0. AppKit controls adapted to accessible Vue controls.
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import { useBrowser } from "../../stores/browser";
import {
  SETTINGS_PANES,
  SEARCH_ENGINES,
  SHORTCUTS,
  settingsPane,
  shortcutFor,
  shortcutText,
  shortcutFromEvent,
  validateShortcuts,
  validSearchTemplate,
  type SettingsPane,
  type Settings,
} from "../../lib/settings";
import type { Space, SpaceColor } from "../../lib/types";
import Icon from "../common/Icon.vue";
import SpaceGlyph from "../common/SpaceGlyph.vue";
import SpaceIdentityEditor from "./SpaceIdentityEditor.vue";
const props = defineProps<{ tabId: string; url: string }>();
const b = useBrowser(),
  query = ref(""),
  scroll = ref<HTMLElement>(),
  recording = ref("");
const notice = ref(""),
  custom = ref(b.settings.customSearchTemplate),
  folder = ref(b.settings.downloadFolder);
const editing = ref<string>(),
  spaceName = ref(""),
  spaceIcon = ref<string>(),
  pickingIcon = ref(false),
  spaceColor = ref<SpaceColor>("sky"),
  translucency = ref(0);
const pending = ref<{
  kind: string;
  id?: string;
  title: string;
  detail: string;
}>();
const modal = ref<HTMLElement>();
let previousFocus: HTMLElement | null = null;
watch(
  () => !!editing.value || !!pending.value,
  async (open) => {
    if (open) {
      previousFocus = document.activeElement as HTMLElement;
      await nextTick();
      modal.value?.querySelector<HTMLElement>("input,button")?.focus();
    } else previousFocus?.focus();
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
const colors: { id: SpaceColor; title: string }[] = [
  { id: "sky", title: "天空" },
  { id: "sage", title: "鼠尾草" },
  { id: "sand", title: "沙色" },
  { id: "rose", title: "玫瑰" },
  { id: "lavender", title: "薰衣草" },
  { id: "mint", title: "薄荷" },
  { id: "peach", title: "蜜桃" },
  { id: "graphite", title: "石墨" },
];
const pane = computed(() => settingsPane(props.url));
const searching = computed(() => !!query.value.trim());
const words = computed(() =>
  query.value
    .normalize("NFKD")
    .toLocaleLowerCase()
    .trim()
    .split(/\s+/)
    .filter(Boolean),
);
function matches(...text: string[]) {
  const joined = text.join(" ").normalize("NFKD").toLocaleLowerCase();
  return words.value.every((word) => joined.includes(word));
}
type Row = {
  id: string;
  title: string;
  detail: string;
  kind: "toggle" | "select" | "input" | "button" | "status";
  setting?: keyof Settings;
  options?: readonly (readonly [string | number, string])[];
  action?: string;
  keywords?: string;
};
type Group = { pane: SettingsPane; title: string; rows: Row[] };
const groups = computed<Group[]>(() => [
  {
    pane: "general",
    title: "搜索",
    rows: [
      {
        id: "engine",
        title: "搜索引擎",
        detail: "输入的内容不是网址时，使用此引擎搜索。",
        kind: "select",
        setting: "searchEngine",
        options: SEARCH_ENGINES.map(([id, name]) => [id, name]),
        keywords: "search engine",
      },
      ...(b.settings.searchEngine === "custom"
        ? [
            {
              id: "custom",
              title: "搜索地址",
              detail:
                "用 %s 表示搜索词，例如 https://example.com/search?q=%s。",
              kind: "input" as const,
              setting: "customSearchTemplate" as const,
            },
          ]
        : []),
      {
        id: "suggestions",
        title: "搜索建议",
        detail:
          b.settings.searchEngine === "google"
            ? "输入的搜索词会发送给 Google 获取建议。"
            : "当前引擎不提供远程建议；输入的搜索词仅在搜索时发送。",
        kind: "toggle",
        setting: "searchSuggestions",
        keywords: "suggestions",
      },
      {
        id: "default",
        title: "默认浏览器",
        detail: !b.native ? "请在 Rove 桌面版中设置默认浏览器。" : !b.defaultBrowser ? "在 Windows 设置中选择 Rove 打开网页链接。" : b.defaultBrowser.http && b.defaultBrowser.https ? "Rove 已是 HTTP 和 HTTPS 链接的默认浏览器。" : b.defaultBrowser.http || b.defaultBrowser.https ? "部分网页链接仍由其他浏览器打开，请在 Windows 中将 HTTP 和 HTTPS 都设为 Rove。" : "Rove 尚未设为默认浏览器。在 Windows 设置中选择 Rove。",
        kind: "button",
        action: "设为默认浏览器",
        keywords: "default browser",
      },
    ],
  },
  {
    pane: "general",
    title: "窗口与外观",
    rows: [
      {
        id: "theme",
        title: "外观",
        detail: "选择浏览器界面的浅色或深色外观。",
        kind: "select",
        options: [
          ["light", "浅色"],
          ["dark", "深色"],
        ],
        keywords: "theme appearance 深色模式 浅色模式",
      },
      {
        id: "names",
        title: "大号工作区名称",
        detail: "在侧栏上方以轻盈的大字号显示名称；关闭后使用普通标题。",
        kind: "toggle",
        setting: "largeSpaceNames",
        keywords: "large space names",
      },
      {
        id: "favorites",
        title: "每行常用网站",
        detail: "自动根据侧栏宽度调整；也可固定每行的数量。",
        kind: "select",
        setting: "favoritesPerRow",
        options: [
          [0, "自动"],
          [2, "2"],
          [3, "3"],
          [4, "4"],
        ],
        keywords: "favorites per row",
      },
      {
        id: "quit",
        title: "退出前询问",
        detail: "关闭窗口前确认，避免意外关闭所有标签页。",
        kind: "toggle",
        setting: "askBeforeQuit",
        keywords: "ask before quitting",
      },
      {
        id: "motion",
        title: "减少动态效果",
        detail: "使用原地淡入淡出，减少滑动和缩放；同时遵循系统设置。",
        kind: "toggle",
        setting: "reduceMotion",
        keywords: "reduce motion animation 动画",
      },
    ],
  },
  {
    pane: "general",
    title: "下载",
    rows: [
      {
        id: "folder",
        title: "保存下载到",
        detail: "填写已存在的文件夹完整路径；留空使用系统下载文件夹。",
        kind: "input",
        setting: "downloadFolder",
        keywords: "downloads folder",
      },
    ],
  },
  {
    pane: "general",
    title: "关于",
    rows: [
      {
        id: "version",
        title: "Rove",
        detail: `版本 ${__APP_VERSION__} · Windows / WebView2`,
        kind: "status",
        action: "浏览器",
        keywords: "version about 版本",
      },
      { id: "onboarding", title: "欢迎引导", detail: "重新选择常用网站，或了解首次使用流程。", kind: "button", action: "打开", keywords: "onboarding welcome 新手" },
    ],
  },
  {
    pane: "passwords",
    title: "自动填充",
    rows: [
      {
        id: "passwords",
        title: "提示保存密码",
        detail: "由 WebView2 管理网站密码的保存提示。",
        kind: "toggle",
        setting: "savePasswords",
        keywords: "save passwords",
      },
      {
        id: "autofill",
        title: "自动填充表单",
        detail: "允许填充表单信息，并在登录页提供已导入密码的账号选择。",
        kind: "toggle",
        setting: "autofill",
        keywords: "forms autofill",
      },
    ],
  },
  {
    pane: "privacy",
    title: "浏览数据",
    rows: [
      {
        id: "remember",
        title: "记录浏览历史",
        detail:
          "将访问过的网页保存在本机，用于地址栏搜索。关闭后保留已有历史。",
        kind: "toggle",
        setting: "rememberHistory",
        keywords: "remember browsing history",
      },
      {
        id: "history",
        title: "浏览历史",
        detail: "清除本机历史记录和已关闭标签页记录。",
        kind: "button",
        action: "清除…",
        keywords: "clear history",
      },
      {
        id: "cache",
        title: "缓存的图片和文件",
        detail: "释放网页缓存。再次访问时会重新下载内容。",
        kind: "button",
        action: "清除…",
        keywords: "clear cache",
      },
      {
        id: "siteData",
        title: "Cookie 和网站数据",
        detail: "清除全部网站的 Cookie 和本地存储，会退出网站登录。",
        kind: "button",
        action: "清除…",
        keywords: "cookies site data storage",
      },
    ],
  },
  {
    pane: "translation",
    title: "网页翻译",
    rows: [
      {
        id: "translation",
        title: "翻译网页",
        detail: "Rove 尚未接入网页翻译服务。",
        kind: "status",
        action: "暂未提供",
        keywords: "translation language 翻译 语言",
      },
    ],
  },
  {
    pane: "extensions",
    title: "浏览器扩展",
    rows: [
      {
        id: "extensions",
        title: "扩展",
        detail: "Rove 尚未提供扩展安装和管理功能。",
        kind: "status",
        action: "暂未提供",
        keywords: "extensions plugins 插件",
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
        matches(meta.title, meta.keywords, group.title) ||
        matches(row.title, row.detail, row.keywords || "", row.action || ""),
    );
    return rows.length ? [{ ...group, rows }] : [];
  }),
);
const visibleSpaces = computed(() =>
  b.state.spaces.filter(
    (space) => !searching.value || matches("工作区 spaces", space.name),
  ),
);
const visibleShortcuts = computed(() =>
  SHORTCUTS.filter(
    (item) =>
      !searching.value ||
      matches(
        "快捷键 shortcuts",
        item.group,
        item.title,
        shortcutText(shortcutFor(item.id, b.settings)),
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
const noResults = computed(
  () =>
    searching.value &&
    !visibleGroups.value.length &&
    !showSpaces.value &&
    !showShortcuts.value,
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
  } else if (row.setting) {
    update(
      row.setting,
      row.setting === "favoritesPerRow" ? Number(selected) : selected,
    );
    if (selected === "custom" && !custom.value)
      nextTick(() => document.getElementById(`custom-${props.tabId}`)?.focus());
  }
}
function saveInput(id: string) {
  if (id === "custom") {
    if (custom.value && !validSearchTemplate(custom.value.trim())) {
      notice.value = "搜索地址需要包含 %s，并使用有效的 HTTP 或 HTTPS 网址。";
      return;
    }
    update("customSearchTemplate", custom.value.trim());
  } else update("downloadFolder", folder.value.trim());
}
function action(row: Row) {
  if (row.id === "onboarding") { b.openOnboarding(); return; }
  if (row.id === "default") {
    if (!b.native) { notice.value = "请在 Rove 桌面版中设置默认浏览器。"; return; }
    b.send({ type: "systemSettings" });
    return;
  }
  pending.value = {
    kind: row.id,
    title: `清除${row.title}？`,
    detail: row.detail,
  };
}
function editSpace(space?: Space) {
  editing.value = space?.id || "new";
  spaceName.value = space?.name || "";
  spaceIcon.value = space?.icon;
  pickingIcon.value = false;
  spaceColor.value = space?.color || "sky";
  translucency.value = space?.translucency || 0;
}
function saveSpace() {
  if (!spaceName.value.trim()) {
    notice.value = "请输入工作区名称。";
    return;
  }
  b.send({
    type: editing.value === "new" ? "spaceCreate" : "spaceUpdate",
    id: editing.value,
    name: spaceName.value.trim(),
    color: spaceColor.value,
    translucency: translucency.value,
    icon: spaceIcon.value ?? null,
    background: true,
  });
  editing.value = undefined;
}
function dropSpace(event: DragEvent, before: string) {
  const id = event.dataTransfer?.getData("text/era-space");
  if (id) b.send({ type: "spaceReorder", id, before });
}
function confirm() {
  if (!pending.value) return;
  if (pending.value.kind === "space")
    b.send({ type: "spaceDelete", id: pending.value.id });
  else if (b.native) {
    b.dataStatus = "正在清除…";
    b.send({ type: "clearBrowsingData", kind: pending.value.kind });
  } else notice.value = "界面预览不保存真实浏览数据；请在 Windows 应用中清除。";
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
  <section class="settings-page" aria-label="设置">
    <div class="settings-layout" :inert="!!editing || !!pending">
      <aside class="settings-navigation" aria-label="设置分类">
        <div class="settings-brand">
          <span>Rove</span>
          <h1>设置</h1>
        </div>
        <nav>
          <button
            v-for="item in SETTINGS_PANES"
            :key="item.id"
            :title="item.title"
            :aria-label="item.title"
            :aria-current="pane === item.id && !searching ? 'page' : undefined"
            @click="pick(item.id)"
          >
            <Icon :name="item.icon" /><span>{{ item.title }}</span>
          </button>
        </nav>
      </aside>
      <div class="settings-main">
        <header class="settings-page-header">
          <h2>
            {{
              searching
                ? "搜索结果"
                : SETTINGS_PANES.find((p) => p.id === pane)?.title
            }}
          </h2>
          <label class="settings-search"
            ><Icon name="search" :size="14" /><input
              v-model="query"
              type="search"
              placeholder="查找设置"
              aria-label="查找设置"
              @keydown.esc.stop="query = ''"
          /></label>
        </header>
        <div ref="scroll" class="settings-scroll">
          <div class="settings-content">
            <p
              v-if="notice || b.dataStatus"
              class="settings-feedback"
              role="status"
            >
              {{ notice || b.dataStatus }}
            </p>
            <section
              v-for="group in visibleGroups"
              :key="`${group.pane}-${group.title}`"
              class="settings-group"
              :aria-label="group.title"
            >
              <h3>
                {{
                  searching
                    ? `${SETTINGS_PANES.find((p) => p.id === group.pane)?.title} · `
                    : ""
                }}{{ group.title }}
              </h3>
              <div v-for="row in group.rows" :key="row.id" class="setting-row">
                <div class="setting-description">
                  <label :for="`setting-${row.id}-${tabId}`">{{
                    row.title
                  }}</label>
                  <p>{{ row.detail }}</p>
                </div>
                <button
                  v-if="row.kind === 'toggle'"
                  :id="`setting-${row.id}-${tabId}`"
                  class="setting-toggle"
                  role="switch"
                  :aria-checked="!!value(row)"
                  :aria-label="row.title"
                  @click="update(row.setting!, !value(row))"
                >
                  <span />
                </button>
                <select
                  v-else-if="row.kind === 'select'"
                  :id="`setting-${row.id}-${tabId}`"
                  :aria-label="row.title"
                  :value="value(row)"
                  @change="select(row, $event)"
                >
                  <option
                    v-for="option in row.options"
                    :key="option[0]"
                    :value="option[0]"
                  >
                    {{ option[1] }}
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
                    aria-label="搜索地址"
                    placeholder="https://example.com/search?q=%s"
                    maxlength="2048"
                    @change="saveInput(row.id)"
                  /><input
                    v-else
                    v-model="folder"
                    aria-label="下载文件夹"
                    placeholder="系统下载文件夹"
                    maxlength="1024"
                    @change="saveInput(row.id)"
                  /><button type="submit" class="setting-button">保存</button>
                </form>
                <button
                  v-else-if="row.kind === 'button'"
                  :id="`setting-${row.id}-${tabId}`"
                  class="setting-button"
                  :disabled="row.id === 'default' && !b.native"
                  :aria-label="row.id === 'default' ? row.action : `${row.action}${row.title}`"
                  @click="action(row)"
                >
                  {{ row.action }}
                </button>
                <span v-else class="setting-status">{{ row.action }}</span>
              </div>
            </section>
            <section
              v-if="showSpaces"
              class="settings-group"
              aria-label="你的工作区"
            >
              <div class="settings-group-heading">
                <h3>你的工作区</h3>
                <button
                  class="setting-button"
                  :disabled="b.state.spaces.length >= 16"
                  @click="editSpace()"
                >
                  新建工作区
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
                <span class="space-color-tile" :data-color="space.color">
                  <SpaceGlyph v-if="space.icon" :icon="space.icon" :size="16" />
                  <Icon v-else name="spaces" :size="18" />
                </span>
                <div class="setting-description">
                  <strong>{{ space.name }}</strong>
                  <p>
                    {{
                      b.state.tabs.filter(
                        (t) => t.spaceId === space.id && !t.pinned,
                      ).length
                    }}
                    个标签页 ·
                    {{
                      b.state.pins?.filter((p) => p.spaceId === space.id)
                        .length || 0
                    }}
                    个固定项
                  </p>
                </div>
                <div class="setting-inline-actions">
                  <button
                    class="setting-button"
                    :aria-label="`编辑 ${space.name}`"
                    @click="editSpace(space)"
                  >
                    编辑</button
                  ><button
                    class="icon-button"
                    :aria-label="`删除 ${space.name}`"
                    :disabled="b.state.spaces.length <= 1"
                    @click="
                      pending = {
                        kind: 'space',
                        id: space.id,
                        title: `删除“${space.name}”？`,
                        detail:
                          '此工作区的标签页与固定项将被移除，共用的常用网站保留。此操作无法撤销。',
                      }
                    "
                  >
                    <Icon name="trash" />
                  </button>
                </div>
              </div>
              <p class="setting-hint">拖动工作区调整顺序。</p>
            </section>
            <section
              v-if="showShortcuts"
              class="settings-group"
              aria-label="快捷键列表"
            >
              <div class="settings-group-heading">
                <h3>快捷键</h3>
                <button
                  class="setting-button"
                  @click="
                    recording = '';
                    update('shortcuts', {});
                  "
                >
                  恢复默认
                </button>
              </div>
              <p class="setting-hint">
                点击按键开始录制，Esc 取消，Backspace 清除。Ctrl + 1–9
                用于切换工作区。
              </p>
              <div
                v-for="item in visibleShortcuts"
                :key="item.id"
                class="setting-row"
              >
                <div class="setting-description">
                  <strong>{{ item.title }}</strong>
                  <p>{{ item.group }}</p>
                </div>
                <button
                  class="setting-shortcut"
                  :class="{ recording: recording === item.id }"
                  :aria-label="`修改快捷键：${item.title}`"
                  @click="
                    notice = '';
                    recording = recording === item.id ? '' : item.id;
                  "
                >
                  <kbd>{{
                    recording === item.id
                      ? "请按下组合键…"
                      : shortcutText(shortcutFor(item.id, b.settings))
                  }}</kbd>
                </button>
              </div>
            </section>
            <p v-if="noResults" class="settings-empty">
              没有找到与“{{ query.trim() }}”匹配的设置。
            </p>
          </div>
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
        :aria-label="editing === 'new' ? '新建工作区' : '编辑工作区'"
      >
        <header>
          <h2>{{ editing === "new" ? "新建工作区" : "编辑工作区" }}</h2>
          <button
            class="icon-button"
            aria-label="关闭编辑"
            @click="editing = undefined"
          >
            <Icon name="close" />
          </button>
        </header>
        <form @submit.prevent="saveSpace">
          <SpaceIdentityEditor v-model:name="spaceName" v-model:icon="spaceIcon" v-model:picking="pickingIcon" />
          <div v-show="!pickingIcon" class="swatches" aria-label="工作区颜色">
            <button
              v-for="color in colors"
              :key="color.id"
              type="button"
              :data-color="color.id"
              :aria-label="color.title"
              :aria-pressed="spaceColor === color.id"
              :class="{ chosen: spaceColor === color.id }"
              @click="spaceColor = color.id"
            >
              <Icon v-if="spaceColor === color.id" name="check" />
            </button>
          </div>
          <label class="glass-control"
            >清透
            <input
              v-model.number="translucency"
              aria-label="清透"
              type="range"
              min="0"
              max="1"
              step="0.01"
          /></label>
          <p class="settings-note">清透背景带轻微模糊，界面和网页保持清晰。</p>
          <p v-if="notice" role="status">{{ notice }}</p>
          <footer>
            <button
              type="button"
              class="setting-button"
              @click="editing = undefined"
            >
              取消</button
            ><button class="primary-button">保存</button>
          </footer>
        </form>
      </section>
      <section
        v-else-if="pending"
        class="settings-space-editor"
        role="alertdialog"
        aria-modal="true"
        :aria-label="pending.title"
      >
        <h2>{{ pending.title }}</h2>
        <p>{{ pending.detail }}</p>
        <footer>
          <button class="setting-button" @click="pending = undefined">
            取消</button
          ><button class="primary-button" @click="confirm">
            {{ pending.kind === "space" ? "删除工作区" : "确认清除" }}
          </button>
        </footer>
      </section>
    </div>
  </section>
</template>
<style src="../../styles/settings.css"></style>
