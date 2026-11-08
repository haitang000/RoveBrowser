import type { Favorite } from "./types";

export const MAX_FAVORITES = 8;

export function favoriteUrl(input: string): string {
  const value = input.trim();
  if (!value || value.length > 2048) throw Error("请输入有效网址（最多 2048 个字符）");
  try {
    const url = new URL(value.includes("://") ? value : `https://${value}`);
    if (
      !["http:", "https:"].includes(url.protocol) ||
      !url.hostname || url.username || url.password
    ) throw Error();
    return url.href;
  } catch {
    throw Error("请输入 HTTP 或 HTTPS 网址，例如 example.com");
  }
}

export function favorite(title: string, input: string): Pick<Favorite, "title" | "url"> {
  const url = favoriteUrl(input);
  const name = title.trim() || new URL(url).hostname;
  if (Array.from(name).length > 80) throw Error("网站名称最多 80 个字符");
  return { title: name, url };
}

export function sameFavoriteUrl(a: string, b: string): boolean {
  try {
    return favoriteUrl(a) === favoriteUrl(b);
  } catch {
    return a === b;
  }
}
