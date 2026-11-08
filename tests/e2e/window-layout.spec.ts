import { test, expect } from "@playwright/test";
import { initialState } from "../../src/lib/preview";
import type { BrowserState, Layout } from "../../src/lib/types";

declare global {
  interface Window {
    __windowTest: {
      layout?: Layout;
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
