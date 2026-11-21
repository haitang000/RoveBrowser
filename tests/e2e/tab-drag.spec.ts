import { expect, test, type Page } from "@playwright/test";
import { initialState } from "../../src/lib/preview";
import { browserMenuTrigger } from "./browser-menu";

const rows = (page: Page) => page.locator('[aria-label="标签页"] .tab-row');
const row = (page: Page, id: string) => page.locator(`.tab-list [data-tab-id="${id}"]`);
const order = (page: Page) => rows(page).evaluateAll((elements) => elements.map((element) => element.getAttribute("data-tab-id")));

async function setup(page: Page, extraTabs = 0) {
  const state = initialState();
  for (const id of ["alpha", "beta", "gamma", ...Array.from({ length: extraTabs }, (_, i) => `extra-${i}`)]) {
    state.tabs.push({ id, title: id, url: `https://${id}.example`, spaceId: "personal", pinned: false });
  }
  await page.route("**/favicon.ico", (route) => route.abort());
  await page.goto("/");
  await page.evaluate((state) => window.__ERA_RECEIVE__!({ type: "state", state }), state);
  await expect(page.locator(".tab-enter-active")).toHaveCount(0);
}

async function start(page: Page, id: string) {
  const rect = (await row(page, id).boundingBox())!;
  await page.mouse.move(rect.x + 40, rect.y + 12);
  await page.mouse.down();
  await page.mouse.move(rect.x + 46, rect.y + 12);
}

async function over(page: Page, id: string, fraction = 0.25) {
  const point = await row(page, id).evaluate((element, fraction) => {
    const list = element.parentElement!;
    return { clientX: list.getBoundingClientRect().left + 40, clientY: list.getBoundingClientRect().top + (element as HTMLElement).offsetTop + element.clientHeight * fraction };
  }, fraction);
  await page.mouse.move(point.clientX, point.clientY);
  return point;
}

test("the full row stays opaque under the pointer while neighbours make room; cancellation restores order", async ({ page }) => {
  await setup(page);
  const original = await order(page);
  await page.evaluate(() => document.documentElement.style.setProperty("--motion-slide", "2000ms"));
  const bounds = (await row(page, "gamma").boundingBox())!;
  await start(page, "gamma");
  const image = page.locator("body > .tab-drag-item");
  await expect(image).toBeVisible();
  await expect(image).toHaveCSS("opacity", "1");
  await expect(image).toHaveCSS("background-color", "rgb(255, 255, 255)");
  await expect(image).toHaveCSS("width", `${bounds.width}px`);
  await expect(image).toHaveCSS("height", `${bounds.height}px`);
  await expect(image.locator(".tab-title")).toHaveText("gamma");
  await expect(image.locator(".tab-picture")).toHaveCount(1);
  await expect(image.locator(".tab-close")).toHaveCSS("visibility", "visible");
  await expect(row(page, "gamma")).toHaveCSS("opacity", "0");
  const point = await over(page, "alpha");
  await expect.poll(async () => (await image.boundingBox())!.y).toBeCloseTo(point.clientY - 12);
  await expect(image.locator(".tab-title")).toHaveCSS("opacity", "1");
  expect(await order(page)).toEqual(["start", "gamma", "alpha", "beta"]);
  await expect(row(page, "alpha")).toHaveClass(/tab-move/);
  expect(await row(page, "alpha").evaluate((element) => getComputedStyle(element).transform)).not.toBe("none");
  // A stationary pointer over the empty slot must not alternate the order.
  for (let i = 0; i < 4; i++) {
    await page.mouse.move(bounds.x + 40, bounds.y - 64 + 12);
  }
  expect(await order(page)).toEqual(["start", "gamma", "alpha", "beta"]);
  await page.keyboard.press("Escape");
  await page.mouse.up();
  await expect(page.locator(".tab-drag-item, .tab-drag-source, .tab-drag-gap")).toHaveCount(0);
  expect(await order(page)).toEqual(original);
});

test("real mouse drags keep a solid row under the pointer and insert after the last regular or pinned tab", async ({ page }) => {
  await setup(page);
  const from = (await row(page, "alpha").boundingBox())!;
  const to = (await row(page, "gamma").boundingBox())!;
  await page.mouse.move(from.x + 60, from.y + 12);
  await page.mouse.down();
  await page.mouse.move(to.x + 80, to.y + 27, { steps: 6 });
  await page.mouse.move(to.x + 80, to.y + 27);
  const item = page.locator("body > .tab-drag-item");
  await expect(item).toBeVisible();
  await expect(item).toHaveCSS("opacity", "1");
  await expect.poll(async () => (await item.boundingBox())!.x).toBeCloseTo(to.x + 20);
  await expect.poll(async () => (await item.boundingBox())!.y).toBeCloseTo(to.y + 15);
  await page.mouse.up();
  expect(await order(page)).toEqual(["start", "beta", "gamma", "alpha"]);
  await row(page, "github").dragTo(row(page, "wiki"), { targetPosition: { x: 60, y: 27 } });
  await expect(page.locator(".pinned-tabs .tab-row").last()).toHaveAttribute("data-tab-id", "github");
  await expect(page.locator(".tab-drag-item, .tab-drag-source, .tab-drag-gap")).toHaveCount(0);
});

