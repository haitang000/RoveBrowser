import { test, expect, type Page } from "@playwright/test";
async function setup(page: Page) {
  await page.addInitScript(() => {
    const animate = Element.prototype.animate;
    Element.prototype.animate = function (frames, options) {
      const animation = animate.call(this, frames, options);
      if (
        this.classList.contains("peek-panel") ||
        this.classList.contains("peek-scrim")
      )
        animation.pause();
      return animation;
    };
  });
  await page.goto("/");
}
async function open(page: Page, id = "motion") {
  await page.evaluate(
    (id) =>
      window.__ERA_RECEIVE__?.({
        type: "peek",
        peek: {
          id,
          spaceId: "personal",
          title: "Motion",
          url: "https://github.com",
          pinned: false,
        },
        phase: "opening",
        ticket: 1,
        origin: { x: 430, y: 240 },
      }),
    id,
  );
  await expect
    .poll(() =>
      page.locator(".peek-panel").evaluate((e) => e.getAnimations().length),
    )
    .toBeGreaterThan(0);
}
async function finish(page: Page) {
  await page
    .locator(".peek-layer")
    .evaluate((e) =>
      e.getAnimations({ subtree: true }).forEach((a) => a.finish()),
    );
}
test("zooms from the hyperlink and reverses along the same path on dismissal", async ({
  page,
}) => {
  await setup(page);
  await open(page);
  await page
    .locator(".peek-panel")
    .evaluate((e) => e.getAnimations().forEach((a) => (a.currentTime = 0)));
  const start = await page.locator(".peek-panel").boundingBox();
  expect(start!.x + start!.width / 2).toBeCloseTo(430, 0);
  expect(start!.y + start!.height / 2).toBeCloseTo(240, 0);
  await page.screenshot({ path: "test-results/peek-enter-origin.png" });
  await finish(page);
  await expect(page.locator(".peek-layer")).toHaveAttribute(
    "data-phase",
    "open",
  );
  const full = await page.locator(".peek-panel").boundingBox();
  expect(full!.width).toBeGreaterThan(start!.width * 4);
  await page.getByRole("button", { name: "关闭 Peek", exact: true }).click();
  await expect(page.locator(".peek-layer")).toHaveAttribute(
    "data-phase",
    "closing",
  );
  await expect
    .poll(() =>
      page.locator(".peek-panel").evaluate((e) => e.getAnimations().length),
    )
    .toBeGreaterThan(0);
  await page
    .locator(".peek-panel")
    .evaluate((e) =>
      e
        .getAnimations()
        .forEach(
          (a) => (a.currentTime = Number(a.effect!.getTiming().duration)),
        ),
    );
  const end = await page.locator(".peek-panel").boundingBox();
  expect(end!.x + end!.width / 2).toBeCloseTo(430, 0);
  expect(end!.y + end!.height / 2).toBeCloseTo(240, 0);
  await expect(page.locator(".peek-panel")).toHaveCSS("opacity", "0");
  await finish(page);
  await expect(page.locator(".peek-layer")).toHaveCount(0);
});
test("closing interrupts entry without jumping, and reopening cancels old completion", async ({
  page,
}) => {
  await setup(page);
  await open(page);
  await page
    .locator(".peek-panel")
    .evaluate((e) => e.getAnimations().forEach((a) => (a.currentTime = 65)));
  const before = await page
    .locator(".peek-panel")
    .evaluate((e) => getComputedStyle(e).transform);
  await page.keyboard.press("Escape");
  await expect
    .poll(() =>
      page
        .locator(".peek-panel")
        .evaluate((e) => e.getAnimations()[0]?.effect?.getTiming().duration),
    )
    .toBe(180);
  expect(
    await page
      .locator(".peek-panel")
      .evaluate((e) => getComputedStyle(e).transform),
  ).toBe(before);
  await open(page, "replacement");
  await finish(page);
  await expect(page.locator(".peek-layer")).toHaveAttribute(
    "data-phase",
    "open",
  );
  await page.evaluate(() =>
    window.__ERA_RECEIVE__?.({ type: "peekMoving", id: "motion", ticket: 2 }),
  );
  await expect(page.locator(".peek-layer")).toHaveAttribute(
    "data-phase",
    "open",
  );
});
test("reduced motion fades without translating or scaling", async ({
  page,
}) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await setup(page);
  await open(page);
  const effect = await page.locator(".peek-panel").evaluate((e) => {
    const a = e.getAnimations()[0];
    return {
      duration: a.effect!.getTiming().duration,
      frames: (a.effect as KeyframeEffect)
        .getKeyframes()
        .map((f) => f.transform),
    };
  });
  expect(effect.duration).toBe(80);
  expect(effect.frames).toEqual(["none", "none"]);
  await finish(page);
  await expect(page.locator(".peek-layer")).toHaveAttribute(
    "data-phase",
    "open",
  );
  await page.getByRole("button", { name: "关闭 Peek", exact: true }).click();
  await expect
    .poll(() =>
      page
        .locator(".peek-panel")
        .evaluate((e) => e.getAnimations()[0]?.effect?.getTiming().duration),
    )
    .toBe(80);
  await finish(page);
  await expect(page.locator(".peek-layer")).toHaveCount(0);
});
test("entry waits for both split backgrounds and decoded pixels before handing off", async ({ page }) => {
  await setup(page);
  await page.evaluate(() => {
    const decode = HTMLImageElement.prototype.decode;
    const releases: (() => void)[] = [];
    Object.assign(window, { releaseBackgrounds: () => releases.splice(0).forEach((release) => release()) });
    HTMLImageElement.prototype.decode = function () {
      const decoded = decode.call(this);
      return this.classList.contains("peek-background")
        ? decoded.then(() => new Promise<void>((resolve) => releases.push(resolve)))
        : decoded;
    };
    window.__ERA_RECEIVE__?.({
      type: "peek", peek: { id: "slow-background", spaceId: "personal", title: "Slow", url: "https://example.com", pinned: false },
      phase: "opening", ticket: 1, backgroundReady: false,
    });
  });
  const layer = page.locator(".peek-layer");
  await expect(layer).toHaveAttribute("data-moving", "0");
  const image = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/l9sAAAAASUVORK5CYII=";
  await page.evaluate((image) => window.__ERA_RECEIVE__?.({ type: "peekBackground", id: "slow-background", pane: 0, background: image, backgroundReady: false }), image);
  await expect(page.locator(".peek-background")).toHaveCount(1);
  await expect(layer).toHaveAttribute("data-moving", "0");
  await page.evaluate((image) => window.__ERA_RECEIVE__?.({ type: "peekBackground", id: "slow-background", pane: 1, background: image, backgroundReady: true }), image);
  await expect(page.locator(".peek-background")).toHaveCount(2);
  await expect.poll(() => page.locator(".peek-background").evaluateAll((images) => images.every((image) => (image as HTMLImageElement).naturalWidth > 0))).toBe(true);
  await expect(layer).toHaveAttribute("data-moving", "0");
  await expect(page.locator(".peek-panel")).toHaveCSS("opacity", "0");
  await page.evaluate(() => (window as unknown as { releaseBackgrounds(): void }).releaseBackgrounds());
  await expect(layer).toHaveAttribute("data-moving", "1");
  await finish(page);
  await expect(layer).toHaveAttribute("data-phase", "open");
});
test("a broken backdrop cancels entry instead of exposing an empty surface", async ({ page }) => {
  await setup(page);
  await page.evaluate(() => {
    window.__ERA_RECEIVE__?.({
      type: "peek", peek: { id: "broken", spaceId: "personal", title: "Broken", url: "https://example.com", pinned: false },
      phase: "opening", ticket: 1, backgroundReady: false,
    });
    window.__ERA_RECEIVE__?.({ type: "peekBackground", id: "broken", pane: 0, background: "data:image/png;base64,invalid", backgroundReady: true });
  });
  await expect(page.locator(".peek-layer")).toHaveCount(0);
  await expect(page.getByRole("alert")).toContainText("无法解码 Peek 背景，请重试");
});
