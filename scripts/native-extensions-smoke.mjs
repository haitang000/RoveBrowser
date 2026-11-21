// Real WebView2 extension lifecycle with local pages and an isolated browsing profile.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createHash, generateKeyPairSync } from "node:crypto";
import { once } from "node:events";
import { mkdir, mkdtemp, readFile, readdir, writeFile } from "node:fs/promises";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { chromium, expect } from "@playwright/test";
import { createLegacyNativeProfile } from "./native-profile-fixture.mjs";

const root = await mkdtemp(join(tmpdir(), "rove-extensions-"));
const profile = join(root, "profile");
const fixture = join(root, "本地扩展");
const packages = join(profile, "Extensions");
await mkdir(profile);
await mkdir(fixture);
createLegacyNativeProfile(profile);

const fixtureName = "Rove 扩展测试";
const executable = resolve(process.env.ERA_TEST_EXE || "native/target/debug/rove.exe");
const debugPort = Number(process.env.ERA_TEST_EXTENSION_DEBUG_PORT || "19832");
const server = createServer((req, res) => {
  if (req.url === "/blocked-fixture") {
    res.end("loopback fixture");
    return;
  }
  res.writeHead(200, { "Content-Type": "text/html; charset=utf-8" });
  res.end("<!doctype html><body>Extension smoke<script>fetch('/blocked-fixture').then(() => document.body.dataset.filter = 'allowed', () => document.body.dataset.filter = 'blocked');</script></body>");
});
server.listen(0, "127.0.0.1");
await once(server, "listening");
const origin = "http://127.0.0.1:" + server.address().port;

let app, uiBrowser, webBrowser, ui;
async function waitFor(check, label) {
  let result, lastError;
  const end = Date.now() + 15000;
  while (Date.now() < end) {
    try {
      result = await check();
      if (result) return result;
    } catch (error) {
      lastError = error;
    }
    await new Promise(resolve => setTimeout(resolve, 80));
  }
  throw new Error("Timed out: " + label, { cause: lastError });
}

function send(command) {
  return ui.evaluate(command => window.ipc.postMessage(JSON.stringify(command)), command);
}
function openSettings() {
  return send({ type: "settingsOpen", pane: "extensions" });
}
async function launch() {
  app = spawn(executable, [], {
    env: {
      ...process.env,
      ERA_DATA_DIR: profile,
      ERA_DEBUG_PORT: String(debugPort),
      ERA_EXTENSION_FIXTURE: fixture,
    },
    windowsHide: true,
    stdio: ["ignore", "ignore", "inherit"],
  });
  uiBrowser = await waitFor(() => chromium.connectOverCDP("http://127.0.0.1:" + debugPort, { timeout: 1000 }), "UI CDP");
  ui = await waitFor(() => uiBrowser.contexts().flatMap(c => c.pages())
    .find(p => p.url() === "https://era.ui/index.html"), "application UI");
  await ui.locator(".sidebar").waitFor();
  const settings = await ui.evaluate(() => window.__ERA_INITIAL_STATE__.settings);
  await send({ type: "settingsUpdate", settings: { ...settings, askBeforeQuit: false, reduceMotion: true } });
}
async function stop() {
  await webBrowser?.close();
  webBrowser = undefined;
  if (app?.exitCode === null) {
    const exit = once(app, "exit");
    let timeout;
    try {
      await send({ type: "window", action: "close" }).catch(error => {
        if (!/closed|disconnect/i.test(error.message)) throw error;
      });
      const [code] = await Promise.race([exit, new Promise((_, reject) => {
        timeout = setTimeout(() => reject(new Error("Rove did not exit")), 8000);
      })]);
      if (code !== 0) console.log("关闭时的进程退出码：" + code);
    } finally {
      clearTimeout(timeout);
    }
  }
  await uiBrowser?.close();
  uiBrowser = undefined;
}
const choose = () => ui.getByRole("button", { name: "加载本地扩展…" });
const review = () => ui.getByRole("region", { name: "确认安装扩展" });
const toggle = () => ui.getByRole("switch", { name: "启用 " + fixtureName });

async function browse(path, enabled) {
  const url = origin + path;
  await send({ type: "new", url });
  webBrowser ??= await waitFor(() => chromium.connectOverCDP("http://127.0.0.1:" + (debugPort + 1), { timeout: 1000 }), "page CDP");
  const page = await waitFor(() => webBrowser.contexts().flatMap(c => c.pages())
    .find(p => p.url() === url), url);
  await expect(page.locator("body")).toHaveAttribute("data-filter", enabled ? "blocked" : "allowed");
  if (enabled) {
    await expect(page.locator("body")).toHaveAttribute("data-extension", "worker-storage-ok");
  } else {
    assert.equal(await page.locator("body").getAttribute("data-extension"), null);
  }
}

