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
      overlays: boolean[];
      layoutCount: number;
      popupUpdates: number;
      observer?: ResizeObserver;
    };
  }
}

async function nativeShell(page: import("@playwright/test").Page, state: BrowserState) {
  await page.route("**/favicon.ico", route => route.abort());
  await page.addInitScript((state) => {
    window.__ERA_NATIVE__ = true;
    window.__ERA_INITIAL_STATE__ = state;
    window.__windowTest = { samples: [], overlays: [], layoutCount: 0, popupUpdates: 0 };
    window.ipc = {
      postMessage(message) {
        const command = JSON.parse(message);
        if (command.type === "layout") {
          window.__windowTest.layout = command;
          window.__windowTest.layoutCount++;
        }
        if (command.type === "popupState" && command.popup?.popupArea) window.__windowTest.popupUpdates++;
        if (command.type === "overlay") window.__windowTest.overlays.push(command.visible);
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
  const favorites = (await page.locator('.favorites').boundingBox())!;
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

test("split dragging updates both live pane bounds without opening an overlay", async ({ page }) => {
  const state = initialState();
  state.activeTab = "github";
  state.tabs.find((t) => t.id === "github")!.splitWith = "wiki";
  await nativeShell(page, state);
  const divider = page.getByRole("separator", { name: "调整分屏比例" });
  const start = (await divider.boundingBox())!;
  const before = await page.locator(".webview-slot").evaluateAll(slots => slots.map(s => s.clientWidth));
  await page.evaluate(() => window.__windowTest.overlays = []);
  // Grab the edge rather than the center: the first move must not jump.
  await page.mouse.move(start.x + 1, start.y + 100);
  await page.mouse.down();
  await expect(page.locator('.page-area')).toHaveCSS('border-radius', '0px');
  await expect(page.locator('.page-area')).toHaveCSS('box-shadow', 'none');
  await expect.poll(() => page.evaluate(() => (window.__windowTest.layout as Layout & { liveResize: boolean })?.liveResize)).toBe(true);
  for (const delta of [1, 160, -220, 1200, -1200, 80]) {
    await page.mouse.move(start.x + 1 + delta, start.y + 100, { steps: 4 });
    const widths = await page.locator(".webview-slot").evaluateAll(slots => slots.map(s => s.getBoundingClientRect().width));
    await expect.poll(() => page.evaluate((widths) => {
      const layout = window.__windowTest.layout!;
      return Math.max(Math.abs(layout.primary.width - widths[0]!), Math.abs(layout.secondary!.width - widths[1]!));
    }, widths)).toBeLessThan(0.1);
    if (delta === 1) expect(Math.abs(widths[0]! - before[0]! - 1)).toBeLessThan(1);
    expect(Number(await divider.getAttribute("aria-valuenow"))).toBeGreaterThanOrEqual(20);
    expect(Number(await divider.getAttribute("aria-valuenow"))).toBeLessThanOrEqual(80);
    expect(await page.evaluate(() => window.__windowTest.overlays)).toEqual([]);
  }
  await page.mouse.up();
  await expect(page.locator('.page-area')).toHaveCSS('border-radius', '8px');
  await expect.poll(() => page.evaluate(() => (window.__windowTest.layout as Layout & { liveResize: boolean })?.liveResize)).toBe(false);
  const settled = await divider.getAttribute("aria-valuenow");
  await page.mouse.move(start.x - 250, start.y + 100);
  await expect(divider).toHaveAttribute("aria-valuenow", settled!);
  expect(await page.evaluate(() => window.__windowTest.overlays)).toEqual([]);
});

test("split input submits the real viewport bounds before waiting for layout or paint", async ({ page }) => {
  const state = initialState();
  state.activeTab = "github";
  state.tabs.find((t) => t.id === "github")!.splitWith = "wiki";
  await nativeShell(page, state);
  const divider = page.getByRole("separator", { name: "调整分屏比例" });
  const start = (await divider.boundingBox())!;
  await page.mouse.move(start.x + 1, start.y + 100);
  await page.mouse.down();
  await page.evaluate(async () => {
    await new Promise<void>(resolve => requestAnimationFrame(() => resolve()));
    window.__windowTest.layoutCount = 0;
    window.__windowTest.popupUpdates = 0;
    const divider = document.querySelector(".split-divider")!;
    for (let i = 0; i < 200; i++) {
      divider.dispatchEvent(new PointerEvent("pointermove", { clientX: 600 + i, clientY: 200, bubbles: true }));
      // Every move reaches the native bridge synchronously, before Vue or rAF.
      const left = window.__windowTest.layout!.primary;
      if (Math.abs(left.x + left.width - (599 + i)) > 0.1) throw Error('Native bounds lag behind input');
    }
  });
  expect(await page.evaluate(() => window.__windowTest.layoutCount)).toBe(200);
  expect(await divider.boundingBox()).toMatchObject({ x: 798 });
  expect(await page.evaluate(() => window.__windowTest.popupUpdates)).toBe(0);
  // A cancelled drag must stop receiving movement just like a normal release.
  await divider.dispatchEvent("pointercancel");
  await expect(page.locator('.page-area')).toHaveCSS('border-radius', '8px');
  await page.mouse.move(start.x - 200, start.y + 100);
  expect(await divider.boundingBox()).toMatchObject({ x: 798 });
  await page.mouse.up();
});

test("native split feedback updates the shell without sending a resize back while captured", async ({ page }) => {
  const state = initialState();
  state.activeTab = "github";
  state.tabs.find(t => t.id === "github")!.splitWith = "wiki";
  await nativeShell(page, state);
  await page.evaluate(() => {
    window.__windowTest.layoutCount = 0;
    window.__ERA_RECEIVE__!({ type: 'splitResize', ratio: 0.6, resizing: true });
  });
  await expect(page.getByRole('separator')).toHaveAttribute('aria-valuenow', '60');
  await expect(page.locator('.page-area')).toHaveCSS('border-radius', '0px');
  expect(await page.evaluate(() => window.__windowTest.layoutCount)).toBe(0);
  await page.evaluate(() => window.__ERA_RECEIVE__!({ type: 'splitResize', ratio: 0.6, resizing: false }));
  await expect(page.locator('.page-area')).toHaveCSS('border-radius', '8px');
  await expect.poll(() => page.evaluate(() => window.__windowTest.layoutCount)).toBe(1);
  await page.evaluate(() => window.__ERA_RECEIVE__!({ type: 'splitResize', ratio: 0.6, resizing: true }));
  await expect(page.locator('.page-area')).toHaveClass(/resizing/);
  state.tabs.find(t => t.id === 'github')!.splitWith = undefined;
  await page.evaluate(state => window.__ERA_RECEIVE__!({ type: 'state', state }), state);
  await expect(page.getByRole('separator')).toHaveCount(0);
  await expect(page.locator('.page-area')).not.toHaveClass(/resizing/);
});
