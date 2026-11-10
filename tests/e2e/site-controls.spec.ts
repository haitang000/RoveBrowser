import { expect, test } from "@playwright/test";

test("tab menu exposes mute and per-page zoom controls", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("tab", { name: "Wikipedia" }).click();
  await page.getByRole("tab", { name: "Wikipedia" }).click({ button: "right" });
  const menu = page.getByRole("menu");
  await expect(menu).toBeVisible();
  await expect(menu.getByRole("menuitem", { name: "静音标签页" })).toBeVisible();
  await expect(menu.getByText("网页缩放 100%")).toBeVisible();
  await expect(menu.getByRole("menuitem", { name: "缩小" })).toBeVisible();
  await expect(menu.getByRole("menuitem", { name: "放大" })).toBeVisible();
  // 100% is already actual size, so Reset is quiet until a zoom changes it.
  await expect(menu.getByRole("menuitem", { name: "100%" })).toBeDisabled();
  await page.keyboard.press("Escape");
});

test("a tab playing sound shows the speaker, and muting changes it", async ({
  page,
}) => {
  await page.goto("/");
  const tabId = await page
    .getByRole("tab", { selected: true })
    .getAttribute("data-tab-id");
  expect(tabId).toBeTruthy();
  await page.evaluate(
    (id) =>
      window.__ERA_RECEIVE__?.({ type: "audio", id: id!, playing: true }),
    tabId,
  );
  const audio = page.locator(".tab-row .tab-audio");
  await expect(audio).toHaveCount(1);
  await expect(audio).not.toHaveClass(/muted/);
  await audio.click();
  await expect(audio).toHaveClass(/muted/);
  // Stopping the sound keeps the muted speaker, as a muted tab still shows it.
  await page.evaluate(
    (id) => window.__ERA_RECEIVE__?.({ type: "audio", id: id!, playing: false }),
    tabId,
  );
  await expect(audio).toHaveCount(1);
  await audio.click();
  await expect(page.locator(".tab-row .tab-audio")).toHaveCount(0);
});

test("zoom events update the menu label and reset becomes available", async ({
  page,
}) => {
  await page.goto("/");
  await page.evaluate(() =>
    window.__ERA_RECEIVE__?.({ type: "zoom", factor: 1.25 }),
  );
  await page.getByRole("tab", { name: "Wikipedia" }).click();
  await page.getByRole("tab", { name: "Wikipedia" }).click({ button: "right" });
  const menu = page.getByRole("menu");
  await expect(menu.getByText("网页缩放 125%")).toBeVisible();
  await expect(menu.getByRole("menuitem", { name: "100%" })).toBeEnabled();
  await page.keyboard.press("Escape");
});

test("zoom and mute appear in the command bar for the current page", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByRole("tab", { name: "Wikipedia" }).click();
  await page.keyboard.press("Control+t");
  const field = page.getByRole("combobox");
  await field.fill("zoom");
  await expect(page.getByText("Zoom In")).toBeVisible();
  await expect(page.getByText("Zoom Out")).toBeVisible();
  await field.fill("mute");
  await expect(page.getByText("Mute Tab")).toBeVisible();
  await page.keyboard.press("Escape");
});
