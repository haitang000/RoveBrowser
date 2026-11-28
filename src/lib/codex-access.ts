export interface CodexAccess {
  allowAll: boolean;
  allowlist: string[];
  blocklist: string[];
}

export type CodexSiteList = "allowlist" | "blocklist";
export type CodexAccessChange =
  | { kind: "allowAll"; enabled: boolean }
  | { kind: "site"; list: CodexSiteList; site: string; add: boolean };

export const CODEX_ALLOW_ALL = "允许 Codex 访问所有网站";
export const CODEX_SITE_LISTS = [["allowlist", "白名单"], ["blocklist", "黑名单"]] as const;

function webHostname(address: string): string | undefined {
  try {
    const url = new URL(address);
    if (/^https?:$/.test(url.protocol) && !url.username && !url.password) {
      return url.hostname.replace(/\.$/, "");
    }
  } catch {
    return undefined;
  }
}

function isIp(domain: string): boolean {
  return domain.startsWith("[") || /^\d+(?:\.\d+){3}$/.test(domain);
}

export function codexSiteDomain(input: string): string | undefined {
  const value = input.trim();
  if (!value || value.length > 8192 || /[\\\s\u0000-\u001f\u007f-\u009f]/.test(value)) return;
  if (!value.includes("://") && /[/?#@]/.test(value)) return;
  const domain = webHostname(value.includes("://") ? value : `https://${value}`);
  if (!domain) return;
  if (isIp(domain) || (domain.length <= 253 && domain.split(".").every((label) =>
    /^[a-z0-9_](?:[a-z0-9_-]{0,61}[a-z0-9_])?$/.test(label),
  ))) return domain;
}

export function codexSiteAccess(access: CodexAccess, url: string): "blocked" | "automatic" | "manual" {
  const host = webHostname(url);
  if (!host) return "manual";
  const matches = (site: string) => host === site || (!isIp(host) && host.endsWith(`.${site}`));
  if (access.blocklist.some(matches)) return "blocked";
  return access.allowAll || access.allowlist.some(matches) ? "automatic" : "manual";
}
