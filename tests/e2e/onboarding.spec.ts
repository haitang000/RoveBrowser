import { expect, test } from "@playwright/test";

async function reachSites(page: import("@playwright/test").Page) {
  await page.getByRole("button", { name: "开始使用", exact: true }).click();
  await page.getByRole("button", { name: "跳过", exact: true }).click();
  await expect(page.getByRole("region", { name: "编辑空间" })).toBeVisible();
  await page.getByRole("button", { name: "继续", exact: true }).click();
  await expect(page.getByRole("radiogroup", { name: "默认搜索引擎" })).toBeVisible();
  await page.getByRole("button", { name: "继续", exact: true }).click();
}

test("native import picker selects data, blocks busy navigation, and retries failures", async ({ page }) => {
  await page.addInitScript(() => {
    const w = window as any;
    w.__ERA_NATIVE__ = true;
    w.__ERA_INITIAL_STATE__ = {
      spaces: [{ id: "personal", name: "Personal", color: "sky" }], activeSpace: "personal", activeTab: "start",
      tabs: [{ id: "start", spaceId: "personal", title: "New Tab", url: "era://newtab", pinned: false }],
      favorites: [], pins: [], theme: "light", sidebarCollapsed: false, borderless: true, closed: [],
      onboarding: { step: "bring", completed: false, imports: [] },
    };
    w.__importCommands = [];
    w.ipc = { postMessage(text: string) {
      const c = JSON.parse(text); w.__importCommands.push(c);
      if (c.type === "importScan") setTimeout(() => w.__ERA_RECEIVE__({ type: "importState", importBusy: false, importSources: [
        { id: "edge", browser: "Microsoft Edge", name: "Microsoft Edge · Work", bookmarks: true, history: true },
        { id: "firefox", browser: "Firefox", name: "Firefox · Personal", bookmarks: true, history: true },
      ] }), 0);
      if (c.type === "importRun" || c.type === "importPasswords") w.__ERA_RECEIVE__({ type: "importState", importBusy: true });
    } };
  });
  await page.goto("/");
  await page.getByRole("checkbox", { name: /Firefox · Personal/ }).check();
  await page.getByRole("checkbox", { name: "历史记录", exact: true }).uncheck();
  await page.getByRole("button", { name: "导入所选数据", exact: true }).click();
  await expect(page.getByRole("button", { name: "跳过", exact: true })).toBeDisabled();
  await expect(page.getByRole("button", { name: "返回", exact: true })).toBeDisabled();
  expect(await page.evaluate(() => (window as any).__importCommands.filter((c: any) => c.type === "importRun").at(-1).request)).toEqual({ sources: ["edge", "firefox"], bookmarks: true, history: false });
  await page.evaluate(() => {
    const w = window as any;
    const state = structuredClone(w.__ERA_INITIAL_STATE__);
    state.onboarding.imports = [{ id: "edge", name: "Edge", bookmarks: 3, history: 0, passwords: 0, skipped: 1, problems: ["历史数据库繁忙，请关闭原浏览器后重试"] }];
    w.__ERA_RECEIVE__({ type: "state", state });
    w.__ERA_RECEIVE__({ type: "importState", importBusy: false });
  });
  await expect(page.getByText(/已导入 3 个书签/)).toBeVisible();
  await expect(page.getByRole("button", { name: "继续", exact: true })).toBeEnabled();
  await page.setViewportSize({ width: 480, height: 400 });
  await expect(page.locator(".onboarding")).toHaveJSProperty("scrollWidth", 480);
  await page.getByRole("button", { name: "重试这个来源", exact: true }).click();
  expect(await page.evaluate(() => (window as any).__importCommands.filter((c: any) => c.type === "importRun").at(-1).request.sources)).toEqual(["edge"]);
  await page.evaluate(() => (window as any).__ERA_RECEIVE__({ type: "importState", importBusy: false }));
  await page.getByRole("button", { name: "继续", exact: true }).scrollIntoViewIfNeeded();
  const footer = await page.locator(".onboarding-footer").boundingBox();
  const card = await page.locator(".onboarding-bring").boundingBox();
  expect(footer!.y).toBeGreaterThanOrEqual(card!.y + card!.height);
  await page.screenshot({ path: "test-results/import-small.png", animations: "disabled" });
});

