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
  await expect.poll(async () => (await page.locator(".page-area").boundingBox())!.x).toBeLessThan(2);
  await page.keyboard.press("Control+Shift+f");
  await expect(page.locator(".sidebar-track")).toHaveCSS("width", "240px");
  await expect(page.getByRole("tab", { name: "GitHub", exact: true })).toBeVisible();
  // The existing shortcut can also recover a fully hidden sidebar.
  await page.keyboard.press("Control+Shift+f");
  await expect(page.locator(".sidebar-track")).toHaveCSS("width", "0px");
  await page.keyboard.press("Control+b");
  await expect(page.locator(".sidebar-track")).toHaveCSS("width", "240px");
});

test("the sidebar and page animate in both directions and can reverse mid-flight", async ({ page }) => {
  await page.goto("/");
  await page.evaluate(() => document.documentElement.style.setProperty("--motion-slide", "600ms"));
  await page.keyboard.press("Control+Shift+f");
  await expect(page.locator(".page-area")).toHaveClass(/sidebar-motion-preview/);
  await expect.poll(() => page.locator(".sidebar-track").evaluate(e => e.getBoundingClientRect().width)).toBeLessThan(220);
  const before = await page.locator(".sidebar-track").evaluate(e => e.getBoundingClientRect().width);
  expect(before).toBeGreaterThan(0);
  await page.keyboard.press("Control+Shift+f");
  const after = await page.locator(".sidebar-track").evaluate(e => e.getBoundingClientRect().width);
  expect(after).toBeLessThan(239);
  await expect(page.locator(".sidebar-track")).toHaveCSS("width", "240px");
  await expect(page.locator(".page-area")).not.toHaveClass(/sidebar-motion-preview/);
  await expect(page.locator(".page-area")).toHaveCSS("transform", "none");
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
  await page.keyboard.press("Control+Shift+f");
  await expect(page.locator(".sidebar-track")).toHaveCSS("width", "240px");
  await expect(page.locator(".page-area")).not.toHaveClass(/sidebar-motion-preview/);
});
