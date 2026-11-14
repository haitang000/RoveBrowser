import { expect, test, type Page } from "@playwright/test";
import { initialState } from "../../src/lib/preview";
import type { BrowserState, Rect } from "../../src/lib/types";

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
    __sidebarTest: { writes: (number | null)[]; layout?: Report; presentedWidth?: number; overlays: boolean[] };
  }
}
async function nativeShell(page: Page, state: BrowserState) {
  await page.route("**/favicon.ico", route => route.abort());
  await page.addInitScript(state => {
    window.__ERA_NATIVE__ = true;
    window.__ERA_INITIAL_STATE__ = state;
    window.__sidebarTest = { writes: [], overlays: [] };
    window.ipc = { postMessage(message) {
      const command = JSON.parse(message);
      if (command.type === "layout") window.__sidebarTest.layout = command;
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
