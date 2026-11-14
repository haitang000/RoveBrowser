// Real WebView2 -> IPC -> Rust -> SQLite -> restart coverage, with an isolated profile.
import { createLegacyNativeProfile } from "./native-profile-fixture.mjs";
import { chromium } from "@playwright/test";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { mkdtemp, mkdir, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import assert from "node:assert/strict";

const profile = await mkdtemp(join(tmpdir(), "rove-new-tab-"));
createLegacyNativeProfile(profile);
const output = resolve("test-results/native-new-tab");
await mkdir(output, { recursive: true });
const executable = resolve(process.env.ERA_TEST_EXE || "native/target/debug/rove.exe");
const checks = [];
let app, browser, ui;
async function waitFor(check, label) {
  const deadline = Date.now() + 20_000;
  while (Date.now() < deadline) {
    try { const value = await check(); if (value) return value; } catch {}
    await new Promise((done) => setTimeout(done, 80));
  }
  throw Error(`Timed out: ${label}`);
}
async function send(command) {
  await ui.evaluate((value) => window.ipc.postMessage(JSON.stringify(value)), command);
}
async function state() { return ui.evaluate(() => window.__newTabTestState); }
function passed(label) { checks.push(label); console.log(`PASS: ${label}`); }
async function launch() {
  app = spawn(executable, [], {
    env: { ...process.env, ERA_DATA_DIR: profile, ERA_DEBUG_PORT: "19896" },
    windowsHide: true, stdio: "pipe",
  });
  app.stderr.on("data", (data) => process.stderr.write(data));
  browser = await waitFor(() => chromium.connectOverCDP("http://127.0.0.1:19896"), "UI debugger");
  ui = await waitFor(() => browser.contexts().flatMap((context) => context.pages()).find((page) => page.url() === "https://era.ui/index.html"), "trusted UI");
  await ui.locator(".new-tab-page").waitFor();
  await ui.evaluate(() => {
    window.__newTabTestState = window.__ERA_INITIAL_STATE__;
    const receive = window.__ERA_RECEIVE__;
    window.__ERA_RECEIVE__ = (event) => {
      if (event.type === "state") window.__newTabTestState = event.state;
      receive(event);
    };
  });
  await send({ type: "ready" });
  await waitFor(async () => Boolean((await state())?.newTab), "native preferences");
  await send({ type: "settingsUpdate", settings: { ...(await state()).settings, askBeforeQuit: false, reduceMotion: true } });
}
async function stop() {
  if (app && app.exitCode === null) {
    const exited = once(app, "exit");
    await send({ type: "window", action: "close" });
    await exited;
  }
  await browser?.close();
  browser = undefined;
}
try {
  await launch();
  assert.equal(await ui.locator(".nt-clock").count(), 1);
  assert.equal(await ui.locator(".nt-suggestion").count(), 5);
  passed("legacy profile opens the new home with a live clock and suggestions");

  await ui.getByRole("button", { name: "编辑小组件", exact: true }).click();
  await ui.getByRole("button", { name: "添加日历", exact: true }).click();
  await ui.getByRole("button", { name: "添加便签", exact: true }).click();
  await ui.getByRole("button", { name: "添加GitHub 贡献图", exact: true }).click();
  await waitFor(async () => (await state()).newTab.widgets.length === 4, "all widgets saved");
  // A blur save immediately followed by another edit must not lose note text.
  await ui.getByRole("textbox", { name: "便签内容", exact: true }).fill("Native note: 记得出发。");
  await ui.getByRole("button", { name: "调整便签宽度", exact: true }).click();
  await ui.getByRole("button", { name: "前移便签", exact: true }).click();
  await ui.getByRole("textbox", { name: "首页称呼", exact: true }).fill("Computer32");
  await ui.getByRole("button", { name: "完成编辑", exact: true }).click();
  await waitFor(async () => {
    const value = (await state()).newTab;
    return value.displayName === "Computer32" && value.widgets[1]?.kind === "notes" && value.widgets[1].size === "wide" && value.widgets[1].text === "Native note: 记得出发。";
  }, "notes, name, size and order acknowledged together");
  passed("adding, resizing and sorting preserve note text through native IPC");

  await ui.getByRole("textbox", { name: "你的 GitHub 用户名", exact: true }).fill("octocat");
  await ui.getByRole("button", { name: "显示贡献图", exact: true }).click();
  await waitFor(async () => (await state()).newTab.widgets.find((widget) => widget.kind === "github")?.username === "octocat", "GitHub configuration");
  await ui.locator(".nt-contribution-summary").waitFor({ timeout: 20_000 });
  assert.ok(await ui.locator(".nt-contribution-grid > span").count() > 100);
  passed("native WebView2 fetches and renders public GitHub contributions");
  assert.equal(await ui.locator(".new-tab-page").evaluate((element) => element.scrollWidth <= element.clientWidth), true);
  passed("all four widgets stay inside the native pane without horizontal overflow");
  const greeting = await ui.locator(".nt-greeting").boundingBox();
  const suggestions = await ui.locator(".nt-suggestions").boundingBox();
  await ui.locator(".nt-widget-grid").evaluate((element) => { element.scrollTop = 600; });
  assert.deepEqual(await ui.locator(".nt-greeting").boundingBox(), greeting);
  assert.deepEqual(await ui.locator(".nt-suggestions").boundingBox(), suggestions);
  assert.equal(await ui.locator(".new-tab-page").evaluate((element) => element.scrollTop), 0);
  passed("widget scrolling keeps the greeting and suggestions fixed in the native pane");
  await ui.locator(".nt-widget-grid").evaluate((element) => { element.scrollTop = 0; });
  await ui.screenshot({ path: join(output, "home.png") });

  const saved = (await state()).newTab;
  await stop();
  await launch();
  assert.deepEqual((await state()).newTab, saved);
  assert.equal(await ui.getByRole("textbox", { name: "便签内容", exact: true }).inputValue(), "Native note: 记得出发。");
  passed("a fresh native process restores the layout, note and GitHub account from SQLite");

  await ui.getByRole("button", { name: "编辑小组件", exact: true }).click();
  await ui.screenshot({ path: join(output, "discover.png") });
  for (const title of ["时钟", "便签", "日历", "GitHub 贡献图"]) {
    await ui.getByRole("button", { name: `移除${title}`, exact: true }).click();
  }
  await ui.getByRole("button", { name: "完成编辑", exact: true }).click();
  await waitFor(async () => (await state()).newTab.widgets.length === 0, "empty layout");
  await stop();
  await launch();
  assert.equal((await state()).newTab.widgets.length, 0);
  assert.equal(await ui.locator("article.nt-widget").count(), 0);
  passed("an intentionally empty layout stays empty after restart");
  await writeFile(join(output, "report.json"), JSON.stringify({ checks, profile }, null, 2));
} catch (error) {
  if (ui && !ui.isClosed()) {
    await ui.screenshot({ path: join(output, "failure.png") }).catch(() => {});
    await writeFile(join(output, "failure.txt"), `${error.stack}\n\n${await ui.locator("body").innerText()}`);
  }
  throw error;
} finally {
  if (app?.exitCode === null) {
    try { await stop(); } catch { app.kill(); }
  }
  await browser?.close();
}
