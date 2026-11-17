import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.route("**/favicon.ico", route => route.abort());
  await page.goto("/");
});

test("quit dialog uses Rove branding and supports cancellation and keyboard confirmation", async ({ page }) => {
  const close = page.getByRole("button", { name: "关闭窗口", exact: true });
  const dialog = page.getByRole("alertdialog", { name: "确定要退出 Rove 吗？" });
  await close.click();
  await expect(dialog).toBeVisible();
  await expect(dialog).toBeFocused();
  await expect(dialog).toHaveCSS("outline-style", "none");
  await expect(dialog.locator("img, svg")).toHaveCount(0);
  await expect(dialog).toHaveCSS("background-color", "rgb(33, 33, 33)");
  await expect(dialog.getByRole("button", { name: "退出", exact: true })).toHaveCSS("background-color", "rgb(255, 152, 168)");
  await page.screenshot({ path: "test-results/quit-dialog.png" });
  await page.keyboard.press("Control+t");
  await expect(page.getByRole("combobox")).toHaveCount(0);
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  await expect(close).toBeFocused();
  await close.click();
  await dialog.getByRole("button", { name: "取消", exact: true }).click();
  await expect(dialog).toHaveCount(0);
  await close.click();
  await page.keyboard.press("Enter");
  await expect(dialog).toHaveCount(0);
  // A normal confirmation retains the preference to ask next time.
  await close.click();
  await expect(dialog).toBeVisible();
});

test("quit dialog traps focus and always quit turns off future prompts", async ({ page }) => {
  const close = page.getByRole("button", { name: "关闭窗口", exact: true });
  await close.click();
  const dialog = page.getByRole("alertdialog");
  const always = dialog.getByRole("button", { name: "始终退出", exact: true });
  await page.keyboard.press("Tab");
  await expect(always).toBeFocused();
  await page.keyboard.press("Shift+Tab");
  await expect(dialog.getByRole("button", { name: "退出", exact: true })).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(always).toBeFocused();
  await always.click();
  await expect(dialog).toHaveCount(0);
  await close.click();
  await expect(dialog).toHaveCount(0);
});

test("quit dialog fits a narrow viewport", async ({ page }) => {
  await page.setViewportSize({ width: 480, height: 640 });
  await page.getByRole("button", { name: "关闭窗口", exact: true }).click();
  const dialog = page.getByRole("alertdialog");
  const bounds = (await dialog.boundingBox())!;
  expect(bounds.x).toBeGreaterThanOrEqual(20);
  expect(bounds.x + bounds.width).toBeLessThanOrEqual(460);
  for (const button of await dialog.getByRole("button").all()) await expect(button).toBeInViewport();
});
