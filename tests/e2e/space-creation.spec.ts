import { expect, test, type Page } from "@playwright/test";
import { initialState } from "../../src/lib/preview";

async function openCreation(page: Page) {
  await page.getByRole("button", { name: "新建", exact: true }).click();
  await page.getByRole("menuitem", { name: "新建 Space", exact: true }).click();
  await expect(page.getByRole("heading", { name: "创建 Space", exact: true })).toBeVisible();
}

test.beforeEach(async ({ page }) => {
  await page.route("**/favicon.ico", route => route.abort());
  await page.goto("/");
});

test("plus opens a keyboard-accessible menu without folders and dismisses outside", async ({ page }) => {
  const trigger = page.getByRole("button", { name: "新建", exact: true });
  await trigger.click();
  const menu = page.getByRole("menu", { name: "新建", exact: true });
  await expect(menu.getByRole("menuitem")).toHaveText(["新建 Space", "新建分屏", "新建标签页"]);
  await expect(menu.getByRole("menuitem", { name: "新建分屏", exact: true })).toBeDisabled();
  await expect(trigger).toHaveAttribute("aria-expanded", "true");
  await expect(menu.getByRole("menuitem", { name: "新建 Space", exact: true })).toBeFocused();
  await page.keyboard.press("ArrowDown");
  await expect(menu.getByRole("menuitem", { name: "新建标签页", exact: true })).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(menu).toHaveCount(0);
  await expect(trigger).toBeFocused();
  await trigger.click();
  await page.locator(".creation-menu-scrim").click({ position: { x: 450, y: 80 } });
  await expect(menu).toHaveCount(0);
  await trigger.click();
  const bounds = (await trigger.boundingBox())!;
  await page.mouse.click(bounds.x + bounds.width / 2, bounds.y + bounds.height / 2);
  await expect(menu).toHaveCount(0);
});

test("creation owns a separate page, validates the name and cancels without changes", async ({ page }) => {
  await page.getByRole("tab", { name: "Wikipedia", exact: true }).click();
  const oldColor = await page.locator("html").getAttribute("data-color");
  const oldTabs = await page.getByRole("tab").count();
  await openCreation(page);
  await expect(page.locator(".space-key")).toHaveCount(3);
  await expect(page.locator(".space-create-page")).toHaveCount(1);
  await expect(page.getByRole("dialog", { name: "工作区与外观" })).toHaveCount(0);
  await expect(page.locator(".page-area")).toHaveAttribute("inert", "");
  await expect(page.locator(".preview-page")).toBeHidden();
  const field = page.getByRole("textbox", { name: "工作区名称", exact: true });
  const submit = page.getByRole("button", { name: "创建 Space", exact: true });
  await expect(field).toBeFocused();
  await expect(submit).toBeDisabled();
  await field.fill("   ");
  await expect(submit).toBeDisabled();
  await field.fill("未保存的空间");
  await page.getByRole("button", { name: "主题…", exact: true }).click();
  await page.getByRole("button", { name: "薄荷", exact: true }).click();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("region", { name: "Space 主题" })).toHaveCount(0);
  await expect(field).toHaveValue("未保存的空间");
  await page.getByRole("button", { name: "取消", exact: true }).click();
  await expect(page.locator(".space-create-page")).toHaveCount(0);
  await expect(page.locator(".sidebar-header .space-name")).toHaveText("Personal");
  await expect(page.locator(".space-key")).toHaveCount(3);
  await expect(page.locator("html")).toHaveAttribute("data-color", oldColor!);
  await expect(page.getByRole("tab", { name: "Wikipedia", exact: true })).toHaveAttribute("aria-selected", "true");
  await expect(page.getByRole("tab")).toHaveCount(oldTabs);
});