test("space editor and search choices persist through reload and completion", async ({ page }) => {
  await page.goto("/?onboarding");
  await expect(page.getByRole("heading", { name: "Rove", exact: true })).toBeVisible();
  await page.screenshot({ path: "test-results/onboarding-hello.png", animations: "disabled" });
  await expect(page.locator(".sidebar")).toHaveCount(0);
  await page.keyboard.press("Control+t");
  await expect(page.getByRole("combobox")).toHaveCount(0);
  await page.keyboard.press("Enter");
  await expect(page.getByText("浏览器预览无法读取本机数据。请在 Rove 桌面版中导入。")).toBeVisible();
  await page.locator(".import-options summary").click();
  await expect(page.getByRole("button", { name: "导入密码文件…" })).toBeDisabled();
  await page.screenshot({ path: "test-results/onboarding-bring.png", animations: "disabled" });
  await page.getByRole("button", { name: "跳过", exact: true }).click();
  const editor = page.getByRole("region", { name: "编辑空间" });
  await editor.getByRole("textbox", { name: "工作区名称" }).fill("我的空间");
  await editor.getByRole("button", { name: "选择图标", exact: true }).click();
  await editor.getByRole("button", { name: "无图标", exact: true }).click();
  const swatch = editor.locator(".look-swatch").nth(3);
  await swatch.click();
  await expect(swatch).toHaveAttribute("aria-pressed", "true");
  await page.screenshot({ path: "test-results/onboarding-spaces.png", animations: "disabled" });
  await page.reload();
  await expect(editor.getByRole("textbox", { name: "工作区名称" })).toHaveValue("我的空间");
  await expect(editor.locator(".look-swatch").nth(3)).toHaveAttribute("aria-pressed", "true");
  await page.getByRole("button", { name: "继续", exact: true }).click();
  await page.getByRole("radio", { name: "Bing", exact: true }).check();
  await page.reload();
  await expect(page.getByRole("radio", { name: "Bing", exact: true })).toBeChecked();
  await page.screenshot({ path: "test-results/onboarding-search.png", animations: "disabled" });
  await page.getByRole("button", { name: "继续", exact: true }).click();
  const sites = page.getByRole("group", { name: "选择常用网站" });
  for (const name of ["Gmail", "Calendar", "YouTube", "Drive", "Notion", "ChatGPT", "Claude", "Spotify", "GitHub"])
    await sites.getByRole("button", { name, exact: true }).click();
  await expect(page.getByText("最多置顶 8 个网站，请先取消一个已选网站。")).toBeVisible();
  await expect(sites.getByRole("button", { pressed: true })).toHaveCount(8);
  await sites.getByRole("button", { name: "Calendar", exact: true }).click();
  await sites.getByRole("button", { name: "GitHub", exact: true }).click();
  await page.screenshot({ path: "test-results/onboarding-sites.png", animations: "disabled" });
  await page.getByRole("button", { name: "继续", exact: true }).click();
  await expect(page.getByRole("radiogroup", { name: "默认浏览器偏好" })).toBeVisible();
  await page.reload();
  await expect(page.getByRole("radiogroup", { name: "默认浏览器偏好" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(sites.getByRole("button", { pressed: true })).toHaveCount(8);
  await expect(sites.getByRole("button", { name: "GitHub", exact: true })).toHaveAttribute("aria-pressed", "true");
  await page.getByRole("button", { name: "继续", exact: true }).click();
  await expect(page.getByRole("button", { name: "暂时跳过", exact: true })).toHaveCount(0);
  await page.getByRole("radio", { name: "暂时不用", exact: true }).check();
  await page.screenshot({ path: "test-results/onboarding-ready.png", animations: "disabled" });
  await page.getByRole("button", { name: "开始浏览", exact: true }).click();
  await expect(page.locator(".sidebar")).toBeVisible();
  await expect(page.locator(".favorites .favorite-tile")).toHaveCount(8);
  await page.reload();
  await expect(page.locator(".onboarding")).toHaveCount(0);
  await page.getByRole("button", { name: "设置", exact: true }).click();
  await expect(page.getByRole("combobox", { name: "搜索引擎", exact: true })).toHaveValue("bing");
  // Embedded behavior must not change the same editor's normal popover mode.
  await page.locator(".sidebar-header .space-name").click();
  await expect(page.getByRole("dialog", { name: "编辑空间" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog", { name: "编辑空间" })).toHaveCount(0);
});

test("skip discards uncommitted picks and small windows keep actions reachable", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.setViewportSize({ width: 480, height: 400 });
  await page.goto("/?onboarding");
  await reachSites(page);
  await page.getByRole("button", { name: "GitHub", exact: true }).click();
  await page.getByRole("button", { name: "暂时跳过", exact: true }).click();
  await page.getByRole("button", { name: "返回", exact: true }).click();
  await expect(page.getByRole("button", { name: "GitHub", exact: true })).toHaveAttribute("aria-pressed", "false");
  await page.screenshot({ path: "test-results/onboarding-small.png", animations: "disabled" });
  await expect(page.locator(".onboarding")).toHaveJSProperty("scrollWidth", 480);
  await page.getByRole("button", { name: "继续", exact: true }).click();
  await page.getByRole("radio", { name: "暂时不用", exact: true }).check();
  await page.getByRole("button", { name: "开始浏览", exact: true }).click();
  await expect(page.locator(".favorites .favorite-tile")).toHaveCount(0);
});

test("existing sessions reopen the welcome without losing tabs or custom search", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "设置", exact: true }).click();
  await page.getByRole("combobox", { name: "外观", exact: true }).selectOption("dark");
  await page.getByRole("combobox", { name: "搜索引擎", exact: true }).selectOption("baidu");
  const originalTabs = await page.getByRole("tab").count();
  await page.getByRole("button", { name: "打开欢迎引导", exact: true }).click();
  await page.getByRole("button", { name: "开始使用", exact: true }).click();
  await page.screenshot({ path: "test-results/onboarding-dark.png", animations: "disabled" });
  await page.getByRole("button", { name: "跳过", exact: true }).click();
  await page.getByRole("button", { name: "继续", exact: true }).click();
  await expect(page.getByText("已保留你在设置中选择的搜索引擎。")).toBeVisible();
  await page.getByRole("button", { name: "继续", exact: true }).click();
  await page.getByRole("button", { name: "暂时跳过", exact: true }).click();
  await page.getByRole("radio", { name: "暂时不用", exact: true }).check();
  await page.getByRole("button", { name: "开始浏览", exact: true }).click();
  await expect(page.getByRole("tab")).toHaveCount(originalTabs);
  await expect(page.locator(".favorites .favorite-tile")).toHaveCount(8);
  await page.getByRole("button", { name: "设置", exact: true }).click();
  await expect(page.getByRole("combobox", { name: "搜索引擎", exact: true })).toHaveValue("baidu");
});
