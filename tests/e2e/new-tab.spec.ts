import { expect, test } from "@playwright/test";
test.use({ timezoneId: "America/New_York" });

test.beforeEach(async ({ page }) => {
  await page.route("**/favicon.ico", (route) => route.abort());
  await page.clock.install({ time: new Date("2026-11-14T17:47:00Z") });
  await page.goto("/");
});

test("new tab follows the reference layout and opens a suggested site", async ({ page }) => {
  await expect(page.locator(".nt-greeting")).toContainText("Good afternoon,");
  await expect(page.locator(".nt-clock time")).toHaveText("12:47");
  await expect(page.locator(".nt-clock > span")).toHaveText("2026-11-14");
  await expect(page.locator(".nt-suggestion")).toHaveCount(5);
  await expect(page.locator(".nt-widget-empty")).toHaveCount(2);
  await page.setViewportSize({ width: 1826, height: 1344 });
  await page.keyboard.press("Control+b");
  await expect(page.locator(".sidebar-track")).toHaveCSS("width", "0px");
  await page.locator(".new-tab-page").screenshot({ path: "test-results/new-tab-home.png" });
  await page.locator(".nt-suggestion").first().click();
  await expect(page.locator(".preview-page")).toContainText("https://github.com/");
});

test("discover adds, resizes, sorts and removes widgets and preserves notes on reload", async ({ page }) => {
  await page.getByRole("button", { name: "编辑小组件", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Discover" })).toBeFocused();
  await page.getByRole("button", { name: "添加日历", exact: true }).click();
  await page.getByRole("button", { name: "添加便签", exact: true }).click();
  await page.getByRole("textbox", { name: "便签内容" }).fill("Remember to explore.\n记得出发。");
  await page.getByRole("textbox", { name: "首页称呼" }).fill("Computer32");
  await page.getByRole("button", { name: "前移便签", exact: true }).click();
  await expect.poll(() => page.locator("article.nt-widget").evaluateAll((items) => items.map((item) => item.getAttribute("aria-label")))).toEqual(["时钟", "便签", "日历"]);
  await page.getByRole("button", { name: "调整便签宽度", exact: true }).click();
  await expect(page.locator('.nt-widget[aria-label="便签"]')).toHaveClass(/nt-widget-wide/);
  await page.getByRole("button", { name: "添加时钟", exact: true }).click();
  await page.getByRole("button", { name: "移除时钟", exact: true }).last().click();
  await page.getByRole("button", { name: "完成编辑", exact: true }).click();
  await expect(page.getByRole("button", { name: "编辑小组件", exact: true })).toBeFocused();
  await expect(page.locator(".nt-greeting")).toContainText("Computer32.");
  await page.reload();
  await expect(page.getByRole("textbox", { name: "便签内容" })).toHaveValue("Remember to explore.\n记得出发。");
  await expect(page.locator(".nt-greeting")).toContainText("Computer32.");
  await expect.poll(() => page.locator("article.nt-widget").evaluateAll((items) => items.map((item) => item.getAttribute("aria-label")))).toEqual(["时钟", "便签", "日历"]);
  await page.getByRole("button", { name: "编辑小组件", exact: true }).click();
  await page.setViewportSize({ width: 1826, height: 1344 });
  await page.locator(".new-tab-page").screenshot({ path: "test-results/new-tab-discover.png" });
});

test("GitHub contribution widget validates usernames, retries failures and renders real response data", async ({ page }) => {
  let requests = 0;
  await page.route("https://github-contributions-api.jogruber.de/**", async (route) => {
    requests++;
    if (requests === 1) { await route.fulfill({ status: 503, body: "unavailable" }); return; }
    await route.fulfill({ json: { contributions: Array.from({ length: 119 }, (_, index) => ({ date: new Date(Date.UTC(2026, 6, 19 + index)).toISOString().slice(0, 10), count: 2, level: 2 })) } });
  });
  await page.getByRole("button", { name: "编辑小组件", exact: true }).click();
  await page.getByRole("button", { name: "添加GitHub 贡献图", exact: true }).click();
  await page.getByRole("button", { name: "完成编辑", exact: true }).click();
  const username = page.getByRole("textbox", { name: "你的 GitHub 用户名" });
  await username.fill("bad/name");
  await page.getByRole("button", { name: "显示贡献图", exact: true }).click();
  await expect(username).toHaveAttribute("aria-invalid", "true");
  expect(requests).toBe(0);
  await username.fill("octocat");
  await page.getByRole("button", { name: "显示贡献图", exact: true }).click();
  await expect(page.locator(".nt-widget-message")).toContainText("暂时无法加载贡献图");
  await page.getByRole("button", { name: "重试", exact: true }).click();
  await expect(page.locator(".nt-contribution-summary")).toContainText("238");
  await expect(page.locator(".nt-contribution-grid [data-level='2']")).toHaveCount(119);
  await page.reload();
  await expect(page.locator(".nt-github-user")).toHaveText("@octocat");
  await expect(page.locator(".nt-contribution-summary")).toContainText("238");
  await page.getByRole("button", { name: "编辑小组件", exact: true }).click();
  await page.getByRole("button", { name: "添加日历", exact: true }).click();
  await page.getByRole("button", { name: "添加便签", exact: true }).click();
  await page.getByRole("button", { name: "完成编辑", exact: true }).click();
  expect(await page.locator(".new-tab-page").evaluate((element) => element.scrollWidth > element.clientWidth)).toBe(false);
});

test("drag sorting, empty layout and widget limit remain usable", async ({ page }) => {
  await page.getByRole("button", { name: "编辑小组件", exact: true }).click();
  await page.getByRole("button", { name: "添加便签", exact: true }).click();
  await page.getByRole("button", { name: "拖动便签排序", exact: true }).dragTo(page.locator('.nt-widget[aria-label="时钟"]'));
  await expect.poll(() => page.locator("article.nt-widget").evaluateAll((items) => items.map((item) => item.getAttribute("aria-label")))).toEqual(["便签", "时钟"]);
  await page.getByRole("button", { name: "移除便签", exact: true }).click();
  await page.getByRole("button", { name: "移除时钟", exact: true }).click();
  await page.getByRole("button", { name: "完成编辑", exact: true }).click();
  await page.reload();
  await expect(page.locator(".nt-widget[aria-label]:not(button)")).toHaveCount(0);
  await page.getByRole("button", { name: "编辑小组件", exact: true }).click();
  for (let i = 0; i < 8; i++) await page.getByRole("button", { name: "添加时钟", exact: true }).click();
  await expect(page.locator(".nt-add-button:disabled")).toHaveCount(4);
  await expect(page.locator(".nt-limit")).toContainText("最多添加 8 个小组件");
});

test("home and discover adapt to narrow panes and dark appearance", async ({ page }) => {
  for (const width of [480, 760, 1280]) {
    await page.setViewportSize({ width, height: 820 });
    const overflow = await page.locator(".new-tab-page").evaluate((element) => element.scrollWidth > element.clientWidth);
    expect(overflow).toBe(false);
    await page.getByRole("button", { name: "编辑小组件", exact: true }).click();
    expect(await page.locator(".new-tab-page").evaluate((element) => element.scrollWidth > element.clientWidth)).toBe(false);
    await page.getByRole("button", { name: "完成编辑", exact: true }).click();
  }
  await page.getByRole("button", { name: "工作区与外观" }).click();
  await page.getByRole("button", { name: "切换深色模式" }).click();
  await page.getByRole("button", { name: "关闭设置", exact: true }).click();
  await expect(page.locator(".new-tab-page")).toHaveCSS("background-color", "rgb(30, 30, 32)");
  await page.screenshot({ path: "test-results/new-tab-dark.png" });
});

test("only widgets scroll while the greeting, suggestions and section header stay fixed", async ({ page }) => {
  await page.evaluate(() => localStorage.setItem("rove.new-tab.v1", JSON.stringify({
    displayName: "Computer32",
    widgets: Array.from({ length: 8 }, (_, index) => ({ id: `clock-${index}`, kind: "clock", size: "wide" })),
  })));
  await page.reload();
  for (const width of [1280, 760]) {
    await page.setViewportSize({ width, height: 820 });
    const greeting = await page.locator(".nt-greeting").boundingBox();
    const suggestions = await page.locator(".nt-suggestions").boundingBox();
    const header = await page.locator(".nt-section-header").boundingBox();
    const grid = page.locator(".nt-widget-grid");
    await grid.evaluate((element) => { element.scrollTop = 0; });
    const box = (await grid.boundingBox())!;
    expect(box.height).toBeGreaterThan(100);
    await page.mouse.move(box.x + box.width / 2, box.y + Math.min(70, box.height / 2));
    await page.mouse.wheel(0, 600);
    await expect.poll(() => grid.evaluate((element) => element.scrollTop)).toBeGreaterThan(100);
    expect(await page.locator(".nt-greeting").boundingBox()).toEqual(greeting);
    expect(await page.locator(".nt-suggestions").boundingBox()).toEqual(suggestions);
    expect(await page.locator(".nt-section-header").boundingBox()).toEqual(header);
    expect(await page.locator(".new-tab-page").evaluate((element) => element.scrollTop)).toBe(0);
    await page.locator(".nt-suggestions").hover();
    await page.mouse.wheel(0, 600);
    expect(await page.locator(".nt-suggestions").boundingBox()).toEqual(suggestions);
    expect(await page.locator(".new-tab-page").evaluate((element) => element.scrollTop)).toBe(0);
  }
});
