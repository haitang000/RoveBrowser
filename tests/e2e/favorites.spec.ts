import { test, expect, type Page } from "@playwright/test";

async function openSettings(page: Page) {
  await page.getByRole("button", { name: "管理顶部常用网站", exact: true }).click();
  return page.getByRole("dialog", { name: "顶部常用网站", exact: true });
}

test.beforeEach(async ({ page }) => { await page.goto("/"); });

test("customize, order and open a top favorite across workspaces", async ({ page }) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  const panel = await openSettings(page);
  await panel.getByRole("textbox", { name: "网站名称" }).fill("开发文档");
  await panel.getByRole("textbox", { name: "网站网址" }).fill("example.com/docs");
  await panel.getByRole("button", { name: "添加网站", exact: true }).click();
  await expect(panel.getByRole("button", { name: "编辑 开发文档", exact: true })).toBeVisible();
  await panel.getByRole("button", { name: "上移 开发文档", exact: true }).click();
  await expect(panel.locator("li").nth(7)).toContainText("开发文档");
  await panel.getByRole("button", { name: "编辑 开发文档", exact: true }).click();
  await panel.getByRole("textbox", { name: "网站名称" }).fill("我的文档");
  await panel.getByRole("textbox", { name: "网站网址" }).fill("https://example.com/guide");
  await panel.getByRole("button", { name: "保存修改", exact: true }).click();
  await expect(panel.locator("li").nth(7)).toContainText("我的文档");
  await panel.getByRole("button", { name: "移除 Figma", exact: true }).click();
  await expect(panel.getByRole("button", { name: "编辑 Figma", exact: true })).toHaveCount(0);
  await page.keyboard.press("Escape");
  await expect(page.getByRole("button", { name: "管理顶部常用网站", exact: true })).toBeFocused();
  const favorite = page.locator(".favorites").getByRole("button", { name: "我的文档", exact: true });
  await favorite.click();
  await expect(favorite).toHaveClass(/chosen/);
  await expect(page.locator('.address-field')).toContainText("example.com");
  const count = await page.getByRole("tab").count();
  await favorite.click();
  await expect(page.getByRole("tab")).toHaveCount(count);
  await page.getByRole("button", { name: "工作区 Work", exact: true }).click();
  await favorite.click();
  await expect(favorite).toHaveClass(/chosen/);
  await expect(page.getByRole('heading', { name: 'Work', exact: true })).toBeVisible();
  await favorite.click({ button: "right" });
  await page.getByRole("menuitem", { name: "编辑常用网站", exact: true }).click();
  await expect(panel).toBeVisible();
  expect(errors).toEqual([]);
});

test("invalid and duplicate addresses keep the form and list intact", async ({ page }) => {
  const panel = await openSettings(page);
  await panel.getByRole("textbox", { name: "网站网址" }).fill("file:///C:/Windows");
  await panel.getByRole("button", { name: "添加网站", exact: true }).click();
  await expect(panel.getByRole("alert")).toContainText("HTTP");
  await expect(panel.locator("li")).toHaveCount(8);
  await panel.getByRole("textbox", { name: "网站网址" }).fill("https://GITHUB.com:443/");
  await panel.getByRole("button", { name: "添加网站", exact: true }).click();
  await expect(panel.getByRole("alert")).toContainText("已在");
  await expect(panel.locator("li")).toHaveCount(8);
  await expect(panel.getByRole("textbox", { name: "网站网址" })).toHaveValue("https://GITHUB.com:443/");
});

test("tab context menu adds a favorite and settings remains usable in a small window", async ({ page }) => {
  await page.getByRole("tab", { name: "Wikipedia" }).click({ button: "right" });
  await page.getByRole("menuitem", { name: "添加到顶部常用网站", exact: true }).click();
  await expect(page.locator(".favorites").getByRole("button", { name: "Wikipedia", exact: true })).toBeVisible();
  await expect(page.getByRole("tab", { name: "Wikipedia" })).toHaveCount(0);
  await page.setViewportSize({ width: 480, height: 400 });
  await page.getByRole("button", { name: "工作区与外观", exact: true }).click();
  await page.getByRole("button", { name: "管理顶部常用网站", exact: true }).last().click();
  const panel = page.getByRole("dialog", { name: "顶部常用网站", exact: true });
  await expect(panel).toBeVisible();
  const bounds = await panel.boundingBox();
  expect(bounds!.x).toBeGreaterThanOrEqual(0);
  expect(bounds!.y).toBeGreaterThanOrEqual(32);
  expect(bounds!.y + bounds!.height).toBeLessThanOrEqual(400);
  await panel.getByRole("textbox", { name: "网站网址" }).fill("example.com");
  await panel.getByRole("button", { name: "添加网站", exact: true }).click();
  await expect(panel.getByRole("button", { name: "编辑 example.com", exact: true })).toBeVisible();
  await page.screenshot({ path: "test-results/ui-favorites-small.png" });
});

test("an empty favorites list can be configured again", async ({ page }) => {
  const panel = await openSettings(page);
  while (await panel.locator("li").count())
    await panel.getByRole("button", { name: /^移除 / }).first().click();
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "添加常用网站", exact: true }).click();
  await panel.getByRole("textbox", { name: "网站网址" }).fill("example.com");
  await panel.getByRole("button", { name: "添加网站", exact: true }).click();
  await expect(panel.locator("li")).toHaveCount(1);
});
