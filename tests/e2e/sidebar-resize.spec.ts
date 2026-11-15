import { expect, test, type Page } from "@playwright/test";
import { initialState } from "../../src/lib/preview";
import type { BrowserState, Rect } from "../../src/lib/types";
import { resizeAppearance } from "../../src/lib/resize-appearance";

const handleFor = (page: Page) => page.getByRole("separator", { name: "调整侧栏宽度" });
const trackFor = (page: Page) => page.locator(".sidebar-track");

async function startDrag(page: Page) {
  const bounds = (await handleFor(page).boundingBox())!;
  const point = { x: bounds.x + 1, y: bounds.y + 100 };
  await page.mouse.move(point.x, point.y);
  await page.mouse.down();
  return point;
}

test("sidebar edge follows the pointer, clamps bounds and keeps its width through collapse", async ({ page }) => {
  await page.goto("/");
  const point = await startDrag(page);
  for (const [delta, width] of [[1, 241], [120, 360], [2000, 480], [-2000, 192], [80, 320]]) {
    await page.mouse.move(point.x + delta!, point.y);
    await expect(trackFor(page)).toHaveCSS("width", `${width}px`);
    await expect(trackFor(page)).toHaveCSS("transition-duration", "0s");
    expect((await page.locator(".page-area").boundingBox())!.x).toBe(width);
    await expect(handleFor(page)).toHaveAttribute("aria-valuenow", String(width));
  }
  await page.mouse.up();
  await page.mouse.move(100, point.y);
  await expect(trackFor(page)).toHaveCSS("width", "320px");
  await expect(page.locator(".browser-shell")).not.toHaveClass(/sidebar-resizing/);
  await page.keyboard.press("Control+b");
  await expect(trackFor(page)).toHaveCSS("width", "0px");
  await expect(handleFor(page)).toHaveCount(0);
  await page.getByRole("button", { name: "展开侧栏", exact: true }).click();
  await expect(trackFor(page)).toHaveCSS("width", "320px");
  await page.screenshot({ path: "test-results/sidebar-resized.png" });
});

test("Escape restores the starting width and cancellation or blur releases capture", async ({ page }) => {
  await page.goto("/");
  let point = await startDrag(page);
  await page.mouse.move(point.x + 100, point.y);
  await expect(trackFor(page)).toHaveCSS("width", "340px");
  await page.keyboard.press("Escape");
  await expect(trackFor(page)).toHaveCSS("width", "240px");
  await page.mouse.up();
  for (const event of ["pointercancel", "lostpointercapture", "blur"]) {
    point = await startDrag(page);
    await page.mouse.move(point.x + 24, point.y);
    const settled = await trackFor(page).evaluate(e => getComputedStyle(e).width);
    if (event === "blur") await page.evaluate(() => window.dispatchEvent(new Event("blur")));
    else await handleFor(page).dispatchEvent(event);
    await expect(page.locator(".browser-shell")).not.toHaveClass(/sidebar-resizing/);
    await page.mouse.move(100, point.y);
    await expect(trackFor(page)).toHaveCSS("width", settled);
    await page.mouse.up();
  }
});

test("keyboard resizing, double-click reset and narrow windows preserve the preferred width", async ({ page }) => {
  await page.goto("/");
  const handle = handleFor(page);
  await handle.focus();
  await page.keyboard.press("ArrowRight");
  await expect(trackFor(page)).toHaveCSS("width", "248px");
  await page.keyboard.press("Shift+ArrowRight");
  await expect(trackFor(page)).toHaveCSS("width", "280px");
  await page.keyboard.press("Home");
  await expect(trackFor(page)).toHaveCSS("width", "192px");
  await page.keyboard.press("End");
  await expect(trackFor(page)).toHaveCSS("width", "480px");
  await page.setViewportSize({ width: 480, height: 400 });
  await expect(trackFor(page)).toHaveCSS("width", "232px");
  expect((await page.locator(".page-area").boundingBox())!.width).toBeGreaterThanOrEqual(240);
  await expect(handle).toHaveAttribute("aria-valuemax", "232");
  await page.setViewportSize({ width: 1280, height: 820 });
  await expect(trackFor(page)).toHaveCSS("width", "480px");
  await handle.dblclick();
  await expect(trackFor(page)).toHaveCSS("width", "240px");
  await page.setViewportSize({ width: 480, height: 400 });
  await expect(trackFor(page)).toHaveCSS("width", "216px");
});

