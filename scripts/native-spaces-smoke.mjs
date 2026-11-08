// Focused WebView2 integration: isolated profile, loopback pages, existing Space IPC.
import { chromium } from '@playwright/test';
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { mkdtemp, mkdir, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { once } from 'node:events';
import assert from 'node:assert/strict';

const profile = await mkdtemp(join(tmpdir(), 'rove-spaces-smoke-'));
const output = resolve(process.env.ERA_SPACES_OUTPUT || 'test-results/native-spaces');
const port = Number(process.env.ERA_SPACES_PORT || 19546);
await mkdir(output, { recursive: true });
const hits = new Map();
const fixture = createServer((req, res) => {
  hits.set(req.url, (hits.get(req.url) || 0) + 1);
  res.setHeader('Content-Type', 'text/html; charset=utf-8');
  const name = req.url === '/work' ? 'Work' : 'Personal';
  res.end(`<!doctype html><title>${name} native fixture</title><style>html{background:${name === 'Work' ? '#385743' : '#234e70'};color:white}</style><h1>${name} native fixture</h1><div style="height:2000px"></div>`);
});
await new Promise((resolve) => fixture.listen(0, '127.0.0.1', resolve));
const base = `http://127.0.0.1:${fixture.address().port}`;
const exe = resolve(process.env.ERA_SPACES_EXE || 'native/target/debug/rove.exe');
let child, uiBrowser, pageBrowser;
const checks = [];
async function waitFor(fn, name) {
  const end = Date.now() + 15000;
  while (Date.now() < end) {
    try { const result = await fn(); if (result) return result; } catch { /* Initializing. */ }
    await new Promise((resolve) => setTimeout(resolve, 60));
  }
  throw Error(`Timed out: ${name}`);
}
async function launch() {
  child = spawn(exe, [], { env: { ...process.env, ERA_DATA_DIR: profile, ERA_DEBUG_PORT: String(port) }, windowsHide: true, stdio: ['ignore', 'ignore', 'pipe'] });
  child.stderr.on('data', (data) => process.stderr.write(data));
  uiBrowser = await waitFor(() => chromium.connectOverCDP(`http://127.0.0.1:${port}`), 'UI debugger');
  const ui = await waitFor(() => uiBrowser.contexts().flatMap((c) => c.pages()).find((p) => p.url().startsWith('https://era.ui/')), 'trusted UI');
  await ui.locator('.space-viewport').waitFor();
  return ui;
}
async function send(ui, command) { await ui.evaluate((c) => window.ipc.postMessage(JSON.stringify(c)), command); }
async function home(ui, name) {
  await waitFor(async () => await ui.locator('.space-list[aria-hidden="false"] .space-name').textContent() === name, `${name} state broadcast`);
  await waitFor(async () => await ui.locator('.space-settling, .space-tracking').count() === 0, 'settle');
  assert.equal(await ui.locator('.space-list').count(), 1);
}
async function swipe(ui, dx) {
  const bounds = await ui.locator('.sidebar-toolbar').boundingBox();
  await ui.mouse.move(bounds.x + 50, bounds.y + 16);
  await ui.mouse.wheel(dx, 0);
}
async function close() {
  await pageBrowser?.close(); pageBrowser = undefined;
  if (child && child.exitCode === null) {
    const done = once(child, 'exit');
    const ui = uiBrowser?.contexts().flatMap((c) => c.pages()).find((p) => p.url().startsWith('https://era.ui/'));
    if (ui) await send(ui, { type: 'window', action: 'close' }).catch(() => {});
    else child.kill();
    await done;
  }
  await uiBrowser?.close(); uiBrowser = undefined;
}
try {
  let ui = await launch();
  await send(ui, { type: 'new', url: base + '/personal' });
  pageBrowser = await waitFor(() => chromium.connectOverCDP(`http://127.0.0.1:${port + 1}`), 'page debugger');
  const personal = await waitFor(() => pageBrowser.contexts().flatMap((c) => c.pages()).find((p) => p.url() === base + '/personal'), 'Personal WebView');
  await personal.locator('h1').waitFor();
  await personal.evaluate(() => { window.__spaceMarker = 'personal state'; scrollTo(0, 300); });
  await swipe(ui, 100);
  await waitFor(async () => await ui.locator('.space-tracking').count() === 1, 'native horizontal wheel tracking');
  await home(ui, 'Work');
  assert.equal(await ui.locator('html').getAttribute('data-color'), 'sage');
  checks.push('真实 UI WebView2 横向滚轮输入经现有 Space IPC 切换 Rust 权威状态及背景色');
  await send(ui, { type: 'new', url: base + '/work' });
  const work = await waitFor(() => pageBrowser.contexts().flatMap((c) => c.pages()).find((p) => p.url() === base + '/work'), 'Work WebView');
  await work.locator('h1').waitFor();
  await work.evaluate(() => { window.__spaceMarker = 'work state'; scrollTo(0, 450); });
  await swipe(ui, -100);
  await home(ui, 'Personal');
  await ui.getByRole('tab', { name: 'Personal native fixture', exact: true }).waitFor();
  assert.deepEqual(await personal.evaluate(() => [window.__spaceMarker, scrollY]), ['personal state', 300]);
  assert.equal(work.isClosed(), false);
  await swipe(ui, 100);
  await home(ui, 'Work');
  assert.deepEqual(await work.evaluate(() => [window.__spaceMarker, scrollY]), ['work state', 450]);
  assert.equal(hits.get('/personal'), 1); assert.equal(hits.get('/work'), 1);
  checks.push('双向切换保留两个真实网页控制器、DOM 标记及滚动位置，服务器无重复加载');
  await ui.screenshot({ path: join(output, 'work-shell.png') });
  await close();
  await waitFor(async () => {
    try { return !(await fetch(`http://127.0.0.1:${port}/json/list`).then((r) => r.json())).some((t) => t.url.startsWith('https://era.ui/')); } catch { return true; }
  }, 'old renderer shutdown');
  ui = await launch();
  await home(ui, 'Work');
  assert.equal(await ui.locator('html').getAttribute('data-color'), 'sage');
  assert.equal(await ui.evaluate(() => window.__ERA_INITIAL_STATE__.activeSpace), 'work');
  checks.push('滑动选择的 Space 经 SQLite 保存，重启首屏恢复正确工作区与颜色');
  await ui.emulateMedia({ reducedMotion: 'reduce' });
  await swipe(ui, -100);
  assert.equal(await ui.locator('.space-track').evaluate((e) => new DOMMatrixReadOnly(getComputedStyle(e).transform).m41), 0);
  await home(ui, 'Personal');
  checks.push('原生 UI 的减少动态效果模式保留切换，不产生列表位移');
  const result = { passed: true, profile, checks };
  await writeFile(join(output, 'result.json'), JSON.stringify(result, null, 2));
  console.log(JSON.stringify(result, null, 2));
} finally {
  await close();
  if (child && child.exitCode === null) child.kill();
  fixture.close();
}
