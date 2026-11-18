import { test, expect } from "@playwright/test";
import { initialState } from "../../src/lib/preview";
import type { BrowserState, Layout, Rect } from "../../src/lib/types";

declare global {
  interface Window {
    __windowTest: {
      layout?: Layout;
      chrome?: { maximize: Rect; viewportWidth: number; anchorLeft: boolean; background: number[]; hover: number[]; foreground: number[]; dragRegions: Rect[] };
      glass: boolean[];
      presented?: { theme?: string; width: number; fonts: string };
      samples: { expected: number; reported: number }[];
      overlays: boolean[];
      layoutCount: number;
      popupUpdates: number;
      observer?: ResizeObserver;
      paletteReads?: number;
      reduceMotion?: boolean;
    };
  }
}

async function nativeShell(page: import("@playwright/test").Page, state: BrowserState) {
  await page.route("**/favicon.ico", route => route.abort());
  await page.addInitScript((state) => {
    window.__ERA_NATIVE__ = true;
    window.__ERA_INITIAL_STATE__ = state;
    window.__windowTest = { samples: [], overlays: [], glass: [], layoutCount: 0, popupUpdates: 0 };
    window.ipc = {
      postMessage(message) {
        const command = JSON.parse(message);
        if (command.type === "layout") {
          window.__windowTest.layout = command;
          window.__windowTest.layoutCount++;
        }
        if (command.type === "popupState" && command.popup?.popupArea) window.__windowTest.popupUpdates++;
        if (command.type === "overlay") window.__windowTest.overlays.push(command.visible);
        if (command.type === "windowChrome") window.__windowTest.chrome = command;
        if (command.type === "resizeAppearance") window.__windowTest.reduceMotion = command.reduceMotion;
        if (command.type === "glass") {
          window.__windowTest.glass.push(command.enabled);
        }
        if (command.type === "presented") {
          window.__windowTest.presented = {
            theme: document.documentElement.dataset.theme,
            width: document.querySelector(".sidebar-track")!.getBoundingClientRect().width,
            fonts: document.fonts.status,
          };
        }
      },
    };
  }, state);
  await page.goto("/");
  await expect.poll(() => page.evaluate(() => !!window.__windowTest.presented)).toBe(true);
}

test("first render restores the native theme and collapsed sidebar before showing", async ({ page }) => {
  const state = initialState();
  state.theme = "dark";
  state.sidebarCollapsed = true;
  await nativeShell(page, state);
  expect(await page.evaluate(() => window.__windowTest.presented)).toEqual({
    theme: "dark", width: 0, fonts: "loaded",
  });
  expect(await page.evaluate(() => window.__windowTest.layout?.primary.x))
    .toBe((await page.locator(".page-pane > .webview-slot").first().boundingBox())!.x);
  await expect(page.getByRole("navigation", { name: "网页导航" })).toBeVisible();
  await expect(page.getByRole("button", { name: "搜索或输入地址", exact: true })).toBeVisible();
});

test("tab state updates reuse caption colors while theme changes refresh them", async ({ page }) => {
  const state = initialState();
  await page.addInitScript(() => {
    const read = CanvasRenderingContext2D.prototype.getImageData;
    CanvasRenderingContext2D.prototype.getImageData = function (...args: Parameters<typeof read>) {
      window.__windowTest.paletteReads = (window.__windowTest.paletteReads || 0) + 1;
      return read.apply(this, args);
    };
  });
  await nativeShell(page, state);
  const reads = await page.evaluate(() => window.__windowTest.paletteReads);
  expect(reads).toBeGreaterThan(0);
  for (const id of ["github", "wiki", "start", "github"]) {
    state.activeTab = id;
    state.spaces[0]!.activeTab = id;
    await page.evaluate(state => window.__ERA_RECEIVE__!({ type: "state", state }), state);
    await expect(page.locator('.tab-row.selected')).toHaveAttribute('data-tab-id', id);
  }
  expect(await page.evaluate(() => window.__windowTest.paletteReads)).toBe(reads);
  state.theme = "dark";
  await page.evaluate(state => window.__ERA_RECEIVE__!({ type: "state", state }), state);
  await expect.poll(() => page.evaluate(() => window.__windowTest.paletteReads)).toBeGreaterThan(reads!);
});

