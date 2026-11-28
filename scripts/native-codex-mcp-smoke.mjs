import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { mkdtemp, realpath, rm } from "node:fs/promises";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { createInterface } from "node:readline";
import { setTimeout as delay } from "node:timers/promises";
import { chromium, expect } from "@playwright/test";
import { createLegacyNativeProfile } from "./native-profile-fixture.mjs";

const temp = await realpath(tmpdir());
const profile = await mkdtemp(join(temp, "rove-codex-mcp-"));
createLegacyNativeProfile(profile, { askBeforeQuit: false, reduceMotion: true, devtoolsMcp: false });
const executable = resolve(process.env.ERA_SMOKE_EXE || "native/target/debug/rove.exe");
const debug = Number(process.env.ERA_TEST_CODEX_PORT || 22660);
const server = createServer((_, res) => {
  res.setHeader("Content-Type", "text/html; charset=utf-8");
  res.end(`<!doctype html><title>Rove Codex fixture</title><h1>Shared test page</h1>
    <label>Message <input id="message"></label><button>Apply message</button><output></output>
    <script>document.querySelector('button').onclick = () => {
      document.querySelector('output').textContent = 'Applied ' + document.querySelector('input').value;
    };</script>`);
});
server.listen(0, "127.0.0.1");
await once(server, "listening");
const url = `http://127.0.0.1:${server.address().port}/`;

const env = { ...process.env, ERA_DATA_DIR: profile, ERA_DEBUG_PORT: String(debug) };
delete env.ROVE_DEVTOOLS_MCP;
const app = spawn(executable, [url], { env, windowsHide: true, stdio: ["ignore", "ignore", "inherit"] });
const helper = spawn(executable, ["--codex-mcp", profile], { windowsHide: true, stdio: ["pipe", "pipe", "inherit"] });
const appExit = once(app, "exit");
const helperExit = once(helper, "exit");
const responses = createInterface({ input: helper.stdout })[Symbol.asyncIterator]();
let requestId = 0;
let browser;

