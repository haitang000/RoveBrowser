import { expect, test } from "@playwright/test";
import type { DownloadItem } from "../../src/lib/types";

function download(patch: Partial<DownloadItem>): DownloadItem {
  return {
    id: "d1",
    url: "https://example.com/file.bin",
    path: "C:\\Downloads\\file.bin",
    filename: "file.bin",
    received: 0,
    total: 0,
    state: "inProgress",
    started: 1,
    exists: false,
    ...patch,
  };
}

const items: DownloadItem[] = [
  download({
    id: "active",
    filename: "movie.mp4",
    received: 5_242_880,
    total: 10_485_760,
    state: "inProgress",
  }),
  download({
    id: "done",
    filename: "report.pdf",
    received: 4096,
    total: 4096,
    state: "finished",
    exists: true,
  }),
  download({ id: "failed", filename: "broken.zip", state: "failed", failure: "网络已断开" }),
];

test.beforeEach(async ({ page }) => {
  await page.goto("/");
  await page.evaluate((list) => {
    window.__ERA_RECEIVE__?.({ type: "downloads", items: list });
  }, items);
});

test("downloads panel opens from the sidebar and lists every state", async ({
  page,
}) => {
  await page.getByRole("button", { name: "下载" }).click();
  const panel = page.getByRole("dialog", { name: "下载" });
  await expect(panel).toBeVisible();
  await expect(panel).toHaveCSS("border-top-width", "0px");
  await expect(panel.locator(".download-row")).toHaveCount(3);
  await expect(panel).toContainText("movie.mp4");
  await expect(panel).toContainText("50% · 5 MB / 10 MB");
  await expect(panel).toContainText("已完成 · 4 KB");
  await expect(panel).toContainText("失败 · 网络已断开");
  // An active row shows its progress line and pause/cancel controls.
  const active = panel.locator(".download-row", { hasText: "movie.mp4" });
  await expect(active.locator(".download-bar")).toBeVisible();
  await expect(active.getByTitle("暂停下载")).toBeVisible();
  await expect(active.getByTitle("取消下载")).toBeVisible();
});

test("the final bytes and engine completion have distinct states", async ({ page }) => {
  await page.getByRole("button", { name: "下载", exact: true }).click();
  const panel = page.getByRole("dialog", { name: "下载" });
  const row = panel.locator(".download-row");
  const pending = download({ received: 999, total: 1000 });
  await page.evaluate((item) => window.__ERA_RECEIVE__?.({ type: "downloads", items: [item] }), pending);
  await expect(row).toContainText("99%");
  await expect(row.getByRole("progressbar")).toHaveAttribute("aria-valuenow", "99");
  await page.evaluate((item) => window.__ERA_RECEIVE__?.({ type: "downloads", items: [item] }), { ...pending, received: 1000 });
  await expect(row).toContainText("正在检查或等待确认");
  await expect(row.getByRole("button", { name: "查看下载提示", exact: true })).toBeVisible();
  await expect(row.getByRole("button", { name: "打开文件", exact: true })).toHaveCount(0);
  await page.evaluate((item) => window.__ERA_RECEIVE__?.({ type: "downloads", items: [item] }),
    { ...pending, received: 1000, state: "finished", exists: true });
  await expect(row).toContainText("已完成");
  await expect(row.getByRole("progressbar")).toHaveCount(0);
  await expect(row.getByRole("button", { name: "打开文件", exact: true })).toBeVisible();
});

test("an executable waiting for confirmation can reveal the native prompt without being marked complete", async ({ page }) => {
  const pending = download({ filename: "setup.exe", received: 1000, total: 1000 });
  await page.evaluate((item) => window.__ERA_RECEIVE__?.({ type: "downloads", items: [item] }), pending);
  await page.getByRole("button", { name: "下载", exact: true }).click();
  const panel = page.getByRole("dialog", { name: "下载" });
  await panel.getByRole("button", { name: "查看下载提示", exact: true }).click();
  await expect(panel).toHaveCount(0);
  await page.getByRole("button", { name: "下载", exact: true }).click();
  await expect(panel).toContainText("正在检查或等待确认");
  await expect(panel.getByRole("button", { name: "打开文件", exact: true })).toHaveCount(0);
  await expect(panel.getByRole("button", { name: "取消下载", exact: true })).toBeVisible();
});

