import { expect, test, type JSHandle, type Page } from "@playwright/test";
import { initialState } from "../../src/lib/preview";

const rows = (page: Page) => page.locator('[aria-label="标签页"] .tab-row');
const row = (page: Page, id: string) => page.locator(`.tab-list [data-tab-id="${id}"]`);
const order = (page: Page) => rows(page).evaluateAll((elements) => elements.map((element) => element.getAttribute("data-tab-id")));

async function setup(page: Page) {
  const state = initialState();
  for (const id of ["alpha", "beta", "gamma"]) {
    state.tabs.push({ id, title: id, url: `https://${id}.example`, spaceId: "personal", pinned: false });
  }
  await page.route("**/favicon.ico", (route) => route.abort());
  await page.goto("/");
  await page.evaluate((state) => window.__ERA_RECEIVE__!({ type: "state", state }), state);
  await expect(page.locator(".tab-enter-active")).toHaveCount(0);
}

async function start(page: Page, id: string) {
  const transfer = await page.evaluateHandle(() => new DataTransfer());
  const rect = (await row(page, id).boundingBox())!;
  await row(page, id).dispatchEvent("dragstart", { dataTransfer: transfer, clientX: rect.x + 40, clientY: rect.y + 12 });
  return transfer;
}

async function over(page: Page, transfer: JSHandle<DataTransfer>, id: string, fraction = 0.25) {
  const point = await row(page, id).evaluate((element, fraction) => {
    const list = element.parentElement!;
    return { clientX: list.getBoundingClientRect().left + 40, clientY: list.getBoundingClientRect().top + (element as HTMLElement).offsetTop + element.clientHeight * fraction };
  }, fraction);
  await row(page, id).dispatchEvent("dragover", { dataTransfer: transfer, ...point });
  return point;
}

test("the complete row is the drag image and neighbours make room before release; cancellation restores order", async ({ page }) => {
  await setup(page);
  const original = await order(page);
  await page.evaluate(() => document.documentElement.style.setProperty("--motion-slide", "2000ms"));
  const bounds = (await row(page, "gamma").boundingBox())!;
  const transfer = await start(page, "gamma");
  const image = page.locator("body > .tab-drag-image");
  await expect(image).toHaveCSS("width", `${bounds.width}px`);
  await expect(image).toHaveCSS("height", `${bounds.height}px`);
  await expect(image.locator(".tab-title")).toHaveText("gamma");
  await expect(image.locator(".tab-picture")).toHaveCount(1);
  await expect(image.locator(".tab-close")).toHaveCSS("visibility", "visible");
  await expect(row(page, "gamma")).toHaveCSS("opacity", "0");
  await over(page, transfer, "alpha");
  expect(await order(page)).toEqual(["start", "gamma", "alpha", "beta"]);
  await expect(row(page, "alpha")).toHaveClass(/tab-move/);
  expect(await row(page, "alpha").evaluate((element) => getComputedStyle(element).transform)).not.toBe("none");
  // A stationary pointer over the empty slot must not alternate the order.
  for (let i = 0; i < 4; i++) {
    await row(page, "gamma").dispatchEvent("dragover", { dataTransfer: transfer, clientX: bounds.x + 40, clientY: bounds.y - 64 + 12 });
  }
  expect(await order(page)).toEqual(["start", "gamma", "alpha", "beta"]);
  await row(page, "gamma").dispatchEvent("dragend", { dataTransfer: transfer });
  await expect(page.locator(".tab-drag-image, .tab-drag-source, .tab-drag-gap")).toHaveCount(0);
  expect(await order(page)).toEqual(original);
});

test("real mouse drags can insert after the last regular or pinned tab", async ({ page }) => {
  await setup(page);
  await row(page, "alpha").dragTo(row(page, "gamma"), { targetPosition: { x: 60, y: 27 } });
  expect(await order(page)).toEqual(["start", "beta", "gamma", "alpha"]);
  await row(page, "github").dragTo(row(page, "wiki"), { targetPosition: { x: 60, y: 27 } });
  await expect(page.locator(".pinned-tabs .tab-row").last()).toHaveAttribute("data-tab-id", "github");
  await expect(page.locator(".tab-drag-image, .tab-drag-source, .tab-drag-gap")).toHaveCount(0);
});

test("crossing the fixed area opens one gap, leaving removes it and dropping converts the tab", async ({ page }) => {
  await setup(page);
  const transfer = await start(page, "alpha");
  await over(page, transfer, "wiki");
  await expect(page.locator(".pinned-tabs .tab-drag-gap")).toHaveCount(1);
  await expect(row(page, "alpha")).toHaveClass(/tab-drag-away/);
  await page.locator(".sidebar-header").dispatchEvent("dragover", { dataTransfer: transfer });
  await expect(page.locator(".tab-drag-gap, .tab-drag-away")).toHaveCount(0);
  const point = await over(page, transfer, "wiki");
  await row(page, "wiki").dispatchEvent("drop", { dataTransfer: transfer, ...point });
  await expect(page.locator('.pinned-tabs [data-tab-id="alpha"]')).toBeVisible();
  await expect(rows(page)).toHaveCount(3);
  await row(page, "alpha").dispatchEvent("dragend", { dataTransfer: transfer });
  await expect(page.locator(".tab-drag-image, .tab-drag-source, .tab-drag-gap")).toHaveCount(0);
});

test("reduced motion keeps the insertion preview without moving neighbouring rows", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await setup(page);
  const transfer = await start(page, "gamma");
  const point = await over(page, transfer, "alpha");
  expect(await order(page)).toEqual(["start", "gamma", "alpha", "beta"]);
  await expect(page.locator(".tab-move")).toHaveCount(0);
  await row(page, "alpha").dispatchEvent("drop", { dataTransfer: transfer, ...point });
  await expect(page.locator(".tab-drag-source, .tab-drag-gap, .tab-drag-image")).toHaveCount(0);
  expect(await order(page)).toEqual(["start", "gamma", "alpha", "beta"]);
});
