import { expect, test, type Page } from "@playwright/test";
import { initialState } from "../../src/lib/preview";

const currentName = (page: Page) => page.locator('.sidebar-header .space-name');
async function wheel(page: Page, dx: number, dy = 0, mode = 0, selector = ".sidebar") {
  return page.locator(selector).evaluate((target, { dx, dy, mode }) => {
    const event = new WheelEvent("wheel", { deltaX: dx, deltaY: dy, deltaMode: mode, bubbles: true, cancelable: true });
    target.dispatchEvent(event);
    return event.defaultPrevented;
  }, { dx, dy, mode });
}
async function offset(page: Page) {
  return page.locator(".space-track").evaluate((e) => new DOMMatrixReadOnly(getComputedStyle(e).transform).m41);
}
async function settled(page: Page) {
  await expect(page.locator(".space-tracking, .space-settling")).toHaveCount(0);
  await expect(page.locator(".space-list")).toHaveCount(1);
}
async function packets(page: Page, deltas: number[]) {
  await page.locator(".sidebar").evaluate(async (target, deltas) => {
    for (const dx of deltas) {
      target.dispatchEvent(new WheelEvent("wheel", { deltaX: dx, bubbles: true, cancelable: true }));
      await new Promise((resolve) => setTimeout(resolve, 20));
    }
  }, deltas);
}

test.beforeEach(async ({ page }) => { await page.goto("/"); });

test("a real horizontal wheel gesture tracks the neighboring list and lands once", async ({ page }) => {
  const bounds = await page.locator(".sidebar-header").boundingBox();
  await page.mouse.move(bounds!.x + 50, bounds!.y + 15);
  await page.mouse.wheel(100, 0);
  await expect(page.locator(".space-tracking")).toHaveCount(1);
  await expect.poll(() => offset(page)).toBeCloseTo(-100, 0);
  await expect(currentName(page)).toHaveText("Personal");
  const incoming = page.locator('[data-space-id="work"]');
  await expect(incoming).toHaveAttribute("inert", "");
  await expect(incoming).toHaveAttribute("aria-hidden", "true");
  expect(await incoming.evaluate((e) => e.getBoundingClientRect().x)).toBeCloseTo(146, 0);
  await expect(page.locator('.space-key[title="Work"]')).toHaveCSS("--space-share", String(100 / 240));
  await expect(currentName(page)).toHaveText("Work");
  await expect(page.locator("html")).toHaveAttribute("data-color", "sage");
  await expect(page.locator('.space-key[title="Work"]')).toHaveAttribute("aria-pressed", "true");
  // Descending packets in one turn are consumed even after the switch.
  await packets(page, [50, 30, 15, 5]);
  await settled(page);
  await expect(currentName(page)).toHaveText("Work");
});

test("small drags spring home; an easy flick lands and a reversed flick cancels", async ({ page }) => {
  await wheel(page, 30);
  await expect.poll(() => offset(page)).toBe(-30);
  await settled(page);
  await expect(currentName(page)).toHaveText("Personal");
  await page.waitForTimeout(260);
  await packets(page, [10, 15]);
  await expect(currentName(page)).toHaveText("Work");
  await settled(page);
  await page.waitForTimeout(260);
  await packets(page, [110, -12]);
  await settled(page);
  await expect(currentName(page)).toHaveText("Work");
});

test("vertical intent locks for the gesture and Ctrl wheel keeps its default", async ({ page }) => {
  expect(await wheel(page, 1, 12)).toBe(false);
  expect(await wheel(page, 120, 0)).toBe(false);
  await expect(page.locator(".space-tracking")).toHaveCount(0);
  await expect(currentName(page)).toHaveText("Personal");
  const prevented = await page.locator(".sidebar").evaluate((e) => {
    const event = new WheelEvent("wheel", { deltaX: 200, ctrlKey: true, bubbles: true, cancelable: true });
    e.dispatchEvent(event); return event.defaultPrevented;
  });
  expect(prevented).toBe(false);
});

test("first and last spaces rubber band without wrapping; notched wheels switch once", async ({ page }) => {
  await wheel(page, -300);
  expect(await offset(page)).toBeGreaterThan(0);
  expect(await offset(page)).toBeLessThan(120);
  await settled(page);
  await expect(currentName(page)).toHaveText("Personal");
  await page.getByRole("button", { name: "工作区 Studio", exact: true }).click();
  await settled(page);
  await page.waitForTimeout(260);
  await wheel(page, 300);
  expect(await offset(page)).toBeLessThan(0);
  expect(await offset(page)).toBeGreaterThan(-120);
  await settled(page);
  await expect(currentName(page)).toHaveText("Studio");
  await page.waitForTimeout(260);
  await wheel(page, -1, 0, 1);
  await wheel(page, -2, 0, 1);
  await expect(currentName(page)).toHaveText("Work");
  await settled(page);
});

test("new input picks up a settling list from its presentation position", async ({ page }) => {
  await wheel(page, 40);
  await expect(page.locator(".space-settling")).toHaveCount(1);
  await page.locator(".space-track").evaluate(async (e) => {
    const animation = e.getAnimations()[0]!;
    animation.pause(); await animation.ready; animation.currentTime = 50;
  });
  const before = await offset(page);
  expect(before).toBeLessThan(0);
  // A reversal begins another gesture while the previous spring is still displayed.
  await wheel(page, -5);
  const after = await offset(page);
  expect(after).toBeCloseTo(before + 5, 0);
  await settled(page);
  await expect(currentName(page)).toHaveText("Personal");
});

