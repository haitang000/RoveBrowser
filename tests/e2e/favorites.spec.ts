import { test, expect, type Page } from "@playwright/test";
import { initialState } from "../../src/lib/preview";

const icon = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGNoSXrxHwAGCgLO2e10KwAAAABJRU5ErkJggg==";
async function removeLast(page: Page) {
  await page.locator('.favorite-open').last().click({ button: 'right' });
  await page.getByRole('menuitem', { name: '移除常用网站', exact: true }).click();
}

test.beforeEach(async ({ page }) => {
  await page.route('**/favicon.ico', route => route.abort());
  await page.goto('/');
});

test('removes the manager and heading, caps additions at eight and allows reordering', async ({ page }) => {
  await expect(page.locator('.favorites-heading, .favorites-panel')).toHaveCount(0);
  await expect(page.getByRole('button', { name: '管理顶部常用网站', exact: true })).toHaveCount(0);
  await expect(page.locator('.favorite-tile')).toHaveCount(8);
  await page.getByRole('tab', { name: 'Wikipedia' }).click({ button: 'right' });
  await expect(page.getByRole('menuitem', { name: '添加到顶部常用网站', exact: true })).toBeDisabled();
  await page.keyboard.press('Escape');
  await page.getByRole('tab', { name: 'Wikipedia' }).dragTo(page.locator('.favorite-tile').first());
  await expect(page.locator('.error-toast')).toContainText('最多置顶 8 个网站');
  await expect(page.locator('.favorite-tile')).toHaveCount(8);
  await expect(page.getByRole('tab', { name: 'Wikipedia' })).toBeVisible();
  await page.locator('.favorite-tile').last().dragTo(page.locator('.favorite-tile').first());
  await expect(page.locator('.favorite-open').first()).toHaveAttribute('aria-label', 'Linear');
  await page.getByRole('button', { name: '工作区与外观', exact: true }).click();
  await expect(page.getByRole('button', { name: '管理顶部常用网站', exact: true })).toHaveCount(0);
});

test('context editing and tab promotion work without the manager', async ({ page }) => {
  await removeLast(page);
  await page.getByRole('tab', { name: 'Wikipedia' }).click({ button: 'right' });
  await page.getByRole('menuitem', { name: '添加到顶部常用网站', exact: true }).click();
  const favorite = page.locator('.favorites').getByRole('button', { name: 'Wikipedia', exact: true });
  await expect(favorite).toBeVisible();
  await favorite.click({ button: 'right' });
  await page.getByRole('menuitem', { name: '编辑常用网站', exact: true }).click();
  await page.getByRole('textbox', { name: '置顶网站名称' }).fill('我的百科');
  await page.getByRole('textbox', { name: '置顶网站网址' }).fill('https://example.com/guide');
  await page.getByRole('button', { name: '保存修改', exact: true }).click();
  const edited = page.locator('.favorites').getByRole('button', { name: '我的百科', exact: true });
  await expect(edited).toBeVisible();
  await page.getByRole('button', { name: '工作区 Work', exact: true }).click();
  await edited.click();
  await expect(edited).toHaveClass(/chosen/);
  await expect(page.getByRole('heading', { name: 'Work', exact: true })).toBeVisible();
  await expect(page.getByRole('dialog', { name: '顶部常用网站', exact: true })).toHaveCount(0);
});

test('saved favicon pixels render offline while unloaded and after reopening', async ({ page }) => {
  const state = initialState();
  state.favorites[0].favicon = icon;
  state.pins![1].favicon = icon;
  await page.evaluate(state => window.__ERA_RECEIVE__?.({ type: 'state', state }), state);
  const tile = page.locator('[data-bookmark-id="favorite-github"]');
  const pin = page.locator('[data-bookmark-id="pin-wiki"]');
  for (const entry of [tile, pin]) {
    await expect(entry.locator('img')).toHaveAttribute('src', icon);
    expect(await entry.locator('img').evaluate((img: HTMLImageElement) => img.complete && img.naturalWidth > 0)).toBe(true);
    await entry.click();
    await entry.click({ button: 'middle' });
    await expect(entry.locator('img')).toHaveAttribute('src', icon);
  }
});

test('empty favorites can be filled using an ordinary tab', async ({ page }) => {
  for (let i = 0; i < 8; i++) await removeLast(page);
  await page.getByRole('button', { name: '打开新标签页以添加置顶网站', exact: true }).click();
  await page.getByRole('combobox').fill('https://example.com');
  await page.keyboard.press('Enter');
  await page.getByRole('tab', { name: /^example.com/ }).click({ button: 'right' });
  await page.getByRole('menuitem', { name: '添加到顶部常用网站', exact: true }).click();
  await expect(page.locator('.favorite-tile')).toHaveCount(1);
});
