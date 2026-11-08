import { test, expect } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.route("**/favicon.ico", route => route.abort());
  await page.goto("/");
});

test("a pin keeps its name and home, unloads without disappearing, and reopens fresh", async ({ page }) => {
  let pin = page.locator('.pinned-tabs [data-bookmark-id="pin-wiki"]');
  await pin.click();
  await page.keyboard.press("Control+l");
  await page.getByRole("combobox").fill("https://en.wikipedia.org/wiki/Browser");
  await page.keyboard.press("Enter");
  await expect(pin.locator('.tab-title')).toHaveText("Wikipedia");
  await expect(pin).toHaveAttribute("title", /\/wiki\/Browser$/);
  await pin.getByRole("button", { name: "返回固定网址 Wikipedia", exact: true }).click();
  await expect(pin).toHaveAttribute("title", "https://en.wikipedia.org");
  const live = await pin.getAttribute("data-tab-id");
  await pin.click({ button: "middle" });
  await expect(pin).toHaveClass(/unloaded/);
  await expect(pin).not.toHaveAttribute("aria-selected", "true");
  await pin.click();
  await expect(pin).not.toHaveClass(/unloaded/);
  await expect(pin).toHaveAttribute("aria-selected", "true");
  expect(await pin.getAttribute("data-tab-id")).not.toBe(live);
});

test("editing a pin changes its saved item without navigating the open page", async ({ page }) => {
  const pin = page.locator('.pinned-tabs [data-bookmark-id="pin-wiki"]');
  await pin.click({ button: "right" });
  await page.getByRole("menuitem", { name: "编辑固定标签", exact: true }).click();
  await page.getByRole("textbox", { name: "固定标签名称" }).fill("我的百科");
  await page.getByRole("textbox", { name: "固定标签网址" }).fill("https://example.com/home");
  await page.getByRole("button", { name: "保存修改", exact: true }).click();
  await expect(pin.locator('.tab-title')).toHaveText("我的百科");
  await expect(pin).toHaveAttribute("title", "https://en.wikipedia.org");
  await pin.click({ button: "middle" });
  await pin.click();
  await expect(pin).toHaveAttribute("title", "https://example.com/home");
});

test("dragging between tabs, favorites and pins converts the saved item", async ({ page }) => {
  await page.locator('.favorite-open').last().click({ button: 'right' });
  await page.getByRole('menuitem', { name: '移除常用网站', exact: true }).click();
  await page.keyboard.press("Control+t");
  await page.getByRole("combobox").fill("https://example.com/docs");
  await page.keyboard.press("Enter");
  const row = page.getByRole("tab", { name: "example.com" });
  const id = await row.getAttribute("data-tab-id");
  await row.dragTo(page.locator('.favorite-tile').first());
  await expect(row).toHaveCount(0);
  // The outgoing row is inert before its leave animation removes the native drag source.
  await expect(page.locator(`[data-tab-id="${id}"]`)).toHaveCount(0);
  const favorite = page.locator('.favorite-tile').filter({ has: page.getByRole("button", { name: "example.com", exact: true }) });
  await expect(favorite).toBeVisible();
  await favorite.dragTo(page.locator('.pinned-tabs').getByRole('tab').first());
  const pinned = page.locator(`.pinned-tabs [data-tab-id="${id}"]`);
  await expect(pinned).toBeVisible();
  await expect(favorite).toHaveCount(0);
  await pinned.dragTo(page.locator('[data-tab-id="start"]'));
  await expect(page.locator(`[aria-label="标签页"] [data-tab-id="${id}"]`)).toBeVisible();
});

test("favorite close unloads its page, while removal returns the open page to ordinary tabs", async ({ page }) => {
  const favorite = page.locator('.favorites').getByRole('button', { name: 'Figma', exact: true });
  await favorite.click();
  await expect(favorite).toHaveClass(/chosen/);
  await page.locator('.favorites').getByRole('button', { name: '卸载 Figma', exact: true }).click({ force: true });
  await expect(favorite).toBeVisible();
  await expect(page.locator('.favorites').getByRole('button', { name: '卸载 Figma', exact: true })).toHaveCount(0);
  await favorite.click();
  await favorite.click({ button: "right" });
  await page.getByRole('menuitem', { name: '移除常用网站', exact: true }).click();
  await expect(favorite).toHaveCount(0);
  await expect(page.locator('[aria-label="标签页"] .tab-row.selected')).toContainText('Figma');
});

test("a saved split reopens a closed half and unloads together from its sidebar entry", async ({ page }) => {
  for (const path of ['a', 'b']) {
    await page.keyboard.press('Control+t');
    await page.getByRole('combobox').fill(`https://example.com/${path}`);
    await page.keyboard.press('Enter');
  }
  await page.locator('[aria-label="标签页"] .tab-row[title="https://example.com/a"]').click({ button: 'right' });
  await page.getByRole('menuitem', { name: '在当前网页旁分屏', exact: true }).click();
  await page.getByRole('button', { name: '固定分屏组合', exact: true }).click();
  await page.keyboard.press('Control+w');
  const group = page.locator('.pinned-tabs .tab-row').filter({ hasText: 'example.com' });
  await expect(group.filter({ has: page.locator('.tab-close:disabled') })).toHaveCount(1);
  await group.filter({ has: page.locator('.tab-close:not(:disabled)') }).click();
  await expect(group.locator('.tab-close:disabled')).toHaveCount(0);
  await expect(page.locator('.page-pane')).toHaveCount(2);
  await group.first().click({ button: 'middle' });
  await expect(group.locator('.tab-close:disabled')).toHaveCount(2);
  await expect(page.locator('.page-pane')).toHaveCount(1);
});
