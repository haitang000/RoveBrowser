// Settings port integration: real WebView2, temporary profile, loopback fixtures.
import { chromium } from "@playwright/test";
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { mkdtemp, mkdir, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { once } from "node:events";
import assert from "node:assert/strict";
const profile = await mkdtemp(join(tmpdir(), "rove-settings-"));
const downloads = join(profile, "Downloads");
const output = resolve("test-results/native-settings");
const executable = resolve(
  process.env.ERA_TEST_EXE || "native/target/debug/rove.exe",
);
await mkdir(downloads);
await mkdir(output, { recursive: true });
const port = 19786,
  checks = [];
let app, uiBrowser, webBrowser, ui;
const server = createServer((req, res) => {
  if (req.url === "/file") {
    res.writeHead(200, {
      "Content-Type": "text/plain",
      "Content-Disposition": 'attachment; filename="settings-port.txt"',
    });
    res.end("Rove settings download");
    return;
  }
  res.writeHead(200, { "Content-Type": "text/html; charset=utf-8" });
  res.end(
    '<!doctype html><title>Settings native fixture</title><h1>Settings native fixture</h1><a href="/file" download>Download fixture</a>',
  );
});
await new Promise((r) => server.listen(0, "127.0.0.1", r));
const base = `http://127.0.0.1:${server.address().port}`;
async function waitFor(fn, label) {
  const until = Date.now() + 20000;
  while (Date.now() < until) {
    try {
      const v = await fn();
      if (v) return v;
    } catch {}
    await new Promise((r) => setTimeout(r, 80));
  }
  throw Error(`Timed out: ${label}`);
}
async function send(command) {
  await ui.evaluate((c) => window.ipc.postMessage(JSON.stringify(c)), command);
}
async function state() {
  return ui.evaluate(() => window.__settingsTest.state);
}
async function launch() {
  app = spawn(executable, [], {
    env: {
      ...process.env,
      ERA_DATA_DIR: profile,
      ERA_DEBUG_PORT: String(port),
    },
    windowsHide: true,
    stdio: "pipe",
  });
  app.stderr.on("data", (d) => process.stderr.write(d));
  uiBrowser = await waitFor(
    () => chromium.connectOverCDP(`http://127.0.0.1:${port}`),
    "UI debugger",
  );
  ui = await waitFor(
    () =>
      uiBrowser
        .contexts()
        .flatMap((c) => c.pages())
        .find((p) => p.url() === "https://era.ui/index.html"),
    "trusted UI",
  );
  await ui.locator(".sidebar").waitFor();
  await ui.evaluate(() => {
    window.__settingsTest = { state: window.__ERA_INITIAL_STATE__, events: [] };
    const receive = window.__ERA_RECEIVE__;
    window.__ERA_RECEIVE__ = (event) => {
      if (event.type === "state") window.__settingsTest.state = event.state;
      window.__settingsTest.events.push(event);
      receive(event);
    };
  });
  await send({ type: "ready" });
}
async function stop() {
  await webBrowser?.close();
  webBrowser = undefined;
  if (app && app.exitCode === null) {
    const exited = once(app, "exit");
    await send({ type: "window", action: "close" });
    await exited;
  }
  await uiBrowser?.close();
  uiBrowser = undefined;
}
try {
  await launch();
  await send({
    type: "settingsUpdate",
    settings: {
      ...(await state()).settings,
      askBeforeQuit: false,
      downloadFolder: downloads,
      searchEngine: "custom",
      customSearchTemplate: base + "/search?q=%s",
      searchSuggestions: false,
      reduceMotion: true,
      largeSpaceNames: false,
      favoritesPerRow: 2,
      savePasswords: false,
      autofill: false,
      shortcuts: { new: { key: 89, ctrl: true, alt: false, shift: true } },
    },
  });
  await waitFor(
    async () => (await state()).settings.searchEngine === "custom",
    "preferences broadcast",
  );
  await send({ type: "settingsOpen", pane: "privacy" });
  await ui.locator(".settings-page").waitFor();
  const id = (await state()).activeTab;
  await ui.getByRole("button", { name: "设置", exact: true }).click();
  assert.equal(
    (await state()).tabs.filter((t) => t.url.startsWith("rove://settings"))
      .length,
    1,
  );
  assert.equal(
    (await state()).tabs.find((t) => t.id === id).url,
    "rove://settings/privacy",
  );
  checks.push("设置以内部标签页展示、复用，分类地址由 Rust 保存");
  await ui.screenshot({ path: join(output, "native-settings-privacy.png") });
  await send({ type: "new", url: "native 中文 & query" });
  await waitFor(async () => {
    const s = await state();
    return s.tabs
      .find((t) => t.id === s.activeTab)
      ?.url.startsWith(base + "/search?");
  }, "native custom search");
  webBrowser = await waitFor(
    () => chromium.connectOverCDP(`http://127.0.0.1:${port + 1}`),
    "page debugger",
  );
  const page = await waitFor(
    () =>
      webBrowser
        .contexts()
        .flatMap((c) => c.pages())
        .find((p) => p.url().startsWith(base + "/search?")),
    "search fixture",
  );
  await page.locator("h1").waitFor();
  // Playwright's CDP default denies downloads. Restore WebView2 behavior without
  // supplying a destination: the application must choose the saved folder.
  const downloadSession = await page.context().newCDPSession(page);
  await downloadSession.send("Browser.setDownloadBehavior", {
    behavior: "default",
  });
  assert.equal(
    new URL(page.url()).searchParams.get("q"),
    "native 中文 & query",
  );
  checks.push("真实网页与 Rust 地址解析使用自定义引擎并正确编码中文和符号");
  await page.evaluate(() => {
    localStorage.setItem("settings-check", "saved");
    document.cookie = "settings-check=present; path=/";
  });
  await page.getByText("Download fixture", { exact: true }).click();
  assert.equal(
    await waitFor(
      () => readFile(join(downloads, "settings-port.txt"), "utf8"),
      "download folder",
    ),
    "Rove settings download",
  );
  await page.getByText("Download fixture", { exact: true }).click();
  assert.equal(
    await waitFor(
      () => readFile(join(downloads, "settings-port (1).txt"), "utf8"),
      "duplicate download",
    ),
    "Rove settings download",
  );
  checks.push("真实下载写入设置的目录");
  await send({ type: "settingsOpen", pane: "privacy" });
  await waitFor(
    async () => (await state()).activeTab === id,
    "return to settings",
  );
  const fixtureId = (await state()).tabs.find((t) =>
    t.url.startsWith(base + "/search?"),
  ).id;
  await send({ type: "split", id, other: fixtureId });
  await ui.locator(".page-area.split").waitFor();
  await ui.locator(".settings-page").waitFor();
  assert.equal(await ui.locator(".page-pane").count(), 2);
  const secondary = await ui
    .locator(".page-pane")
    .nth(1)
    .locator(".webview-slot")
    .boundingBox();
  await waitFor(
    async () =>
      Math.abs((await page.evaluate(() => innerWidth)) - secondary.width) < 2,
    "native web pane resized beside settings",
  );
  await waitFor(
    async () =>
      (await state()).tabs.find((t) => t.id === id).splitWith === fixtureId,
    "settings split state",
  );
  await ui.screenshot({ path: join(output, "native-settings-split.png") });
  await send({ type: "unsplit" });
  await ui.locator(".page-area.split").waitFor({ state: "hidden" });
  checks.push("内部设置与真实网页支持分屏并可恢复单页");
  for (const kind of ["history", "cache", "siteData"]) {
    await send({ type: "clearBrowsingData", kind });
    await waitFor(
      () =>
        ui.evaluate(
          (kind) =>
            window.__settingsTest.events.some(
              (e) =>
                e.type === "dataCleared" &&
                e.action === kind &&
                e.message.startsWith("已清除"),
            ),
          kind,
        ),
      "clear " + kind,
    );
  }
  await send({ type: "history", query: "" });
  await waitFor(
    () =>
      ui.evaluate(() =>
        window.__settingsTest.events.some(
          (e) => e.type === "history" && e.entries.length === 0,
        ),
      ),
    "empty history",
  );
  await page.reload();
  await page.locator("h1").waitFor();
  assert.deepEqual(
    await page.evaluate(() => ({
      local: localStorage.getItem("settings-check"),
      cookies: document.cookie,
    })),
    { local: null, cookies: "" },
  );
  checks.push(
    "清除 SQLite 历史、WebView2 缓存与 Cookie／DOM 存储，等待原生完成回调",
  );
  await send({
    type: "settingsUpdate",
    settings: { ...(await state()).settings, rememberHistory: false },
  });
  await send({ type: "theme" });
  await waitFor(async () => (await state()).theme === "dark", "dark theme");
  await stop();
  await launch();
  const restored = await state();
  assert.equal(restored.settings.downloadFolder, downloads);
  assert.equal(restored.settings.searchEngine, "custom");
  assert.equal(restored.settings.rememberHistory, false);
  assert.equal(restored.settings.reduceMotion, true);
  assert.equal(restored.settings.shortcuts.new.key, 89);
  assert.equal(restored.theme, "dark");
  assert.equal(
    restored.tabs.find((t) => t.id === id).url,
    "rove://settings/privacy",
  );
  await ui.locator(".settings-page").waitFor();
  await ui.screenshot({ path: join(output, "native-settings-restored.png") });
  checks.push("重启后恢复引擎、外观、动态效果、快捷键、下载路径及设置分类");
  await writeFile(
    join(output, "native-settings.json"),
    JSON.stringify({ profile, checks }, null, 2),
  );
  console.log(JSON.stringify({ profile, checks }, null, 2));
} finally {
  if (app?.exitCode === null) {
    try {
      await stop();
    } catch {
      app.kill();
    }
  }
  await uiBrowser?.close().catch(() => {});
  await webBrowser?.close().catch(() => {});
  server.close();
}
