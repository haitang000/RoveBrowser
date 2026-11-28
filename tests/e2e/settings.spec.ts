import { chooseBrowserMenuItem } from "./browser-menu";
import { expect, test } from "@playwright/test";
import type { Settings } from "../../src/lib/settings";
import type { Command } from "../../src/lib/types";

test.beforeEach(async ({ page }) => {
  await page.route("**/favicon.ico", (route) => route.abort());
  await page.goto("/");
  await chooseBrowserMenuItem(page, "设置");
});

test("search engine dropdown opens in the page and updates its label and icon", async ({ page }) => {
  const engine = page.getByRole("combobox", { name: "搜索引擎", exact: true });
  const openEngine = engine.and(page.locator("select:open"));
  const selectedContent = engine.locator("selectedcontent");

  await engine.click();
  await expect(openEngine).toBeVisible();
  await expect(openEngine.locator("option:checked")).toHaveText("Google");
  const bing = openEngine.locator('option[value="bing"]');
  const bingIcon = await bing.locator("img").getAttribute("src");
  await bing.click();
  await expect(openEngine).toHaveCount(0);
  await expect(engine).toHaveValue("bing");
  await expect(selectedContent).toHaveText("Bing");
  await expect(selectedContent.locator("img")).toHaveAttribute("src", bingIcon!);

  await engine.press("Space");
  await expect(openEngine).toBeVisible();
  await page.keyboard.press("ArrowUp");
  await page.keyboard.press("Escape");
  await expect(openEngine).toHaveCount(0);
  await expect(engine).toBeFocused();
  await expect(engine).toHaveValue("bing");

  await engine.click();
  await openEngine.locator('option[value="custom"]').click();
  await expect(page.getByRole("textbox", { name: "搜索地址", exact: true })).toBeFocused();
});

test("default page zoom is selectable and searchable in accessibility settings", async ({ page }) => {
  const zoom = page.getByRole("combobox", { name: "默认页面缩放", exact: true });
  await expect(zoom).toHaveCount(0);
  await page.getByRole("button", { name: "无障碍", exact: true }).click();
  await expect(zoom).toHaveValue("1");
  await zoom.selectOption({ label: "125%" });
  await page.getByRole("button", { name: "通用", exact: true }).click();
  await expect(zoom).toHaveCount(0);
  await page.getByRole("searchbox", { name: "查找设置" }).fill("默认页面缩放");
  await expect(zoom).toHaveValue("1.25");
});

