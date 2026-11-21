import type { Rect } from "./types";

export const permissions = {
  camera: { name: "摄像头", question: "允许使用摄像头吗？", icon: "camera" },
  microphone: { name: "麦克风", question: "允许使用麦克风吗？", icon: "microphone" },
  location: { name: "位置", question: "允许访问你的位置吗？", icon: "location" },
  notifications: { name: "通知", question: "允许显示通知吗？", icon: "bell" },
  screen: { name: "屏幕共享", question: "允许共享屏幕吗？", icon: "screen" },
} as const;

export interface PermissionScope {
  page: string;
  requester: string;
  kind: keyof typeof permissions;
}

export type PermissionRule = [scope: PermissionScope, allowed: boolean];

export interface PermissionPrompt {
  id: string;
  scope: PermissionScope;
  canRemember: boolean;
  remember: boolean;
  pane: Rect;
  waiting: number;
}
