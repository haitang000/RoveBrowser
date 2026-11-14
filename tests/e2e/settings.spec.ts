import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.route("**/favicon.ico", (route) => route.abort());
  await page.goto("/");
  await page.getByRole("button", { name: "设置", exact: true }).click();
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
  await page.getByRole("button", { name: "设置", exact: true }).click();
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

test("quick workspace editor keeps a single live clear control", async ({ page }) => {
  await page.getByRole("button", { name: "工作区与外观", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "工作区与外观", exact: true });
  await dialog.getByRole("button", { name: "效果", exact: true }).click();
  const clear = dialog.getByRole("slider", { name: "清透", exact: true });
  await expect(clear).toBeVisible();
  await expect(dialog.getByText("磨砂", { exact: true })).toHaveCount(0);
  await clear.fill("0.7");
  await dialog.getByRole("button", { name: "保存", exact: true }).click();
  await page.getByRole("button", { name: "工作区与外观", exact: true }).click();
  await expect(clear).toHaveValue("0.7");
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