type Report = { primary: Rect; secondary?: Rect; liveResize: boolean };
declare global {
  interface Window {
    __sidebarTest: { writes: (number | null)[]; layout?: Report; presentedWidth?: number; overlays: boolean[]; layouts: number; appearance?: ReturnType<typeof resizeAppearance> };
  }
}
async function nativeShell(page: Page, state: BrowserState) {
  await page.route("**/favicon.ico", route => route.abort());
  await page.addInitScript(state => {
    window.__ERA_NATIVE__ = true;
    window.__ERA_INITIAL_STATE__ = state;
    window.__sidebarTest = { writes: [], overlays: [], layouts: 0 };
    window.chrome = { webview: {
      postMessage() {},
      addEventListener(_, listener) { window.addEventListener("__nativeSidebar", listener as EventListener); },
      removeEventListener(_, listener) { window.removeEventListener("__nativeSidebar", listener as EventListener); },
    } };
    window.ipc = { postMessage(message) {
      const command = JSON.parse(message);
      if (command.type === "resizeAppearance") window.__sidebarTest.appearance = command;
      if (command.type === "layout") { window.__sidebarTest.layout = command; window.__sidebarTest.layouts++; }
      if (command.type === "overlay") window.__sidebarTest.overlays.push(command.visible);
      if (command.type === "presented") window.__sidebarTest.presentedWidth = document.querySelector(".sidebar-track")!.getBoundingClientRect().width;
      if (command.type === "sidebarWidth") {
        window.__sidebarTest.writes.push(command.width);
        if (command.width === null) delete state.sidebarWidth;
        else state.sidebarWidth = command.width;
        queueMicrotask(() => window.__ERA_RECEIVE__!({ type: "state", state: structuredClone(state) }));
      }
    } };
  }, state);
  await page.goto("/");
  await expect.poll(() => page.evaluate(() => window.__sidebarTest.presentedWidth)).toBe(state.sidebarWidth ?? 240);
}

test("native resize colors update from the active Space, custom stops and appearance", async ({ page }) => {
  const state = initialState();
  state.activeTab = "github";
  await nativeShell(page, state);
  const active = state.spaces.find(space => space.id === state.activeSpace)!;
  await expect.poll(() => page.evaluate(() => window.__sidebarTest.appearance?.start))
    .toEqual(resizeAppearance(active, state.theme, false).start);
  active.color = "sage";
  state.theme = "dark";
  await page.evaluate(state => window.__ERA_RECEIVE__!({ type: "state", state }), state);
  await expect.poll(() => page.evaluate(() => window.__sidebarTest.appearance?.start))
    .toEqual(resizeAppearance(active, "dark", false).start);
  Object.assign(active, { hue: 0, hue2: 0.34, brightness: 0.85, brightness2: 0.8, angle: 185 });
  await page.evaluate(state => window.__ERA_RECEIVE__!({ type: "state", state }), state);
  await expect.poll(() => page.evaluate(() => window.__sidebarTest.appearance?.angle)).toBe(275);
  const actual = await page.evaluate(() => window.__sidebarTest.appearance!);
  expect(actual.start[0]).toBeGreaterThan(actual.start[1]);
  expect(actual.end[1]).toBeGreaterThan(actual.end[0]);
  expect(actual.dark).toBe(true);
});

for (const split of [false, true]) test(`native ${split ? "split" : "single"} pages follow resizing, with one preference write on release`, async ({ page }) => {
  const state = initialState();
  state.sidebarWidth = 336;
  state.activeTab = "github";
  if (split) state.tabs.find(tab => tab.id === "github")!.splitWith = "wiki";
  await nativeShell(page, state);
  const point = await startDrag(page);
  for (const delta of [1, 100, -80, 24]) {
    await page.mouse.move(point.x + delta, point.y);
    await expect(trackFor(page)).toHaveCSS("width", `${336 + delta}px`);
    await expect.poll(() => page.evaluate(() => {
      const slots = [...document.querySelectorAll(".page-pane > .webview-slot")];
      const layout = window.__sidebarTest.layout!;
      return Math.max(...slots.map((slot, index) => {
        const actual = slot.getBoundingClientRect();
        const reported = index ? layout.secondary! : layout.primary;
        return Math.max(Math.abs(actual.x - reported.x), Math.abs(actual.width - reported.width));
      }));
    })).toBeLessThan(0.1);
    expect(await page.evaluate(() => window.__sidebarTest.layout!.liveResize)).toBe(true);
    expect(await page.evaluate(() => window.__sidebarTest.writes)).toEqual([]);
  }
  await page.mouse.up();
  await expect.poll(() => page.evaluate(() => window.__sidebarTest.layout!.liveResize)).toBe(false);
  expect(await page.evaluate(() => window.__sidebarTest.writes)).toEqual([360]);
  expect(await page.evaluate(() => window.__sidebarTest.overlays)).toEqual([]);
});