test("downloads replace the command panel and the actions menu supports the keyboard", async ({ page }) => {
  await page.keyboard.press("Control+l");
  await expect(page.locator(".command-panel")).toBeVisible();
  await page.keyboard.press("Control+j");
  const panel = page.getByRole("dialog", { name: "下载" });
  await expect(panel).toBeVisible();
  await expect(page.locator(".command-panel")).toHaveCount(0);
  await panel.getByRole("button", { name: "更多下载操作" }).focus();
  await page.keyboard.press("ArrowDown");
  await expect(panel.getByRole("menuitem")).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(panel.getByRole("menu")).toHaveCount(0);
  await expect(panel).toBeVisible();
  await expect(panel.getByRole("button", { name: "更多下载操作" })).toBeFocused();
});

test("pause, resume and cancel follow Zigzag's rules", async ({ page }) => {
  await page.getByRole("button", { name: "下载" }).click();
  const panel = page.getByRole("dialog", { name: "下载" });
  const active = panel.locator(".download-row", { hasText: "movie.mp4" });
  await active.getByTitle("暂停下载").click();
  await expect(active).toContainText("已暂停");
  await active.getByTitle("继续下载").click();
  await expect(active).toContainText("50%");
  await active.getByTitle("取消下载").click();
  await expect(active).toContainText("已取消");
  // A cancelled transfer offers removing its row, and Clear removes it too.
  await expect(active.getByTitle("从列表中移除")).toBeVisible();
});

test("Ctrl+J and Escape toggle the panel, and a new download reveals it", async ({
  page,
}) => {
  await page.keyboard.press("Control+j");
  const panel = page.getByRole("dialog", { name: "下载" });
  await expect(panel).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(panel).toHaveCount(0);
  await page.evaluate(() => {
    window.__ERA_RECEIVE__?.({
      type: "downloads",
      items: [
        {
          id: "fresh",
          url: "https://example.com/fresh.bin",
          path: "",
          filename: "fresh.bin",
          received: 0,
          total: 0,
          state: "inProgress",
          started: 2,
        },
      ],
    });
    window.__ERA_RECEIVE__?.({ type: "downloadStarted", id: "fresh" });
  });
  await expect(panel).toBeVisible();
  await expect(panel).toContainText("fresh.bin");
});

test("Clear keeps the unfinished row and removes the rest", async ({ page }) => {
  await page.getByRole("button", { name: "下载" }).click();
  const panel = page.getByRole("dialog", { name: "下载" });
  await page.evaluate(() => {
    window.__ERA_RECEIVE__?.({
      type: "downloads",
      items: [
        {
          id: "active",
          url: "https://example.com/movie.mp4",
          path: "",
          filename: "movie.mp4",
          received: 1,
          total: 2,
          state: "inProgress",
          started: 1,
        },
        {
          id: "done",
          url: "https://example.com/report.pdf",
          path: "C:\\Downloads\\report.pdf",
          filename: "report.pdf",
          received: 4096,
          total: 4096,
          state: "finished",
          started: 1,
          exists: true,
        },
      ],
    });
  });
  await expect(panel.locator(".download-row")).toHaveCount(2);
  await panel.getByRole("button", { name: "更多下载操作" }).click();
  await panel.getByRole("menuitem", { name: "清除已结束的下载" }).click();
  await expect(panel.locator(".download-row")).toHaveCount(1);
  await expect(panel).toContainText("movie.mp4");
  await expect(panel).not.toContainText("report.pdf");
});

test("outside clicks close the panel across the page, sidebar and title bar", async ({ page }) => {
  const panel = page.getByRole("dialog", { name: "下载" });
  for (const point of [{ x: 900, y: 400 }, { x: 100, y: 100 }, { x: 600, y: 12 }]) {
    await page.getByRole("button", { name: "下载", exact: true }).click();
    await expect(panel).toBeVisible();
    await panel.getByRole("heading", { name: "下载" }).click();
    await expect(panel).toBeVisible();
    await page.mouse.click(point.x, point.y);
    await expect(panel).toHaveCount(0);
    await expect(page.getByRole("button", { name: "下载", exact: true })).toHaveAttribute("aria-expanded", "false");
  }
});

