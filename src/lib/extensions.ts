export const CHROME_STORE_URL = "https://chromewebstore.google.com/";

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

export interface ExtensionsState {
  items: ExtensionItem[] | null;
  review: ExtensionReview | null;
  popup: boolean;
  busy: boolean;
  error: string | null;
}
