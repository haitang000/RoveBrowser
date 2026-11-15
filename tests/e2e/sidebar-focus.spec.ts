import { expect, test } from "@playwright/test";
import { initialState } from "../../src/lib/preview";

test("Ctrl+Shift+F fully hides the sidebar and restores it without opening Find", async ({ page }) => {
  await page.goto("/");
  await page.keyboard.press("Control+Shift+f");
  await expect(page.locator(".sidebar-track")).toHaveCSS("width", "0px");
  await expect(page.getByRole("button", { name: "展开侧栏", exact: true })).toHaveCount(0);
  await expect(page.getByRole("separator", { name: "调整侧栏宽度" })).toHaveCount(0);
  await expect(page.locator(".sidebar-track")).toHaveAttribute("inert", "");
  await expect(page.locator(".find-bar")).toHaveCount(0);
  await expect.poll(() => page.locator(".page-area").evaluate(e => {
    const rect = e.getBoundingClientRect();
    return { left: rect.left, right: innerWidth - rect.right };
  })).toEqual({ left: 8, right: 8 });
  await page.setViewportSize({ width: 480, height: 400 });
  await expect.poll(() => page.locator(".page-area").evaluate(e => {
    const rect = e.getBoundingClientRect();
    return { left: rect.left, right: innerWidth - rect.right };
  })).toEqual({ left: 8, right: 8 });
  await page.setViewportSize({ width: 1280, height: 820 });
  await page.keyboard.press("Control+Shift+f");
  await expect(page.locator(".sidebar-track")).toHaveCSS("width", "240px");
  await expect(page.getByRole("tab", { name: "GitHub", exact: true })).toBeVisible();
  // The existing shortcut can also recover a fully hidden sidebar.
  await page.keyboard.press("Control+Shift+f");
  await expect(page.locator(".sidebar-track")).toHaveCSS("width", "0px");
  await page.keyboard.press("Control+b");
  await expect(page.locator(".sidebar-track")).toHaveCSS("width", "240px");
});

test("the sidebar reverses mid-flight while the page only blurs", async ({ page }) => {
  await page.goto("/");
  // Observe and reverse in the same browser frame, so automation latency cannot
  // accidentally turn the reversal into a second toggle after completion.
  const middle = await page.evaluate(async () => {
    document.documentElement.style.setProperty("--motion-slide", "600ms");
    const track = document.querySelector(".sidebar-track")!;
    const area = document.querySelector(".page-area")!;
    const slot = document.querySelector(".page-pane > .webview-slot")!;
    const toggle = () => window.dispatchEvent(new KeyboardEvent("keydown", { key: "F", code: "KeyF", ctrlKey: true, shiftKey: true, bubbles: true }));
    toggle();
    return await new Promise<{ width: number; blur: number; transform: string }>(resolve => {
      const sample = () => {
        const width = track.getBoundingClientRect().width;
        const blur = parseFloat(getComputedStyle(slot).filter.replace("blur(", "")) || 0;
        if (width < 220 && blur > 0.2) {
          const transform = getComputedStyle(area).transform;
          toggle();
          resolve({ width, blur, transform });
        } else requestAnimationFrame(sample);
      };
      requestAnimationFrame(sample);
    });
  });
  expect(middle.width).toBeGreaterThan(0);
  expect(middle.width).toBeLessThan(220);
  expect(middle.transform).toBe("none");
  await expect(page.locator(".sidebar-track")).toHaveCSS("width", "240px");
  await expect(page.locator(".page-area")).not.toHaveClass(/sidebar-motion-preview/);
  await expect(page.locator(".page-area")).toHaveCSS("transform", "none");
  await expect(page.locator(".page-pane > .webview-slot").first()).toHaveCSS("filter", "none");
});

for (const split of [false, true]) test(`native ${split ? "split" : "single"} layout marks motion and restores the saved width`, async ({ page }) => {
  const state = initialState();
  state.activeTab = "github";
  state.sidebarWidth = 336;
  if (split) state.tabs.find(tab => tab.id === "github")!.splitWith = "wiki";
  await page.addInitScript(state => {
    window.__ERA_NATIVE__ = true;
    window.__ERA_INITIAL_STATE__ = state;
    const samples: { sidebarMotion: boolean; primary: { x: number; width: number }; secondary?: { width: number } }[] = [];
    Object.assign(window, { focusSamples: samples });
    window.ipc = { postMessage(message) {
      const command = JSON.parse(message);
      if (command.type === "layout") samples.push(command);
      if (command.type === "sidebarFocus") {
        state.sidebarCollapsed = !state.sidebarCollapsed;
        state.sidebarHidden = state.sidebarCollapsed;
        window.__ERA_RECEIVE__!({ type: "state", state: structuredClone(state) });
      }
    } };
  }, state);
  await page.goto("/");
  const samples = () => page.evaluate(() => (window as unknown as { focusSamples: { sidebarMotion: boolean; primary: { x: number; width: number }; secondary?: { width: number } }[] }).focusSamples);
  await expect(page.locator(".sidebar-track")).toHaveCSS("width", "336px");
  await page.keyboard.press("Control+Shift+f");
  await expect(page.locator(".sidebar-track")).toHaveCSS("width", "0px");
  await expect(page.getByRole("button", { name: "展开侧栏", exact: true })).toHaveCount(0);
  await expect.poll(async () => (await samples()).at(-1)?.sidebarMotion).toBe(false);
  const collapsed = (await samples()).at(-1)!;
  expect(collapsed.primary.x).toBe(9);
  const lastPane = await page.locator(".page-pane > .webview-slot").last().boundingBox();
  expect(1280 - lastPane!.x - lastPane!.width).toBe(9);
  expect((await samples()).some(s => s.sidebarMotion && s.primary.x > 1 && s.primary.x < 336)).toBe(true);
  if (split) expect((await samples()).filter(s => s.sidebarMotion).every(s => s.secondary && s.secondary.width > 0)).toBe(true);
  await page.keyboard.press("Control+Shift+f");
  await expect(page.locator(".sidebar-track")).toHaveCSS("width", "336px");
  await expect.poll(async () => (await samples()).at(-1)?.sidebarMotion).toBe(false);
});

test("reduced motion settles fully hidden mode without a perspective transform", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.goto("/");
  await page.keyboard.press("Control+Shift+f");
  await expect(page.locator(".sidebar-track")).toHaveCSS("width", "0px");
  await expect(page.locator(".page-area")).toHaveCSS("transform", "none");
  await expect(page.locator(".page-pane > .webview-slot").first()).toHaveCSS("filter", "none");
  await page.keyboard.press("Control+Shift+f");
  await expect(page.locator(".sidebar-track")).toHaveCSS("width", "240px");
  await expect(page.locator(".page-area")).not.toHaveClass(/sidebar-motion-preview/);
});
