// Settings port integration: real WebView2, temporary profile, loopback fixtures.
import { createLegacyNativeProfile } from './native-profile-fixture.mjs';
import { chromium } from "@playwright/test";
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { mkdtemp, mkdir, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { once } from "node:events";
import assert from "node:assert/strict";
const profile = await mkdtemp(join(tmpdir(), "rove-settings-"));
createLegacyNativeProfile(profile);
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
  const env = { ...process.env, ERA_DATA_DIR: profile, ERA_DEBUG_PORT: String(port) };
  delete env.ROVE_DEVTOOLS_MCP;
  app = spawn(executable, [], {
    env,
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
async function hasDirectWebRtcCandidates(page) {
  return page.evaluate(async () => {
    const connection = new RTCPeerConnection({ iceServers: [] });
    let hasDirectUdp = false;
    let timeout;
    try {
      const gathered = new Promise((resolve, reject) => {
        timeout = setTimeout(() => reject(new Error("ICE gathering timed out")), 5000);
        connection.onicecandidate = ({ candidate }) => {
          if (candidate === null) resolve(hasDirectUdp);
          else if (candidate.type === "host" && candidate.protocol === "udp") hasDirectUdp = true;
        };
      });
      connection.createDataChannel("settings-fixture");
      await connection.setLocalDescription(await connection.createOffer());
      return await gathered;
    } finally {
      clearTimeout(timeout);
      connection.close();
    }
  });
}
async function openWebRtcFixture() {
  const url = base + "/webrtc";
  await send({ type: "new", url });
  webBrowser = await waitFor(
    () => chromium.connectOverCDP(`http://127.0.0.1:${port + 1}`),
    "page debugger",
  );
  return waitFor(() => webBrowser.contexts().flatMap((c) => c.pages())
    .find((p) => p.url() === url), "WebRTC fixture");
}
async function debugTargets(port) {
  const response = await fetch(`http://127.0.0.1:${port}/json/list`, {
    signal: AbortSignal.timeout(1000),
  });
  return response.json();
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
  const popup = await waitFor(() => ui.context().pages().find(p => p.url().endsWith('?popup')), 'popup renderer');
  await ui.getByRole('button', { name: '浏览器菜单', exact: true }).click();
  await popup.getByRole('menuitem', { name: '设置', exact: true }).click();
  assert.equal(
    (await state()).tabs.filter((t) => t.url.startsWith("rove://settings"))
      .length,
    1,
  );
  assert.equal(
    (await state()).tabs.find((t) => t.id === id).url,
    "rove://settings/privacy",
  );
  const webRtcToggle = (name) => ui.getByRole("switch", { name, exact: true });
  assert.equal(await webRtcToggle("WebRTC 地址保护").getAttribute("aria-checked"), "false");
  checks.push("设置以内部标签页展示、复用，分类地址由 Rust 保存");
  await ui.screenshot({ path: join(output, "native-settings-privacy.png") });
  await ui.getByRole("button", { name: "关于 Rove", exact: true }).click();
  await waitFor(
    async () => (await state()).tabs.find((t) => t.id === id).url === "rove://settings/about",
    "about settings address",
  );
  await ui.getByRole("region", { name: "版本与软件更新" }).waitFor();
  await ui.screenshot({ path: join(output, "native-settings-about.png"), animations: "disabled" });
  await ui.getByRole("button", { name: "隐私", exact: true }).click();
  await waitFor(
    async () => (await state()).tabs.find((t) => t.id === id).url === "rove://settings/privacy",
    "return to privacy settings",
  );
  checks.push("关于页在原生设置标签中拥有独立地址和更新面板");
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
  assert.equal(await hasDirectWebRtcCandidates(page), true);
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
  await ui.getByRole("button", { name: "通用", exact: true }).click();
  await ui.getByRole("combobox", { name: "界面语言", exact: true }).selectOption("en");
  await waitFor(async () => (await state()).settings.language === "en", "English preference saved");
  await ui.getByRole("heading", { name: "General", exact: true }).waitFor();
  await ui.getByRole("button", { name: "Browser menu", exact: true }).click();
  await popup.getByRole("menuitem", { name: "Settings", exact: true }).click();
  await ui.getByRole("button", { name: "Privacy", exact: true }).click();
  await waitFor(async () => (await state()).tabs.find((t) => t.id === id)?.url === "rove://settings/privacy", "restore Privacy pane");
  await webRtcToggle("WebRTC address protection").click();
  await waitFor(async () => (await state()).settings.webRtcAddressProtection, "enable WebRTC address protection");
  await stop();
  await launch();
  const restored = await state();
  assert.equal(restored.settings.downloadFolder, downloads);
  assert.equal(restored.settings.searchEngine, "custom");
  assert.equal(restored.settings.rememberHistory, false);
  assert.equal(restored.settings.reduceMotion, true);
  assert.equal(restored.settings.language, "en");
  assert.equal(restored.settings.shortcuts.new.key, 89);
  assert.equal(restored.theme, "dark");
  assert.equal(
    restored.tabs.find((t) => t.id === id).url,
    "rove://settings/privacy",
  );
  await ui.locator(".settings-page").waitFor();
  await ui.getByRole("heading", { name: "Privacy", exact: true }).waitFor();
  assert.equal(await webRtcToggle("WebRTC address protection").getAttribute("aria-checked"), "true");
  await ui.screenshot({ path: join(output, "native-settings-restored.png") });
  checks.push("设置和浏览器菜单即时切换英语；重启后恢复界面语言、引擎、外观、动态效果、快捷键、下载路径及设置分类");
  assert.equal(await hasDirectWebRtcCandidates(await openWebRtcFixture()), false);
  await send({ type: "settingsOpen", pane: "privacy" });
  await webRtcToggle("WebRTC address protection").click();
  await waitFor(async () => (await state()).settings.webRtcAddressProtection === false, "disable WebRTC address protection");
  await stop();
  await launch();
  assert.equal((await state()).settings.webRtcAddressProtection, false);
  assert.equal(await hasDirectWebRtcCandidates(await openWebRtcFixture()), true);
  checks.push("WebRTC 地址保护默认关闭，重启后阻止直连 UDP 候选，关闭后再次重启恢复直连 UDP");

  await send({ type: "settingsOpen" });
  await ui.getByRole("button", { name: "Experimental", exact: true }).click();
  const mcpToggle = () => ui.getByRole("switch", { name: "Chrome DevTools MCP", exact: true });
  const discovery = join(profile, "WebView2", "EBWebView", "DevToolsActivePort");
  assert.equal(await mcpToggle().getAttribute("aria-checked"), "false");
  await mcpToggle().click();
  await waitFor(async () => (await state()).settings.devtoolsMcp, "enable DevTools MCP");
  await assert.rejects(readFile(discovery), { code: "ENOENT" });
  await stop();
  await launch();
  await ui.getByRole("heading", { name: "Experimental", exact: true }).waitFor();
  assert.equal(await mcpToggle().getAttribute("aria-checked"), "true");
  const mcpUrl = base + "/devtools";
  await send({ type: "new", url: mcpUrl });
  const mcpPort = await waitFor(async () => {
    const [port] = (await readFile(discovery, "utf8")).split(/\r?\n/);
    const targets = await debugTargets(port);
    return targets.some(target => target.url === mcpUrl) && port;
  }, "DevTools MCP endpoint after restart");
  assert.ok((await debugTargets(mcpPort)).every(target => !target.url.startsWith("https://era.ui/")));
  await send({ type: "settingsOpen", pane: "experimental" });
  await mcpToggle().click();
  await waitFor(async () => (await state()).settings.devtoolsMcp === false, "disable DevTools MCP");
  assert.ok((await debugTargets(mcpPort)).some(target => target.url === mcpUrl));
  await stop();
  await launch();
  assert.equal(await mcpToggle().getAttribute("aria-checked"), "false");
  await send({ type: "new", url: mcpUrl });
  await waitFor(async () => (await debugTargets(port + 1)).some(target => target.url === mcpUrl),
    "webpage launches with normal debug options");
  await assert.rejects(debugTargets(mcpPort));
  checks.push("实验性设置开关持久化，重启后开启或关闭网页的 MCP 调试端口");
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
