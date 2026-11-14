// Formatting and status rules for the downloads panel, ported from Zigzag's
// DownloadRowView/DownloadManager presentation (Apache-2.0).
import type { DownloadItem } from "./types";

/** Byte sizes the way the panel writes them: compact, 1024-based, one decimal. */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const digits = unit === 0 || value >= 10 || Number.isInteger(value) ? 0 : 1;
  return `${value.toFixed(digits)} ${units[unit]}`;
}

export function downloadFraction(item: Pick<DownloadItem, "received" | "total">): number {
  if (!item.total || item.total <= 0) return 0;
  return Math.min(1, Math.max(0, item.received / item.total));
}

export function downloadPercent(item: Pick<DownloadItem, "received" | "total">): number {
  const percent = Math.round(downloadFraction(item) * 100);
  return item.received < item.total ? Math.min(99, percent) : percent;
}

/** All bytes arrived, but the engine may still need a scan or user decision. */
export function isFinalizingDownload(item: Pick<DownloadItem, "state" | "received" | "total">): boolean {
  return item.state === "inProgress" && item.total > 0 && item.received >= item.total;
}

/** The line under the file name, in Zigzag's plain words. */
export function downloadStatus(item: DownloadItem): string {
  switch (item.state) {
    case "inProgress":
      if (isFinalizingDownload(item)) {
        return `正在检查或等待确认 · ${formatBytes(item.received)}`;
      }
      return item.total > 0
        ? `${downloadPercent(item)}% · ${formatBytes(item.received)} / ${formatBytes(item.total)}`
        : `正在下载 · ${formatBytes(item.received)}`;
    case "paused":
      return item.total > 0
        ? `已暂停 · ${downloadPercent(item)}% · ${formatBytes(item.received)} / ${formatBytes(item.total)}`
        : "已暂停";
    case "finished":
      return item.exists === false
        ? "文件已被移动或删除"
        : `已完成 · ${formatBytes(item.total || item.received)}`;
    case "failed":
      return item.failure ? `失败 · ${item.failure}` : "下载失败";
    case "cancelled":
      return "已取消";
    default:
      return "";
  }
}

export function isActiveDownload(item: Pick<DownloadItem, "state">): boolean {
  return item.state === "inProgress" || item.state === "paused";
}

/** Clear never throws away a half-finished file, as in Zigzag. */
export function clearableDownloads(items: DownloadItem[]): DownloadItem[] {
  return items.filter((item) => !isActiveDownload(item));
}

export function activeDownloadCount(items: DownloadItem[]): number {
  return items.filter(isActiveDownload).length;
}

/** The action button on a row, following Zigzag: pause/resume while active,
 * reveal when finished, remove once the transfer cannot continue. */
export type DownloadRowAction = "pause" | "resume" | "cancel" | "reveal" | "remove";

export function downloadRowAction(item: DownloadItem): DownloadRowAction {
  if (item.state === "inProgress") return "pause";
  if (item.state === "paused") return "resume";
  if (item.state === "finished") return "reveal";
  return "remove";
}