test("legacy frosted spaces use the single clear appearance and caption alpha", async ({ page }) => {
  const state = initialState();
  const space = state.spaces[0]!;
  Object.assign(space, { translucency: 1, clearGlass: false });
  await nativeShell(page, state);
  const alpha = () => page.evaluate(() => window.__windowTest.chrome?.background[3]);
  await expect(page.locator('.browser-shell')).toHaveClass(/glass/);
  await expect.poll(alpha).toBe(64);
  expect(await page.evaluate(() => window.__windowTest.glass.at(-1))).toBe(true);
  state.activeSpace = 'work';
  await page.evaluate(state => window.__ERA_RECEIVE__!({ type: 'state', state }), state);
  await expect.poll(alpha).toBe(255);
  await expect.poll(() => page.evaluate(() => window.__windowTest.glass.at(-1))).toBe(false);
  state.activeSpace = 'personal';
  state.theme = 'dark';
  await page.evaluate(state => window.__ERA_RECEIVE__!({ type: 'state', state }), state);
  await expect.poll(alpha).toBe(64);
  space.translucency = 0;
  await page.evaluate(state => window.__ERA_RECEIVE__!({ type: 'state', state }), state);
  await expect(page.locator('.browser-shell')).not.toHaveClass(/glass/);
  await expect.poll(alpha).toBe(255);
});

async function pauseFrame(page: import("@playwright/test").Page, state: BrowserState, time = 100) {
  return page.evaluate(async ({ state, time }) => {
    window.__ERA_RECEIVE__!({ type: "state", state });
    await new Promise<void>(resolve => requestAnimationFrame(() => resolve()));
    const shell = document.querySelector<HTMLElement>(".browser-shell")!;
    const animation = shell.getAnimations()[0]!;
    animation.pause();
    await animation.ready;
    animation.currentTime = time;
    const style = getComputedStyle(shell);
    const context = document.createElement("canvas").getContext("2d")!;
    const rgba = (color: string) => {
      context.clearRect(0, 0, 1, 1);
      context.fillStyle = color;
      context.fillRect(0, 0, 1, 1);
      return [...context.getImageData(0, 0, 1, 1).data];
    };
    return {
      color: rgba(style.backgroundColor),
      image: style.backgroundImage,
      start: rgba(style.getPropertyValue("--frame-gradient-start")),
      end: rgba(style.getPropertyValue("--frame-gradient-end")),
      grain: Number(getComputedStyle(shell, "::before").opacity),
      stops: (animation.effect as KeyframeEffect).getKeyframes().map(frame => ({
        start: rgba(String(frame["--frame-gradient-start"])),
        end: rgba(String(frame["--frame-gradient-end"])),
      })),
      glass: window.__windowTest.glass.at(-1),
    };
  }, { state, time });
}

async function finishFrame(page: import("@playwright/test").Page) {
  await page.locator(".browser-shell").evaluate(shell => shell.getAnimations().forEach(animation => animation.finish()));
  await expect.poll(() => page.locator(".browser-shell").evaluate(shell => shell.getAnimations().length)).toBe(0);
}

for (const custom of [false, true]) {
  test(`${custom ? "custom" : "named"} glass spaces interpolate alpha and retain the native blur until opaque`, async ({ page }) => {
    const state = initialState();
    state.spaces[0]!.translucency = 1;
    state.spaces[1]!.translucency = 0.45;
    if (custom) {
      Object.assign(state.spaces[0]!, { hue: 0.6, brightness: 0.2 });
      Object.assign(state.spaces[1]!, { hue: 0.3, brightness: 0.4 });
    }
    await nativeShell(page, state);
    state.activeSpace = "work";
    const betweenGlass = await pauseFrame(page, state);
    expect(betweenGlass.color[3]).toBeGreaterThan(64);
    expect(betweenGlass.color[3]).toBeLessThan(169);
    expect(betweenGlass.glass).toBe(true);
    await finishFrame(page);

    state.activeSpace = "studio";
    const toOpaque = await pauseFrame(page, state);
    expect(toOpaque.color[3]).toBeGreaterThan(169);
    expect(toOpaque.color[3]).toBeLessThan(255);
    expect(toOpaque.glass).toBe(true);
    await finishFrame(page);
    expect(await page.evaluate(() => window.__windowTest.glass.at(-1))).toBe(false);

    state.activeSpace = "personal";
    const toGlass = await pauseFrame(page, state);
    expect(toGlass.color[3]).toBeGreaterThan(64);
    expect(toGlass.color[3]).toBeLessThan(255);
    expect(toGlass.glass).toBe(true);
    await finishFrame(page);
  });
}

