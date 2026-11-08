export function address(input: string): string {
  const value = input.trim();
  if (!value) return "era://newtab";
  if (value === "era://newtab") return value;
  if (/^https?:\/\//i.test(value)) {
    try {
      return new URL(value).href;
    } catch {
      throw new Error("地址格式不正确");
    }
  }
  if (/^[a-z][a-z\d+.-]*:/i.test(value) && !/^localhost:\d/i.test(value))
    throw new Error("仅支持 HTTP 和 HTTPS 网页");
  if (
    !/\s/.test(value) &&
    (/^[\w-]+(\.[\w-]+)+(?::\d+)?(?:[/?#]|$)/.test(value) ||
      /^localhost(?::\d+)?(?:\/|$)/.test(value))
  )
    return new URL(`https://${value}`).href;
  return `https://www.google.com/search?q=${encodeURIComponent(value)}`;
}
export function host(url: string): string {
  try {
    const u = new URL(url);
    return /^https?:$/.test(u.protocol) ? u.hostname.replace(/^www\./, "") : "";
  } catch {
    return "";
  }
}
