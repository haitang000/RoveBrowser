import { describe, expect, it } from "vitest";
import {
  activeDownloadCount,
  clearableDownloads,
  downloadFraction,
  downloadPercent,
  downloadRowAction,
  downloadStatus,
  formatBytes,
  isActiveDownload,
  isFinalizingDownload,
} from "../src/lib/downloads";
import type { DownloadItem } from "../src/lib/types";

function item(patch: Partial<DownloadItem>): DownloadItem {
  return {
    id: "one",
    url: "https://example.com/file.bin",
    path: "C:\\Downloads\\file.bin",
    filename: "file.bin",
    received: 0,
    total: 0,
    state: "inProgress",
    started: 1,
    ...patch,
  };
}

describe("download sizes", () => {
  it("formats bytes compactly with 1024-based units", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(-4)).toBe("0 B");
    expect(formatBytes(999)).toBe("999 B");
    expect(formatBytes(1024)).toBe("1 KB");
    expect(formatBytes(1536)).toBe("1.5 KB");
    expect(formatBytes(5 * 1024 * 1024)).toBe("5 MB");
    expect(formatBytes(120 * 1024 * 1024)).toBe("120 MB");
    expect(formatBytes(2.5 * 1024 * 1024 * 1024)).toBe("2.5 GB");
  });
  it("keeps fractions inside 0..1 even for an unknown total", () => {
    expect(downloadFraction(item({ received: 10, total: 0 }))).toBe(0);
    expect(downloadFraction(item({ received: 50, total: 100 }))).toBe(0.5);
    expect(downloadFraction(item({ received: 300, total: 100 }))).toBe(1);
    expect(downloadPercent(item({ received: 200, total: 300 }))).toBe(67);
    expect(downloadPercent(item({ received: 999, total: 1000 }))).toBe(99);
    expect(downloadPercent(item({ received: 1000, total: 1000 }))).toBe(100);
  });
});

describe("download rows", () => {
  it("writes Zigzag's status lines for every state", () => {
    expect(downloadStatus(item({ state: "inProgress", received: 2048, total: 4096 }))).toBe(
      "50% · 2 KB / 4 KB",
    );
    expect(downloadStatus(item({ state: "inProgress", received: 128 }))).toBe("正在下载 · 128 B");
    expect(downloadStatus(item({ state: "paused", received: 2048, total: 4096 }))).toContain("已暂停");
    expect(downloadStatus(item({ state: "finished", received: 4096, total: 4096, exists: true }))).toBe(
      "已完成 · 4 KB",
    );
    expect(downloadStatus(item({ state: "finished", total: 4096, exists: false }))).toBe(
      "文件已被移动或删除",
    );
    expect(downloadStatus(item({ state: "failed", failure: "网络已断开" }))).toBe("失败 · 网络已断开");
    expect(downloadStatus(item({ state: "failed" }))).toBe("下载失败");
    expect(downloadStatus(item({ state: "cancelled" }))).toBe("已取消");
  });
  it("keeps a full transfer active until the engine confirms completion", () => {
    const pending = item({ received: 4096, total: 4096 });
    expect(downloadStatus(pending)).toBe("正在检查或等待确认 · 4 KB");
    expect(isFinalizingDownload(pending)).toBe(true);
    expect(isFinalizingDownload({ ...pending, state: "paused" })).toBe(false);
    expect(isFinalizingDownload({ ...pending, total: 0 })).toBe(false);
    expect(isFinalizingDownload({ ...pending, received: 4000 })).toBe(false);
    expect(isActiveDownload(pending)).toBe(true);
    expect(clearableDownloads([pending])).toEqual([]);
    expect(downloadStatus({ ...pending, state: "finished", exists: true })).toBe("已完成 · 4 KB");
  });
  it("clear keeps running and paused rows and counts only active downloads", () => {
    const items = [
      item({ id: "running", state: "inProgress" }),
      item({ id: "paused", state: "paused" }),
      item({ id: "done", state: "finished" }),
      item({ id: "failed", state: "failed" }),
    ];
    expect(activeDownloadCount(items)).toBe(2);
    expect(items.filter(isActiveDownload).map((entry) => entry.id)).toEqual(["running", "paused"]);
    expect(clearableDownloads(items).map((entry) => entry.id)).toEqual(["done", "failed"]);
  });
  it("maps each state to the action button Zigzag shows", () => {
    expect(downloadRowAction(item({ state: "inProgress" }))).toBe("pause");
    expect(downloadRowAction(item({ state: "paused" }))).toBe("resume");
    expect(downloadRowAction(item({ state: "finished" }))).toBe("reveal");
    expect(downloadRowAction(item({ state: "failed" }))).toBe("remove");
    expect(downloadRowAction(item({ state: "cancelled" }))).toBe("remove");
  });
});