test("click/keyboard switches slide by a sidebar width and background colors retarget continuously", async ({ page }) => {
  await page.getByRole("button", { name: "工作区 Work", exact: true }).click();
  await page.locator(".space-track").evaluate(async (e) => {
    const animation = e.getAnimations()[0]!;
    animation.pause(); await animation.ready; animation.currentTime = 80;
  });
  const before = await offset(page);
  expect(before).toBeGreaterThan(0);
  await page.locator(".browser-shell").evaluate(async (e) => {
    const a = e.getAnimations().find((a) => (a.effect as KeyframeEffect).getKeyframes().some((k) => "backgroundColor" in k))!;
    a.pause(); await a.ready; a.currentTime = 70;
  });
  const color = await page.locator(".browser-shell").evaluate((e) => getComputedStyle(e).backgroundColor);
  await page.keyboard.press("Control+3");
  await expect(currentName(page)).toHaveText("Studio");
  const keyframes = await page.locator(".browser-shell").evaluate((e) => (e.getAnimations()[0]!.effect as KeyframeEffect).getKeyframes());
  expect(keyframes[0]?.backgroundColor).toBe(color);
  const start = await page.locator(".space-track").evaluate((e) => {
    const frames = (e.getAnimations()[0]!.effect as KeyframeEffect).getKeyframes();
    return new DOMMatrixReadOnly(String(frames[0]?.transform)).m41;
  });
  expect(start).toBeCloseTo(before + 240, 0);
  await settled(page);
  await expect(page.locator(".browser-shell")).toHaveCSS("background-color", "rgb(232, 221, 253)");
});

test("space scroll positions survive switches; overflowed space keys retain scrolling", async ({ page }) => {
  const state = initialState();
  for (let i = 0; i < 40; i++) state.tabs.push({ id: `extra-${i}`, spaceId: "personal", title: `Extra ${i}`, url: "https://example.com", pinned: false });
  for (let i = 0; i < 15; i++) state.spaces.push({ id: `space-${i}`, name: `Space ${i}`, color: "sky" });
  await page.evaluate((state) => window.__ERA_RECEIVE__?.({ type: "state", state }), state);
  await page.locator('[data-space-id="personal"]').evaluate((e) => { e.scrollTop = 350; });
  await page.keyboard.press("Control+2");
  await settled(page);
  await page.keyboard.press("Control+1");
  await settled(page);
  expect(await page.locator('[data-space-id="personal"]').evaluate((e) => e.scrollTop)).toBe(350);
  expect(await wheel(page, 120, 0, 0, ".space-keys")).toBe(false);
  await expect(currentName(page)).toHaveText("Personal");
});

test("reduced motion keeps lists still and switches with a same-place cross-fade", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await wheel(page, 100);
  expect(await offset(page)).toBe(0);
  await expect(currentName(page)).toHaveText("Personal");
  await expect(page.locator('[data-space-id="work"]')).toHaveCSS("opacity", "0");
  await expect(currentName(page)).toHaveText("Work");
  await expect(page.locator(".space-settling")).toHaveCount(1);
  const transforms = await page.locator(".space-list").evaluateAll((lists) => lists.map((e) => getComputedStyle(e).transform));
  expect(transforms.every((t) => t === "none")).toBe(true);
  await settled(page);
  expect(await page.locator(".browser-shell").evaluate((e) => e.getAnimations().length)).toBe(0);
});

test("opening an overlay cancels an unfinished swipe without changing space", async ({ page }) => {
  await wheel(page, 110);
  await page.keyboard.press("Control+t");
  await expect(page.getByRole("combobox")).toBeVisible();
  await settled(page);
  await expect(currentName(page)).toHaveText("Personal");
  await page.keyboard.press("Escape");
});

test("a cancelled reduced-motion gesture leaves the current list fully visible", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  const result = await page.locator(".sidebar").evaluate(async (target) => {
    target.dispatchEvent(new WheelEvent("wheel", { deltaX: 30, bubbles: true, cancelable: true }));
    await new Promise((resolve) => setTimeout(resolve, 170));
    const panes = Array.from(document.querySelectorAll(".space-list"));
    return panes.map((e) => ({ name: e.getAttribute("aria-label"), opacity: getComputedStyle(e).opacity, animations: e.getAnimations().length }));
  });
  expect(result).toEqual([{ name: "Personal", opacity: "1", animations: 0 }]);
});

test("same-color native state broadcasts leave the background animation alone", async ({ page }) => {
  const state = initialState();
  state.tabs.find((t) => t.id === "start")!.title = "Updated page title";
  await page.evaluate(async (state) => {
    window.__ERA_RECEIVE__?.({ type: "state", state });
    await new Promise(requestAnimationFrame);
  }, state);
  expect(await page.locator(".browser-shell").evaluate((e) => e.getAnimations().length)).toBe(0);
  await page.getByRole("button", { name: "工作区 Work", exact: true }).click();
  await page.locator(".browser-shell").evaluate(async (e) => {
    const animation = e.getAnimations()[0]!;
    animation.pause(); await animation.ready; animation.currentTime = 70;
    (window as any).__spaceColorAnimation = animation;
  });
  state.activeSpace = "work";
  await page.evaluate(async (state) => {
    window.__ERA_RECEIVE__?.({ type: "state", state });
    await new Promise(requestAnimationFrame);
  }, state);
  expect(await page.locator(".browser-shell").evaluate((e) => e.getAnimations()[0] === (window as any).__spaceColorAnimation)).toBe(true);
});
