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

test("space editor creates and deletes a space in place with confirmation", async ({
  page,
}) => {
  await page.getByRole("button", { name: "工作区", exact: true }).click();
  await page.getByRole("button", { name: "新建工作区", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "新建工作区", exact: true });
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