test("glass gradients and grain retarget from the visible frame, including switches to and from solid colors", async ({ page }) => {
  const state = initialState();
  Object.assign(state.spaces[0]!, { hue: 0.6, brightness: 0.12, hue2: 0.9, brightness2: 0.6, angle: 90, translucency: 0.8, grain: 0.8 });
  Object.assign(state.spaces[1]!, { hue: 0.1, brightness: 0.3, hue2: 0.4, brightness2: 0.5, angle: 30, translucency: 0.2, grain: 0.2 });
  await nativeShell(page, state);
  state.activeSpace = "work";
  const between = await pauseFrame(page, state, 80);
  expect(between.image).toContain("linear-gradient");
  expect(between.start[3]).toBeGreaterThan(102);
  expect(between.start[3]).toBeLessThan(217);
  expect(between.end[3]).toBe(between.start[3]);
  for (const stop of between.stops) {
    expect(between.start).not.toEqual(stop.start);
    expect(between.end).not.toEqual(stop.end);
  }
  expect(between.grain).toBeGreaterThan(0.036);
  expect(between.grain).toBeLessThan(0.144);

  state.activeSpace = "studio";
  const interrupted = await pauseFrame(page, state, 0);
  expect(interrupted.image).toBe(between.image);
  expect(interrupted.grain).toBeCloseTo(between.grain, 5);
  await finishFrame(page);
  await expect(page.locator(".browser-shell")).toHaveCSS("background-image", "none");
  expect(await page.locator(".browser-shell").evaluate(shell => getComputedStyle(shell, "::before").opacity)).toBe("0");
  expect(await page.evaluate(() => window.__windowTest.glass.at(-1))).toBe(false);

  state.activeSpace = "personal";
  const toGradient = await pauseFrame(page, state);
  expect(toGradient.image).toContain("linear-gradient");
  expect(toGradient.start[3]).toBeGreaterThan(102);
  expect(toGradient.start[3]).toBeLessThan(255);
  expect(toGradient.grain).toBeGreaterThan(0);
  expect(toGradient.grain).toBeLessThan(0.144);
  await finishFrame(page);
  expect(await page.locator(".browser-shell").evaluate(shell => Number(getComputedStyle(shell, "::before").opacity))).toBeCloseTo(0.144);
});

test("a quick return to glass cancels the outgoing backdrop shutdown", async ({ page }) => {
  const state = initialState();
  state.spaces[0]!.translucency = 1;
  await nativeShell(page, state);
  state.activeSpace = "work";
  const outgoing = await pauseFrame(page, state, 70);
  state.activeSpace = "personal";
  const returning = await pauseFrame(page, state, 0);
  expect(returning.color).toEqual(outgoing.color);
  await finishFrame(page);
  expect(await page.evaluate(() => window.__windowTest.glass)).toEqual([true]);
});

for (const preference of ["system", "app"] as const) {
  test(`${preference} reduced motion settles an in-progress glass transition and its native backdrop`, async ({ page }) => {
    const state = initialState();
    state.spaces[0]!.translucency = 1;
    await nativeShell(page, state);
    state.activeSpace = "work";
    await pauseFrame(page, state);
    if (preference === "system") await page.emulateMedia({ reducedMotion: "reduce" });
    else {
      state.settings!.reduceMotion = true;
      await page.evaluate(state => window.__ERA_RECEIVE__!({ type: "state", state }), state);
    }
    await expect.poll(() => page.locator(".browser-shell").evaluate(shell => shell.getAnimations().length)).toBe(0);
    expect(await page.evaluate(() => window.__windowTest.glass.at(-1))).toBe(false);
    state.activeSpace = "personal";
    await page.evaluate(state => window.__ERA_RECEIVE__!({ type: "state", state }), state);
    await expect(page.locator(".browser-shell")).toHaveClass(/glass/);
    expect(await page.locator(".browser-shell").evaluate(shell => shell.getAnimations().length)).toBe(0);
    expect(await page.evaluate(() => window.__windowTest.glass.at(-1))).toBe(true);
  });
}

test("reduced transparency keeps custom glass gradients opaque", async ({ page }) => {
  const session = await page.context().newCDPSession(page);
  await session.send("Emulation.setEmulatedMedia", {
    features: [{ name: "prefers-reduced-transparency", value: "reduce" }],
  });
  const state = initialState();
  Object.assign(state.spaces[0]!, { hue: 0.6, brightness: 0.12, hue2: 0.9, brightness2: 0.6, translucency: 1 });
  await nativeShell(page, state);
  await expect(page.locator(".browser-shell")).not.toHaveClass(/glass/);
  expect(await page.evaluate(() => window.__windowTest.glass.at(-1))).toBe(false);
  const alpha = await page.locator(".browser-shell").evaluate(shell => {
    const context = document.createElement("canvas").getContext("2d")!;
    context.fillStyle = getComputedStyle(shell).getPropertyValue("--frame-gradient-start");
    context.fillRect(0, 0, 1, 1);
    return context.getImageData(0, 0, 1, 1).data[3];
  });
  expect(alpha).toBe(255);
});

