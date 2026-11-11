import { expect, test } from "@playwright/test";

test("sidebar scrollbar stays hidden until the list scrolls", async ({
  page,
}) => {
  await page.goto("/");
  const list = page.locator(".space-list").first();
  await expect(list).not.toHaveClass(/scrolling/);
  // Let the mount-time scroll restoration pass before simulating a scroll.
  await page.waitForTimeout(120);
  await list.evaluate((element) => element.dispatchEvent(new Event("scroll")));
  await expect(list).toHaveClass(/scrolling/);
  // The bar fades out shortly after the last scroll event.
  await expect(list).not.toHaveClass(/scrolling/, { timeout: 2500 });
});

test("a real wheel over the tab list shows the ambient bar, then it fades", async ({
  page,
}) => {
  await page.setViewportSize({ width: 480, height: 400 });
  await page.goto("/");
  for (let index = 0; index < 6; index++) {
    await page.keyboard.press("Control+t");
    await page.getByRole("combobox").fill(`https://example.com/${index}`);
    await page.keyboard.press("Enter");
  }
  const list = page.locator(".space-list").first();
  await expect
    .poll(async () =>
      list.evaluate((element) => element.scrollHeight > element.clientHeight),
    )
    .toBe(true);
  await list.hover();
  await page.mouse.wheel(0, 120);
  await expect(list).toHaveClass(/scrolling/);
  await expect(list).not.toHaveClass(/scrolling/, { timeout: 2500 });
});
