import { expect, test, type Page } from "@playwright/test";
import { chooseBrowserMenuItem } from "./browser-menu";

async function openPage(page: Page, url: string) {
  await page.keyboard.press("Control+t");
  await page.getByRole("combobox").fill(url);
  await page.getByRole("combobox").press("Enter");
}

test.beforeEach(async ({ page }) => {
  await page.route("**/favicon.ico", (route) => route.abort());
  await page.goto("/");
});

test("history opens in a reusable tab and supports narrower windows", async ({ page }) => {
  await chooseBrowserMenuItem(page, "历史记录");
  const tab = page.getByRole("tab", { name: "历史记录", exact: true });
  const heading = page.getByRole("heading", { name: "历史记录", exact: true });
  await expect(heading).toBeVisible();
  await openPage(page, "https://notes.example/alpha");
  await page.keyboard.press("Control+h");
  await expect(tab).toHaveCount(1);
  await expect(tab).toHaveAttribute("aria-selected", "true");
  await expect(page.getByRole("link", { name: /notes\.example\/alpha/ })).toBeVisible();

  await page.setViewportSize({ width: 480, height: 480 });
  const history = page.locator(".history-scroll");
  await expect(history.getByRole("searchbox")).toBeVisible();
  expect(await history.evaluate((element) => element.scrollWidth)).toBeLessThanOrEqual(await history.evaluate((element) => element.clientWidth));

  const historyId = await tab.getAttribute("data-tab-id");
  const link = history.locator('a[href="https://notes.example/alpha"]');
  await link.click({ modifiers: ["Control"] });
  await expect(tab).toHaveAttribute("aria-selected", "false");
  await page.keyboard.press("Control+h");
  await expect(tab).toHaveAttribute("data-tab-id", historyId!);
  await link.click();
  await expect(page.getByRole("tab", { selected: true })).toHaveAttribute("data-tab-id", historyId!);
  await expect(page.locator(".preview-page small")).toHaveText("https://notes.example/alpha");
});

test("site clusters expand, search excludes unrelated URLs, and edits can be undone", async ({ page }) => {
  await openPage(page, "https://docs.example/alpha");
  await openPage(page, "https://docs.example/beta");
  await openPage(page, "rove://history");
  const history = page.locator(".history-page");
  const cluster = history.getByRole("button", { name: /docs\.example.*2 个页面/ });
  await expect(cluster).toHaveAttribute("aria-expanded", "false");
  await cluster.click();
  const alpha = history.getByRole("link", { name: /docs\.example\/alpha/ });
  await expect(alpha).toBeVisible();
  await history.getByRole("searchbox").fill("docs alpha");
  await expect(cluster).toHaveCount(0);
  await expect(history.getByRole("link")).toHaveCount(1);
  await alpha.focus();
  await page.keyboard.press("Delete");
  await expect(history.getByText("没有匹配的历史记录")).toBeVisible();
  await page.keyboard.press("Control+z");
  await expect(alpha).toBeVisible();

  await history.getByRole("button", { name: "清除历史记录", exact: true }).click();
  await history.getByRole("menuitem", { name: "最近一小时", exact: true }).click();
  const dialog = history.getByRole("alertdialog");
  await expect(dialog.getByRole("button", { name: "取消" })).toBeFocused();
  await dialog.getByRole("button", { name: "确定移除" }).click();
  await expect(history.getByText("没有匹配的历史记录")).toBeVisible();
});

test("recently closed restores the chosen tab and retains older close history", async ({ page }) => {
  await openPage(page, "https://first.example/");
  await page.keyboard.press("Control+w");
  await openPage(page, "https://second.example/");
  await page.keyboard.press("Control+w");
  await chooseBrowserMenuItem(page, "历史记录");
  const recent = page.locator(".history-recent");
  await recent.getByText("最近关闭的标签页", { exact: true }).click();
  await recent.getByRole("button", { name: /first\.example/ }).click();
  await expect(page.getByRole("tab", { name: "first.example", exact: true })).toHaveAttribute("aria-selected", "true");
  await page.keyboard.press("Control+Shift+t");
  await expect(page.getByRole("tab", { name: "second.example", exact: true })).toHaveAttribute("aria-selected", "true");
});
