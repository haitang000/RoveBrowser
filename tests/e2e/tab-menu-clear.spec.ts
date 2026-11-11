import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.goto("/");
});

async function openPage(page: Page, url: string) {
  await page.keyboard.press("Control+t");
  await page.getByRole("combobox").fill(url);
  await page.keyboard.press("Enter");
}
const regularRows = (page: Page) =>
  page.locator(".space-list > .tab-list:not(.pinned-tabs) .tab-row");

test("tab menu completes rename, copy, duplicate and close groups", async ({
  page,
}) => {
  await openPage(page, "https://example.com/docs");
  const row = page.getByRole("tab", { name: "example.com" });
  await row.click({ button: "right" });
  const menu = page.getByRole("menu");
  await expect(menu.getByRole("menuitem", { name: "重命名标签页" })).toBeVisible();
  await expect(menu.getByRole("menuitem", { name: "复制标签页" })).toBeVisible();
  await expect(menu.getByRole("menuitem", { name: "复制链接" })).toBeVisible();
  await expect(
    menu.getByRole("menuitem", { name: "关闭其他标签页" }),
  ).toBeEnabled();
  await menu.getByRole("menuitem", { name: "复制链接" }).click();
  await expect(page.locator(".action-toast")).toContainText("已复制链接");

  await row.click({ button: "right" });
  await menu.getByRole("menuitem", { name: "重命名标签页" }).click();
  await page.getByRole("textbox", { name: "标签名称" }).fill("我的文档");
  await page.getByRole("button", { name: "保存名称" }).click();
  await expect(page.getByRole("tab", { name: "我的文档" })).toBeVisible();
  await expect(page.getByRole("tab", { name: "example.com" })).toHaveCount(0);

  // A renamed tab offers going back to the page title it came from.
  await page.getByRole("tab", { name: "我的文档" }).click({ button: "right" });
  await expect(
    menu.getByRole("menuitem", { name: "恢复网页标题" }),
  ).toBeVisible();
  await page.keyboard.press("Escape");
});

test("Clear closes the other tabs and previews the rows it would remove", async ({
  page,
}) => {
  await openPage(page, "https://example.com/docs");
  const before = await regularRows(page).count();
  const clear = page.locator(".clear-tabs");
  // Enter the list first so the hover-only button becomes hit-testable.
  await page.locator(".space-list .section-label").hover();
  await clear.hover();
  await expect(clear).toBeVisible();
  // The rows it would close sink back while the button is hovered.
  await expect(page.locator(".tab-row.doomed")).toHaveCount(before - 1);
  await page.waitForTimeout(400); // Clear arms after 350ms on screen.
  await clear.click();
  await expect(regularRows(page)).toHaveCount(before - 1);
  await expect(page.locator(".action-toast")).toContainText(/已关闭 \d+ 个标签/);
});

test("the Clear button arms only after it has been visible for a moment", async ({
  page,
}) => {
  await openPage(page, "https://example.com/docs");
  const count = await regularRows(page).count();
  await page.locator(".space-list .section-label").hover();
  const clear = page.locator(".clear-tabs");
  await clear.click({ force: true });
  // A click during the arming window must not wipe the tab list.
  await expect(regularRows(page)).toHaveCount(count);
});
