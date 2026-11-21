// Live Chrome Web Store integration. Only this opt-in smoke test needs Google.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { once } from "node:events";
import { mkdtemp, readFile, readdir } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { chromium, expect } from "@playwright/test";
import { createLegacyNativeProfile } from "./native-profile-fixture.mjs";

const profile = await mkdtemp(join(tmpdir(), "rove-chrome-store-"));
const packages = join(profile, "Extensions");
createLegacyNativeProfile(profile);
const id = "ddkjiahejlhfcafbddmgiahcphecmpfh";
const detail = "https://chromewebstore.google.com/detail/ublock-origin-lite/" + id + "?hl=en";
const port = Number(process.env.ERA_TEST_CHROME_STORE_DEBUG_PORT || "19864");
const executable = resolve(process.env.ERA_TEST_EXE || "native/target/debug/rove.exe");

let app, browser, pageBrowser, ui;
async function waitFor(check, label) {
  let lastError;
  const end = Date.now() + 15000;
  while (Date.now() < end) {
    try {
      const result = await check();
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
async function launch() {
  app = spawn(executable, [], {
    env: { ...process.env, ERA_DATA_DIR: profile, ERA_DEBUG_PORT: String(port) },
    windowsHide: true,
    stdio: ["ignore", "ignore", "inherit"],
  });
  browser = await waitFor(() => chromium.connectOverCDP("http://127.0.0.1:" + port, { timeout: 1000 }), "UI CDP");
  ui = await waitFor(() => browser.contexts().flatMap(context => context.pages())
    .find(page => page.url() === "https://era.ui/index.html"), "application UI");
  await ui.locator(".sidebar").waitFor();
  const settings = await ui.evaluate(() => window.__ERA_INITIAL_STATE__.settings);
  await send({ type: "settingsUpdate", settings: { ...settings, askBeforeQuit: false, reduceMotion: true } });
}
async function stop() {
  if (app.exitCode === null) {
    const exit = once(app, "exit");
    let timeout;
    try {
      await send({ type: "window", action: "close" }).catch(error => {
        if (!/closed|disconnect/i.test(error.message)) throw error;
      });
      await Promise.race([exit, new Promise((_, reject) => {
        timeout = setTimeout(() => reject(new Error("Rove did not exit")), 10000);
      })]);
    } finally {
      clearTimeout(timeout);
    }
  }
  await pageBrowser?.close();
  pageBrowser = undefined;
  await browser.close();
}

const review = () => ui.getByRole("region", { name: "确认安装扩展" });
const storeInput = () => ui.getByRole("textbox", { name: "商店扩展链接或 ID" });
const permissions = () => ui.getByRole("button", { name: "查看权限" });
const toggle = () => ui.getByRole("switch", { name: /uBlock Origin Lite/i });
const icon = () => ui.locator(".extension-row", { has: toggle() }).locator(".extension-icon img");

async function expectVerifiedReview(panel, alert) {
  await expect(panel.getByText("已验证 Chrome 网上应用店签名", { exact: false }).or(alert)).toBeVisible({ timeout: 45000 });
  if (await alert.isVisible()) throw new Error(await alert.innerText());
  await expect(panel).toContainText(/uBlock Origin Lite/i);
}

async function expectLoadedImage(image) {
  await expect(image).toBeVisible();
  await expect.poll(() => image.evaluate(img => img.naturalWidth)).toBeGreaterThan(0);
}

try {
  await launch();
  await send({ type: "settingsOpen", pane: "extensions" });
  await storeInput().fill("https://example.com/detail/sample/" + id);
  await permissions().click();
  await expect(ui.getByRole("alert")).toContainText("扩展详情链接或扩展 ID");

  await storeInput().fill(detail);
  await permissions().click();
  await expectVerifiedReview(review(), ui.getByRole("alert"));
  await review().getByRole("button", { name: "取消" }).click();
  await expect(review()).toHaveCount(0);
  assert.deepEqual(await readdir(packages), []);

  await send({ type: "new", url: detail });
  const popup = await waitFor(() => ui.context().pages().find(page => page.url().endsWith("?popup")), "menu renderer");
  const dialog = popup.getByRole("dialog", { name: "确认安装扩展" });
  const confirm = dialog.getByRole("button", { name: "安装扩展" });
  const settings = ui.locator(".settings-page");
  await send({ type: "theme" });
  await expect(popup.locator("html")).toHaveAttribute("data-theme", "dark");
  await ui.getByRole("button", { name: "浏览器菜单", exact: true }).click();
  await popup.getByRole("menuitem", { name: "安装此商店扩展…" }).click();
  await expectVerifiedReview(dialog, dialog.getByRole("alert"));
  const darkAccent = await confirm.evaluate(button => getComputedStyle(button).backgroundColor);
  await dialog.getByRole("button", { name: "取消" }).press("Escape");
  await expect(dialog).toHaveCount(0);
  await expect(settings).toHaveCount(0);
  assert.deepEqual(await readdir(packages), []);
  await send({ type: "theme" });
  await expect(popup.locator("html")).toHaveAttribute("data-theme", "light");

  await send({ type: "new", url: detail.replace("hl=en", "hl=zh-CN") });
  pageBrowser = await waitFor(() => chromium.connectOverCDP(`http://127.0.0.1:${port + 1}`, { timeout: 1000 }), "webpage CDP");
  const listing = await waitFor(() => pageBrowser.contexts().flatMap(context => context.pages())
    .find(page => page.url() === detail.replace("hl=en", "hl=zh-CN")), "store listing");
  const add = listing.getByRole("button", { name: "添加到 Rove", exact: true });
  await expect(add).toBeVisible({ timeout: 15000 });
  assert.equal(await add.evaluate(button => Boolean(button.getRootNode().host.nextElementSibling?.matches("button"))), true);
  await add.click();
  await expectVerifiedReview(dialog, dialog.getByRole("alert"));
  await expectLoadedImage(dialog.locator(".extension-icon img"));
  await expectLoadedImage(popup.locator(".extension-install-backdrop img"));
  assert.notEqual(await confirm.evaluate(button => getComputedStyle(button).backgroundColor), darkAccent);
  await popup.screenshot({ path: join(profile, "extension-dialog.png") });
  await expect(settings).toHaveCount(0);
  await confirm.click();
  await expect(dialog).toHaveCount(0);
  await expect(popup.getByText("扩展已安装", { exact: true })).toBeVisible();
  await expect(settings).toHaveCount(0);
  await expect(add).toBeVisible();
  await send({ type: "settingsOpen", pane: "extensions" });
  await expect(toggle()).toHaveAttribute("aria-checked", "true");
  await expectLoadedImage(icon());
  const [installed] = await readdir(packages);
  const record = JSON.parse(await readFile(join(packages, installed, "record.json"), "utf8"));
  assert.equal(record.source, "chromeStore");
  assert.equal(record.id, id);
  const manifest = JSON.parse(await readFile(join(packages, installed, "payload", "manifest.json"), "utf8"));
  const fromKey = createHash("sha256").update(Buffer.from(manifest.key, "base64")).digest("hex").slice(0, 32)
    .replace(/[0-9a-f]/g, digit => String.fromCharCode(97 + parseInt(digit, 16)));
  assert.equal(fromKey, id);

  await toggle().click();
  await expect(toggle()).toHaveAttribute("aria-checked", "false");
  await stop();
  await launch();
  await send({ type: "settingsOpen", pane: "extensions" });
  await expect(toggle()).toHaveAttribute("aria-checked", "false");
  await expectLoadedImage(icon());
  await ui.getByRole("button", { name: /卸载 uBlock Origin Lite/i }).click();
  await ui.getByRole("button", { name: "确认卸载" }).click();
  await expect(ui.getByText("还没有安装扩展。")).toBeVisible();
  assert.deepEqual(await readdir(packages), []);
  await stop();
  console.log("通过：商店签名、原页弹窗、取消和安装、磨砂背景、主题色、扩展图标、重启恢复和卸载。");
  console.log("弹窗截图：" + join(profile, "extension-dialog.png"));
} finally {
  if (app?.exitCode === null) app.kill();
  await pageBrowser?.close().catch(() => {});
  await browser?.close().catch(() => {});
}
