import { test, expect } from "@playwright/test";
import { initialState } from "../../src/lib/preview";

test.beforeEach(async ({ page }) => {
  // These tests use explicit snapshot fixtures and favicon fallbacks, without network timing.
  await page.route("**/favicon.ico", route => route.abort());
  await page.goto("/", { waitUntil: "domcontentloaded" });
});
const preview = "#tab-hover-preview";

test("short hovers cancel; after opening, another row updates immediately; leaving dismisses", async ({ page }) => {
  const wiki = page.locator('[data-bookmark-id="pin-wiki"]');
  const github = page.locator('.pinned-tabs [data-bookmark-id="pin-github"]');
  await wiki.hover();
  await page.waitForTimeout(150);
  await expect(page.locator(preview)).toHaveCount(0);
  await page.mouse.move(900, 600);
  await page.waitForTimeout(650);
  await expect(page.locator(preview)).toHaveCount(0);
  await wiki.hover();
  await expect(page.locator(preview)).toHaveAttribute("data-preview-id", "pin-wiki");
  await expect(page.locator(preview)).toContainText("Wikipedia");
  await github.hover();
  await expect(page.locator(preview)).toHaveAttribute("data-preview-id", "pin-github", { timeout: 400 });
  await page.mouse.move(900, 600);
  await expect(page.locator(preview)).toHaveCount(0);
});

test("active pages, overlays, pointer presses, wheel and Escape dismiss previews", async ({ page }) => {
  const wiki = page.locator('[data-bookmark-id="pin-wiki"]');
  await wiki.click();
  await page.waitForTimeout(700);
  await expect(page.locator(preview)).toHaveCount(0);
  const github = page.locator('.pinned-tabs [data-bookmark-id="pin-github"]');
  for (const action of ["Escape", "Control+l", "wheel", "click"]) {
    await page.mouse.move(900, 600);
    await github.hover();
    await expect(page.locator(preview)).toBeVisible();
    if (action === "wheel") await page.mouse.wheel(0, 50);
    else if (action === "click") await github.click();
    else await page.keyboard.press(action);
    await expect(page.locator(preview)).toHaveCount(0);
    if (action === "Control+l") await page.keyboard.press("Escape");
  }
});

test("unloaded favorites remain unloaded; snapshots update while a split is previewed", async ({ page }) => {
  const figma = page.locator('.favorite-tile[data-bookmark-id="favorite-figma"]');
  const before = await page.getByRole("tab").count();
  await figma.hover();
  await expect(page.locator(preview)).toContainText("Figma");
  await expect(page.locator(preview)).toContainText("界面预览");
  await expect(figma.locator('.favorite-close')).toHaveCount(0);
  expect(await page.getByRole("tab").count()).toBe(before);
  await page.mouse.move(900, 600);
  const ids: string[] = [];
  for (const path of ["preview-a", "preview-b"]) {
    await page.keyboard.press("Control+t");
    await page.getByRole("combobox").fill(`https://example.com/${path}`);
    await page.keyboard.press("Enter");
    ids.push((await page.getByRole("tab", { selected: true }).getAttribute("data-tab-id"))!);
  }
  await page.locator(`[data-tab-id="${ids[0]}"]`).click({ button: "right" });
  await page.getByRole("menuitem", { name: "在当前网页旁分屏", exact: true }).click();
  await page.locator('[data-tab-id="start"]').click();
  await page.locator(`[data-tab-id="${ids[0]}"]`).hover();
  await expect(page.locator(`${preview} .tab-preview-caption`)).toHaveCount(2);
  await expect(page.locator(`${preview} .tab-preview-empty`)).toHaveCount(2);
  await page.evaluate((ids) => {
    const image = 'data:image/svg+xml,' + encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" width="480" height="320"><rect width="480" height="320" fill="teal"/></svg>');
    window.__ERA_RECEIVE__!({ type: 'thumbnails', images: Object.fromEntries(ids.map(id => [id, image])) });
  }, ids);
  await expect(page.locator(`${preview} img`)).toHaveCount(2);
  await page.keyboard.press("Escape");
});

test("preview stays inside a narrow viewport and works with reduced motion and dark theme", async ({ page }) => {
  await page.setViewportSize({ width: 480, height: 400 });
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.getByRole('button', { name: '工作区与外观', exact: true }).click();
  await page.getByRole('button', { name: '切换深色模式', exact: true }).click();
  await page.getByRole('button', { name: '关闭设置', exact: true }).click();
  await page.locator('.pinned-tabs [data-bookmark-id="pin-wiki"]').hover();
  await expect(page.locator(preview)).toBeVisible();
  const rect = await page.locator(preview).boundingBox();
  expect(rect!.x + rect!.width).toBeLessThanOrEqual(480 - 15);
  expect(rect!.y).toBeGreaterThanOrEqual(48);
  expect(rect!.y + rect!.height).toBeLessThanOrEqual(400 - 15);
});