test("search filters filenames and completed rows offer explicit open, reveal and remove actions", async ({ page }) => {
  await page.getByRole("button", { name: "下载", exact: true }).click();
  const panel = page.getByRole("dialog", { name: "下载" });
  await panel.getByRole("button", { name: "搜索下载", exact: true }).click();
  const search = panel.getByRole("searchbox", { name: "搜索下载文件" });
  await expect(search).toBeFocused();
  await search.fill("REPORT");
  await expect(panel.locator(".download-row")).toHaveCount(1);
  await expect(panel.getByRole("button", { name: "打开文件", exact: true })).toBeVisible();
  await expect(panel.getByRole("button", { name: "在文件夹中显示" })).toBeVisible();
  await panel.getByRole("button", { name: "从列表中移除" }).click();
  await expect(panel).toContainText("未找到匹配的下载");
  await search.fill("");
  await expect(panel.locator(".download-row")).toHaveCount(2);
  await page.keyboard.press("Escape");
  await page.keyboard.press("Control+j");
  await expect(panel.getByRole("searchbox")).toHaveCount(0);
  await expect(panel.locator(".download-row")).toHaveCount(2);
});

test("long names wrap, history scrolls and the bubble stays anchored on resize", async ({ page }) => {
  await page.setViewportSize({ width: 800, height: 600 });
  await page.evaluate((list) => window.__ERA_RECEIVE__?.({ type: "downloads", items: list }),
    Array.from({ length: 18 }, (_, i) => download({ ...items[1], id: `long-${i}`, filename: `BaiduNetdisk_${"longfilename".repeat(8)}_${i}.exe` })),
  );
  await page.getByRole("button", { name: "下载", exact: true }).click();
  const panel = page.getByRole("dialog", { name: "下载" });
  await expect(panel).toBeVisible();
  await panel.evaluate((element) => Promise.all(element.getAnimations({ subtree: true }).map((animation) => animation.finished)));
  const geometry = await panel.evaluate((element) => {
    const list = element.querySelector(".downloads-list")!;
    const name = element.querySelector(".download-name")!;
    const box = element.getBoundingClientRect();
    return { x: box.x, y: box.y, right: box.right, bottom: box.bottom, scrolls: list.scrollHeight > list.clientHeight, wraps: name.clientHeight > 30, overflows: element.scrollWidth > element.clientWidth };
  });
  expect(geometry).toMatchObject({ scrolls: true, wraps: true, overflows: false });
  expect(geometry.y).toBeGreaterThanOrEqual(30);
  expect(geometry.right).toBeLessThanOrEqual(800);
  await page.setViewportSize({ width: 1000, height: 760 });
  await expect.poll(async () => panel.evaluate((element) => {
    const anchor = document.querySelector(".downloads-button")!.getBoundingClientRect();
    const box = element.getBoundingClientRect();
    const tip = parseFloat(getComputedStyle(element).getPropertyValue("--downloads-tip"));
    return Math.abs(box.x + tip - anchor.x - anchor.width / 2) < 1 && Math.abs(box.bottom + 10 - anchor.y) < 1;
  })).toBe(true);
});

test("reduced motion removes scaling and quick close/reopen leaves one working panel", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.keyboard.press("Control+j");
  const panel = page.getByRole("dialog", { name: "下载" });
  await expect(panel).toBeVisible();
  expect(await panel.evaluate((element) => getComputedStyle(element).transform)).toBe("none");
  await page.keyboard.press("Escape");
  await page.keyboard.press("Control+j");
  await expect(panel).toHaveCount(1);
  await expect(panel.getByRole("button", { name: "暂停下载" })).toBeVisible();
  await panel.getByRole("button", { name: "关闭下载面板" }).click();
  await expect(panel).toHaveCount(0);
  await page.emulateMedia({ reducedMotion: "no-preference" });
  await page.keyboard.press("Control+j");
  await page.keyboard.press("Escape");
  await page.keyboard.press("Control+j");
  await expect(panel).toHaveCount(1);
  await panel.getByRole("button", { name: "暂停下载" }).click();
  await expect(panel).toContainText("已暂停");
});