test("native caption geometry keeps three equal buttons flush with the window edge", async ({ page }) => {
  await nativeShell(page, initialState());
  for (const width of [1280, 480, 960]) {
    await page.setViewportSize({ width, height: 820 });
    await expect.poll(() => page.evaluate(() => window.__windowTest.chrome?.viewportWidth)).toBe(width);
    const buttons = await page.locator('.window-controls button').evaluateAll(buttons =>
      buttons.map(button => {
        const { x, y, width, height } = button.getBoundingClientRect();
        return { x, y, width, height };
      }),
    );
    expect(buttons).toEqual([0, 1, 2].map(index => ({
      x: width - 138 + index * 46, y: 0, width: 46, height: 32,
    })));
    expect(await page.evaluate(() => window.__windowTest.chrome?.maximize)).toEqual(buttons[1]);
    const navigation = (await page.locator('.browser-navigation').boundingBox())!;
    const address = (await page.locator('.address-field').boundingBox())!;
    expect(navigation.y).toBeGreaterThanOrEqual(0);
    expect(navigation.y + navigation.height).toBeLessThanOrEqual(32);
    expect(address.y).toBeGreaterThanOrEqual(0);
    expect(address.y + address.height).toBeLessThanOrEqual(32);
    expect(address.width).toBeGreaterThan(100);
    expect(navigation.x + navigation.width).toBeLessThan(address.x);
    expect(address.x + address.width).toBeLessThan(buttons[0]!.x);
  }
});

test("native titlebar dragging avoids WebView2 app-region windows and toolbar controls", async ({ page }) => {
  await nativeShell(page, initialState());
  for (const width of [1280, 480, 960]) {
    await page.setViewportSize({ width, height: 820 });
    await expect.poll(() => page.evaluate(() => window.__windowTest.chrome?.viewportWidth)).toBe(width);
    const regions = await page.evaluate(() => window.__windowTest.chrome!.dragRegions);
    expect(regions.length).toBeGreaterThan(0);
    expect(await page.locator('.titlebar').evaluate(el => getComputedStyle(el).getPropertyValue('app-region'))).not.toBe('drag');
    const controls = await page.locator('.browser-navigation, .address-field, .window-controls').evaluateAll(elements => elements.map(el => {
      const { x, y, width, height } = el.getBoundingClientRect();
      return { x, y, width, height };
    }));
    for (const region of regions) {
      for (const control of controls) {
        expect(region.x >= control.x + control.width || region.x + region.width <= control.x
          || region.y >= control.y + control.height || region.y + region.height <= control.y).toBe(true);
      }
    }
  }
});

test("titlebar hit regions follow sidebar resizing even when toolbar controls keep their sizes", async ({ page }) => {
  await nativeShell(page, initialState());
  const original = (await page.locator('.browser-navigation').boundingBox())!;
  for (const width of [360, 200, 300]) {
    await page.evaluate(width => window.__ERA_RECEIVE__!({ type: 'sidebarResize', width, resizing: true }), width);
    await expect.poll(async () => (await page.locator('.browser-navigation').boundingBox())!.x).toBe(width + 6);
    expect((await page.locator('.browser-navigation').boundingBox())!.width).toBe(original.width);
    await expect.poll(() => page.evaluate(() => {
      const regions = window.__windowTest.chrome!.dragRegions;
      return [...document.querySelectorAll('.browser-navigation, .address-field, .window-controls')].every(element => {
        const control = element.getBoundingClientRect();
        return regions.every(region => region.x >= control.right || region.x + region.width <= control.left
          || region.y >= control.bottom || region.y + region.height <= control.top);
      });
    })).toBe(true);
  }
  await page.evaluate(() => window.__ERA_RECEIVE__!({ type: 'sidebarResize', width: 300, resizing: false }));
});