try {
  await launch();
  await send({ type: "settingsOpen" });
  await ui.getByRole("searchbox", { name: "查找设置" }).fill("Chrome 扩展");
  await ui.getByRole("button", { name: /加载本地 Chrome 扩展/ }).click();
  await expect(ui.getByText("还没有安装扩展。")).toBeVisible();
  await choose().click();
  await expect(ui.getByRole("alert")).toContainText("缺少 manifest.json");

  await writeFile(join(fixture, "manifest.json"), JSON.stringify({
    manifest_version: 3,
    name: fixtureName,
    version: "1.0.0",
    permissions: ["storage", "declarativeNetRequest"],
    optional_permissions: ["tabs"],
    host_permissions: ["http://127.0.0.1/*"],
    content_scripts: [{ matches: ["<all_urls>"], js: ["content.js"] }],
    background: { service_worker: "worker.js" },
    declarative_net_request: { rule_resources: [{ id: "fixture", enabled: true, path: "rules.json" }] },
  }));
  const originalScript = "chrome.runtime.sendMessage('fixture').then(value => document.body.dataset.extension = value);";
  await writeFile(join(fixture, "content.js"), originalScript);
  await writeFile(join(fixture, "worker.js"), [
    "chrome.runtime.onMessage.addListener((message, sender, respond) => {",
    "  if (message !== 'fixture') return;",
    "  chrome.storage.local.set({ fixture: 'worker-storage-ok' })",
    "    .then(() => chrome.storage.local.get('fixture'))",
    "    .then(value => respond(value.fixture));",
    "  return true;",
    "});",
  ].join("\n"));
  await writeFile(join(fixture, "rules.json"), JSON.stringify([
    { id: 1, priority: 1, action: { type: "block" }, condition: { urlFilter: "/blocked-fixture", resourceTypes: ["xmlhttprequest"] } },
  ]));

  await choose().click();
  await expect(review().getByText(fixtureName, { exact: false })).toBeVisible();
  for (const declaration of ["declarativeNetRequest", "tabs", "http://127.0.0.1/*", "<all_urls>"]) {
    await expect(review().getByText(declaration, { exact: true })).toBeVisible();
  }
  await review().getByRole("button", { name: "取消" }).click();
  await expect(review()).toHaveCount(0);
  assert.deepEqual(await readdir(packages), []);

  await choose().click();
  await expect(review()).toBeVisible();
  await writeFile(join(fixture, "content.js"), "document.body.dataset.extension = 'changed-source';");
  await review().getByRole("button", { name: "安装扩展" }).click();
  await expect(toggle()).toHaveAttribute("aria-checked", "true");
  const [installed] = await readdir(packages);
  assert.equal(await readFile(join(packages, installed, "payload", "content.js"), "utf8"), originalScript);
  await browse("/installed", true);
  const popup = ui.context().pages().find(page => page.url().endsWith("?popup"));
  assert.ok(popup);
  for (const page of [ui, popup]) {
    assert.equal(await page.locator("body").getAttribute("data-extension"), null);
  }

  await openSettings();
  await toggle().click();
  await expect(toggle()).toHaveAttribute("aria-checked", "false");
  await browse("/disabled", false);
  await stop();

  await launch();
  await openSettings();
  await expect(toggle()).toHaveAttribute("aria-checked", "false");
  await toggle().click();
  await expect(toggle()).toHaveAttribute("aria-checked", "true");
  await browse("/reenabled", true);

  await openSettings();
  await choose().click();
  await review().getByRole("button", { name: "安装扩展" }).click();
  await expect(ui.getByRole("alert")).toContainText("该扩展已安装");
  assert.deepEqual(await readdir(packages), [installed]);
  await ui.getByRole("button", { name: "卸载 " + fixtureName }).click();
  await ui.getByRole("button", { name: "确认卸载" }).click();
  await expect(ui.getByText("还没有安装扩展。")).toBeVisible();
  assert.deepEqual(await readdir(packages), []);
  await browse("/removed", false);

  const manifestPath = join(fixture, "manifest.json");
  const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
  const key = generateKeyPairSync("rsa", { modulusLength: 2048 }).publicKey.export({ type: "spki", format: "der" });
  manifest.key = key.toString("base64");
  await writeFile(manifestPath, JSON.stringify(manifest));
  await openSettings();
  await choose().click();
  await review().getByRole("button", { name: "安装扩展" }).click();
  await expect(toggle()).toHaveAttribute("aria-checked", "true");
  const [keyedPackage] = await readdir(packages);
  const record = JSON.parse(await readFile(join(packages, keyedPackage, "record.json"), "utf8"));
  const expectedId = createHash("sha256").update(key).digest("hex").slice(0, 32)
    .replace(/[0-9a-f]/g, hex => String.fromCharCode(97 + parseInt(hex, 16)));
  assert.equal(record.id, expectedId);
  await ui.getByRole("button", { name: "卸载 " + fixtureName }).click();
  await ui.getByRole("button", { name: "确认卸载" }).click();
  await expect(ui.getByText("还没有安装扩展。")).toBeVisible();
  await stop();
  console.log("通过：目录校验、权限确认、取消、安装副本、网页脚本与拦截、界面隔离、启停、重启、重复安装、公钥标识和卸载。");
} finally {
  if (app?.exitCode === null) app.kill();
  await webBrowser?.close().catch(() => {});
  await uiBrowser?.close().catch(() => {});
  server.close();
}