test("privacy settings expose selectable defaults and a confirmation for selected data", async ({ page }) => {
  const privacy = page.getByRole("button", { name: "隐私", exact: true });
  await privacy.click();
  const tracking = page.getByRole("combobox", { name: "跟踪防护", exact: true });
  await expect(tracking).toHaveValue("balanced");
  await tracking.selectOption("strict");
  await expect(tracking).toHaveValue("strict");
  await expect(tracking).toHaveAccessibleDescription(/请刷新网页/);
  const permissions = page.getByRole("region", { name: "网站权限" });
  const camera = permissions.getByRole("button", { name: "摄像头" });
  const more = permissions.getByRole("button", { name: "更多权限" });
  await expect(permissions.getByRole("listitem")).toHaveCount(4);
  await expect(more).toHaveAttribute("aria-expanded", "false");
  await more.click();
  await expect(permissions.getByRole("button", { name: "屏幕共享" })).toBeVisible();
  await camera.click();
  const details = page.getByRole("region", { name: "摄像头", exact: true });
  await expect(details.getByRole("heading", { name: "摄像头" })).toBeFocused();
  await expect(page.getByRole("tab", { name: "设置", exact: true })).toHaveAttribute("title", "rove://settings/privacy/permissions/camera");
  await expect(details.getByRole("radio", { name: "网站可以请求使用摄像头" })).toBeChecked();
  await details.getByRole("radio", { name: "阻止网站使用摄像头" }).check();
  const blockedSites = details.getByRole("group", { name: "阻止的网站" });
  const allowedSites = details.getByRole("group", { name: "允许的网站" });
  await blockedSites.getByRole("button", { name: "添加" }).click();
  const add = page.getByRole("dialog", { name: "添加阻止的网站" });
  const site = add.getByRole("textbox", { name: "网站地址" });
  await expect(site).toBeFocused();
  await site.fill("file:///example.com");
  await add.getByRole("button", { name: "添加" }).click();
  await expect(add.getByRole("alert")).toBeVisible();
  await site.fill("example.com");
  await site.press("Enter");
  const rule = details.getByRole("combobox", { name: "更改 https://example.com 的权限" });
  await expect(blockedSites.getByRole("combobox")).toHaveValue("block");
  const filter = details.getByRole("searchbox", { name: "筛选网站" });
  await filter.fill("another.site");
  await expect(rule).toHaveCount(0);
  await expect(blockedSites.getByText("没有匹配的网站。")).toBeVisible();
  await filter.fill("EXAMPLE");
  await expect(rule).toBeVisible();
  await rule.selectOption("allow");
  await expect(allowedSites.getByRole("combobox")).toHaveValue("allow");
  await page.setViewportSize({ width: 360, height: 640 });
  await expect(page.locator(".settings-scroll")).toHaveJSProperty("scrollLeft", 0);
  expect(await page.locator(".settings-scroll").evaluate((e) => e.scrollWidth <= e.clientWidth)).toBe(true);
  await page.setViewportSize({ width: 1280, height: 820 });
  await rule.selectOption("default");
  await expect(rule).toHaveCount(0);
  await details.getByRole("button", { name: "返回网站权限" }).click();
  await expect(camera).toBeFocused();
  await expect(camera).toHaveAccessibleDescription("阻止网站使用摄像头");
  await expect(permissions.getByRole("button", { name: "麦克风" })).toHaveAccessibleDescription("网站可以请求使用麦克风");

  const clear = page.getByRole("button", { name: "清除浏览数据" });
  const dialog = page.getByRole("dialog", { name: "清除浏览数据" });
  await clear.click();
  const confirm = dialog.getByRole("button", { name: "确认清除" });
  const checkboxes = dialog.getByRole("checkbox");
  await expect(checkboxes).toHaveCount(4);
  for (const checkbox of await checkboxes.all()) await expect(checkbox).not.toBeChecked();
  await expect(confirm).toBeDisabled();
  const cookies = dialog.getByRole("checkbox", { name: "Cookie 和网站数据" });
  await cookies.check();
  await expect(confirm).toBeEnabled();
  await expect(dialog.getByRole("status")).toContainText("退出网站登录");
  await dialog.getByRole("button", { name: "取消" }).click();
  await expect(clear).toBeFocused();
  await clear.click();
  await expect(cookies).not.toBeChecked();
  await dialog.press("Escape");
  await expect(dialog).toHaveCount(0);

  await page.getByRole("button", { name: "转到搜索建议" }).click();
  await expect(page.getByRole("heading", { name: "通用", exact: true })).toBeVisible();
  await expect(page.getByRole("switch", { name: "搜索建议", exact: true })).toBeFocused();
  await page.getByRole("combobox", { name: "界面语言", exact: true }).selectOption("zh-TW");
  await page.getByRole("button", { name: "隱私", exact: true }).click();
  await expect(page.getByRole("combobox", { name: "追蹤防護", exact: true })).toHaveValue("strict");
  await page.getByRole("searchbox", { name: "尋找設定" }).fill("攝影機");
  await page.getByRole("region", { name: "網站權限" }).getByRole("button", { name: "攝影機" }).click();
  await expect(page.getByRole("radio", { name: "封鎖網站使用攝影機" })).toBeChecked();
});

