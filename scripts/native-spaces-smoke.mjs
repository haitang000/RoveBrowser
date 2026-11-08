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
const rebounds = [];
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
  const ui = await waitFor(() => uiBrowser.contexts().flatMap((c) => c.pages()).find((p) => p.url() === 'https://era.ui/index.html'), 'trusted UI');
  await ui.locator('.space-viewport').waitFor();
  // App.vue reveals the native window after fonts and two paint opportunities.
  // Do not measure compositor motion while the startup window is still hidden.
  await ui.evaluate(async () => {
    await document.fonts.ready;
    await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
  });
  return ui;
}
async function send(ui, command) { await ui.evaluate((c) => window.ipc.postMessage(JSON.stringify(c)), command); }
async function home(ui, name) {
  await waitFor(async () => await ui.locator('.sidebar-header .space-name').textContent() === name, `${name} state broadcast`);
  await waitFor(async () => await ui.locator('.space-settling, .space-tracking').count() === 0, 'settle');
  assert.equal(await ui.locator('.space-list').count(), 1);
}
async function swipe(ui, dx) {
  const bounds = await ui.locator('.sidebar-header').boundingBox();
  await ui.mouse.move(bounds.x + 50, bounds.y + 16);
  await ui.mouse.wheel(dx, 0);
}
async function inertiaSwipe(ui, direction) {
  return ui.locator('.sidebar').evaluate(async (target, direction) => {
    const names = [];
    for (const dx of [12, 28, 42, 38, 30, 22, 16, 11, 7, 4, 2, 1, 0.5, 0.25]) {
      target.dispatchEvent(new WheelEvent('wheel', { deltaX: direction * dx, bubbles: true, cancelable: true }));
      await new Promise((resolve) => setTimeout(resolve, 20));
      names.push(document.querySelector('.sidebar-header .space-name').textContent);
    }
    return names;
  }, direction);
}
async function rebound(ui, deltas) {
  return ui.locator('.sidebar').evaluate(async (target, deltas) => {
    const frames = [], start = performance.now();
    for (const dx of deltas) {
      target.dispatchEvent(new WheelEvent('wheel', { deltaX: dx, bubbles: true, cancelable: true }));
      await new Promise((resolve) => setTimeout(resolve, 20));
      frames.push({ time: performance.now() - start,
        tracking: Boolean(document.querySelector('.space-tracking')),
        settling: Boolean(document.querySelector('.space-settling')),
        offset: new DOMMatrixReadOnly(getComputedStyle(document.querySelector('.space-track')).transform).m41 });
    }
    return frames;
  }, deltas);
}
async function close() {
  await pageBrowser?.close(); pageBrowser = undefined;
  if (child && child.exitCode === null) {
    const done = once(child, 'exit');
    const ui = uiBrowser?.contexts().flatMap((c) => c.pages()).find((p) => p.url() === 'https://era.ui/index.html');
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
  for (const [name, deltas] of [
    ['edge', [-12, -28, -42, -38, -30, -22, -16, -11, -7, -4, -2, -1, -0.5, -0.25]],
    ['small', [2, 4, 6, 5.5, 4, 3, 2, 1.5, 1, 0.7, 0.4, 0.2, 0.1, 0.05]],
    ['noisy edge', [-8, -18, -30, -28, -22, -24, -18, -14, -15, -11, -8, -9, -6, -4]],
  ]) {
    const frames = await rebound(ui, deltas);
    await writeFile(join(output, `rebound-${name.replaceAll(' ', '-')}.json`), JSON.stringify(frames, null, 2));
    assert.ok(frames.slice(7).every((frame) => !frame.tracking), `${name} must release during inertia`);
    assert.ok(frames[7].settling, `${name} must start its spring during inertia`);
    assert.ok(Math.abs(frames.at(-1).offset) < Math.abs(frames[6].offset) / 2, `${name} must visibly return during inertia`);
    rebounds.push({ name, firstSpringMs: frames.find((frame) => frame.settling).time, frames });
    await home(ui, 'Personal');
  }
  checks.push('边界、小幅及带抖动的惯性尾流仍在输入时，回弹已经启动且屏幕位移持续回归');
  const stopped = await ui.locator('.sidebar').evaluate(async (target) => {
    const start = performance.now();
    let firstSpringMs;
    const observer = new MutationObserver(() => {
      if (firstSpringMs === undefined && document.querySelector('.space-settling')) firstSpringMs = performance.now() - start;
    });
    observer.observe(document.querySelector('.space-viewport'), { attributes: true, attributeFilter: ['class'] });
    target.dispatchEvent(new WheelEvent('wheel', { deltaX: 40, bubbles: true, cancelable: true }));
    await new Promise((resolve) => setTimeout(resolve, 55));
    observer.disconnect();
    const frames = [];
    for (let i = 0; i < 6; i++) {
      const track = document.querySelector('.space-track'), animation = track.getAnimations()[0];
      frames.push({ time: performance.now() - start, offset: new DOMMatrixReadOnly(getComputedStyle(track).transform).m41,
        currentTime: animation?.currentTime, startTime: animation?.startTime, pending: animation?.pending,
        visibility: document.visibilityState });
      await new Promise(requestAnimationFrame);
    }
    return { firstSpringMs, tracking: Boolean(document.querySelector('.space-tracking')),
      offset: frames[0].offset, frames };
  });
  await writeFile(join(output, 'rebound-stopped.json'), JSON.stringify(stopped, null, 2));
  assert.equal(stopped.tracking, false);
  assert.ok(stopped.firstSpringMs < 55, `Stopped drag spring delayed: ${JSON.stringify(stopped)}`);
  assert.ok(Math.abs(stopped.offset) < 40);
  rebounds.push({ name: 'stopped drag', ...stopped });
  await home(ui, 'Personal');
  checks.push('无惯性尾流时55ms内启动回弹，随后实际位移已回归，不再等待固定80ms');
  const names = await inertiaSwipe(ui, 1);
  assert.ok(names.slice(8).every((name) => name === 'Work'), `Space must commit before the inertia stream ends: ${names}`);
  await home(ui, 'Work');
  assert.equal(await ui.locator('html').getAttribute('data-color'), 'sage');
  checks.push('真实 UI WebView2 在触控板惯性尾流结束前经 Space IPC 切换 Rust 权威状态及背景色，尾流不多跳');
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
  const result = { passed: true, profile, checks, rebounds };
  await writeFile(join(output, 'result.json'), JSON.stringify(result, null, 2));
  console.log(JSON.stringify({ ...result, rebounds: rebounds.map(({ name, firstSpringMs }) => ({ name, firstSpringMs })) }, null, 2));
} finally {
  await close();
  if (child && child.exitCode === null) child.kill();
  fixture.close();
}
