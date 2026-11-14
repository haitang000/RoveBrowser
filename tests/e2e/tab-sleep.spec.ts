import { test, expect, type Page } from "@playwright/test";
import { initialState } from "../../src/lib/preview";

async function setup(page: Page) {
  const state = initialState();
  for (const id of ["alpha", "beta"])
    state.tabs.push({
      id,
      spaceId: "personal",
      title: id,
      url: `https://${id}.example/`,
      pinned: false,
    });
  await page.goto("/");
  await page.evaluate((state) => window.__ERA_RECEIVE__?.({ type: "state", state }), state);
  await expect(page.locator(".tab-enter-active")).toHaveCount(0);
  return state;
}

test("a sleeping tab shows Zigzag's dashed ring and says why", async ({ page }) => {
  await setup(page);
  const row = page.locator('[data-tab-id="alpha"]');
  await expect(row).not.toHaveClass(/sleeping/);
  await expect(row).not.toHaveAttribute("data-sleeping");

  await page.evaluate(() =>
    window.__ERA_RECEIVE__?.({ type: "sleep", id: "alpha", asleep: true }),
  );
  await expect(row).toHaveClass(/sleeping/);
  await expect(row).toHaveAttribute("data-sleeping", "true");
  await expect(row).toHaveAttribute("title", /已休眠以节省内存/);
  const ring = await row.evaluate((element) => {
    const style = getComputedStyle(element.querySelector(".tab-picture")!, "::after");
    return {
      borderStyle: style.borderStyle,
      borderWidth: style.borderTopWidth,
      radius: style.borderTopLeftRadius,
    };
  });
  expect(ring.borderStyle).toBe("dashed");
  expect(ring.borderWidth).toBe("1px");
  // Even and round, as Zigzag draws it round the site's own icon.
  expect(parseFloat(ring.radius)).toBeGreaterThanOrEqual(50);

  // Waking clears the ring and the reason.
  await page.evaluate(() =>
    window.__ERA_RECEIVE__?.({ type: "sleep", id: "alpha", asleep: false }),
  );
  await expect(row).not.toHaveClass(/sleeping/);
  await expect(row).not.toHaveAttribute("data-sleeping");
  await expect(row).toHaveAttribute("title", "https://alpha.example/");
});

test("sleepState restores rings for a renderer that starts late, and a closed tab's ring goes", async ({
  page,
}) => {
  const state = await setup(page);
  await page.evaluate(() =>
    window.__ERA_RECEIVE__?.({
      type: "sleepState",
      ids: ["alpha", "beta"],
    }),
  );
  await expect(page.locator('[data-tab-id="alpha"]')).toHaveClass(/sleeping/);
  await expect(page.locator('[data-tab-id="beta"]')).toHaveClass(/sleeping/);

  // A tab that closes while asleep must not leave its ring behind when a new
  // tab takes the same id again (the store filters runtime state on each state).
  const withoutAlpha = {
    ...state,
    tabs: state.tabs.filter((tab) => tab.id !== "alpha"),
    activeTab: "start",
  };
  await page.evaluate(
    (state) => window.__ERA_RECEIVE__?.({ type: "state", state }),
    withoutAlpha,
  );
  await expect(page.locator('[data-tab-id="alpha"]')).toHaveCount(0);
  await page.evaluate(
    (state) => window.__ERA_RECEIVE__?.({ type: "state", state }),
    state,
  );
  await expect(page.locator('[data-tab-id="alpha"]')).toBeVisible();
  await expect(page.locator('[data-tab-id="alpha"]')).not.toHaveClass(/sleeping/);
  // The other page keeps its ring: it never left the session.
  await expect(page.locator('[data-tab-id="beta"]')).toHaveClass(/sleeping/);
});
