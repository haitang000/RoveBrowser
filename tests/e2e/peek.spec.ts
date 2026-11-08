import { test, expect } from "@playwright/test";
test("Peek UI dismissal, command stacking, promotion and minimum layout", async ({
  page,
}) => {
  await page.goto("/");
  await page
    .getByRole("tab", { name: "GitHub", exact: true })
    .click({ button: "right" });
  await page.getByRole("menuitem", { name: "在 Peek 中预览" }).click();
  const panel = page.locator(".peek-panel");
  await expect(panel).toBeVisible();
  await expect(page.locator("iframe")).toHaveCount(0);
  await page.keyboard.press("Control+l");
  await expect(page.getByRole("combobox")).toHaveValue("https://github.com/");
  await page.keyboard.press("Escape");
  await expect(panel).toBeVisible();
  await page.keyboard.press('Control+w');await expect(panel).toHaveCount(0);
  await expect(page.getByRole('tab',{name:'New Tab',exact:true})).toBeVisible();
  await page.getByRole('tab',{name:'GitHub',exact:true}).click({button:'right'});
  await page.getByRole('menuitem',{name:'在 Peek 中预览'}).click();
  await page.setViewportSize({ width: 560, height: 400 });
  const bounds = await panel.boundingBox();
  expect(bounds!.x).toBeGreaterThan(216);
  expect(bounds!.y + bounds!.height).toBeLessThan(400);
  await page.screenshot({ path: "test-results/peek-small.png" });
  await page.keyboard.press("Escape");
  await expect(panel).toHaveCount(0);
  await page.setViewportSize({ width: 1280, height: 820 });
  await page
    .getByRole("tab", { name: "GitHub", exact: true })
    .click({ button: "right" });
  await page.getByRole("menuitem", { name: "在 Peek 中预览" }).click();
  await page.getByRole("button", { name: "将预览转为标签页" }).click();
  await expect(panel).toHaveCount(0);
  await expect(page.locator(".tab-row.selected")).toContainText("github.com");
});