test("native gradients fill the outer frame and caption ink retains its own alpha", async ({ page }) => {
  const state = initialState();
  Object.assign(state.spaces[0]!, { hue: 0.62, brightness: 0.18, hue2: 0.95, brightness2: 0.58, angle: 125 });
  await nativeShell(page, state);
  const shell = page.locator('.browser-shell');
  await expect(shell).toHaveCSS('clip-path', 'none');
  await expect(shell).toHaveCSS('border-radius', '0px');
  expect(await shell.evaluate(e => getComputedStyle(e).backgroundImage)).toContain('linear-gradient');
  await expect(page.locator('.window-controls svg').first()).toHaveCSS('visibility', 'hidden');
  // Native hover and glyphs are layered over the live gradient, so neither
  // may arrive precomposited onto the space's single fallback color.
  await expect.poll(() => page.evaluate(() => window.__windowTest.chrome?.foreground[3])).toBe(158);
  expect(await page.evaluate(() => window.__windowTest.chrome?.hover[3])).toBe(97);
  await page.evaluate(() => document.documentElement.dataset.theme = 'dark');
  await page.evaluate(() => window.dispatchEvent(new Event('resize')));
  await expect.poll(() => page.evaluate(() => window.__windowTest.chrome?.foreground[3])).toBe(168);
  expect(await page.evaluate(() => window.__windowTest.chrome?.hover[3])).toBe(13);
});

test("restored chrome inset keeps the address field below the resize strip", async ({ page }) => {
  await nativeShell(page, initialState());
  await page.setViewportSize({ width: 1280, height: 820 });
  const place = (selector: string) => page.locator(selector).boundingBox();
  await page.evaluate(() => document.documentElement.style.setProperty("--chrome-inset", "6px"));
  const titlebar = (await place(".titlebar"))!;
  const address = (await place(".address-field"))!;
  const navigation = (await place(".browser-navigation"))!;
  const body = (await place(".browser-body"))!;
  expect(titlebar).toMatchObject({ y: 0, height: 38 });
  expect(address.y).toBeGreaterThanOrEqual(6);
  expect(address.y + address.height).toBeLessThanOrEqual(titlebar.height);
  expect(navigation.y).toBeGreaterThanOrEqual(6);
  expect(navigation.y + navigation.height).toBeLessThanOrEqual(titlebar.height);
  expect(body.y).toBe(titlebar.height);
  const buttons = await page.locator(".window-controls button").evaluateAll((nodes) =>
    nodes.map((button) => {
      const { y, height } = button.getBoundingClientRect();
      const icon = button.querySelector("svg")!.getBoundingClientRect();
      return { y, height, iconCenter: icon.y + icon.height / 2 };
    }),
  );
  expect(buttons.map(({ y, height }) => ({ y, height }))).toEqual([
    { y: 0, height: 38 },
    { y: 0, height: 38 },
    { y: 0, height: 38 },
  ]);
  const addressCenter = address.y + address.height / 2;
  for (const button of buttons) expect(Math.abs(button.iconCenter - addressCenter)).toBeLessThan(1);
  await page.evaluate(() => document.documentElement.style.setProperty("--chrome-inset", "0px"));
  expect(await place(".titlebar")).toMatchObject({ y: 0, height: 32 });
  expect((await page.locator(".window-controls button").first().boundingBox())!.height).toBe(32);
  expect((await place(".address-field"))!.y).toBeGreaterThanOrEqual(0);
  expect((await place(".address-field"))!.y).toBeLessThan(6);
});

test("restores the full-width titlebar and framed page even for a saved borderless session", async ({ page }) => {
  await page.route("**/favicon.ico", route => route.abort());
  const state = initialState();
  state.borderless = true;
  await nativeShell(page, state);
  const titlebar = (await page.locator('.titlebar').boundingBox())!;
  const name = (await page.locator('.app-name').boundingBox())!;
  const sidebar = (await page.locator('.sidebar-track').boundingBox())!;
  const area = (await page.locator('.page-area').boundingBox())!;
  expect(titlebar).toMatchObject({ x: 0, y: 0, width: 1280, height: 32 });
  expect(name.x).toBe(12);
  expect(sidebar).toMatchObject({ x: 0, y: 32, width: 240, height: 788 });
  const heading = (await page.locator('.sidebar-header .space-name').boundingBox())!;
  const favorites = (await page.locator('.favorites').boundingBox())!;
  expect(heading.y).toBeGreaterThanOrEqual(sidebar.y);
  expect(heading.y + heading.height).toBeLessThanOrEqual(favorites.y);
  await expect(page.locator('.sidebar .address-field, .sidebar .browser-navigation, .space-list .space-name')).toHaveCount(0);
  await expect(page.getByRole('button', { name: '折叠侧栏', exact: true })).toHaveCount(0);
  expect(area).toMatchObject({ x: 240, y: 32, width: 1032, height: 780 });
  expect(await page.locator('.page-area').evaluate(e => getComputedStyle(e).borderRadius)).toBe('8px');
  expect(await page.locator('.browser-shell').getAttribute('class')).not.toContain('borderless');
  const content = (await page.locator('.nt-home').boundingBox())!;
  expect(Math.abs(content.x + content.width / 2 - area.x - area.width / 2)).toBeLessThan(1);
});

