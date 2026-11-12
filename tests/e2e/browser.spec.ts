import { test, expect } from "@playwright/test";
test.beforeEach(async ({ page }) => {
  await page.goto("/");
});
test("source-derived sidebar metrics and responsive layouts", async ({
  page,
}) => {
  const metrics = await page.locator(".sidebar").evaluate((e) => ({
    width: e.getBoundingClientRect().width,
    row: document.querySelector(".tab-row")!.getBoundingClientRect().height,
    fav: document.querySelector(".favorites button")!.getBoundingClientRect()
      .height,
  }));
  expect(metrics).toEqual({ width: 240, row: 30, fav: 36 });
  await page.evaluate(() => document.fonts.ready);
  expect(
    await page.evaluate(() =>
      document.fonts.check('13px "Inter Variable"', "Personal"),
    ),
  ).toBe(true);
  expect(
    await page
      .locator(".space-name")
      .evaluate((e) => getComputedStyle(e).fontFamily),
  ).toContain("Inter Variable");
  await page.screenshot({ path: "test-results/ui-light.png" });
  for (const size of [
    { width: 480, height: 400 },
    { width: 560, height: 400 },
    { width: 1920, height: 1080 },
  ]) {
    await page.setViewportSize(size);
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= innerWidth,
      ),
    ).toBe(true);
    await expect.poll(async () => (await page.locator(".page-area").boundingBox())!.width).toBeGreaterThan(size.width === 480 ? 220 : 300);
    await page.keyboard.press("Control+t");
    const panel = await page.locator(".command-panel").boundingBox();
    expect(panel!.y + panel!.height).toBeLessThanOrEqual(size.height);
    await page.keyboard.press("Escape");
  }
});
test("command keyboard navigation, close and restore", async ({ page }) => {
  await page.keyboard.press("Control+t");
  const field = page.getByRole("combobox");
  await expect(field).toBeFocused();
  await field.fill("example.com");
  await page.keyboard.press("Enter");
  await expect(page.getByRole("tab", { name: "example.com" })).toBeVisible();
  await page.keyboard.press("Control+w");
  await expect(page.getByRole("tab", { name: "example.com" })).toHaveCount(0);
  await page.keyboard.press("Control+Shift+t");
  await expect(page.getByRole("tab", { name: "example.com" })).toBeVisible();
  await page.keyboard.press("Control+l");
  await expect(field).toHaveValue("https://example.com/");
  await expect(page.locator(".command-scrim")).toHaveCSS("opacity", "1");
  await page.screenshot({ path: "test-results/ui-command.png" });
  await page.keyboard.press("Escape");
  await expect(field).toHaveCount(0);
});
test("spaces, dark mode, pinning and split container", async ({ page }) => {
  await page.getByRole("button", { name: "工作区 Work", exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Work', exact: true })).toBeVisible();
  await page
    .getByRole("button", { name: "工作区 Personal", exact: true })
    .click();
  // Coming back restores Personal's New Tab page, so select the pinned web page
  // the split needs explicitly instead of relying on the first tab.
  await page.getByRole("tab", { name: "GitHub" }).click();
  await page.getByRole("tab", { name: "Wikipedia" }).click({ button: "right" });
  await page.getByRole("menuitem", { name: "在当前网页旁分屏" }).click();
  await expect(page.locator(".page-pane")).toHaveCount(2);
  await page.getByRole("button", { name: "固定分屏组合" }).click();
  await expect(page.locator(".pinned-tabs .tab-row.selected")).toHaveCount(1);
  await expect(page.locator("iframe")).toHaveCount(0);
  await page.getByRole("button", { name: "关闭分屏" }).click();
  await expect(page.locator(".page-pane")).toHaveCount(1);
  await page.getByRole("button", { name: "工作区与外观" }).click();
  await page.getByRole("button", { name: "切换深色模式" }).click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await expect(page.locator(".browser-shell")).toHaveCSS(
    "background-color",
    "rgb(29, 39, 54)",
  );
  await page.getByRole("button", { name: "关闭设置" }).click();
  await page.screenshot({ path: "test-results/ui-dark.png" });
  await page.keyboard.press("Control+Shift+Backslash");
  await expect(
    page.getByRole("dialog", { name: "全部标签页", exact: true }),
  ).toBeVisible();
  await page.keyboard.press("Escape");
  await page.keyboard.press("Control+b");
  await expect(page.locator(".sidebar-track")).toHaveCSS("width", "0px");
  await page.getByRole("button", { name: "展开侧栏", exact: true }).click();
  await expect(page.locator(".sidebar-track")).toHaveCSS("width", "240px");
});
