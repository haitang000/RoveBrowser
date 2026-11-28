import { expect, test, type Locator, type Page } from "@playwright/test";
import type { ExtensionsState } from "../../src/lib/extensions";

const steps = ["download", "verifySignature", "unpack", "readManifest"] as const;

async function showExtensionState(page: Page, state: Partial<ExtensionsState>) {
  await page.evaluate((extensions) => window.__ERA_RECEIVE__?.({
    type: "extensionsState",
    extensions: {
      items: null, review: null, removal: null, popup: true, busy: false, progress: null, error: null,
      ...extensions,
    },
  }), state);
}

async function finishHeightTransition(frame: Locator) {
  await expect.poll(() => frame.evaluate(el => el.getAnimations().some(animation =>
    animation instanceof CSSTransition && animation.transitionProperty === "height"))).toBe(true);
  await frame.evaluate(el => el.getAnimations().forEach(animation => animation.finish()));
}

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => { window.__ERA_POPUP__ = true; });
});

test("extension stages report their own progress and the dialog resizes in both directions", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "no-preference" });
  await page.addInitScript(() => {
    document.addEventListener("transitionrun", event => {
      const target = event.target;
      if (event.propertyName !== "height" || !(target instanceof HTMLElement)
        || !target.matches(".extension-dialog-viewport")) return;
      for (const animation of target.getAnimations()) animation.pause();
    });
  });
  await page.goto("/");
  await showExtensionState(page, {});
  const dialog = page.getByRole("dialog", { name: "确认安装扩展" });
  const frame = dialog.locator(".extension-dialog-viewport");
  const initial = (await dialog.boundingBox())!.height;

  await showExtensionState(page, { busy: true, progress: { steps: [...steps], current: "download", percent: 25 } });
  await expect(dialog.getByRole("progressbar")).toHaveCount(4);
  await expect(dialog.getByRole("progressbar", { name: "下载扩展" })).toHaveAttribute("aria-valuenow", "25");
  await expect(dialog.getByRole("progressbar", { name: "验证商店签名" })).toHaveAttribute("aria-valuetext", "等待中");
  await finishHeightTransition(frame);
  const expanded = (await dialog.boundingBox())!.height;
  expect(expanded).toBeGreaterThan(initial);

  await showExtensionState(page, { busy: true, progress: { steps: [...steps], current: "verifySignature", percent: null } });
  await expect(dialog.getByRole("progressbar", { name: "下载扩展" })).toHaveAttribute("aria-valuenow", "100");
  const verify = dialog.getByRole("progressbar", { name: "验证商店签名" });
  await expect(verify).toHaveAttribute("aria-valuetext", "进行中");
  await expect(verify).not.toHaveAttribute("aria-valuenow");

  await showExtensionState(page, { error: "无法准备商店扩展" });
  await expect(dialog.getByRole("alert")).toHaveText("无法准备商店扩展");
  await expect(dialog.getByRole("button", { name: "关闭" })).toBeFocused();
  await expect(dialog.getByRole("progressbar")).toHaveCount(0);
  await finishHeightTransition(frame);
  expect((await dialog.boundingBox())!.height).toBeLessThan(expanded);
});

test("reduced motion keeps a single indeterminate install bar visible without animation", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.goto("/");
  await showExtensionState(page, {});
  const dialog = page.getByRole("dialog", { name: "确认安装扩展" });
  const initial = (await dialog.boundingBox())!.height;

  await showExtensionState(page, { busy: true, progress: { steps: ["install"], current: "install", percent: null } });
  const bar = dialog.getByRole("progressbar");
  await expect(bar).toHaveCount(1);
  await expect(bar).toHaveAccessibleName("安装扩展");
  await expect(bar.locator(".extension-progress-fill")).toHaveCSS("animation-name", "none");
  await expect(dialog.locator(".extension-dialog-viewport")).toHaveCSS("transition-property", "none");
  await expect.poll(async () => (await dialog.boundingBox())!.height).toBeGreaterThan(initial);
});
