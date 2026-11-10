import { expect, test, type Page } from "@playwright/test";

/** Record CSS animation names on the find bar so timing races cannot hide them. */
async function recordAnimations(page: Page) {
  await page.evaluate(() => {
    const seen: string[] = [];
    (window as unknown as { __findAnimations: string[] }).__findAnimations = seen;
    document.addEventListener(
      "animationstart",
      (event) => {
        const target = event.target as Element;
        if (
          target.classList.contains("find-bar") ||
          target.classList.contains("find-bar-surface")
        )
          seen.push((event as AnimationEvent).animationName);
      },
      true,
    );
  });
}
async function animations(page: Page) {
  return page.evaluate(
    () =>
      (window as unknown as { __findAnimations: string[] }).__findAnimations ||
      [],
  );
}

test("find bar opens, focuses, types, steps and closes", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("tab", { name: "Wikipedia" }).click();
  await page.keyboard.press("Control+f");
  const bar = page.getByRole("search", { name: "页内查找" });
  await expect(bar).toBeVisible();
  const field = page.getByRole("textbox", { name: "查找内容" });
  await expect(field).toBeFocused();
  await field.fill("wiki");
  // The standalone preview has no real page to search; the field and its
  // controls still have to stay usable, and the counter settles at no results
  // rather than pretending WebView2 matched something.
  await expect(field).toHaveValue("wiki");
  await expect(page.locator(".find-count")).toHaveText("无结果");
  await page.keyboard.press("Escape");
  await expect(bar).toHaveCount(0);
  await expect(page.getByRole("tab", { name: "Wikipedia" })).toHaveAttribute(
    "aria-selected",
    "true",
  );
});

test("find bar settles in and leaves with motion", async ({ page }) => {
  await page.goto("/");
  await recordAnimations(page);
  await page.keyboard.press("Control+f");
  const bar = page.locator(".find-bar");
  await expect(bar).toHaveCount(1);
  await expect
    .poll(async () => (await animations(page)).includes("find-settle-in"))
    .toBe(true);
  expect(await animations(page)).toContain("find-fade-in");
  await page.keyboard.press("Escape");
  // The outer box stays one transition long so the native popup region keeps
  // covering the fading pill; the surface animates while it is still mounted.
  expect(
    await page.evaluate(async () => {
      await new Promise((resolve) => setTimeout(resolve, 25));
      return Boolean(document.querySelector(".find-bar"));
    }),
  ).toBe(true);
  await expect
    .poll(async () => (await animations(page)).includes("find-settle-out"))
    .toBe(true);
  await expect(bar).toHaveCount(0);
});

test("reduce motion keeps only the quick fade", async ({ page }) => {
  await page.goto("/");
  await page.evaluate(() => {
    document.documentElement.dataset.reduceMotion = "true";
  });
  await recordAnimations(page);
  await page.keyboard.press("Control+f");
  await expect(page.locator(".find-bar")).toHaveCount(1);
  await expect
    .poll(async () => (await animations(page)).includes("find-fade-in"))
    .toBe(true);
  expect(await animations(page)).not.toContain("find-settle-in");
  await page.keyboard.press("Escape");
  await expect(page.locator(".find-bar")).toHaveCount(0);
});

test("find bar toggles on Ctrl+F while typing and from the page", async ({
  page,
}) => {
  await page.goto("/");
  await page.keyboard.press("Control+f");
  const bar = page.getByRole("search", { name: "页内查找" });
  await expect(bar).toBeVisible();
  const field = page.getByRole("textbox", { name: "查找内容" });
  await field.fill("abc");
  await page.keyboard.press("Control+f");
  await expect(bar).toHaveCount(0);
  await page.keyboard.press("Control+f");
  await expect(bar).toBeVisible();
  await expect(field).toHaveValue("");
});

test("find is ignored while a dialog owns the keyboard", async ({ page }) => {
  await page.goto("/");
  await page.keyboard.press("Control+t");
  await expect(page.getByRole("combobox")).toBeFocused();
  await page.keyboard.press("Control+f");
  await expect(page.getByRole("combobox")).toBeVisible();
  await expect(page.getByRole("search", { name: "页内查找" })).toHaveCount(0);
  await page.keyboard.press("Escape");
  await expect(page.getByRole("search", { name: "页内查找" })).toHaveCount(0);
});

test("find bar stays inside the page area at small and large sizes", async ({
  page,
}) => {
  await page.goto("/");
  for (const size of [
    { width: 1280, height: 820 },
    { width: 480, height: 400 },
  ]) {
    await page.setViewportSize(size);
    await page.keyboard.press("Control+f");
    const area = await page.locator(".page-area").boundingBox();
    const bar = await page.locator(".find-bar").boundingBox();
    expect(bar!.x).toBeGreaterThanOrEqual(area!.x);
    expect(bar!.x + bar!.width).toBeLessThanOrEqual(area!.x + area!.width);
    expect(bar!.y).toBeGreaterThanOrEqual(area!.y);
    expect(bar!.y + bar!.height).toBeLessThanOrEqual(area!.y + area!.height);
    await expect(page.locator(".find-bar")).toBeVisible();
    await page.locator(".find-bar input").press("Escape");
  }
});