test("creates and activates one Space with its name, icon and draft theme", async ({ page }) => {
  await openCreation(page);
  await page.getByRole("textbox", { name: "工作区名称", exact: true }).fill("  研究空间  ");
  await page.getByRole("button", { name: "选择图标", exact: true }).click();
  await page.getByRole("button", { name: "书", exact: true }).click();
  await page.getByRole("button", { name: "主题…", exact: true }).click();
  await page.getByRole("button", { name: "薄荷", exact: true }).click();
  const previewFrame = await page.locator(".browser-shell").getAttribute("style");
  await page.getByRole("button", { name: "关闭主题", exact: true }).click();
  await page.getByRole("button", { name: "创建 Space", exact: true }).click();
  await expect(page.locator(".space-create-page")).toHaveCount(0);
  await expect(page.locator(".space-key")).toHaveCount(4);
  await expect(page.locator(".sidebar-header .space-name")).toHaveText("📖研究空间");
  await expect(page.getByRole("button", { name: "工作区 研究空间", exact: true })).toHaveAttribute("aria-pressed", "true");
  await expect(page.locator(".browser-shell")).toHaveAttribute("style", previewFrame!);
  await expect(page.locator(".page-area")).not.toHaveAttribute("inert");
  await expect(page.getByRole("tab", { name: "New Tab", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "工作区 Personal", exact: true }).click();
  await expect(page.locator("html")).toHaveAttribute("data-color", "sky");
  await page.getByRole("button", { name: "工作区 研究空间", exact: true }).click();
  await expect(page.locator(".browser-shell")).toHaveAttribute("style", previewFrame!);
});

test("new tab and split menu actions use the existing browser flows", async ({ page }) => {
  await page.getByRole("button", { name: "新建", exact: true }).click();
  await page.getByRole("menuitem", { name: "新建标签页", exact: true }).click();
  await expect(page.getByRole("combobox")).toBeFocused();
  await page.keyboard.press("Escape");
  await page.getByRole("tab", { name: "GitHub", exact: true }).click();
  await page.getByRole("tab", { name: "Wikipedia", exact: true }).click();
  await page.getByRole("button", { name: "新建", exact: true }).click();
  await expect(page.getByRole("menuitem", { name: "新建分屏", exact: true })).toBeEnabled();
  await page.getByRole("menuitem", { name: "新建分屏", exact: true }).click();
  await expect(page.locator(".page-pane")).toHaveCount(2);
});

test("command action opens the creation page and Enter submits the name", async ({ page }) => {
  await page.keyboard.press("Control+t");
  await page.getByRole("combobox").fill("new space");
  await page.keyboard.press("Enter");
  await expect(page.locator(".space-key")).toHaveCount(3);
  await expect(page.locator(".space-create-page")).toBeVisible();
  await page.getByRole("textbox", { name: "工作区名称", exact: true }).fill("写作");
  await page.keyboard.press("Enter");
  await expect(page.locator(".sidebar-header .space-name")).toHaveText("写作");
  await expect(page.locator(".space-key")).toHaveCount(4);
});

test("creation and theme editor fit narrow, short and dark windows", async ({ page }) => {
  await page.getByRole("button", { name: "设置", exact: true }).click();
  await page.getByRole("combobox", { name: "外观", exact: true }).selectOption("dark");
  await page.keyboard.press("Control+w");
  await openCreation(page);
  await expect.poll(() => page.locator(".browser-shell").evaluate(e => e.getAnimations().length)).toBe(0);
  await page.screenshot({ path: "test-results/space-creation-dark.png" });
  for (const viewport of [{ width: 480, height: 400 }, { width: 900, height: 640 }]) {
    await page.setViewportSize(viewport);
    await expect(page.getByRole("button", { name: "取消", exact: true })).toBeInViewport();
    expect(await page.locator(".space-create-page").evaluate(e => e.scrollWidth <= e.clientWidth)).toBe(true);
    expect(await page.locator(".space-create-scroll").evaluate(e => e.scrollWidth <= e.clientWidth && e.scrollLeft === 0)).toBe(true);
    await page.getByRole("button", { name: "主题…", exact: true }).click();
    const panel = await page.locator(".space-create-theme").boundingBox();
    expect(panel!.x).toBeGreaterThanOrEqual(0);
    expect(panel!.x + panel!.width).toBeLessThanOrEqual(viewport.width);
    expect(panel!.y + panel!.height).toBeLessThanOrEqual(viewport.height);
    await page.keyboard.press("Escape");
  }
  await page.keyboard.press("Escape");
  await expect(page.locator(".space-create-page")).toHaveCount(0);
});

test("Space limit disables only the creation menu item", async ({ page }) => {
  const state = initialState();
  state.spaces = Array.from({ length: 16 }, (_, i) => ({ id: `space-${i}`, name: `Space ${i}`, color: "sky" }));
  state.activeSpace = "space-0";
  await page.evaluate(state => window.__ERA_RECEIVE__!({ type: "state", state }), state);
  await page.getByRole("button", { name: "新建", exact: true }).click();
  await expect(page.getByRole("menuitem", { name: "新建 Space", exact: true })).toBeDisabled();
  await expect(page.getByRole("menuitem", { name: "新建标签页", exact: true })).toBeEnabled();
});