for (const split of [false, true]) test(`sidebar input submits ${split ? "both viewports" : "the viewport"} before layout observers run`, async ({ page }) => {
  const state = initialState();
  state.activeTab = "github";
  if (split) state.tabs.find(tab => tab.id === "github")!.splitWith = "wiki";
  await nativeShell(page, state);
  const point = await startDrag(page);
  await page.evaluate(async point => {
    await new Promise<void>(resolve => requestAnimationFrame(() => resolve()));
    const handle = document.querySelector('.sidebar-resize-handle')!;
    const bounds = Element.prototype.getBoundingClientRect;
    let reads = 0;
    Element.prototype.getBoundingClientRect = function () { reads++; return bounds.call(this); };
    try {
    for (const delta of [1, 100, -40, 80]) {
      reads = 0;
      handle.dispatchEvent(new PointerEvent('pointermove', {
        pointerId: 1, clientX: point.x + delta, clientY: point.y, bubbles: true,
      }));
      if (reads !== 0) throw Error('Sidebar pointer input forced a synchronous DOM layout');
      const layout = window.__sidebarTest.layout!;
      const slots = [...document.querySelectorAll('.page-pane > .webview-slot')];
      slots.forEach((slot, i) => {
        const rect = slot.getBoundingClientRect();
        const reported = i ? layout.secondary! : layout.primary;
        if (Math.abs(rect.x - reported.x) > 0.1 || Math.abs(rect.width - reported.width) > 0.1)
          throw Error('Page bounds lag behind sidebar input');
      });
      const viewport = (layout as Report & { viewport: { width: number; height: number } }).viewport;
      if (viewport.width !== innerWidth || viewport.height !== innerHeight)
        throw Error('Layout is missing its source viewport');
    }
    } finally { Element.prototype.getBoundingClientRect = bounds; }
  }, point);
  await page.mouse.up();
});

test('native sidebar feedback paints the shell without sending intermediate layouts or preference writes back', async ({ page }) => {
  const state = initialState();
  state.activeTab = 'github';
  state.tabs.find(tab => tab.id === 'github')!.splitWith = 'wiki';
  await nativeShell(page, state);
  const originalRootWidth = await page.evaluate(() => document.documentElement.style.getPropertyValue('--sidebar-width'));
  await page.evaluate(() => {
    window.__sidebarTest.layouts = 0;
    window.dispatchEvent(new MessageEvent("__nativeSidebar", { data: { type: 'sidebarResize', width: 400, resizing: true } }));
  });
  await expect(trackFor(page)).toHaveCSS('width', '400px');
  await expect(page.locator('.sidebar')).toHaveCSS('width', '400px');
  expect(await page.evaluate(() => document.documentElement.style.getPropertyValue('--sidebar-width'))).toBe(originalRootWidth);
  await expect(trackFor(page)).toHaveCSS('transition-duration', '0s');
  await expect(page.locator('.browser-shell')).toHaveClass(/sidebar-resizing/);
  expect(await page.evaluate(() => window.__sidebarTest.layouts)).toBe(0);
  await page.evaluate(() => window.dispatchEvent(new MessageEvent("__nativeSidebar", { data: { type: 'sidebarResize', width: 320, resizing: true } })));
  await expect(trackFor(page)).toHaveCSS('width', '320px');
  expect(await page.evaluate(() => window.__sidebarTest.layouts)).toBe(0);
  await page.evaluate(() => window.dispatchEvent(new MessageEvent("__nativeSidebar", { data: { type: 'sidebarResize', width: 320, resizing: false } })));
  await expect(page.locator('.browser-shell')).not.toHaveClass(/sidebar-resizing/);
  await expect.poll(() => page.evaluate(() => window.__sidebarTest.layouts)).toBeGreaterThan(0);
  expect(await page.evaluate(() => window.__sidebarTest.writes)).toEqual([]);
  expect(await page.evaluate(() => window.__sidebarTest.overlays)).toEqual([]);
  await page.evaluate(() => window.dispatchEvent(new MessageEvent("__nativeSidebar", { data: { type: 'sidebarResize', width: 240, resizing: false, reset: true } })));
  await expect(trackFor(page)).toHaveCSS('width', '240px');
  // A complete native drag can be delivered before Vue renders even once.
  // The final feedback must not accidentally restart the collapse transition.
  expect(await page.evaluate(() => {
    window.dispatchEvent(new MessageEvent("__nativeSidebar", { data: { type: 'sidebarResize', width: 240, resizing: true } }));
    window.dispatchEvent(new MessageEvent("__nativeSidebar", { data: { type: 'sidebarResize', width: 400, resizing: false } }));
    return document.querySelector('.sidebar-track')!.getBoundingClientRect().width;
  })).toBe(400);
});
