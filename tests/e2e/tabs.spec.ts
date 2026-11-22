import { test, expect, type Page } from "@playwright/test";
import { initialState } from "../../src/lib/preview";

async function setup(page: Page, onlyTab = false) {
  const state = initialState();
  if (onlyTab) { state.tabs = state.tabs.filter((tab) => tab.id === "start"); state.pins = []; }
  else {
    for (const id of ["alpha", "beta", "gamma"])
      state.tabs.push({
        id,
        spaceId: "personal",
        title: id,
        url: `https://${id}.example/`,
        pinned: false,
      });
  }
  await page.goto("/");
  await page.evaluate((state) => window.__ERA_RECEIVE__?.({ type: "state", state }), state);
  await expect(page.locator(".tab-enter-active")).toHaveCount(0);
}

async function slowMotion(page: Page) {
  await page.evaluate(() => {
    for (const token of ["--motion-slide", "--motion-selection", "--motion-quick"])
      document.documentElement.style.setProperty(token, "2000ms");
  });
}

async function finishMotion(page: Page) {
  await page.locator(".tab-list").evaluateAll((lists) =>
    lists.forEach((list) => list.getAnimations({ subtree: true }).forEach((animation) => animation.finish())),
  );
  await expect(page.locator(".tab-enter-active, .tab-leave-active, .tab-move")).toHaveCount(0);
}

test("Ctrl+Tab previews recent tabs and commits the highlighted tab on release", async ({ page }) => {
  const picture = "data:image/svg+xml," + encodeURIComponent(
    '<svg xmlns="http://www.w3.org/2000/svg" width="12" height="8"><rect width="12" height="8" fill="teal"/></svg>',
  );
  await page.route("**/favicon.ico", (route) => route.abort());
  await setup(page);
  await page.locator('[data-tab-id="alpha"]').click();
  await page.locator('[data-tab-id="beta"]').click();
  await page.evaluate((image) => window.__ERA_RECEIVE__?.({ type: "thumbnail", id: "alpha", image }), picture);

  const menu = page.getByRole("listbox", { name: "切换标签页" });
  const chosen = menu.getByRole("option", { selected: true });
  await page.keyboard.down("Control");
  await page.keyboard.press("Tab");
  await expect(chosen).toHaveText("alpha");
  await expect(menu).toBeFocused();
  await expect(menu).toHaveCSS("outline-style", "none");
  await expect(chosen.locator(".tab-switcher-snapshot")).toHaveAttribute("src", picture);
  await chosen.locator(".tab-switcher-snapshot").evaluate((image: HTMLImageElement) => image.decode());
  const fallback = menu.getByRole("option", { name: "beta" });
  await expect(fallback.locator(".tab-switcher-snapshot")).toHaveCount(0);
  await expect(fallback.locator(".tab-switcher-picture svg")).toBeVisible();
  await expect(page.locator('[data-tab-id="beta"]')).toHaveAttribute("aria-selected", "true");
  await page.keyboard.press("Tab");
  await expect(chosen).toHaveText("New Tab");
  await page.keyboard.press("Shift+Tab");
  await expect(chosen).toHaveText("alpha");
  await page.keyboard.up("Control");
  await expect(menu).toBeHidden();
  await expect(page.locator('[data-tab-id="alpha"]')).toHaveAttribute("aria-selected", "true");

  await page.keyboard.down("Control");
  await page.keyboard.press("Tab");
  await expect(chosen).toHaveText("beta");
  await page.keyboard.press("Escape");
  await page.keyboard.up("Control");
  await expect(menu).toBeHidden();
  await expect(page.locator('[data-tab-id="alpha"]')).toHaveAttribute("aria-selected", "true");
});

