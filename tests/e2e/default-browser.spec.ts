import { expect, test } from "@playwright/test";

test("default browser status distinguishes partial associations and invokes the native settings action", async ({ page }) => {
  await page.addInitScript(() => {
    const w = window as any;
    w.__ERA_NATIVE__ = true;
    w.__ERA_INITIAL_STATE__ = {
      spaces: [{ id: "personal", name: "Personal", color: "sky" }], activeSpace: "personal", activeTab: "settings",
      tabs: [{ id: "settings", spaceId: "personal", title: "设置", url: "rove://settings/general", pinned: false }],
      favorites: [], pins: [], theme: "light", sidebarCollapsed: false, borderless: true, closed: [],
    };
    w.__commands = [];
    w.ipc = { postMessage(text: string) { w.__commands.push(JSON.parse(text)); } };
  });
  await page.goto("/");
  await page.getByRole("button", { name: "设为默认浏览器", exact: true }).click();
  expect(await page.evaluate(() => (window as any).__commands.some((c: any) => c.type === "systemSettings"))).toBe(true);
  await page.evaluate(() => window.__ERA_RECEIVE__?.({ type: "defaultBrowser", defaultBrowser: { http: false, https: false } }));
  await expect(page.getByText("Rove 尚未设为默认浏览器。在 Windows 设置中选择 Rove。")).toBeVisible();
  await page.evaluate(() => window.__ERA_RECEIVE__?.({ type: "defaultBrowser", defaultBrowser: { http: true, https: false } }));
  await expect(page.getByText(/部分网页链接仍由其他浏览器打开/)).toBeVisible();
  await page.evaluate(() => window.__ERA_RECEIVE__?.({ type: "defaultBrowser", defaultBrowser: { http: true, https: true } }));
  await expect(page.getByText("Rove 已是 HTTP 和 HTTPS 链接的默认浏览器。")).toBeVisible();
});

test("standalone preview explains that default-browser changes need the desktop app", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "设置", exact: true }).click();
  await expect(page.getByRole("button", { name: "设为默认浏览器", exact: true })).toBeDisabled();
  await expect(page.getByText("请在 Rove 桌面版中设置默认浏览器。").first()).toBeVisible();
});
