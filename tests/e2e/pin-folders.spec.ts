import { expect, test, type Page } from "@playwright/test";

async function createFolder(page: Page, name: string) {
  await page.getByRole("button", { name: "新建文件夹", exact: true }).click();
  await page.getByRole("textbox", { name: "文件夹名称" }).fill(name);
  await page.getByRole("button", { name: "创建文件夹" }).click();
  return page.getByRole("button", { name, exact: true });
}

const folderRows = (page: Page, name: string) => page.getByRole("tablist", { name: `${name}内的固定标签` });
const root = (page: Page) => page.locator(".pinned-tabs");

test.beforeEach(async ({ page }) => {
  await page.route("**/favicon.ico", (route) => route.abort());
  await page.goto("/");
});

test("folders accept a drop while collapsed and can be renamed or dissolved without losing pins", async ({ page }) => {
  const folder = await createFolder(page, "项目");
  const pin = root(page).locator('[data-bookmark-id="pin-wiki"]');
  const liveId = await pin.getAttribute("data-tab-id");
  await folder.click();
  await expect(folder).toHaveAttribute("aria-expanded", "false");
  await pin.dragTo(folder);
  await expect(pin).toHaveCount(0);
  await folder.click();
  await expect(folderRows(page, "项目").getByRole("tab", { name: "Wikipedia" })).toHaveAttribute("data-tab-id", liveId!);

  await page.getByRole("button", { name: "项目的文件夹操作" }).click();
  await page.getByRole("menuitem", { name: "重命名文件夹" }).click();
  await page.getByRole("textbox", { name: "文件夹名称" }).fill("参考");
  await page.getByRole("button", { name: "保存名称" }).click();
  await expect(folderRows(page, "参考").getByRole("tab", { name: "Wikipedia" })).toBeVisible();

  await page.getByRole("button", { name: "参考的文件夹操作" }).click();
  await page.getByRole("menuitem", { name: "解散文件夹" }).click();
  await expect(root(page).locator(".pin-folder")).toHaveCount(0);
  await expect(pin).toHaveAttribute("data-tab-id", liveId!);
});

test("folders reorder among root pins and their contents can move by drag or menu", async ({ page }) => {
  const first = await createFolder(page, "第一组");
  const second = await createFolder(page, "第二组");
  const github = root(page).locator(':scope > [data-bookmark-id="pin-github"]');
  await second.dragTo(github, { targetPosition: { x: 40, y: 4 } });
  await expect(root(page).locator(":scope > .pin-folder").first()).toContainText("第二组");
  await expect(root(page).locator(":scope > :is(.tab-row, .pin-folder)").first()).toHaveClass("pin-folder");

  await github.click({ button: "right" });
  await page.getByRole("menuitem", { name: "移至文件夹…" }).click();
  await page.getByRole("menuitem", { name: "第一组", exact: true }).click();
  await expect(folderRows(page, "第一组").getByRole("tab", { name: "GitHub" })).toBeVisible();
  await folderRows(page, "第一组").getByRole("tab", { name: "GitHub" }).dragTo(second);
  await expect(folderRows(page, "第二组").getByRole("tab", { name: "GitHub" })).toBeVisible();
  await folderRows(page, "第二组").getByRole("tab", { name: "GitHub" }).dragTo(page.locator("[data-pin-root-target]"));
  await expect(root(page).locator(':scope > [data-bookmark-id="pin-github"]')).toBeVisible();
  await expect(first).toBeVisible();
});