test("a split occupies one sidebar row and reopens both panes from either title", async ({ page }) => {
  await setup(page);
  const tabs = page.getByRole("tablist", { name: "标签页", exact: true });
  await tabs.getByRole("tab", { name: "gamma" }).click();
  await tabs.getByRole("tab", { name: "alpha" }).click({ button: "right" });
  await page.getByRole("menuitem", { name: "在当前网页旁分屏" }).click();

  const split = tabs.getByRole("tab", { name: "分屏：gamma / alpha" });
  await expect(tabs.getByRole("tab")).toHaveCount(3);
  await expect(split).toHaveAttribute("aria-selected", "true");
  await expect(split.locator(".tab-label")).toHaveText(["gamma", "alpha"]);
  await expect(split.locator(".tab-picture")).toHaveCount(2);
  await expect(split.locator(".tab-close")).toHaveCount(1);

  await tabs.getByRole("tab", { name: "beta" }).click();
  await expect(split).toHaveAttribute("aria-selected", "false");
  await expect(page.locator(".split-divider")).toHaveCount(0);
  await split.getByText("alpha", { exact: true }).click();
  await expect(split).toHaveAttribute("aria-selected", "true");
  await expect(page.locator(".split-divider")).toBeVisible();

  await split.getByText("alpha", { exact: true }).click({ button: "right" });
  await expect(page.getByRole("menu").locator(".menu-tab-title")).toHaveText("alpha");
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "关闭分屏", exact: true }).click();
  await expect(tabs.getByRole("tab", { name: "alpha" })).toBeVisible();
  await expect(tabs.getByRole("tab", { name: "gamma" })).toBeVisible();
});

test("a saved split uses one pinned row while unloaded and when reopened", async ({ page }) => {
  const state = initialState();
  const pins = state.pins!;
  pins[0].splitWith = pins[1].id;
  pins[1].splitWith = pins[0].id;
  state.tabs = state.tabs.filter((tab) => !tab.pinned);
  await page.goto("/");
  await page.evaluate((state) => window.__ERA_RECEIVE__?.({ type: "state", state }), state);

  const pinned = page.getByRole("tablist", { name: "固定标签页" });
  const row = pinned.getByRole("tab", { name: "分屏：GitHub / Wikipedia" });
  await expect(pinned.getByRole("tab")).toHaveCount(1);
  await expect(row).toHaveClass(/unloaded/);
  await row.click();
  await expect(page.locator(".split-divider")).toBeVisible();
  await expect(row).toHaveAttribute("aria-selected", "true");
  await row.getByRole("button", { name: "卸载 GitHub" }).click();
  await expect(row).toHaveClass(/unloaded/);
  await expect(pinned.getByRole("tab")).toHaveCount(1);
});

test("middle click closes regular and pinned background tabs without activating them", async ({ page }) => {
  await setup(page);
  for (const id of ["beta", "wiki"]) {
    await page.locator(`[data-tab-id="${id}"]`).click({ button: "middle" });
    await expect(page.locator(`[data-tab-id="${id}"]`)).toHaveCount(0);
    await expect(page.locator(".tab-row.selected")).toHaveAttribute("data-tab-id", "start");
  }
  await expect(page.locator('.pinned-tabs .tab-row.unloaded')).toHaveCount(1);
  // Unloading a pin doesn't put it in the recently closed ordinary-tab stack.
  await page.keyboard.press("Control+Shift+t");
  await expect(page.getByRole("tab", { name: "Wikipedia" })).toBeVisible();
  await expect(page.getByRole("tab", { name: "beta" })).toBeVisible();
  await expect(page.locator(".tab-row.selected")).toHaveAttribute("data-tab-id", "beta");
});

test("middle click over the close icon and in the overview closes the intended tab", async ({ page }) => {
  await setup(page);
  await page.locator('[data-tab-id="alpha"] .tab-close').click({ button: "middle", force: true });
  await expect(page.locator('[data-tab-id="alpha"]')).toHaveCount(0);
  await page.getByRole("button", { name: "全部标签页", exact: true }).click();
  await page.locator(".desk-card").filter({ hasText: "beta" }).click({ button: "middle" });
  await expect(page.locator(".desk-card").filter({ hasText: "beta" })).toHaveCount(0);
  await expect(page.getByRole("dialog", { name: "全部标签页", exact: true })).toBeVisible();
  await expect(page.locator(".tab-row.selected")).toHaveAttribute("data-tab-id", "start");
});

test("closing the last tab immediately selects a new blank tab", async ({ page }) => {
  await setup(page, true);
  await page.getByRole("tab").click({ button: "middle" });
  await expect(page.getByRole("tab")).toHaveCount(1);
  await expect(page.getByRole("tab")).toHaveAttribute("aria-selected", "true");
  await expect(page.getByRole("tab")).not.toHaveAttribute("data-tab-id", "start");
  await expect(page.locator(".tab-leave-active")).toHaveCount(0);
});

