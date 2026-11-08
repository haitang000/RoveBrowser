import { test, expect, type Page } from "@playwright/test";
import { initialState } from "../../src/lib/preview";

async function setup(page: Page, onlyTab = false) {
  const state = initialState();
  if (onlyTab) state.tabs = state.tabs.filter((tab) => tab.id === "start");
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

test("middle click closes regular and pinned background tabs without activating them", async ({ page }) => {
  await setup(page);
  for (const id of ["beta", "wiki"]) {
    await page.locator(`[data-tab-id="${id}"]`).click({ button: "middle" });
    await expect(page.locator(`[data-tab-id="${id}"]`)).toHaveCount(0);
    await expect(page.locator(".tab-row.selected")).toHaveAttribute("data-tab-id", "start");
  }
  await page.keyboard.press("Control+Shift+t");
  await expect(page.getByRole("tab", { name: "Wikipedia" })).toBeVisible();
  await page.keyboard.press("Control+Shift+t");
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

test("dropping reorders regular and pinned rows with position transitions", async ({ page }) => {
  await setup(page);
  await slowMotion(page);
  for (const [source, target, list] of [
    ["gamma", "alpha", '[aria-label="标签页"]'],
    ["wiki", "github", ".pinned-tabs"],
  ]) {
    await page.locator(`[data-tab-id="${source}"]`).dragTo(page.locator(`[data-tab-id="${target}"]`));
    await expect(page.locator(`[data-tab-id="${source}"]`)).toHaveClass(/tab-move/);
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