test("split chrome aligns the panes and relocates native window buttons when the sidebar collapses", async ({ page }) => {
  const state = initialState();
  state.activeTab = "github";
  state.tabs.find((t) => t.id === "github")!.splitWith = "wiki";
  await nativeShell(page, state);
  const left = page.getByRole("region", { name: "左侧窗格" });
  const right = page.getByRole("region", { name: "右侧窗格" });
  const caption = page.locator(".window-controls");
  await expect(page.locator(".titlebar")).toHaveCount(0);
  expect((await left.boundingBox())!.y).toBe((await right.boundingBox())!.y);
  expect((await caption.boundingBox())!.y).toBe((await left.boundingBox())!.y);
  expect((await caption.boundingBox())!.x).toBeLessThan((await left.boundingBox())!.x);
  await expect.poll(() => page.evaluate(() => window.__windowTest.chrome?.anchorLeft)).toBe(true);

  await page.evaluate((state) => window.__ERA_RECEIVE__!({ type: "state", state }), { ...state, sidebarCollapsed: true, sidebarHidden: true });
  await page.setViewportSize({ width: 480, height: 400 });
  await expect(page.locator(".sidebar-track")).toHaveCSS("width", "0px");
  await expect.poll(() => page.evaluate(() => window.__windowTest.chrome?.anchorLeft)).toBe(false);
  const buttons = (await caption.boundingBox())!;
  const rightPane = (await right.boundingBox())!;
  expect(buttons.x).toBeGreaterThan(rightPane.x);
  expect(buttons.x + buttons.width).toBeLessThan(rightPane.x + rightPane.width);
  await expect.poll(() => page.evaluate(() => {
    const regions = window.__windowTest.chrome?.dragRegions ?? [];
    const controls = [...document.querySelectorAll(".pane-header [data-window-control], .window-controls")]
      .map(el => el.getBoundingClientRect()).filter(rect => rect.width && rect.height);
    return regions.length > 0 && regions.length <= 32 && regions.every(region => controls.every(control =>
      region.x >= control.right || region.x + region.width <= control.x
      || region.y >= control.bottom || region.y + region.height <= control.y));
  })).toBe(true);
  expect((await right.getByRole("button", { name: "关闭分屏" }).boundingBox())!.x).toBeLessThan(buttons.x);
});

test("split chrome crossfades through a quick reversal while native bounds settle immediately", async ({ page }) => {
  const single = initialState();
  single.activeTab = "github";
  const split = structuredClone(single);
  split.tabs.find(tab => tab.id === "github")!.splitWith = "wiki";
  await nativeShell(page, single);
  await page.evaluate(() => {
    const animate = Element.prototype.animate;
    Element.prototype.animate = function (...args: Parameters<Element["animate"]>) {
      const animation = animate.apply(this, args);
      if (this.matches("[data-chrome-motion], [data-chrome-copy]")) {
        // Hold a presentation frame so the assertions don't depend on CPU timing.
        animation.pause();
        animation.currentTime = 40;
      }
      return animation;
    };
  });
  await page.evaluate(state => window.__ERA_RECEIVE__!({ type: "state", state }), split);
  const oldTitlebar = page.locator('[data-chrome-copy="titlebar"]');
  const leftHeader = page.locator('[data-chrome-motion="left-pane"]');
  const opacity = (element: import("@playwright/test").Locator) =>
    element.evaluate(node => Number(getComputedStyle(node).opacity));
  await expect(oldTitlebar).toBeAttached();
  await expect.poll(() => opacity(leftHeader)).toBeGreaterThan(0);
  const titlebarOpacity = await opacity(oldTitlebar);
  const leftOpacity = await opacity(leftHeader);
  expect(titlebarOpacity).toBeGreaterThan(0);
  expect(titlebarOpacity).toBeLessThan(1);
  expect(leftOpacity).toBeLessThan(1);
  await expect(page.locator(".chrome-outgoing")).toHaveAttribute("inert", "");
  await expect(page.getByRole("button", { name: "刷新" })).toHaveCount(2);
  expect(await page.evaluate(() => window.__windowTest.chrome?.anchorLeft)).toBe(true);
  const slot = (await page.locator(".webview-slot").first().boundingBox())!;
  expect(await page.evaluate(() => window.__windowTest.layout?.primary.y)).toBe(slot.y);

  await page.evaluate(state => window.__ERA_RECEIVE__!({ type: "state", state }), single);
  const newTitlebar = page.locator('[data-chrome-motion="titlebar"]');
  await expect(newTitlebar).toBeAttached();
  const restart = await newTitlebar.evaluate(node => {
    const effect = node.getAnimations()[0]!.effect as KeyframeEffect;
    return Number(effect.getKeyframes()[0]!.opacity);
  });
  expect(restart).toBeCloseTo(titlebarOpacity);
  await expect(page.locator(".chrome-outgoing")).toHaveCount(1);
  await expect(page.getByRole("button", { name: "刷新" })).toHaveCount(1);
  expect(await page.evaluate(() => window.__windowTest.chrome?.anchorLeft)).toBe(false);
  await page.evaluate(() => {
    for (const animation of document.getAnimations()) {
      if ((animation.effect as KeyframeEffect).target?.matches("[data-chrome-motion], [data-chrome-copy]")) animation.finish();
    }
  });
  await expect(page.locator(".chrome-outgoing")).toHaveCount(0);
  await expect(newTitlebar).toHaveCSS("opacity", "1");

  await page.emulateMedia({ reducedMotion: "reduce" });
  await expect.poll(() => page.evaluate(() => window.__windowTest.reduceMotion)).toBe(true);
  await page.evaluate(state => window.__ERA_RECEIVE__!({ type: "state", state }), split);
  await expect.poll(() => leftHeader.evaluate(node => node.getAnimations()[0]?.effect?.getTiming().duration)).toBe(80);
});

