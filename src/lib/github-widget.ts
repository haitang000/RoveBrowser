import { validGithubUsername } from "./new-tab";

export interface Contribution { date: string; count: number; level: number }
const cache = new Map<string, { time: number; days: Contribution[] }>();
/** Public contribution counts only; no GitHub credentials are needed. */
export async function githubContributions(username: string, signal: AbortSignal): Promise<Contribution[]> {
  if (!validGithubUsername(username)) throw Error("请输入有效的 GitHub 用户名");
  const key = username.toLowerCase();
  const saved = cache.get(key);
  if (saved && Date.now() - saved.time < 3_600_000) return saved.days;
  const response = await fetch(`https://github-contributions-api.jogruber.de/v4/${encodeURIComponent(username)}?y=last`, {
    signal, credentials: "omit", referrerPolicy: "no-referrer",
  });
  if (!response.ok) throw Error(response.status === 404 ? "未找到这个 GitHub 用户" : "暂时无法加载贡献图");
  const body = await response.json();
  if (!body || !Array.isArray(body.contributions) || !body.contributions.length) throw Error("暂无可用的贡献记录");
  const days: Contribution[] = body.contributions.filter((item: Contribution | null) =>
    item && /^\d{4}-\d{2}-\d{2}$/.test(item.date) && Number.isInteger(item.count) && item.count >= 0 && Number.isInteger(item.level) && item.level >= 0 && item.level <= 4,
  ).sort((a: Contribution, b: Contribution) => a.date.localeCompare(b.date)).slice(-371);
  if (!days.length) throw Error("暂无可用的贡献记录");
  if (cache.size >= 16) cache.delete(cache.keys().next().value!);
  cache.set(key, { time: Date.now(), days });
  return days;
}
