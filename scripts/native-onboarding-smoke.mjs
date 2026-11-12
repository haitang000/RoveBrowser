// First-run integration in real WebView2 with a disposable profile.
import { chromium } from "@playwright/test";
import { execFile, spawn } from "node:child_process";
import { mkdtemp, mkdir, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { once } from "node:events";
import { promisify } from "node:util";
import assert from "node:assert/strict";

const profile = await mkdtemp(join(tmpdir(), "rove-onboarding-"));
const output = resolve("test-results/native-onboarding");
const executable = resolve(process.env.ERA_TEST_EXE || "native/target/debug/rove.exe");
await mkdir(output, { recursive: true });
let child, browser, ui, port = 19830;
const checks = [];
const execFileAsync = promisify(execFile);
async function waitFor(fn, label) {
  const deadline = Date.now() + 20000;
  while (Date.now() < deadline) {
    try { const result = await fn(); if (result) return result; } catch {}
    await new Promise((r) => setTimeout(r, 100));
  }
  throw Error(`Timed out: ${label}`);
}
async function launch() {
  port += 4;
  child = spawn(executable, [], { env: { ...process.env, ERA_DATA_DIR: profile, ERA_IMPORT_FIXTURES: join(profile, "empty-import-sources"), ERA_DEBUG_PORT: String(port) }, windowsHide: true, stdio: "pipe" });
  child.stderr.on("data", (data) => process.stderr.write(data));
  browser = await waitFor(() => chromium.connectOverCDP(`http://127.0.0.1:${port}`), "UI debugger");
  ui = await waitFor(() => browser.contexts().flatMap((c) => c.pages()).find((p) => p.url() === "https://era.ui/index.html"), "trusted UI");
  await ui.locator(".onboarding, .sidebar").first().waitFor();
  await waitFor(async () => {
    const { stdout } = await execFileAsync("powershell.exe", ["-NoProfile", "-Command", `(Get-Process -Id ${child.pid}).MainWindowHandle.ToInt64()`], { windowsHide: true });
    return Number(stdout.trim()) > 0;
  }, "native window visible after first paint");
  await ui.evaluate(() => {
    window.__onboardingTest = { state: window.__ERA_INITIAL_STATE__ };
    const receive = window.__ERA_RECEIVE__;
    window.__ERA_RECEIVE__ = (event) => {
      if (event.type === "state") window.__onboardingTest.state = event.state;
      receive(event);
    };
  });
}
async function state() { return ui.evaluate(() => window.__onboardingTest.state); }
async function send(command) { await ui.evaluate((c) => window.ipc.postMessage(JSON.stringify(c)), command); }
async function stop() {
  if (child?.exitCode === null) {
    const exited = once(child, "exit"); child.kill(); await exited;
  }
  await browser?.close(); browser = undefined;
}
try {
  await launch();
  assert.equal((await state()).onboarding.completed, false);
  assert.equal(await ui.locator(".sidebar").count(), 0);
  await send({ type: "new", url: "https://example.com" });
  await send({ type: "ready" });
  await waitFor(async () => (await state()).onboarding.step === "hello", "initial state");
  assert.equal((await state()).tabs.length, 1);
  checks.push("Fresh profile presents a visible native welcome window; browser commands cannot bypass it");
  await ui.getByRole("button", { name: "开始使用", exact: true }).click();
  await ui.getByRole("button", { name: "跳过", exact: true }).click();
  await ui.getByRole("button", { name: "GitHub", exact: true }).click();
  await ui.getByRole("button", { name: "Gmail", exact: true }).click();
  await ui.getByRole("button", { name: "继续", exact: true }).click();
  await waitFor(async () => (await state()).onboarding.step === "ads", "sites saved");
  assert.deepEqual((await state()).favorites.map((f) => f.title).sort(), ["GitHub", "Gmail"]);
  await ui.screenshot({ path: join(output, "ads.png") });
  await stop();

  await launch();
  assert.equal((await state()).onboarding.step, "ads");
  assert.equal((await state()).favorites.length, 2);
  checks.push("SQLite restores the unfinished step and chosen favorites after process exit");
  await ui.getByRole("button", { name: "继续", exact: true }).click();
  await ui.getByRole("button", { name: "暂时不用", exact: true }).click();
  await ui.getByRole("button", { name: "开始浏览", exact: true }).click();
  await ui.locator(".sidebar").waitFor();
  assert.equal((await state()).onboarding.completed, true);
  assert.equal(await ui.locator(".favorite-tile").count(), 2);
  await stop();

  await launch();
  assert.equal((await state()).onboarding.completed, true);
  assert.equal(await ui.locator(".onboarding").count(), 0);
  checks.push("Finished onboarding stays finished across restart");
  await ui.getByRole("button", { name: "设置", exact: true }).click();
  await ui.getByRole("button", { name: "打开欢迎引导", exact: true }).click();
  await ui.getByRole("button", { name: "开始使用", exact: true }).click();
  await ui.getByRole("button", { name: "跳过", exact: true }).click();
  await ui.getByRole("button", { name: "GitHub", exact: true }).waitFor();
  assert.equal(await ui.getByRole("button", { name: "GitHub", exact: true }).getAttribute("aria-pressed"), "true");
  await ui.screenshot({ path: join(output, "sites.png") });
  checks.push("Settings reopens onboarding; actual saved favorites are preselected");
  await writeFile(join(output, "checks.json"), JSON.stringify({ profile, checks }, null, 2));
  console.log(JSON.stringify({ profile, checks }, null, 2));
} finally { await stop(); }
