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
  const filter = history.getByRole("button", { name: "筛选", exact: true });
  await filter.click();
  const panel = history.getByRole("dialog", { name: "筛选历史记录" });
  const bounds = await panel.boundingBox();
  const pageBounds = await history.boundingBox();
  expect(bounds!.x).toBeGreaterThanOrEqual(pageBounds!.x);
  expect(bounds!.x + bounds!.width).toBeLessThanOrEqual(pageBounds!.x + pageBounds!.width);
  await panel.getByRole("button", { name: "今天", exact: true }).click();
  await page.keyboard.press("Escape");
  await expect(panel).toHaveCount(0);
  await expect(filter).toBeFocused();
  await filter.click();
  await expect(panel.getByRole("button", { name: "今天", exact: true })).toHaveAttribute("aria-pressed", "false");
  await panel.getByRole("button", { name: "完成", exact: true }).click();

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

  await alpha.focus();
  await page.keyboard.press("Control+a");
  await page.keyboard.press("Delete");
  const dialog = history.getByRole("alertdialog");
  await expect(dialog).toHaveAccessibleName("移除这 1 个页面？");
  await expect(dialog.getByRole("button", { name: "取消" })).toBeFocused();
  await dialog.getByRole("button", { name: "确定移除" }).click();
  await expect(history.getByText("没有匹配的历史记录")).toBeVisible();
  await history.getByRole("searchbox").fill("beta");
  await expect(history.getByRole("link")).toHaveAttribute("href", "https://docs.example/beta");
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

test("the new filter combines local date ranges, search and history sort orders", async ({ page }) => {
  const visit = async (month: number, day: number, address: string, hour = 12) => {
    await page.clock.setFixedTime(new Date(2026, month - 1, day, hour));
    await openPage(page, address);
    await page.keyboard.press("Control+w");
  };
  await visit(10, 30, "https://old.example/older");
  await visit(11, 3, "https://gamma.example/monthly");
  await visit(11, 19, "https://beta.example/weekly");
  await visit(11, 21, "https://zeta.example/popular");
  await visit(11, 21, "https://zeta.example/popular");
  await visit(11, 21, "https://alpha.example/today", 13);
  await chooseBrowserMenuItem(page, "历史记录");
  const history = page.locator(".history-page");
  const links = history.getByRole("link");
  const apply = async (...labels: string[]) => {
    await history.getByRole("button", { name: "筛选", exact: true }).click();
    const panel = history.getByRole("dialog", { name: "筛选历史记录" });
    for (const label of labels) await panel.getByRole("button", { name: label, exact: true }).click();
    await panel.getByRole("button", { name: "完成", exact: true }).click();
  };
  const urls = () => links.evaluateAll((items) => items.map((item) => item.getAttribute("href")));
  await expect(history.getByRole("button", { name: "清除历史记录", exact: true })).toHaveCount(0);
  await history.getByRole("searchbox").fill("example");
  await apply("今天", "按访问次数");
  await expect.poll(urls).toEqual(["https://zeta.example/popular", "https://alpha.example/today"]);
  await expect(links.first()).toContainText("访问 2 次");
  await apply("本周", "按最近访问");
  await expect.poll(urls).toEqual(["https://alpha.example/today", "https://zeta.example/popular", "https://beta.example/weekly"]);
  await apply("本月", "按站点");
  await expect.poll(urls).toEqual([
    "https://alpha.example/today", "https://beta.example/weekly", "https://gamma.example/monthly", "https://zeta.example/popular",
  ]);
  await apply("已关闭的标签页");
  await history.getByRole("searchbox").fill("beta weekly");
  await expect(history.getByRole("link")).toHaveCount(0);
  await expect(history.locator(".history-recent-tab")).toHaveCount(1);
  await history.locator(".history-recent-tab").click();
  await expect(page.getByRole("tab", { name: "beta.example", exact: true })).toHaveAttribute("aria-selected", "true");
});
