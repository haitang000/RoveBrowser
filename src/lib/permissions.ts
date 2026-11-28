import type { Rect } from "./types";

export const permissions = {
  camera: { name: "摄像头", question: "允许使用摄像头吗？", icon: "camera" },
  microphone: { name: "麦克风", question: "允许使用麦克风吗？", icon: "microphone" },
  location: { name: "位置", question: "允许访问你的位置吗？", icon: "location" },
  notifications: { name: "通知", question: "允许显示通知吗？", icon: "bell" },
  screen: { name: "屏幕共享", question: "允许共享屏幕吗？", icon: "screen" },
} as const;

export type PermissionKind = keyof typeof permissions;
export type PermissionDefault = "ask" | "block";
export const permissionKinds = Object.keys(permissions) as PermissionKind[];
export const sitePermissionOrder = ["location", "camera", "microphone", "notifications", "screen"] as const;

export const permissionBehavior = {
  location: { ask: "网站可以请求访问你的位置", block: "阻止网站访问你的位置" },
  camera: { ask: "网站可以请求使用摄像头", block: "阻止网站使用摄像头" },
  microphone: { ask: "网站可以请求使用麦克风", block: "阻止网站使用麦克风" },
  notifications: { ask: "网站可以请求发送通知", block: "阻止网站发送通知" },
  screen: { ask: "网站可以请求共享屏幕", block: "阻止网站共享屏幕" },
} as const satisfies Record<PermissionKind, Record<PermissionDefault, string>>;

export interface PermissionScope {
  page: string;
  requester: string;
  kind: PermissionKind;
}

export type PermissionRule = [scope: PermissionScope, allowed: boolean];

export function samePermissionScope(a: PermissionScope, b: PermissionScope): boolean {
  return a.kind === b.kind && a.requester === b.requester && a.page === b.page;
}

export function permissionOrigin(input: string): string | undefined {
  const address = input.trim();
  if (!address || /[\s\\*]/.test(address)) return;
  try {
    const url = new URL(address.includes("://") ? address : `https://${address}`);
    if ((url.protocol === "https:" || url.protocol === "http:") && !url.username && !url.password) {
      return url.origin;
    }
  } catch {
    return;
  }
}

export interface PermissionPrompt {
  id: string;
  scope: PermissionScope;
  canRemember: boolean;
  remember: boolean;
  pane: Rect;
  waiting: number;
}
