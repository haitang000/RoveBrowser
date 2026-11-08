import { test, expect } from "@playwright/test";
import { initialState } from "../../src/lib/preview";
import type { BrowserState, Layout, Rect } from "../../src/lib/types";

declare global {
  interface Window {
    __windowTest: {
      layout?: Layout;
      chrome?: { maximize: Rect; viewportWidth: number };
      presented?: { theme?: string; width: number; fonts: string };
      samples: { expected: number; reported: number }[];
      observer?: ResizeObserver;
    };
  }
}

async function nativeShell(page: import("@playwright/test").Page, state: BrowserState) {
  await page.addInitScript((state) => {
    window.__ERA_NATIVE__ = true;
    window.__ERA_INITIAL_STATE__ = state;
    window.__windowTest = { samples: [] };
    window.ipc = {
      postMessage(message) {
        const command = JSON.parse(message);
        if (command.type === "layout") window.__windowTest.layout = command;
        if (command.type === "windowChrome") window.__windowTest.chrome = command;
        if (command.type === "presented") {
          window.__windowTest.presented = {
            theme: document.documentElement.dataset.theme,
            width: document.querySelector(".sidebar-track")!.getBoundingClientRect().width,
            fonts: document.fonts.status,
          };
        }
      },
    };
  }, state);
  await page.goto("/");
  await expect.poll(() => page.evaluate(() => !!window.__windowTest.presented)).toBe(true);
}

test("first render restores the native theme and collapsed sidebar before showing", async ({ page }) => {
  const state = initialState();
  state.theme = "dark";
  state.sidebarCollapsed = true;
  await nativeShell(page, state);
  expect(await page.evaluate(() => window.__windowTest.presented)).toEqual({
    theme: "dark", width: 0, fonts: "loaded",
  });
  expect(await page.evaluate(() => window.__windowTest.layout?.primary.x))
    .toBe((await page.locator(".page-pane > .webview-slot").first().boundingBox())!.x);
  await expect(page.getByRole("navigation", { name: "网页导航" })).toBeVisible();
  await expect(page.getByRole("button", { name: "搜索或输入地址", exact: true })).toBeVisible();
});

test("native caption geometry keeps three equal buttons flush with the window edge", async ({ page }) => {
  await nativeShell(page, initialState());
  for (const width of [1280, 480, 960]) {
    await page.setViewportSize({ width, height: 820 });
    await expect.poll(() => page.evaluate(() => window.__windowTest.chrome?.viewportWidth)).toBe(width);
    const buttons = await page.locator('.window-controls button').evaluateAll(buttons =>
      buttons.map(button => {
        const { x, y, width, height } = button.getBoundingClientRect();
        return { x, y, width, height };
      }),
    );
    expect(buttons).toEqual([0, 1, 2].map(index => ({
      x: width - 138 + index * 46, y: 0, width: 46, height: 32,
    })));
    expect(await page.evaluate(() => window.__windowTest.chrome?.maximize)).toEqual(buttons[1]);
    const navigation = (await page.locator('.browser-navigation').boundingBox())!;
    const address = (await page.locator('.address-field').boundingBox())!;
    expect(navigation.y).toBeGreaterThanOrEqual(0);
    expect(navigation.y + navigation.height).toBeLessThanOrEqual(32);
    expect(address.y).toBeGreaterThanOrEqual(0);
    expect(address.y + address.height).toBeLessThanOrEqual(32);
    expect(address.width).toBeGreaterThan(100);
    expect(navigation.x + navigation.width).toBeLessThan(address.x);
    expect(address.x + address.width).toBeLessThan(buttons[0]!.x);
  }
});

test("restores the full-width titlebar and framed page even for a saved borderless session", async ({ page }) => {
  await page.route("**/favicon.ico", route => route.abort());
  const state = initialState();
  state.borderless = true;
  await nativeShell(page, state);
  const titlebar = (await page.locator('.titlebar').boundingBox())!;
  const name = (await page.locator('.app-name').boundingBox())!;
  const sidebar = (await page.locator('.sidebar-track').boundingBox())!;
  const area = (await page.locator('.page-area').boundingBox())!;
  expect(titlebar).toMatchObject({ x: 0, y: 0, width: 1280, height: 32 });
  expect(name.x).toBe(12);
  expect(sidebar).toMatchObject({ x: 0, y: 32, width: 240, height: 788 });
  const heading = (await page.locator('.sidebar-header .space-name').boundingBox())!;
  const favorites = (await page.locator('.favorites-heading').boundingBox())!;
  expect(heading.y).toBeGreaterThanOrEqual(sidebar.y);
  expect(heading.y + heading.height).toBeLessThanOrEqual(favorites.y);
  await expect(page.locator('.sidebar .address-field, .sidebar .browser-navigation, .space-list .space-name')).toHaveCount(0);
  await expect(page.getByRole('button', { name: '折叠侧栏', exact: true })).toHaveCount(0);
  expect(area).toMatchObject({ x: 240, y: 32, width: 1032, height: 780 });
  expect(await page.locator('.page-area').evaluate(e => getComputedStyle(e).borderRadius)).toBe('8px');
  expect(await page.locator('.browser-shell').getAttribute('class')).not.toContain('borderless');
  const content = (await page.locator('.new-tab-content').boundingBox())!;
  expect(Math.abs(content.x + content.width / 2 - area.x - area.width / 2)).toBeLessThan(1);
});

test("sidebar motion reports both pane bounds in the same paint frame, including reversal", async ({ page }) => {
  const state = initialState();
  state.activeTab = "github";
  state.tabs.find((t) => t.id === "github")!.splitWith = "wiki";
  await nativeShell(page, state);
  await page.evaluate(() => {
    document.documentElement.style.setProperty("--motion-slide", "400ms");
    window.__windowTest.observer = new ResizeObserver(() => {
      document.querySelectorAll(".page-pane > .webview-slot").forEach((slot, index) => {
        const rect = slot.getBoundingClientRect();
        const layout = window.__windowTest.layout;
        const reported = index === 0 ? layout?.primary : layout?.secondary;
        window.__windowTest.samples.push({ expected: rect.width, reported: reported?.width ?? -1 });
      });
    });
    window.__windowTest.observer.observe(document.querySelector(".page-area")!);
  });
  const collapse = (collapsed: boolean) => page.evaluate((state) => {
    window.__ERA_RECEIVE__!({ type: "state", state });
  }, { ...state, sidebarCollapsed: collapsed });
  await collapse(true);
  // Reverse while the track is still moving, then complete both directions.
  await expect.poll(() => page.locator(".sidebar-track").evaluate((e) => e.getBoundingClientRect().width)).toBeLessThan(200);
  await collapse(false);
  await expect(page.locator(".sidebar-track")).toHaveCSS("width", "240px");
  await collapse(true);
  await expect(page.locator(".sidebar-track")).toHaveCSS("width", "0px");
  const samples = await page.evaluate(() => {
    window.__windowTest.observer?.disconnect();
    return window.__windowTest.samples;
  });
  expect(samples.length).toBeGreaterThan(6);
  expect(Math.max(...samples.map((s) => Math.abs(s.expected - s.reported)))).toBeLessThan(0.1);
});