test("DevTools MCP has a dedicated experimental setting and appears in search", async ({ page }) => {
  const toggle = page.getByRole("switch", { name: "Chrome DevTools MCP", exact: true });
  await expect(toggle).toHaveCount(0);
  await page.getByRole("button", { name: "实验性", exact: true }).click();
  await expect(page.getByRole("heading", { name: "实验性", exact: true })).toBeVisible();
  await expect(toggle).not.toBeChecked();
  await expect(toggle).toHaveAccessibleDescription(/更改后重启 Rove 生效/);
  await toggle.click();
  await expect(toggle).toBeChecked();

  await page.getByRole("button", { name: "通用", exact: true }).click();
  await expect(toggle).toHaveCount(0);
  await page.getByRole("searchbox", { name: "查找设置" }).fill("DevTools MCP");
  await expect(toggle).toBeChecked();
  await toggle.click();
  await expect(toggle).not.toBeChecked();
});

test("desktop settings offer a restart until all startup preferences are restored", async ({ page }) => {
  type RestartTestWindow = Window & { restartCommands: Command[] };
  await page.addInitScript(() => {
    const w = window as RestartTestWindow;
    w.__ERA_NATIVE__ = true;
    w.__ERA_INITIAL_STATE__ = {
      spaces: [{ id: "personal", name: "Personal", color: "sky" }],
      activeSpace: "personal", activeTab: "settings",
      tabs: [{ id: "settings", spaceId: "personal", title: "设置", url: "rove://settings/privacy", pinned: false }],
      favorites: [], pins: [], theme: "dark", sidebarCollapsed: false, closed: [],
    };
    const initial = w.__ERA_INITIAL_STATE__;
    w.restartCommands = [];
    w.ipc = { postMessage(text) {
      const command: Command = JSON.parse(text);
      if (command.type === "settingsUpdate") {
        w.__ERA_RECEIVE__?.({ type: "state", state: { ...initial, settings: command.settings as Settings } });
      } else if (command.type === "restart") {
        w.restartCommands.push(command);
      }
    } };
  });
  await page.reload();

  const header = page.locator(".settings-page-header");
  const restart = header.getByRole("button", { name: "立即重启", exact: true });
  const dns = page.getByRole("switch", { name: "安全 DNS", exact: true });
  const rtc = page.getByRole("switch", { name: "WebRTC 地址保护", exact: true });
  await page.getByRole("combobox", { name: "跟踪防护", exact: true }).selectOption("strict");
  await expect(restart).toHaveCount(0);
  await dns.click();
  await expect(header.getByRole("status")).toHaveText("重启 Rove 以应用设置更改。");
  await rtc.click();
  await dns.click();
  await expect(restart).toBeVisible();
  await rtc.click();
  await expect(restart).toHaveCount(0);

  await header.getByRole("searchbox").fill("DevTools MCP");
  await page.getByRole("switch", { name: "Chrome DevTools MCP", exact: true }).click();
  await expect(restart).toBeVisible();
  await restart.click();
  await expect.poll(() => page.evaluate(() => (window as RestartTestWindow).restartCommands)).toEqual([{ type: "restart" }]);
});

test("read aloud starts off and the accessibility switch follows the interface language", async ({ page }) => {
  const search = page.getByRole("searchbox", { name: "查找设置" });
  await search.fill("文本朗读");
  const readAloud = page.getByRole("switch", { name: "文本朗读", exact: true });
  await expect(readAloud).not.toBeChecked();
  await readAloud.press("Space");
  await expect(readAloud).toBeChecked();

  await search.fill("界面语言");
  await page.getByRole("combobox", { name: "界面语言" }).selectOption("en");
  await page.getByRole("button", { name: "Accessibility", exact: true }).click();
  const english = page.getByRole("switch", { name: "Read aloud", exact: true });
  await expect(english).toBeChecked();
  await english.click();
  await expect(english).not.toBeChecked();
});