test("crossing the fixed area opens one gap, leaving removes it and dropping converts the tab", async ({ page }) => {
  await setup(page);
  await start(page, "alpha");
  await over(page, "wiki");
  await expect(page.locator(".pinned-tabs .tab-drag-gap")).toHaveCount(1);
  await expect(row(page, "alpha")).toHaveClass(/tab-drag-away/);
  const header = (await page.locator(".sidebar-header").boundingBox())!;
  await page.mouse.move(header.x + 40, header.y + header.height / 2);
  await expect(page.locator(".tab-drag-gap, .tab-drag-away")).toHaveCount(0);
  await over(page, "wiki");
  await page.mouse.up();
  await expect(page.locator('.pinned-tabs [data-tab-id="alpha"]')).toBeVisible();
  await expect(rows(page)).toHaveCount(3);
  await expect(page.locator(".tab-drag-item, .tab-drag-source, .tab-drag-gap")).toHaveCount(0);
});

test("only a clear move into the page shows the split guide while the row can cross the sidebar first", async ({ page }) => {
  await setup(page);
  await row(page, "alpha").click();
  const from = (await row(page, "beta").boundingBox())!;
  const sidebar = (await page.locator(".sidebar").boundingBox())!;
  const area = (await page.locator(".page-area").boundingBox())!;
  const fromX = from.x + from.width - 38;
  const edge = sidebar.x + sidebar.width;
  const y = from.y + 12;
  const guide = page.locator(".split-drop-target");
  const preview = page.locator(".tab-drag-item");
  await page.mouse.move(fromX, y);
  await page.mouse.down();
  await page.mouse.move(fromX, y + 70);
  await expect(preview).toBeVisible();
  await expect(guide).toHaveCount(0);
  await page.mouse.move(area.x + 5, y + 70);
  await expect.poll(async () => {
    const rect = (await preview.boundingBox())!;
    return rect.x + rect.width;
  }).toBeGreaterThan(edge);
  await expect(guide).toHaveCount(0);
  await page.mouse.move(edge + 80, y + 70);
  await expect(guide).toBeVisible();
  await page.mouse.move(fromX, y + 70);
  await expect(guide).toHaveCount(0);
  await page.mouse.up();
  await expect(preview).toHaveCount(0);
  await expect(page.locator(".split-divider")).toHaveCount(0);
});

test("the wheel and the list edge scroll during a drag and dropping uses the scrolled row positions", async ({ page }) => {
  await setup(page, 42);
  await start(page, "alpha");
  const scroller = page.locator('.space-list[data-space-id="personal"]');
  const bounds = (await scroller.boundingBox())!;
  const x = bounds.x + 46;
  const y = bounds.y + bounds.height / 2;
  const scrollTop = () => scroller.evaluate((list) => list.scrollTop);
  await page.mouse.move(x, y);
  await page.mouse.wheel(0, 500);
  await expect.poll(scrollTop).toBeGreaterThan(400);
  await expect.poll(async () => (await page.locator(".tab-drag-item").boundingBox())!.y).toBeCloseTo(y - 12);
  await page.mouse.move(x, bounds.y + bounds.height - 5);
  await expect.poll(scrollTop).toBeGreaterThan(520);
  await page.mouse.move(x, y);
  const previewOrder = await order(page);
  expect(previewOrder.indexOf("alpha")).toBeGreaterThan(10);
  await page.mouse.up();
  await expect(page.locator(".tab-drag-item, .tab-drag-source, .tab-drag-gap")).toHaveCount(0);
  expect(await order(page)).toEqual(previewOrder);
});

test("dragging a tab to the page creates a split and removes the drag affordances", async ({ page }) => {
  await setup(page);
  await row(page, "alpha").click();
  const area = page.locator(".page-area");
  const { width, height } = (await area.boundingBox())!;
  await row(page, "beta").dragTo(area, { targetPosition: { x: width * 0.75, y: height / 2 } });
  await expect(page.getByRole("region", { name: "左侧窗格" }).locator(".address-field")).toHaveText("alpha.example");
  await expect(page.getByRole("region", { name: "右侧窗格" }).locator(".address-field")).toHaveText("beta.example");
  await expect(page.locator(".split-drop-target, .tab-drag-item")).toHaveCount(0);
});

test("holding a dragged tab over the browser menu reveals workspace drop targets", async ({ page }) => {
  await setup(page);
  const from = (await row(page, "alpha").boundingBox())!;
  const trigger = browserMenuTrigger(page);
  const to = (await trigger.boundingBox())!;
  await page.mouse.move(from.x + 40, from.y + 12);
  await page.mouse.down();
  await page.mouse.move(to.x + to.width / 2, to.y + to.height / 2, { steps: 12 });
  await page.mouse.move(to.x + to.width / 2, to.y + to.height / 2);
  const work = page.getByRole("menuitem", { name: "工作区 Work", exact: true });
  await expect(work).toBeVisible();
  const target = (await work.boundingBox())!;
  await page.mouse.move(target.x + 40, target.y + target.height / 2, { steps: 8 });
  await page.mouse.move(target.x + 40, target.y + target.height / 2);
  await page.mouse.up();
  await expect(trigger).toHaveAttribute("aria-expanded", "false");
  await expect(row(page, "alpha")).toHaveCount(0);
  await page.keyboard.press("Control+2");
  await expect(row(page, "alpha")).toBeVisible();
});

test("reduced motion keeps the insertion preview without moving neighbouring rows", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await setup(page);
  await start(page, "gamma");
  await over(page, "alpha");
  expect(await order(page)).toEqual(["start", "gamma", "alpha", "beta"]);
  await expect(page.locator(".tab-move")).toHaveCount(0);
  await page.mouse.up();
  await expect(page.locator(".tab-drag-source, .tab-drag-gap, .tab-drag-item")).toHaveCount(0);
  expect(await order(page)).toEqual(["start", "gamma", "alpha", "beta"]);
});
