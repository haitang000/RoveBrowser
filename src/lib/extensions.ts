export const CHROME_STORE_URL = "https://chromewebstore.google.com/";
export const MAX_PINNED_EXTENSIONS = 4;

export function chromeStoreDetailId(value?: string) {
  if (!value) return undefined;
  try {
    const url = new URL(value);
    if (url.protocol !== "https:" || url.port || url.username || url.password) return undefined;
    const prefix = url.hostname === "chromewebstore.google.com" ? "/detail/"
      : url.hostname === "chrome.google.com" ? "/webstore/detail/" : "";
    if (!prefix || !url.pathname.startsWith(prefix)) return undefined;
    return url.pathname.slice(prefix.length).match(/^(?:[^/]+\/)?([a-p]{32})\/*$/)?.[1];
  } catch {
    return undefined;
  }
}

type ExtensionSource = "local" | "chromeStore";

export interface ExtensionItem {
  id: string;
  name: string;
  version: string;
  enabled: boolean;
  hasPage: boolean;
  source: ExtensionSource;
  icon?: string;
}

export interface ExtensionReview {
  ticket: string;
  name: string;
  version: string;
  source: ExtensionSource;
  icon?: string;
  permissions: string[];
  hosts: string[];
}

export type ExtensionStep = "download" | "verifySignature" | "unpack" | "copyFiles"
  | "readManifest" | "install" | "remove" | "enable" | "disable";

export interface ExtensionProgress {
  steps: ExtensionStep[];
  current: ExtensionStep;
  percent: number | null;
}

export interface ExtensionsState {
  items: ExtensionItem[] | null;
  review: ExtensionReview | null;
  removal: { ticket: string; item: ExtensionItem } | null;
  popup: boolean;
  busy: boolean;
  progress: ExtensionProgress | null;
  error: string | null;
}
