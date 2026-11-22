// Uses WebView2 permission requests from a real loopback site. No location
// provider, microphone, or camera is needed for these browser policy checks.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { mkdtemp } from "node:fs/promises";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { setTimeout as wait } from "node:timers/promises";
import { chromium } from "@playwright/test";
import { createLegacyNativeProfile } from "./native-profile-fixture.mjs";

const server = createServer((request, res) => {
  res.setHeader("Content-Type", "text/html; charset=utf-8");
  const embed = request.url === "/embed"
    ? `<iframe name="embedded" src="http://localhost:${server.address().port}/frame" allow="geolocation"></iframe>`
    : "";
  res.end(`<!doctype html><title>Permission fixture</title>
    <button id="notifications">Request notifications</button>
    <button id="screen">Share screen</button>
    <button id="location">Request location</button>${embed}
    <script>
      document.getElementById('notifications').onclick = async () => { document.title = await Notification.requestPermission(); };
      document.getElementById('screen').onclick = async () => {
        try { (await navigator.mediaDevices.getDisplayMedia()).getTracks().forEach(track => track.stop()); }
        catch (error) { document.title = error.name; }
      };
      document.getElementById('location').onclick = () => navigator.geolocation.getCurrentPosition(
        () => { document.title = 'location allowed'; },
        error => { document.title = 'location ' + error.code; }
      );
    </script>`);
});
server.listen(0, "127.0.0.1");
await once(server, "listening");
const host = server.address().port;
const origin = `http://127.0.0.1:${host}`;
const address = `${origin}/permission`;
const otherAddress = `http://localhost:${host}/other`;
const debugPort = Number(process.env.ERA_SMOKE_PORT || 19822);
const profile = await mkdtemp(join(tmpdir(), "rove-native-permissions-"));
createLegacyNativeProfile(profile);
let app, shell, popup, pages;

async function until(find, label) {
  for (let attempt = 0; attempt < 150; attempt++) {
    const result = await find().catch(() => undefined);
    if (result) return result;
    await wait(100);
  }
  throw Error(`Timed out: ${label}`);
}

function connect(port) {
  return until(() => chromium.connectOverCDP(`http://127.0.0.1:${port}`, { timeout: 1000 }), `debugger on ${port}`);
}

async function launch() {
  app = spawn(resolve(process.env.ERA_SMOKE_EXE || "native/target/debug/rove.exe"), [], {
    env: { ...process.env, ERA_DATA_DIR: profile, ERA_DEBUG_PORT: String(debugPort) },
    windowsHide: true,
    stdio: ["ignore", "ignore", "inherit"],
  });
  const browser = await connect(debugPort);
  [shell, popup] = await until(async () => {
    const targets = browser.contexts().flatMap((context) => context.pages());
    const main = targets.find((page) => page.url() === "https://era.ui/index.html");
    const floating = targets.find((page) => page.url().endsWith("?popup"));
    if (main && floating) return [main, floating];
  }, "trusted browser surfaces");
  await shell.locator(".sidebar").waitFor({ timeout: 10000 });
  pages = undefined;
}

async function stop() {
  if (!app || app.exitCode !== null) return;
  const exited = once(app, "exit");
  app.kill();
  await exited;
}

function send(command) {
  return shell.evaluate((value) => window.ipc.postMessage(JSON.stringify(value)), command);
}

async function pageAt(url) {
  if (!pages) pages = await connect(debugPort + 1);
  return until(async () => pages.contexts().flatMap((context) => context.pages())
    .find((page) => page.url() === url), url);
}

async function ask(page) {
  await page.getByRole("button", { name: "Request notifications" }).click();
}

async function expectTitle(page, value) {
  await until(async () => (await page.title()) === value, `notification result ${value}`);
}