test("sidebar motion reports both pane bounds in the same paint frame, including reversal", async ({ page }) => {
  const state = initialState();
  state.activeTab = "github";
  state.tabs.find((t) => t.id === "github")!.splitWith = "wiki";
  await nativeShell(page, state);
  await page.evaluate(() => {
    document.documentElement.style.setProperty("--motion-slide", "400ms");
    window.__windowTest.observer = new ResizeObserver(() => {
      document.querySelectorAll(".page-pane > .webview-slot").forEach((slot, index) => {
        const rect = slot.getBoundingClientRect();
        const layout = window.__windowTest.layout;
        const reported = index === 0 ? layout?.primary : layout?.secondary;
        window.__windowTest.samples.push({ expected: rect.width, reported: reported?.width ?? -1 });
      });
    });
    window.__windowTest.observer.observe(document.querySelector(".page-area")!);
  });
  const collapse = (collapsed: boolean) => page.evaluate((state) => {
    window.__ERA_RECEIVE__!({ type: "state", state });
  }, { ...state, sidebarCollapsed: collapsed });
  await collapse(true);
  // Reverse while the track is still moving, then complete both directions.
  await expect.poll(() => page.locator(".sidebar-track").evaluate((e) => e.getBoundingClientRect().width)).toBeLessThan(200);
  await collapse(false);
  await expect(page.locator(".sidebar-track")).toHaveCSS("width", "240px");
  await collapse(true);
  await expect(page.locator(".sidebar-track")).toHaveCSS("width", "0px");
  const samples = await page.evaluate(() => {
    window.__windowTest.observer?.disconnect();
    return window.__windowTest.samples;
  });
  expect(samples.length).toBeGreaterThan(6);
  expect(Math.max(...samples.map((s) => Math.abs(s.expected - s.reported)))).toBeLessThan(0.1);
});

test("split dragging updates both live pane bounds without opening an overlay", async ({ page }) => {
  const state = initialState();
  state.activeTab = "github";
  state.tabs.find((t) => t.id === "github")!.splitWith = "wiki";
  await nativeShell(page, state);
  const divider = page.getByRole("separator", { name: "调整分屏比例" });
  const start = (await divider.boundingBox())!;
  const before = await page.locator(".webview-slot").evaluateAll(slots => slots.map(s => s.clientWidth));
  await page.evaluate(() => window.__windowTest.overlays = []);
  // Grab the edge rather than the center: the first move must not jump.
  await page.mouse.move(start.x + 1, start.y + 100);
  await page.mouse.down();
  await expect(page.locator('.page-area')).toHaveCSS('border-radius', '0px');
  await expect(page.locator('.page-area')).toHaveCSS('box-shadow', 'none');
  await expect.poll(() => page.evaluate(() => (window.__windowTest.layout as Layout & { liveResize: boolean })?.liveResize)).toBe(true);
  for (const delta of [1, 160, -220, 1200, -1200, 80]) {
    await page.mouse.move(start.x + 1 + delta, start.y + 100, { steps: 4 });
    const widths = await page.locator(".webview-slot").evaluateAll(slots => slots.map(s => s.getBoundingClientRect().width));
    await expect.poll(() => page.evaluate((widths) => {
      const layout = window.__windowTest.layout!;
      return Math.max(Math.abs(layout.primary.width - widths[0]!), Math.abs(layout.secondary!.width - widths[1]!));
    }, widths)).toBeLessThan(0.1);
    if (delta === 1) expect(Math.abs(widths[0]! - before[0]! - 1)).toBeLessThan(1);
    expect(Number(await divider.getAttribute("aria-valuenow"))).toBeGreaterThanOrEqual(20);
    expect(Number(await divider.getAttribute("aria-valuenow"))).toBeLessThanOrEqual(80);
    expect(await page.evaluate(() => window.__windowTest.overlays)).toEqual([]);
  }
  await page.mouse.up();
  await expect(page.locator('.page-area')).toHaveCSS('border-radius', '8px');
  await expect.poll(() => page.evaluate(() => (window.__windowTest.layout as Layout & { liveResize: boolean })?.liveResize)).toBe(false);
  const settled = await divider.getAttribute("aria-valuenow");
  await page.mouse.move(start.x - 250, start.y + 100);
  await expect(divider).toHaveAttribute("aria-valuenow", settled!);
  expect(await page.evaluate(() => window.__windowTest.overlays)).toEqual([]);
});