test("opening fades and slides in; closing frees the gap while the old row fades out", async ({ page }) => {
  await setup(page);
  await slowMotion(page);
  await page.keyboard.press("Control+t");
  await page.getByRole("combobox").fill("delta.example");
  await page.keyboard.press("Enter");
  const entering = page.locator(".tab-enter-active");
  await expect(entering).toHaveCount(1);
  await expect.poll(() => entering.evaluate((row) => row.getAnimations().length)).toBeGreaterThan(0);
  expect(await entering.evaluate((row) => getComputedStyle(row).transform)).not.toBe("none");
  await finishMotion(page);

  const closing = page.locator('[data-tab-id="beta"]');
  const bounds = await closing.boundingBox();
  await closing.click({ button: "middle" });
  await expect(closing).toHaveClass(/tab-leave-active/);
  await expect(closing).toHaveAttribute("inert", "");
  await expect(closing).toHaveAttribute("aria-hidden", "true");
  await expect(closing).toHaveCSS("pointer-events", "none");
  expect(await closing.evaluate((row) => (row as HTMLElement).offsetWidth)).toBeCloseTo(bounds!.width);
  await expect(page.locator('[data-tab-id="gamma"]')).toHaveClass(/tab-move/);
  await finishMotion(page);
  await expect(closing).toHaveCount(0);
});

test("dragging reorders regular and pinned neighbours before release and commits that order", async ({ page }) => {
  await setup(page);
  await slowMotion(page);
  for (const [source, target, list] of [
    ["gamma", "alpha", '[aria-label="标签页"]'],
    ["wiki", "github", ".pinned-tabs"],
  ]) {
    const from = (await page.locator(`[data-tab-id="${source}"]`).boundingBox())!;
    const to = (await page.locator(`[data-tab-id="${target}"]`).boundingBox())!;
    await page.mouse.move(from.x + 60, from.y + 12);
    await page.mouse.down();
    await page.mouse.move(to.x + 60, to.y + 6, { steps: 6 });
    await page.mouse.move(to.x + 60, to.y + 6);
    await expect(page.locator(`[data-tab-id="${target}"]`)).toHaveClass(/tab-move/);
    await page.mouse.up();
    const order = await page.locator(`${list} .tab-row`).evaluateAll((rows) => rows.map((row) => row.getAttribute("data-tab-id")));
    expect(order.indexOf(source)).toBe(order.indexOf(target) - 1);
    await finishMotion(page);
  }
});

test("rapid close and restore does not leave inert or duplicate rows behind", async ({ page }) => {
  await setup(page);
  await slowMotion(page);
  await page.locator('[data-tab-id="beta"]').click({ button: "middle" });
  await page.keyboard.press("Control+Shift+t");
  await expect(page.getByRole("tab", { name: "beta" })).toHaveCount(1);
  await finishMotion(page);
  const restored = page.locator('[data-tab-id="beta"]');
  await expect(restored).toHaveCount(1);
  await expect(restored).not.toHaveAttribute("inert");
  await expect(restored).not.toHaveAttribute("aria-hidden");
  await restored.click();
  await expect(restored).toHaveAttribute("aria-selected", "true");
});

test("reduced motion avoids slide and reorder transforms", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await setup(page);
  const movement = await page.evaluate(() => {
    const row = document.createElement("div");
    document.body.append(row);
    const styles = ["tab-enter-from", "tab-leave-to", "tab-move"].map((name) => {
      row.className = `tab-row ${name}`;
      const style = getComputedStyle(row);
      return { transform: style.transform, duration: parseFloat(style.transitionDuration) };
    });
    row.remove();
    return styles;
  });
  for (const style of movement) {
    expect(style.transform).toBe("none");
    expect(style.duration).toBeLessThan(0.001);
  }
  await page.locator('[data-tab-id="gamma"]').dragTo(page.locator('[data-tab-id="alpha"]'));
  await expect(page.locator(".tab-move")).toHaveCount(0);
  await page.locator('[data-tab-id="beta"]').click({ button: "middle" });
  await expect(page.locator('[data-tab-id="beta"]')).toHaveCount(0);
});
