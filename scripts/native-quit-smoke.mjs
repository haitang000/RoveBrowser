// Real WebView2 quit flow, including WM_CLOSE and persisted preferences.
import { createLegacyNativeProfile } from "./native-profile-fixture.mjs";
import { chromium } from "@playwright/test";
import { spawn, execFile } from "node:child_process";
import { once } from "node:events";
import { mkdtemp, mkdir } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { promisify } from "node:util";
import { DatabaseSync } from "node:sqlite";
import assert from "node:assert/strict";

const profile = await mkdtemp(join(tmpdir(), "rove-quit-"));
createLegacyNativeProfile(profile);
const output = resolve("test-results/native-quit");
await mkdir(output, { recursive: true });
const executable = resolve(process.env.ERA_TEST_EXE || "native/target/debug/rove.exe");
const port = Number(process.env.ERA_TEST_PORT || 19886);
const execFileAsync = promisify(execFile);
let app, browser, ui, popup;
function savedState() {
  const db = new DatabaseSync(join(profile, "era.sqlite"), { readOnly: true });
  try { return JSON.parse(db.prepare("SELECT json FROM session WHERE id=1").get().json); }
  finally { db.close(); }
}
// Keep this test local and independent of internet availability.
{
  const state = savedState();
  state.tabs = state.tabs.filter(tab => tab.id === "start");
  state.favorites = [];
  const db = new DatabaseSync(join(profile, "era.sqlite"));
  try { db.prepare("UPDATE session SET json=? WHERE id=1").run(JSON.stringify(state)); }
  finally { db.close(); }
}
async function waitFor(fn, label) {
  const deadline = Date.now() + 20000;
  while (Date.now() < deadline) {
    try { const value = await fn(); if (value) return value; } catch {}
    await new Promise(resolve => setTimeout(resolve, 80));
  }
  throw Error(`Timed out: ${label}`);
}
async function send(command) {
  await ui.evaluate(command => window.ipc.postMessage(JSON.stringify(command)), command);
}
async function launch() {
  app = spawn(executable, [], {
    env: { ...process.env, ERA_DATA_DIR: profile, ERA_DEBUG_PORT: String(port) },
    windowsHide: true, stdio: "pipe",
  });
  app.stderr.on("data", chunk => process.stderr.write(chunk));
  browser = await waitFor(() => chromium.connectOverCDP(`http://127.0.0.1:${port}`), "UI debugger");
  const pages = () => browser.contexts().flatMap(context => context.pages());
  ui = await waitFor(() => pages().find(page => page.url() === "https://era.ui/index.html"), "shell");
  popup = await waitFor(() => pages().find(page => page.url().endsWith("?popup")), "popup renderer");
  await ui.locator(".sidebar").waitFor();
  await popup.waitForFunction(() => Boolean(window.__ERA_RECEIVE__));
}
async function finish(action) {
  const exited = once(app, "exit");
  await action().catch(error => {
    // A click/keypress can lose its CDP target as the native process exits.
    if (!/closed|disconnect/i.test(error.message)) throw error;
  });
  await Promise.race([exited, new Promise((_, reject) => {
    const timer = setTimeout(() => reject(Error("Quit did not exit the application")), 10000);
    timer.unref();
  })]);
  await browser.close();
}
const dialog = () => popup.getByRole("alertdialog", { name: "确定要退出 Rove 吗？" });
try {
  await launch();
  // A response cannot bypass the host's pending-confirmation check.
  await send({ type: "quitResponse", confirm: true, always: true });
  await ui.getByRole("button", { name: "关闭窗口", exact: true }).click();
  await dialog().waitFor();
  assert.equal(await dialog().locator("img, svg").count(), 0);
  assert.equal(await dialog().evaluate(element => getComputedStyle(element).outlineStyle), "none");
  assert.equal(await popup.locator(".quit-scrim").evaluate(element => element.getBoundingClientRect().width),
    await popup.evaluate(() => innerWidth));
  await dialog().screenshot({ path: join(output, "quit-dialog.png") });
  await dialog().getByRole("button", { name: "取消", exact: true }).click();
  await dialog().waitFor({ state: "detached" });
  assert.equal(app.exitCode, null);

  // Windows' own close request follows the same path as the custom close button.
  await execFileAsync("powershell.exe", ["-NoProfile", "-Command",
    `Add-Type -TypeDefinition 'using System; using System.Runtime.InteropServices; public class QuitSmoke { [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr hwnd, uint msg, IntPtr wParam, IntPtr lParam); }'; $quitWindow = (Get-Process -Id ${app.pid}).MainWindowHandle; if ($quitWindow -eq 0) { throw 'No window' }; [QuitSmoke]::PostMessage($quitWindow, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null`,
  ], { windowsHide: true });
  await dialog().waitFor();
  await send({ type: "window", action: "close" });
  assert.equal(await popup.getByRole("alertdialog").count(), 1);
  await popup.keyboard.press("Escape");
  await dialog().waitFor({ state: "detached" });
  assert.equal(app.exitCode, null);

  await send({ type: "window", action: "close" });
  await dialog().waitFor();
  await finish(() => popup.keyboard.press("Enter"));
  assert.equal(savedState().settings.askBeforeQuit, true);

  await launch();
  await send({ type: "window", action: "close" });
  await dialog().waitFor();
  await finish(() => dialog().getByRole("button", { name: "始终退出", exact: true }).click());
  assert.equal(savedState().settings.askBeforeQuit, false);

  await launch();
  await finish(() => ui.getByRole("button", { name: "关闭窗口", exact: true }).click());
  console.log("PASS: native close, WM_CLOSE, cancel, Esc, Enter, repeated close, and persisted Always quit");
} finally {
  if (app && app.exitCode === null) app.kill();
  await browser?.close().catch(() => {});
}