test("split input submits the real viewport bounds before waiting for layout or paint", async ({ page }) => {
  const state = initialState();
  state.activeTab = "github";
  state.tabs.find((t) => t.id === "github")!.splitWith = "wiki";
  await nativeShell(page, state);
  const divider = page.getByRole("separator", { name: "调整分屏比例" });
  const start = (await divider.boundingBox())!;
  await page.mouse.move(start.x + 1, start.y + 100);
  await page.mouse.down();
  await page.evaluate(async () => {
    await new Promise<void>(resolve => requestAnimationFrame(() => resolve()));
    window.__windowTest.layoutCount = 0;
    window.__windowTest.popupUpdates = 0;
    const divider = document.querySelector(".split-divider")!;
    for (let i = 0; i < 200; i++) {
      divider.dispatchEvent(new PointerEvent("pointermove", { clientX: 600 + i, clientY: 200, bubbles: true }));
      // Every move reaches the native bridge synchronously, before Vue or rAF.
      const left = window.__windowTest.layout!.primary;
      if (Math.abs(left.x + left.width - (599 + i)) > 0.1) throw Error('Native bounds lag behind input');
    }
  });
  expect(await page.evaluate(() => window.__windowTest.layoutCount)).toBe(200);
  expect(await divider.boundingBox()).toMatchObject({ x: 798 });
  expect(await page.evaluate(() => window.__windowTest.popupUpdates)).toBe(0);
  // A cancelled drag must stop receiving movement just like a normal release.
  await divider.dispatchEvent("pointercancel");
  await expect(page.locator('.page-area')).toHaveCSS('border-radius', '8px');
  await page.mouse.move(start.x - 200, start.y + 100);
  expect(await divider.boundingBox()).toMatchObject({ x: 798 });
  await page.mouse.up();
});

test("native split feedback updates the shell without sending a resize back while captured", async ({ page }) => {
  const state = initialState();
  state.activeTab = "github";
  state.tabs.find(t => t.id === "github")!.splitWith = "wiki";
  await nativeShell(page, state);
  await page.evaluate(() => {
    window.__windowTest.layoutCount = 0;
    window.__ERA_RECEIVE__!({ type: 'splitResize', ratio: 0.6, resizing: true });
  });
  await expect(page.getByRole('separator', { name: '调整分屏比例' })).toHaveAttribute('aria-valuenow', '60');
  await expect(page.locator('.page-area')).toHaveCSS('border-radius', '0px');
  expect(await page.evaluate(() => window.__windowTest.layoutCount)).toBe(0);
  await page.evaluate(() => window.__ERA_RECEIVE__!({ type: 'splitResize', ratio: 0.6, resizing: false }));
  await expect(page.locator('.page-area')).toHaveCSS('border-radius', '8px');
  await expect.poll(() => page.evaluate(() => window.__windowTest.layoutCount)).toBe(1);
  await page.evaluate(() => window.__ERA_RECEIVE__!({ type: 'splitResize', ratio: 0.6, resizing: true }));
  await expect(page.locator('.page-area')).toHaveClass(/resizing/);
  state.tabs.find(t => t.id === 'github')!.splitWith = undefined;
  await page.evaluate(state => window.__ERA_RECEIVE__!({ type: 'state', state }), state);
  await expect(page.getByRole('separator', { name: '调整分屏比例' })).toHaveCount(0);
  await expect(page.locator('.page-area')).not.toHaveClass(/resizing/);
});