test("interface language updates live, works in settings search and survives a reload", async ({ page }) => {
  const html = page.locator("html");
  const search = page.getByRole("searchbox", { name: /查找设置|Search settings/ });
  const selector = page.getByRole("combobox", { name: /^(界面语言|介面語言|Interface language)$/ });
  await expect(html).toHaveAttribute("lang", "zh-CN");
  await search.fill("界面语言");
  await expect(selector).toHaveValue("zh-CN");
  await expect(selector.locator("option")).toHaveText(["简体中文", "繁體中文", "English"]);
  await selector.selectOption("en");
  await expect(html).toHaveAttribute("lang", "en");
  await expect(selector).toHaveValue("en");
  await expect(page.getByRole("tab", { name: "Settings", exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Privacy", exact: true })).toBeVisible();

  await search.fill("cached images");
  const clear = page.getByRole("region", { name: "Browsing data", exact: true });
  await expect(clear.locator(".setting-description p mark")).toHaveText(["cached", "images"]);
  await expect(clear.getByRole("button", { name: "Clear browsing data", exact: true })).toBeVisible();
  await page.reload();
  await page.getByRole("button", { name: "Browser menu", exact: true }).click();
  await page.getByRole("menuitem", { name: "Settings", exact: true }).click();
  await expect(selector).toHaveValue("en");
  await page.setViewportSize({ width: 480, height: 640 });
  await expect(selector).toBeVisible();
  expect(await page.locator(".settings-scroll").evaluate((e) => e.scrollWidth <= e.clientWidth)).toBe(true);

  await selector.selectOption("zh-TW");
  await expect(html).toHaveAttribute("lang", "zh-TW");
  await expect(selector).toHaveValue("zh-TW");
  await expect(page.getByRole("tab", { name: "設定", exact: true })).toBeVisible();
  await expect(page.getByRole("heading", { name: "一般", exact: true })).toBeVisible();

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
  await page.getByRole("button", { name: "外观", exact: true }).click();
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
  await expect(page.getByRole("switch", { name: "减少动态效果", exact: true })).toHaveCount(0);
  await page.getByRole("button", { name: "无障碍", exact: true }).click();
  await expect(page.getByRole("heading", { name: "无障碍", exact: true })).toBeVisible();
  await page.getByRole("switch", { name: "减少动态效果", exact: true }).click();
  await expect(page.locator("html")).toHaveAttribute(
    "data-reduce-motion",
    "true",
  );
  await page.getByRole("button", { name: "外观", exact: true }).click();
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
  await page.getByRole("combobox", { name: "搜索或输入地址", exact: true }).fill("独立搜索词");
  await page.keyboard.press("Enter");
  await expect(page.getByRole("tab", { selected: true })).toHaveAttribute(
    "title",
    /baidu.com\/s\?wd=/,
  );
});

test("sidebar density updates row and tile spacing and survives a reload", async ({ page }) => {
  const density = page.getByRole("combobox", { name: "侧边栏紧凑程度", exact: true });
  const space = page.locator('.space-list[data-space-id="personal"]');
  const tab = space.locator(".tab-row").first();
  const newTab = space.locator(".new-tab");
  const favorites = page.locator(".sidebar .favorites");
  await expect(density).toHaveCount(0);
  await page.getByRole("button", { name: "外观", exact: true }).click();
  await expect(page.getByRole("tab", { name: "设置", exact: true })).toHaveAttribute("title", "rove://settings/appearance");
  await expect(density).toHaveValue("standard");
  await expect(tab).toHaveCSS("height", "30px");
  await expect(favorites).toHaveCSS("gap", "6px");

  await density.selectOption("compact");
  await expect(tab).toHaveCSS("height", "26px");
  await expect(tab).toHaveCSS("margin-bottom", "1px");
  await expect(newTab).toHaveCSS("padding", "2px 3px");
  await expect(favorites).toHaveCSS("gap", "3px");

  const search = page.getByRole("searchbox", { name: "查找设置" });
  await search.fill("sidebar density");
  await expect(page.getByRole("region", { name: "侧边栏设置", exact: true }).locator("h3")).toHaveText(/外观\s*›\s*侧边栏设置/);
  await density.selectOption("comfortable");
  await expect(tab).toHaveCSS("height", "34px");
  await expect(tab).toHaveCSS("margin-bottom", "4px");
  await expect(newTab).toHaveCSS("padding", "6px 8px");
  await expect(favorites).toHaveCSS("gap", "9px");

  await page.reload();
  await expect(newTab).toHaveCSS("padding", "6px 8px");
  await chooseBrowserMenuItem(page, "设置");
  await page.getByRole("button", { name: "外观", exact: true }).click();
  await expect(density).toHaveValue("comfortable");
});

test("the action toast follows the selected appearance and fades with reduced motion", async ({ page }) => {
  await page.getByRole("button", { name: "无障碍", exact: true }).click();
  await page.getByRole("switch", { name: "减少动态效果", exact: true }).click();
  await page.getByRole("button", { name: "外观", exact: true }).click();
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
  await expect(toast).toHaveAttribute("data-motion", "opacity 0.08s");
  await page.getByRole("combobox", { name: "外观", exact: true }).selectOption("dark");
  await expect(surface).toHaveCSS("background-image", /linear-gradient.*rgb\(52, 54, 60\)/);
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
  await expect(page.locator(".settings-result-pane mark")).toHaveText(["隐私", "隐私", "隐私"]);
  await search.fill("DNS over HTTPS");
  const secureDns = page.getByRole("switch", { name: "安全 DNS", exact: true });
  await expect(secureDns).toHaveAccessibleDescription(/使用系统 DNS.*重启 Rove 生效/);
  await expect(secureDns).toHaveAttribute("aria-checked", "true");
  await secureDns.press("Space");
  await expect(secureDns).toHaveAttribute("aria-checked", "false");
  await search.fill("WebRTC");
  const protection = page.getByRole("switch", { name: "WebRTC 地址保护", exact: true });
  await expect(protection).toHaveAccessibleDescription(/重启 Rove 生效/);
  await expect(protection).toHaveAttribute("aria-checked", "false");
  await protection.press("Space");
  await expect(protection).toHaveAttribute("aria-checked", "true");
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
  await page.getByRole("button", { name: "外观", exact: true }).click();
  for (const theme of ["light", "dark"]) {
    await search.fill("");
    await page.getByRole("combobox", { name: "外观", exact: true }).selectOption(theme);
    await search.fill("自动填充");
    for (const width of [1280, 700, 480]) {
      await page.setViewportSize({ width, height: 640 });
      await expect(page.locator(".setting-description label mark")).toHaveText("自动填充");
      await expect.poll(() => page.locator(".settings-scroll").evaluate((e) => e.scrollWidth <= e.clientWidth)).toBe(true);
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
  await status("downloading", "正在下载并校验更新包（42%）。");
  await expect(page.getByRole("status")).toHaveText("正在下载并校验更新包（42%）。");
  await expect(update).toHaveCount(0);
  await status("error", "更新包校验失败，已取消安装。");
  await expect(page.getByRole("status")).toHaveText("更新包校验失败，已取消安装。");
  await expect(check).toBeEnabled();
  await expect(channel).toBeEnabled();
});

test("settings keep search aligned and visible while the content scrolls", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const nav = page.getByRole("navigation", { name: "设置分类" });
  await expect(nav.getByRole("button", { name: "通用" })).toHaveAttribute("aria-current", "page");
  const field = page.getByRole("searchbox", { name: "查找设置" });
  const group = page.getByRole("region", { name: "搜索", exact: true });
  await expect(group).toBeVisible();
  await expect.poll(() => group.evaluate((group) => {
    const search = group.closest(".settings-main")!.querySelector(".settings-search")!;
    const searchBox = search.getBoundingClientRect();
    const groupBox = group.getBoundingClientRect();
    return Math.max(Math.abs(searchBox.x - groupBox.x), Math.abs(searchBox.width - groupBox.width));
  })).toBeLessThan(1);
  const scroll = page.locator(".settings-scroll");
  await scroll.evaluate((el) => { el.scrollTop = el.scrollHeight; });
  await expect.poll(() => scroll.evaluate((el) => el.scrollTop)).toBeGreaterThan(0);
  await expect(field).toBeInViewport();
  await field.fill("自动填充");
  await expect(page.getByRole("heading", { name: "搜索结果" })).toBeVisible();
  await expect(scroll).toHaveJSProperty("scrollTop", 0);
});

test("settings adapt to narrow panes without horizontal overflow", async ({
  page,
}) => {
  for (const width of [1280, 900, 700, 360, 480]) {
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