test("native handoff keeps the card until its page region is restored, ignoring old acknowledgments", async ({ page }) => {
  await page.addInitScript(() => {
    const messages: { type: string; rect?: unknown; ticket?: number }[] = [];
    Object.assign(window, { __ERA_NATIVE__: true, __previewMessages: messages, ipc: { postMessage(text: string) {
      const command = JSON.parse(text); messages.push(command);
      if ((command.type === 'tabPreview' && command.rect) || command.type === 'tabPreviewClose') queueMicrotask(() => window.__ERA_RECEIVE__?.({ type: 'tabPreviewBackdrop', ticket: command.ticket, backgrounds: [] }));
      if (command.type === 'tabPreviewReady') queueMicrotask(() => window.__ERA_RECEIVE__?.({ type: 'tabPreviewPresented', ticket: command.ticket }));
    } } });
  });
  await page.reload({ waitUntil: "domcontentloaded" });
  await page.locator('.pinned-tabs [data-bookmark-id="pin-wiki"]').hover();
  await expect(page.locator(preview)).toBeVisible();
  await expect(page.locator(preview)).toHaveAttribute('data-phase', 'open');
  await page.mouse.move(900, 600);
  await expect(page.locator(preview)).toHaveAttribute('data-closing', 'true');
  await expect.poll(() => page.evaluate(() => (window as unknown as { __previewMessages: { type: string; rect?: unknown }[] }).__previewMessages.some(m => m.type === 'tabPreview' && !m.rect))).toBe(true);
  const ticket = await page.evaluate(() => {
    const messages = (window as unknown as { __previewMessages: { type: string; rect?: unknown; ticket: number }[] }).__previewMessages;
    return messages.filter(m => m.type === 'tabPreview' && !m.rect).at(-1)!.ticket;
  });
  await page.evaluate(ticket => window.__ERA_RECEIVE__!({ type: 'tabPreviewHidden', ticket: ticket - 1 }), ticket);
  await expect(page.locator(preview)).toBeVisible();
  await page.evaluate(ticket => window.__ERA_RECEIVE__!({ type: 'tabPreviewHidden', ticket }), ticket);
  await expect(page.locator(preview)).toHaveCount(0);
});

test("entry and exit fade reverse from the displayed opacity", async ({ page }) => {
  await page.evaluate(() => {
    const animate = Element.prototype.animate;
    Element.prototype.animate = function (...args) {
      const result = animate.apply(this, args);
      if (this.id === 'tab-hover-preview') result.pause();
      return result;
    };
  });
  const row = page.locator('.pinned-tabs [data-bookmark-id="pin-wiki"]');
  const card = page.locator(preview);
  await row.hover();
  await expect(card).toHaveAttribute('data-phase', 'opening');
  const entry = await card.evaluate(e => {
    const a = e.getAnimations()[0]; a.currentTime = 80;
    return { value: Number(getComputedStyle(e).opacity), duration: a.effect!.getTiming().duration };
  });
  expect(entry.duration).toBe(160);
  expect(entry.value).toBeGreaterThan(0);
  expect(entry.value).toBeLessThan(1);
  await card.evaluate(e => e.getAnimations()[0].finish());
  await expect(card).toHaveAttribute('data-phase', 'open');
  await page.mouse.move(900, 600);
  await expect(card).toHaveAttribute('data-phase', 'closing');
  const leaving = await card.evaluate(e => {
    const a = e.getAnimations()[0]; a.currentTime = 60;
    return Number(getComputedStyle(e).opacity);
  });
  expect(leaving).toBeGreaterThan(0);
  expect(leaving).toBeLessThan(1);
  await row.hover();
  await expect(card).toHaveAttribute('data-phase', 'opening');
  const resumed = await card.evaluate(e => Number((e.getAnimations()[0].effect as KeyframeEffect).getKeyframes()[0].opacity));
  expect(resumed).toBeCloseTo(leaving, 2);
  await card.evaluate(e => e.getAnimations()[0].finish());
  await expect(card).toHaveAttribute('data-phase', 'open');
});

test("reduced motion keeps fades to 80ms without moving the card", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.evaluate(() => {
    const animate = Element.prototype.animate;
    Element.prototype.animate = function (...args) {
      const result = animate.apply(this, args);
      if (this.id === 'tab-hover-preview') result.pause();
      return result;
    };
  });
  await page.locator('.pinned-tabs [data-bookmark-id="pin-wiki"]').hover();
  const card = page.locator(preview);
  await expect(card).toHaveAttribute('data-phase', 'opening');
  expect(await card.evaluate(e => e.getAnimations()[0].effect!.getTiming().duration)).toBe(80);
  expect(await card.evaluate(e => getComputedStyle(e).transform)).toBe('none');
  await card.evaluate(e => e.getAnimations()[0].finish());
  await expect(card).toHaveAttribute('data-phase', 'open');
  await page.mouse.move(900, 600);
  await expect(card).toHaveAttribute('data-phase', 'closing');
  expect(await card.evaluate(e => e.getAnimations()[0].effect!.getTiming().duration)).toBe(80);
  await card.evaluate(e => e.getAnimations()[0].finish());
  await expect(card).toHaveCount(0);
});

test("saved preview cache survives state broadcasts and expires when its URL changes", async ({ page }) => {
  const state = initialState();
  const image = 'data:image/svg+xml,' + encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" width="320" height="200"><rect width="320" height="200" fill="teal"/></svg>');
  await page.evaluate(({ state, image }) => {
    window.__ERA_RECEIVE__!({ type: 'state', state });
    window.__ERA_RECEIVE__!({ type: 'thumbnails', images: { 'favorite-figma': image } });
    window.__ERA_RECEIVE__!({ type: 'state', state: JSON.parse(JSON.stringify(state)) });
  }, { state, image });
  const row = page.locator('.favorite-tile[data-bookmark-id="favorite-figma"]');
  await row.hover();
  await expect(page.locator(`${preview} .tab-preview-pane > img`)).toHaveCount(1);
  await page.keyboard.press('Escape');
  await expect(page.locator(preview)).toHaveCount(0);
  state.favorites.find(f => f.id === 'favorite-figma')!.url = 'https://figma.com/updated';
  await page.evaluate(state => window.__ERA_RECEIVE__!({ type: 'state', state }), state);
  await page.mouse.move(900, 600); await row.hover();
  await expect(page.locator(preview)).toContainText('figma.com/updated');
  await expect(page.locator(`${preview} .tab-preview-pane > img`)).toHaveCount(0);
});