async function rpc(method, params) {
  const id = ++requestId;
  helper.stdin.write(JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n");
  const timeout = new AbortController();
  try {
    const next = await Promise.race([responses.next(), delay(17000, null, { signal: timeout.signal }).then(() => {
      throw new Error(`MCP did not respond to ${method}`);
    })]);
    assert.ok(!next.done, "MCP closed its output");
    const response = JSON.parse(next.value);
    assert.equal(response.id, id);
    return response;
  } finally {
    timeout.abort();
  }
}
async function call(name, args = {}, error) {
  const { result } = await rpc("tools/call", { name, arguments: args });
  assert.ok(result, `Expected a tool result from ${name}`);
  const text = result.content[0].text;
  assert.equal(result.isError, Boolean(error), text);
  if (error) assert.match(text, error);
  else return JSON.parse(text);
}
async function waitFor(check, description) {
  let cause;
  for (let count = 0; count < 150; count++) {
    try { const value = await check(); if (value) return value; } catch (error) { cause = error; }
    await delay(100);
  }
  throw new Error(`Timed out waiting for ${description}`, { cause });
}

try {
  const initialized = await rpc("initialize", { protocolVersion: "2025-11-25", capabilities: {}, clientInfo: { name: "smoke", version: "1" } });
  assert.equal(initialized.result.serverInfo.name, "rove");
  helper.stdin.write('{"jsonrpc":"2.0","method":"notifications/initialized"}\n');
  assert.equal((await rpc("tools/list")).result.tools.length, 5);
  await call("rove_shared_tab", {}, /share a tab/i);
  browser = await waitFor(() => chromium.connectOverCDP(`http://127.0.0.1:${debug}`, { timeout: 750 }), "Rove UI");
  const ui = await waitFor(() => browser.contexts().flatMap(ctx => ctx.pages())
    .find(page => page.url() === "https://era.ui/index.html"), "shell page");
  const popup = await waitFor(() => ui.context().pages().find(page => page.url().endsWith("?popup")), "browser menu surface");
  const send = command => ui.evaluate(command => window.ipc.postMessage(JSON.stringify(command)), command);
  const stop = ui.getByRole("button", { name: "停止与 Codex 共享", exact: true });
  const agentGroup = ui.locator(".pinned-area").getByRole("tablist", { name: "Agent 操作中", exact: true });
  const regularTabs = ui.getByRole("tablist", { name: "标签页", exact: true });
  async function share() {
    await ui.getByRole("button", { name: "浏览器菜单", exact: true }).click();
    await popup.getByRole("menuitem", { name: "允许 Codex 使用此标签页", exact: true }).click();
    await expect(stop).toBeVisible();
  }
  await expect(agentGroup).toHaveCount(0);
  await share();
  const tab = await call("rove_shared_tab");
  assert.equal(tab.url, url);
  const tabSelector = `[data-tab-id="${tab.id}"]`;
  await expect(agentGroup.locator(tabSelector)).toBeVisible();
  await expect(regularTabs.locator(tabSelector)).toHaveCount(0);
  const agentGroupToggle = ui.locator("[data-agent-pin-group] > summary");
  await agentGroupToggle.click();
  await expect(agentGroup).toBeHidden();
  await agentGroupToggle.click();
  await expect(agentGroup.locator(tabSelector)).toBeVisible();
  await call("rove_snapshot", { id: "other-tab" }, /tab_not_shared/);
  const frame = await rpc("tools/call", { name: "rove_snapshot", arguments: { id: tab.id, frame: "other-frame" } });
  assert.equal(frame.error.code, -32602);
  const snapshot = await call("rove_snapshot", { id: tab.id });
  assert.match(snapshot.text, /Shared test page/);
  const target = label => ({ id: tab.id, snapshot: snapshot.snapshot, element: snapshot.elements.find(entry => entry.label === label).element });
  await call("rove_fill", { ...target("Message"), text: "hello" });
  await call("rove_click", target("Apply message"));
  const latest = await call("rove_snapshot", { id: tab.id });
  assert.match(latest.text, /Applied hello/);
  await stop.click();
  await expect(stop).toHaveCount(0);
  await expect(agentGroup).toHaveCount(0);
  await expect(regularTabs.locator(tabSelector)).toBeVisible();
  await call("rove_snapshot", { id: tab.id }, /tab_not_shared/);
  await share();
  await call("rove_click", { ...target("Apply message"), snapshot: latest.snapshot }, /stale_snapshot/);
  await send({ type: "navigate", id: tab.id, url: url.replace("127.0.0.1", "localhost") });
  await expect(stop).toHaveCount(0);
  await expect(agentGroup).toHaveCount(0);
  await call("rove_shared_tab", {}, /tab_not_shared/);

  const spaceId = "personal";
  const folderName = "Existing pins";
  await send({ type: "pinFolderCreate", spaceId, name: folderName });
  const folder = ui.getByRole("button", { name: folderName, exact: true });
  await expect(folder).toBeVisible();
  const folderId = await folder.locator("..").getAttribute("data-pin-folder-header");
  assert.ok(folderId);
  await send({ type: "pinPlace", id: tab.id, spaceId, folderId });
  const savedRow = ui.getByRole("tablist", { name: `${folderName}内的固定标签` }).locator(tabSelector);
  await expect(savedRow).toBeVisible();
  const bookmarkId = await savedRow.getAttribute("data-bookmark-id");
  assert.ok(bookmarkId);
  await folder.click();
  await expect(folder).toHaveAttribute("aria-expanded", "false");
  await share();
  await expect(agentGroup.locator(tabSelector)).toHaveAttribute("data-bookmark-id", bookmarkId);
  await send({ type: "new", url });
  await expect(stop).toHaveCount(0);
  await expect(agentGroup).toHaveCount(0);
  await expect(folder).toHaveAttribute("aria-expanded", "false");
  await folder.click();
  await expect(savedRow).toHaveAttribute("data-bookmark-id", bookmarkId);
  await call("rove_shared_tab", {}, /tab_not_shared/);
  console.log("Rove Codex MCP verified scoped access, revocation, and temporary grouping of regular and pinned tabs.");
} finally {
  helper.stdin.end();
  helper.kill();
  await helperExit;
  await browser?.close().catch(() => {});
  app.kill();
  await appExit;
  server.close();
  const target = await realpath(profile);
  if (dirname(target) === temp && basename(target).startsWith("rove-codex-mcp-")) {
    await rm(target, { recursive: true, force: true, maxRetries: 8, retryDelay: 150 });
  }
}
