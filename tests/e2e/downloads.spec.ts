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
  await panel.getByRole("button", { name: "清除" }).click();
  await expect(panel.locator(".download-row")).toHaveCount(1);
  await expect(panel).toContainText("movie.mp4");
  await expect(panel).not.toContainText("report.pdf");
});
