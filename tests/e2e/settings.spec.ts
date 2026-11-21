import { chooseBrowserMenuItem } from "./browser-menu";
import { expect, test } from "@playwright/test";
import type { Command } from "../../src/lib/types";

test.beforeEach(async ({ page }) => {
  await page.route("**/favicon.ico", (route) => route.abort());
  await page.goto("/");
  await chooseBrowserMenuItem(page, "设置");
});

test("interface language updates live, works in settings search and survives a reload", async ({ page }) => {
  const html = page.locator("html");
  const search = page.getByRole("searchbox", { name: /查找设置|Search settings/ });
  const selector = page.getByRole("combobox", { name: /^(界面语言|Interface language)$/ });
  await expect(html).toHaveAttribute("lang", "zh-CN");
  await search.fill("界面语言");
  await expect(selector).toHaveValue("zh-CN");
  await expect(selector.locator("option")).toHaveText(["简体中文", "English"]);
  await selector.selectOption("en");
  await expect(html).toHaveAttribute("lang", "en");
  await expect(selector).toHaveValue("en");
  await expect(page.getByRole("tab", { name: "Settings", exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Privacy", exact: true })).toBeVisible();

  await search.fill("cached images");
  await expect(page.locator(".setting-description label mark")).toHaveText(["Cached", "images"]);
  await page.reload();
  await page.getByRole("button", { name: "Browser menu", exact: true }).click();
  await page.getByRole("menuitem", { name: "Settings", exact: true }).click();
  await expect(selector).toHaveValue("en");
  await page.setViewportSize({ width: 480, height: 640 });
  await expect(selector).toBeVisible();
  expect(await page.locator(".settings-scroll").evaluate((e) => e.scrollWidth <= e.clientWidth)).toBe(true);

  await selector.selectOption("zh-CN");
  await expect(html).toHaveAttribute("lang", "zh-CN");
  await expect(page.getByRole("heading", { name: "通用", exact: true })).toBeVisible();
});

test("settings are a reusable tab with searchable sections and live preferences", async ({
  page,
}) => {
  await expect(
    page.getByRole("tab", { name: "设置", exact: true }),
  ).toBeVisible();
  await expect(page.locator(".settings-page")).toBeVisible();
  await page
    .getByRole("combobox", { name: "搜索引擎", exact: true })
    .selectOption("baidu");
  await page
    .getByRole("switch", { name: "大号工作区名称", exact: true })
    .click();
  await expect(page.locator(".space-name")).toHaveCSS("font-size", "13px");
  await page
    .getByRole("combobox", { name: "每行常用网站", exact: true })
    .selectOption("2");
  expect(
    await page
      .locator(".favorites")
      .evaluate(
        (e) => getComputedStyle(e).gridTemplateColumns.split(" ").length,
      ),
  ).toBe(2);
  await page.getByRole("switch", { name: "减少动态效果", exact: true }).click();
  await expect(page.locator("html")).toHaveAttribute(
    "data-reduce-motion",
    "true",
  );
  await page
    .getByRole("combobox", { name: "外观", exact: true })
    .selectOption("dark");
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await page.getByRole("searchbox", { name: "查找设置" }).fill("自动填充");
  await expect(
    page.getByRole("switch", { name: "自动填充表单", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("combobox", { name: "搜索引擎", exact: true }),
  ).toHaveCount(0);
  await page.getByRole("searchbox", { name: "查找设置" }).fill("no-result-xyz");
  await expect(page.locator(".settings-empty")).toBeVisible();
  await page.getByRole("button", { name: "隐私", exact: true }).click();
  await expect(
    page.getByRole("tab", { name: "设置", exact: true }),
  ).toHaveAttribute("title", "rove://settings/privacy");
  await chooseBrowserMenuItem(page, "设置");
  await expect(
    page.getByRole("tab", { name: "设置", exact: true }),
  ).toHaveCount(1);
  await expect(
    page.getByRole("switch", { name: "记录浏览历史", exact: true }),
  ).toBeVisible();
  await page.screenshot({ path: "test-results/settings-dark-privacy.png" });
  await page.keyboard.press("Control+t");
  await page.getByRole("combobox").fill("独立搜索词");
  await page.keyboard.press("Enter");
  await expect(page.getByRole("tab", { selected: true })).toHaveAttribute(
    "title",
    /baidu.com\/s\?wd=/,
  );
});

test("the action toast follows the selected appearance and fades with reduced motion", async ({ page }) => {
  await page.getByRole("switch", { name: "减少动态效果", exact: true }).click();
  await page.evaluate(() => {
    document.addEventListener("transitionrun", (event) => {
      const target = event.target;
      if (!(target instanceof HTMLElement)) return;
      const toast = target.closest<HTMLElement>(".action-toast");
      if (!toast || (target !== toast && !target.matches(".toast-surface"))) return;
      const motion = `${event.propertyName} ${getComputedStyle(target).transitionDuration}`;
      toast.dataset.motion = `${toast.dataset.motion ?? ""} ${motion}`.trim();
    });
    window.__ERA_RECEIVE__?.({ type: "toast", message: "已复制链接" });
  });
  const toast = page.locator(".action-toast");
  const surface = toast.locator(".toast-surface");
  await expect(surface).toHaveCSS("background-image", /linear-gradient.*rgb\(255, 255, 255\)/);
  await expect(surface.locator("svg")).toBeVisible();
  await page.getByRole("combobox", { name: "外观", exact: true }).selectOption("dark");
  await expect(surface).toHaveCSS("background-image", /linear-gradient.*rgb\(52, 54, 60\)/);
  await expect(toast).toHaveAttribute("data-motion", "opacity 0.08s");
});

test("custom engine validates its address and shortcut recording changes actual keys", async ({
  page,
}) => {
  await page
    .getByRole("combobox", { name: "搜索引擎", exact: true })
    .selectOption("custom");
  const template = page.getByRole("textbox", { name: "搜索地址", exact: true });
  await template.fill("javascript:%s");
  await template.press("Enter");
  await expect(page.getByRole("status")).toContainText("需要包含 %s");
  await template.fill("https://example.com/search?q=%s");
  await template.press("Enter");
  await page.getByRole("button", { name: "快捷键", exact: true }).click();
  await page
    .getByRole("button", { name: "修改快捷键：新建标签页", exact: true })
    .click();
  await page.keyboard.press("Control+Shift+y");
  await expect(
    page.getByRole("button", { name: "修改快捷键：新建标签页", exact: true }),
  ).toContainText("Ctrl + Shift + Y");
  await expect(
    page.locator('.space-list[data-space-id="personal"] .new-tab kbd'),
  ).toHaveText("Ctrl + Shift + Y");
  await page.keyboard.press("Control+Shift+y");
  await expect(page.getByRole("combobox")).toBeFocused();
  await page.getByRole("combobox").fill("a & b");
  await page.keyboard.press("Enter");
  await expect(page.getByRole("tab", { selected: true })).toHaveAttribute(
    "title",
    "https://example.com/search?q=a%20%26%20b",
  );
  await page.keyboard.press("Control+Comma");
  await page
    .getByRole("button", { name: "修改快捷键：新建标签页", exact: true })
    .click();
  await page.keyboard.press("Control+l");
  await expect(page.getByRole("status")).toContainText("重复");
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "恢复默认", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "修改快捷键：新建标签页", exact: true }),
  ).toContainText("Ctrl + T");
  await expect(
    page.locator('.space-list[data-space-id="personal"] .new-tab kbd'),
  ).toHaveText("Ctrl + T");
});

test("settings search highlights titles, descriptions and result paths without changing controls", async ({ page }) => {
  const search = page.getByRole("searchbox", { name: "查找设置" });
  const group = page.getByRole("region", { name: "自动填充", exact: true });
  await search.fill("自动填充");
  await expect(group.locator("h3")).toHaveText(/密码\s*›\s*自动填充/);
  await expect(group.locator("h3 mark")).toHaveText("自动填充");
  await expect(group.locator("label mark")).toHaveText("自动填充");
  const toggle = group.getByRole("switch", { name: "自动填充表单", exact: true });
  await toggle.click();
  await expect(toggle).toHaveAttribute("aria-checked", "false");
  await expect(group.locator("label").filter({ hasText: "自动填充表单" })).toHaveText("自动填充表单");

  await search.fill("表单 登录");
  await expect(group.locator("label mark")).toHaveText("表单");
  await expect(group.locator(".setting-description p mark")).toHaveText(["表单", "登录"]);
  await search.fill("隐私");
  await expect(page.locator(".settings-result-pane mark")).toHaveText("隐私");
  await search.press("Escape");
  await expect(search).toHaveValue("");
  await expect(page.locator(".settings-page mark")).toHaveCount(0);
  await expect(page.getByRole("heading", { name: "通用", exact: true })).toBeVisible();
});

test("shortcut search highlights normalized key names and overlapping words", async ({ page }) => {
  const search = page.getByRole("searchbox", { name: "查找设置" });
  await search.fill("ＣＴＲＬ 标签");
  const group = page.getByRole("region", { name: "快捷键列表" });
  await expect(group.locator(".setting-row")).toHaveCount(6);
  await expect(group.locator("kbd mark")).toHaveText(Array(6).fill("Ctrl"));
  await expect(group.locator("strong mark")).toHaveText(Array(6).fill("标签"));
  await search.fill("自动 自动填充");
  await expect(page.locator(".setting-description label mark")).toHaveText("自动填充");
  await search.fill("[.*+?]");
  await expect(page.locator(".settings-empty")).toBeVisible();
  await expect(page.locator(".settings-page mark")).toHaveCount(0);
});

test("workspace search preserves Unicode and renders user text safely", async ({ page }) => {
  await page.getByRole("button", { name: "工作区", exact: true }).click();
  await page.getByRole("button", { name: "新建工作区", exact: true }).click();
  const name = "Café Ｒｏｖｅ <b>.*</b> 👩‍💻";
  const dialog = page.getByRole("dialog", { name: "新建工作区", exact: true });
  await dialog.getByRole("textbox", { name: "工作区名称", exact: true }).fill(name);
  await dialog.getByRole("button", { name: "保存", exact: true }).click();
  const search = page.getByRole("searchbox", { name: "查找设置" });
  const group = page.getByRole("region", { name: "你的工作区", exact: true });
  await search.fill("cafe rove 👩");
  await expect(group.locator("strong")).toHaveText(name);
  await expect(group.locator("strong mark")).toHaveText(["Café", "Ｒｏｖｅ", "👩‍💻"]);
  await search.fill("<b>.*</b>");
  await expect(group.locator("strong mark")).toHaveText("<b>.*</b>");
  await expect(group.locator("strong b")).toHaveCount(0);
  await expect(group.getByRole("button", { name: `编辑 ${name}`, exact: true })).toBeVisible();
  await page.getByRole("button", { name: "通用", exact: true }).click();
  await expect(search).toHaveValue("");
  await expect(page.locator(".settings-page mark")).toHaveCount(0);
});

test("search highlights adapt to light, dark and narrow settings panes", async ({ page }) => {
  const search = page.getByRole("searchbox", { name: "查找设置" });
  for (const theme of ["light", "dark"]) {
    await search.fill("");
    await page.getByRole("combobox", { name: "外观", exact: true }).selectOption(theme);
    await search.fill("自动填充");
    for (const width of [1280, 700, 480]) {
      await page.setViewportSize({ width, height: 640 });
      await expect(page.locator(".setting-description label mark")).toHaveText("自动填充");
      expect(await page.locator(".settings-scroll").evaluate((e) => e.scrollWidth <= e.clientWidth)).toBe(true);
      await page.screenshot({ path: `test-results/settings-search-${theme}-${width}.png` });
    }
  }
});

test("space editor creates and deletes a space in place with confirmation", async ({
  page,
}) => {
  await page.getByRole("button", { name: "工作区", exact: true }).click();
  await page.getByRole("button", { name: "新建工作区", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "新建工作区", exact: true });
  await dialog.getByRole("button", { name: "效果", exact: true }).click();
  await expect(dialog.getByRole("slider", { name: "清透", exact: true })).toBeVisible();
  await expect(dialog.getByText("磨砂", { exact: true })).toHaveCount(0);
  await dialog
    .getByRole("textbox", { name: "工作区名称", exact: true })
    .fill("研究");
  await dialog.getByRole("button", { name: "薄荷", exact: true }).click();
  await dialog.getByRole("button", { name: "保存", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "编辑 研究", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("tab", { name: "设置", exact: true }),
  ).toHaveAttribute("aria-selected", "true");
  await page.getByRole("button", { name: "删除 研究", exact: true }).click();
  await expect(page.getByRole("alertdialog")).toBeVisible();
  await page.getByRole("button", { name: "取消", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "编辑 研究", exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: "删除 研究", exact: true }).click();
  await page.getByRole("button", { name: "删除工作区", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "编辑 研究", exact: true }),
  ).toHaveCount(0);
});

test("sidebar Space editor uses the shared form and preserves the theme on rename", async ({ page }) => {
  await page.locator(".sidebar-header .space-name").click();
  const dialog = page.locator(".sidebar").getByRole("dialog", { name: "编辑空间", exact: true });
  await expect(dialog.getByRole("heading", { name: "编辑 Space", exact: true })).toBeVisible();
  await expect(page.locator(".page-area")).toHaveAttribute("inert", "");
  const field = dialog.getByRole("textbox", { name: "工作区名称", exact: true });
  await expect(field).toBeFocused();
  const themeButton = dialog.getByRole("button", { name: "主题…", exact: true });
  const theme = page.getByRole("region", { name: "Space 主题", exact: true });
  await themeButton.click();
  await theme.getByRole("button", { name: "薄荷", exact: true }).click();
  await theme.getByRole("button", { name: "效果", exact: true }).click();
  const clear = theme.getByRole("slider", { name: "清透", exact: true });
  await expect(clear).toBeVisible();
  await expect(theme.getByText("磨砂", { exact: true })).toHaveCount(0);
  await clear.fill("0.7");
  await page.keyboard.press("Escape");
  await expect(theme).toHaveCount(0);
  await expect(themeButton).toBeFocused();
  await dialog.getByRole("button", { name: "完成", exact: true }).click();
  await expect(page.locator(".page-area")).not.toHaveAttribute("inert");
  await chooseBrowserMenuItem(page, "工作区 Personal");
  await field.fill("研究空间");
  await themeButton.click();
  await expect(clear).toHaveValue("0.7");
  await page.keyboard.press("Escape");
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  await expect(page.locator(".sidebar-header .space-name")).toHaveText("研究空间");
});

test("about is a separate, searchable settings pane with both update channels", async ({ page }) => {
  const channel = page.getByRole("combobox", { name: "更新通道", exact: true });
  await expect(channel).toHaveCount(0);
  await page.getByRole("searchbox", { name: "查找设置" }).fill("更新通道");
  await page.getByRole("button", { name: /^关于 Rove.*更新通道/ }).click();
  await expect(page.getByRole("heading", { name: "关于 Rove" })).toBeVisible();
  await expect(page.getByRole("tab", { name: "设置", exact: true })).toHaveAttribute("title", "rove://settings/about");
  await expect(channel).toHaveValue("preview");
  await expect(page.locator(".settings-page")).toContainText("GitHub");
  await channel.selectOption("canary");
  await expect(page.locator(".settings-page")).toContainText("每次提交");
  await expect(page.locator(".settings-page")).toContainText("windows-installer");
  await page.getByRole("button", { name: "检查更新", exact: true }).click();
  await expect(page.locator(".settings-page")).toContainText("请在 Rove 桌面版中检查更新");
  await page.setViewportSize({ width: 480, height: 640 });
  expect(await page.locator(".settings-scroll").evaluate((e) => e.scrollWidth <= e.clientWidth)).toBe(true);
});

test("an available update waits for explicit confirmation and sends the displayed offer only once", async ({ page }) => {
  type UpdateTestWindow = Window & { updateCommands: Command[] };
  await page.addInitScript(() => {
    const w = window as UpdateTestWindow;
    w.__ERA_NATIVE__ = true;
    w.__ERA_INITIAL_STATE__ = {
      spaces: [{ id: "personal", name: "Personal", color: "sky" }],
      activeSpace: "personal", activeTab: "settings",
      tabs: [{ id: "settings", spaceId: "personal", title: "设置", url: "rove://settings/about", pinned: false }],
      favorites: [], pins: [], theme: "dark", sidebarCollapsed: false, closed: [],
    };
    w.updateCommands = [];
    w.ipc = { postMessage(text) { w.updateCommands.push(JSON.parse(text)); } };
  });
  await page.reload();
  const offerId = "6c5f6e5b-3602-4dd7-bf84-e06d2a887a6c";
  const commands = () => page.evaluate(() => (window as UpdateTestWindow).updateCommands.filter((c) => c.type.startsWith("update")));
  const status = (phase: string, message: string) => page.evaluate(
    (event) => window.__ERA_RECEIVE__?.(event),
    { type: "updateStatus", phase, channel: "preview", message, version: "1.2.3", offerId },
  );
  const check = page.getByRole("button", { name: "检查更新", exact: true });
  const channel = page.getByRole("combobox", { name: "更新通道" });
  await check.click();
  await status("checking", "正在检查更新…");
  await expect(check).toBeDisabled();
  await expect(channel).toBeDisabled();
  await status("available", "发现新版本 1.2.3。");
  await expect(page.getByRole("status")).toHaveText("发现新版本 1.2.3。");
  expect(await commands()).toEqual([{ type: "updateCheck" }]);

  const update = page.getByRole("button", { name: "更新并重启…", exact: true });
  const dialog = page.getByRole("alertdialog", { name: "更新并重启 Rove？" });
  const cancel = dialog.getByRole("button", { name: "取消" });
  await update.click();
  await expect(dialog).toContainText("1.2.3");
  await expect(cancel).toBeFocused();
  await cancel.click();
  await expect(dialog).toHaveCount(0);
  await expect(update).toBeFocused();
  expect(await commands()).toEqual([{ type: "updateCheck" }]);

  await update.click();
  await dialog.getByRole("button", { name: "确认更新并重启" }).click();
  expect(await commands()).toEqual([{ type: "updateCheck" }, { type: "updateInstall", offerId }]);
  await status("downloading", "正在下载并校验更新包。");
  await expect(page.getByRole("status")).toHaveText("正在下载并校验更新包。");
  await expect(update).toHaveCount(0);
  await status("error", "更新包校验失败，已取消安装。");
  await expect(page.getByRole("status")).toHaveText("更新包校验失败，已取消安装。");
  await expect(check).toBeEnabled();
  await expect(channel).toBeEnabled();
});

test("settings adapt to narrow panes without horizontal overflow", async ({
  page,
}) => {
  for (const width of [1280, 900, 700, 480]) {
    await page.setViewportSize({ width, height: 640 });
    await page.getByRole("button", { name: "通用", exact: true }).click();
    await expect(
      page.getByRole("combobox", { name: "搜索引擎", exact: true }),
    ).toBeVisible();
    expect(
      await page
        .locator(".settings-page")
        .evaluate((e) => e.scrollWidth <= e.clientWidth),
    ).toBe(true);
    expect(
      await page
        .locator(".settings-scroll")
        .evaluate((e) => e.scrollWidth <= e.clientWidth),
    ).toBe(true);
  }
  await page.screenshot({ path: "test-results/settings-narrow.png" });
});