try {
  await launch();
  await send({ type: "new", url: address });
  let site = await pageAt(address);
  const card = () => popup.getByRole("dialog", { name: "允许显示通知吗？" });
  await ask(site);
  await card().waitFor({ timeout: 8000 });
  assert.match(await card().innerText(), new RegExp(`${host}`));
  await card().getByRole("checkbox").uncheck();
  await card().getByRole("button", { name: "不允许", exact: true }).click();
  await expectTitle(site, "denied");
  await card().waitFor({ state: "hidden" });

  await Promise.all([site.waitForEvent("domcontentloaded"), send({ type: "reload" })]);
  await ask(site);
  await card().waitFor({ timeout: 8000 });
  await send({ type: "new", url: otherAddress });
  const other = await pageAt(otherAddress);
  await card().waitFor({ state: "hidden", timeout: 8000 });
  const pendingTab = shell.getByRole("tab", { name: /有权限请求/ });
  await pendingTab.waitFor({ timeout: 8000 });
  await pendingTab.click();
  await card().waitFor({ timeout: 8000 });
  await card().getByRole("button", { name: "允许", exact: true }).click();
  await expectTitle(site, "granted");

  await shell.getByTitle(otherAddress, { exact: true }).click();
  await ask(other);
  await card().waitFor({ timeout: 8000 });
  assert.match(await card().innerText(), /http:\/\/localhost:/);
  await card().press("Escape");
  await card().waitFor({ state: "hidden", timeout: 8000 });

  await send({ type: "settingsOpen", pane: "privacy" });
  const defaultNotifications = () => shell.getByRole("combobox", { name: "通知", exact: true });
  assert.equal(await defaultNotifications().inputValue(), "ask");
  await defaultNotifications().selectOption("block");
  await until(async () => (await defaultNotifications().inputValue()) === "block", "block notifications by default");
  await send({ type: "new", url: `${otherAddress}?blocked` });
  const blocked = await pageAt(`${otherAddress}?blocked`);
  await ask(blocked);
  await expectTitle(blocked, "denied");
  assert.equal(await card().count(), 0);

  await stop();
  await launch();
  await send({ type: "new", url: `${address}?restart` });
  site = await pageAt(`${address}?restart`);
  await ask(site);
  await expectTitle(site, "granted");
  assert.equal(await card().count(), 0);

  await send({ type: "settingsOpen", pane: "privacy" });
  const rule = shell.getByLabel(`通知 · ${origin}`);
  await rule.waitFor({ timeout: 8000 });
  assert.equal(await defaultNotifications().inputValue(), "block");
  assert.equal(await rule.inputValue(), "allow");
  await rule.selectOption({ label: "使用默认设置" });
  await rule.waitFor({ state: "detached", timeout: 8000 });
  await send({ type: "new", url: `${address}?reset` });
  site = await pageAt(`${address}?reset`);
  await ask(site);
  await expectTitle(site, "denied");
  assert.equal(await card().count(), 0);

  await send({ type: "settingsOpen", pane: "privacy" });
  await defaultNotifications().selectOption("ask");
  await send({ type: "new", url: `${address}?ask` });
  site = await pageAt(`${address}?ask`);
  await ask(site);
  await card().waitFor({ timeout: 8000 });
  await card().getByRole("button", { name: "暂不处理" }).click();
  await card().waitFor({ state: "hidden", timeout: 8000 });

  await site.getByRole("button", { name: "Share screen" }).click();
  const screen = popup.getByRole("dialog", { name: "允许共享屏幕吗？" });
  await screen.waitFor({ timeout: 8000 });
  assert.equal(await screen.getByRole("checkbox").count(), 0);
  await screen.getByRole("button", { name: "不允许", exact: true }).click();
  await expectTitle(site, "NotAllowedError");

  await send({ type: "new", url: `${origin}/embed` });
  const embed = await pageAt(`${origin}/embed`);
  const frame = await until(async () => embed.frame({ name: "embedded", url: /\blocalhost:/ }), "embedded frame");
  await frame.evaluate(() => document.getElementById("location").click());
  const location = popup.getByRole("dialog", { name: "允许访问你的位置吗？" });
  await location.waitFor({ timeout: 8000 });
  assert.match(await location.innerText(), /http:\/\/localhost:\d+\s*嵌入在 http:\/\/127\.0\.0\.1:\d+/);
  await location.getByRole("button", { name: "不允许", exact: true }).click();
  await until(async () => embed.frame({ name: "embedded" })?.evaluate(() => document.title === "location 1"), "embedded location denial");

  await ask(embed);
  await card().waitFor({ timeout: 8000 });
  await Promise.all([embed.waitForEvent("domcontentloaded"), send({ type: "reload" })]);
  await card().waitFor({ state: "hidden", timeout: 8000 });

  await send({ type: "settingsOpen", pane: "privacy" });
  await shell.getByRole("combobox", { name: "屏幕共享", exact: true }).selectOption("block");
  await send({ type: "new", url: `${address}?screen-blocked` });
  site = await pageAt(`${address}?screen-blocked`);
  await site.getByRole("button", { name: "Share screen" }).click();
  await expectTitle(site, "NotAllowedError");
  assert.equal(await screen.count(), 0);
  console.log("Native permission defaults, saved-rule priority, persistence, navigation, screen sharing, and embedded-origin checks passed.");
} finally {
  await stop();
  server.close();
}
