import { expect, test } from "@playwright/test";

test("command bar floats over the current page and follows its bounds", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("tab", { name: "Wikipedia" }).click();
  const tabs = await page.getByRole("tab").count();
  for (const size of [{ width: 1280, height: 820 }, { width: 480, height: 400 }]) {
    await page.setViewportSize(size);
    await page.keyboard.press("Control+l");
    await expect(page.locator(".command-scrim")).toHaveCSS("opacity", "1");
    await expect(page.getByRole("combobox")).toHaveValue(/^https:\/\/en\.wikipedia\.org\/?$/);
    for (let i = 0; i < 2; i++) {
      const area = await page.locator(".page-area").boundingBox();
      const panel = await page.locator(".command-panel").boundingBox();
      expect(Math.abs(panel!.x + panel!.width / 2 - area!.x - area!.width / 2)).toBeLessThan(1);
      expect(panel!.x).toBeGreaterThan(area!.x);
      expect(panel!.y + panel!.height).toBeLessThan(area!.y + area!.height);
      await page.keyboard.press("Control+b");
      const sidebarWidth = await page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue("--sidebar-width").trim());
      await expect(page.locator(".sidebar-track")).toHaveCSS("width", i ? sidebarWidth : "0px");
    }
    await expect(page.locator(".preview-page h2")).toHaveText("Wikipedia");
    await page.keyboard.press("Escape");
    await expect(page.getByRole("combobox")).toHaveCount(0);
    expect(await page.getByRole("tab").count()).toBe(tabs);
    await expect(page.getByRole("tab", { name: "Wikipedia" })).toHaveAttribute("aria-selected", "true");
  }
});

test("new command stays over the current page and dismisses outside the panel", async ({ page }) => {
  await page.goto("/");
  const entry = page.getByRole("button", { name: "搜索或输入地址", exact: true });
  await entry.click();
  await expect(page.getByRole("combobox")).toBeFocused();
  await page.locator(".command-scrim").click({ position: { x: 8, y: 8 } });
  await expect(page.getByRole("combobox")).toHaveCount(0);
  await expect(entry).toBeFocused();
  const selected = await page.getByRole("tab", { selected: true }).getAttribute("data-tab-id");
  await page.keyboard.press("Control+t");
  await expect(page.getByRole("combobox")).toHaveValue("");
  await entry.click();
  await expect(page.getByRole("combobox")).toBeFocused();
  await expect(page.getByRole("tab", { selected: true })).toHaveAttribute("data-tab-id", selected!);
  await page.getByRole("button", { name: "工作区 Work", exact: true }).click();
  await expect(page.getByRole("combobox")).toHaveCount(0);
  await expect(page.locator(".sidebar-header .space-name")).toHaveText("Work");
});

test("tab searches one site and a named command runs", async ({ page }) => {
  await page.goto("/");
  await page.keyboard.press("Control+t");
  const field = page.getByRole("combobox");
  await field.fill("you");
  await expect(page.getByText("Search YouTube")).toBeVisible();
  await page.keyboard.press("Tab");
  await expect(field).toHaveAttribute("placeholder", "Search YouTube");
  await field.fill("cats");
  await page.keyboard.press("Enter");
  await expect(page.getByRole("tab", { selected: true })).toHaveAttribute("title", /search_query=cats/);
  await page.keyboard.press("Control+t");
  await field.fill("settings");
  await page.keyboard.press("Enter");
  await expect(page.locator(".settings-page")).toBeVisible();
});
