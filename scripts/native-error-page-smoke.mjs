import { createLegacyNativeProfile } from './native-profile-fixture.mjs';
import { chromium } from '@playwright/test';
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { mkdtemp, mkdir, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import assert from 'node:assert/strict';

let recovered = false;
const requests = new Set();
const server = createServer((req, res) => {
  requests.add(req.url);
  if (req.url.startsWith('/failure') && !recovered) { req.socket.destroy(); return; }
  if (req.url === '/slow') return;
  res.writeHead(req.url === '/missing' ? 404 : 200, { 'Content-Type': 'text/html; charset=utf-8' });
  res.end(`<!doctype html><title>Fixture ${req.url}</title><h1>Fixture ${req.url}</h1>`);
});
await new Promise(done => server.listen(0, '127.0.0.1', done));
const base = `http://127.0.0.1:${server.address().port}`;
const profile = await mkdtemp(join(tmpdir(), 'rove-error-page-'));
createLegacyNativeProfile(profile);
const output = resolve(process.env.ERA_SMOKE_OUTPUT || 'test-results/native-error-page');
const port = Number(process.env.ERA_SMOKE_PORT || 19906);
await mkdir(output, { recursive: true });
const checks = [];
let child, uiBrowser, pageBrowser, ui, web;
async function waitFor(fn, label) {
  for (let i = 0; i < 200; i++) {
    try { const value = await fn(); if (value) return value; } catch {}
    await new Promise(done => setTimeout(done, 100));
  }
  throw Error('Timed out: ' + label);
}
async function send(command) {
  await ui.evaluate(command => window.ipc.postMessage(JSON.stringify(command)), command);
}
async function errorAt(url, page = web) {
  await waitFor(async () => await page.locator('#address').textContent() === url, 'custom error page at ' + url);
  const session = await page.context().newCDPSession(page);
  const history = await session.send('Page.getNavigationHistory');
  await session.detach();
  assert.equal(history.entries[history.currentIndex].url, url, 'native failed URL is preserved');
  assert.equal(await page.title(), '无法打开网页');
  assert.equal(await page.getByRole('button').count(), 1);
  assert.equal(await page.locator('svg, img, canvas, iframe').count(), 0, 'no decorations');
}
try {
  child = spawn(resolve(process.env.ERA_SMOKE_EXE || 'native/target/debug/rove.exe'), [], {
    env: { ...process.env, ERA_DATA_DIR: profile, ERA_DEBUG_PORT: String(port) }, windowsHide: true, stdio: 'pipe',
  });
  child.stderr.on('data', chunk => process.stderr.write(chunk));
  uiBrowser = await waitFor(() => chromium.connectOverCDP(`http://127.0.0.1:${port}`), 'UI debugger');
  ui = await waitFor(() => uiBrowser.contexts().flatMap(context => context.pages()).find(page => page.url() === 'https://era.ui/index.html'), 'UI page');
  await ui.waitForSelector('.sidebar');
  await send({ type: 'settingsUpdate', settings: { ...await ui.evaluate(() => window.__ERA_INITIAL_STATE__.settings), askBeforeQuit: false } });
  await ui.evaluate(() => {
    window.__lastState = window.__ERA_INITIAL_STATE__;
    const receive = window.__ERA_RECEIVE__;
    window.__ERA_RECEIVE__ = event => {
      if (event.state) window.__lastState = event.state;
      receive?.(event);
    };
  });
  await send({ type: 'navigate', url: base + '/first' });
  pageBrowser = await waitFor(() => chromium.connectOverCDP(`http://127.0.0.1:${port + 1}`), 'page debugger');
  web = await waitFor(() => pageBrowser.contexts().flatMap(context => context.pages()).find(page => page.url() === base + '/first'), 'fixture page');
  await web.getByRole('heading', { name: 'Fixture /first' }).waitFor();

  const failed = base + '/failure?value=%3Cscript%3E%22%26';
  await send({ type: 'navigate', url: failed });
  await errorAt(failed);
  assert.ok(await web.locator('#code').textContent());
  await waitFor(() => ui.evaluate(url => {
    const tab = window.__lastState.tabs.find(tab => tab.id === window.__lastState.activeTab);
    return tab.url === url && !tab.loading;
  }, failed), 'host URL and loading state');
  checks.push('连接失败显示自定义错误页，地址和加载状态正确，特殊字符按文本显示');
  await web.screenshot({ path: join(output, 'error-light.png') });

  await send({ type: 'back' });
  await web.getByRole('heading', { name: 'Fixture /first' }).waitFor();
  await send({ type: 'forward' });
  await errorAt(failed);
  checks.push('前进和后退保留原生导航历史');

  await send({ type: 'theme' });
  await waitFor(() => web.evaluate(() => getComputedStyle(document.documentElement).backgroundColor === 'rgb(0, 0, 0)'), 'dark background');
  assert.equal(await web.evaluate(() => getComputedStyle(document.documentElement).color), 'rgb(255, 255, 255)');
  await web.screenshot({ path: join(output, 'error-dark.png') });
  await send({ type: 'theme' });
  await waitFor(() => web.evaluate(() => getComputedStyle(document.documentElement).backgroundColor === 'rgb(255, 255, 255)'), 'light background');
  checks.push('错误页实时跟随明暗主题，仅使用黑白');

  await web.evaluate(() => document.body.dataset.beforeReload = 'true');
  await send({ type: 'reload' });
  await waitFor(() => web.evaluate(() => !document.body.dataset.beforeReload), 'fresh error document after reload');
  await errorAt(failed);
  recovered = true;
  await web.getByRole('button', { name: '重新加载', exact: true }).click();
  await web.getByRole('heading', { name: 'Fixture /failure?value=%3Cscript%3E%22%26' }).waitFor();
  assert.equal(web.url(), failed);
  checks.push('工具栏重载和页面重试均访问原网址，恢复后正常显示网站');

  await send({ type: 'navigate', url: base + '/missing' });
  await web.getByRole('heading', { name: 'Fixture /missing' }).waitFor();
  assert.equal(await web.locator('#retry').count(), 0);
  checks.push('保留网站自身的 HTTP 404 页面');

  await send({ type: 'navigate', url: base + '/slow' });
  await waitFor(() => requests.has('/slow'), 'pending request');
  await send({ type: 'navigate', url: base + '/second' });
  await web.getByRole('heading', { name: 'Fixture /second' }).waitFor();
  assert.equal(await web.locator('#retry').count(), 0);
  checks.push('被后续导航取消的请求不会覆盖新网页');

  recovered = false;
  const previous = web;
  const initialFailure = base + '/failure-initial';
  await send({ type: 'new', url: initialFailure });
  web = await waitFor(() => pageBrowser.contexts().flatMap(context => context.pages()).find(page => page !== previous), 'new tab renderer');
  await errorAt(initialFailure);
  checks.push('新标签首次加载失败也显示自定义错误页');

  const peekFailure = base + '/failure-peek';
  await send({ type: 'peekOpen', url: peekFailure });
  const peek = await waitFor(() => pageBrowser.contexts().flatMap(context => context.pages()).find(page => page !== previous && page !== web), 'Peek renderer');
  await errorAt(peekFailure, peek);
  recovered = true;
  await peek.getByRole('button', { name: '重新加载', exact: true }).click();
  await peek.getByRole('heading', { name: 'Fixture /failure-peek' }).waitFor();
  assert.equal(await web.locator('#address').textContent(), initialFailure, 'Peek retry leaves source tab unchanged');
  checks.push('Peek 首次加载失败使用同一错误页，重试不影响来源标签');

  console.log(JSON.stringify({ passed: true, profile, checks }, null, 2));
  await writeFile(join(output, 'result.json'), JSON.stringify({ passed: true, profile, checks }, null, 2));
} catch (error) {
  await web?.screenshot({ path: join(output, 'failure-web.png') }).catch(() => {});
  const details = await web?.evaluate(() => ({ url: location.href, html: document.documentElement.outerHTML.slice(0, 3000) })).catch(() => null);
  console.error(JSON.stringify({ error: String(error), details, checks }, null, 2));
  throw error;
} finally {
  await pageBrowser?.close();
  await uiBrowser?.close();
  child?.kill();
  server.closeAllConnections();
  server.close();
}
