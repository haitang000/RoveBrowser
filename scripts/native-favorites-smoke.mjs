// Real WebView2 favicon persistence and top-favorite limits, using an isolated profile.
import { createLegacyNativeProfile } from './native-profile-fixture.mjs';
import { chromium } from '@playwright/test';
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { mkdtemp, mkdir, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { once } from 'node:events';
import assert from 'node:assert/strict';

const profile = await mkdtemp(join(tmpdir(), 'rove-favorites-'));
createLegacyNativeProfile(profile);
const output = resolve(process.env.ERA_SMOKE_OUTPUT || 'test-results/native-favorites');
await mkdir(output, { recursive: true });
const icon = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGNoSXrxHwAGCgLO2e10KwAAAABJRU5ErkJggg==', 'base64');
const server = createServer((req, res) => {
  if (req.url === '/favicon.ico') { res.setHeader('Content-Type', 'image/png'); res.end(icon); return; }
  res.setHeader('Content-Type', 'text/html');
  res.end('<!doctype html><link rel="icon" href="/favicon.ico"><title>Cached icon fixture</title><h1>Local fixture</h1>');
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
const base = `http://127.0.0.1:${server.address().port}`;
const exe = resolve(process.env.ERA_SMOKE_EXE || 'native/target/debug/rove.exe');
const port = Number(process.env.ERA_SMOKE_PORT || 19646);
let child, browser, pageBrowser, ui;
const checks = [];
async function waitFor(fn, label) {
  for (let i = 0; i < 150; i++) {
    try { const result = await fn(); if (result) return result; } catch {}
    await new Promise(resolve => setTimeout(resolve, 100));
  }
  throw Error(`Timed out: ${label}`);
}
async function send(command) { await ui.evaluate(command => window.ipc.postMessage(JSON.stringify(command)), command); }
async function launch() {
  child = spawn(exe, [], { env: { ...process.env, ERA_DATA_DIR: profile, ERA_DEBUG_PORT: String(port) }, windowsHide: true, stdio: 'pipe' });
  child.stderr.on('data', data => process.stderr.write(data));
  await waitFor(async () => {
    const targets = await fetch(`http://127.0.0.1:${port}/json/list`, { signal: AbortSignal.timeout(500) }).then(r => r.json());
    return targets.some(t => t.url.startsWith('https://era.ui/'));
  }, 'UI renderer');
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
  ui = await waitFor(() => Promise.resolve(browser.contexts().flatMap(c => c.pages()).find(p => p.url() === 'https://era.ui/index.html')), 'UI page');
  await ui.waitForSelector('.sidebar');
  await ui.evaluate(() => window.ipc.postMessage(JSON.stringify({ type: 'settingsUpdate', settings: { ...window.__ERA_INITIAL_STATE__.settings, askBeforeQuit: false } })));
  await ui.evaluate(() => {
    window.__favoriteErrors = [];
    const receive = window.__ERA_RECEIVE__;
    window.__ERA_RECEIVE__ = event => {
      if (event.type === 'error') window.__favoriteErrors.push(event.message);
      if (event.type === 'state') window.__favoriteState = event.state;
      receive(event);
    };
  });
  await send({ type: 'ready' });
}
async function stop() {
  await pageBrowser?.close(); pageBrowser = null;
  const exited = once(child, 'exit');
  await send({ type: 'window', action: 'close' }).catch(() => {});
  await exited;
  await browser.close(); browser = null;
  await waitFor(async () => {
    try {
      const targets = await fetch(`http://127.0.0.1:${port}/json/list`, { signal: AbortSignal.timeout(500) }).then(r => r.json());
      return !targets.some(t => t.url.startsWith('https://era.ui/'));
    } catch { return true; }
  }, 'old renderer shutdown');
}
try {
  await launch();
  assert.equal(await ui.locator('.favorites-heading, .favorites-panel').count(), 0);
  await send({ type: 'favoriteSave', title: 'Ninth', url: base + '/ninth' });
  await waitFor(() => ui.evaluate(() => window.__favoriteErrors.some(e => e.includes('8'))), 'native limit rejects ninth favorite');
  assert.equal(await ui.locator('.favorite-tile').count(), 8);
  await ui.locator('.favorite-open').last().click({ button: 'right' });
  await ui.context().pages().find(p => p.url().endsWith('?popup')).getByRole('menuitem', { name: '移除常用网站', exact: true }).click();
  await ui.locator('.favorite-open').first().click({ button: 'right' });
  await ui.context().pages().find(p => p.url().endsWith('?popup')).getByRole('menuitem', { name: '编辑常用网站', exact: true }).click();
  const popup = ui.context().pages().find(p => p.url().endsWith('?popup'));
  await popup.getByRole('textbox', { name: '置顶网站名称' }).fill('本地图标');
  await popup.getByRole('textbox', { name: '置顶网站网址' }).fill(base + '/home');
  await popup.getByRole('button', { name: '保存修改', exact: true }).click();
  await send({ type: 'new', url: base + '/other' });
  pageBrowser = await waitFor(() => chromium.connectOverCDP(`http://127.0.0.1:${port + 1}`), 'page debugger');
  const web = await waitFor(() => Promise.resolve(pageBrowser.contexts().flatMap(c => c.pages()).find(p => p.url() === base + '/other')), 'fixture page');
  await web.waitForSelector('h1');
  await waitFor(() => ui.evaluate(() => window.__favoriteState?.tabs.some(t => t.url.endsWith('/other') && t.favicon?.startsWith('data:image/png;base64,'))), 'native favicon capture');
  await ui.getByRole('tab', { name: /^Cached icon fixture/ }).click({ button: 'right' });
  await ui.context().pages().find(p => p.url().endsWith('?popup')).getByRole('menuitem', { name: '添加到顶部常用网站', exact: true }).click();
  assert.equal(await ui.locator('.favorite-tile').count(), 8);
  const favorite = () => ui.locator('.favorite-tile').filter({ has: ui.getByRole('button', { name: '本地图标', exact: true }) });
  const cached = await favorite().locator('img').getAttribute('src');
  assert.match(cached, /^data:image\/png;base64,/);
  assert(await favorite().locator('img').evaluate(async img => { await img.decode(); return img.naturalWidth > 0; }));
  checks.push('管理弹窗和文字标题移除；原生拒绝第九个网站，右键添加保留当前图标');
  await favorite().locator('.favorite-open').click();
  await favorite().click({ button: 'middle' });
  assert.equal(await favorite().locator('img').getAttribute('src'), cached);
  await ui.screenshot({ path: join(output, 'cached.png') });
  checks.push('真实 GetFavicon 返回 PNG 图片，同源普通标签填充置顶缓存，卸载后继续显示');
  await stop();
  await new Promise(resolve => server.close(resolve));
  await launch();
  assert.equal(await ui.locator('.favorite-tile').count(), 8);
  const stored = await ui.evaluate(() => window.__ERA_INITIAL_STATE__.favorites.find(f => f.title === '本地图标').favicon);
  assert.equal(stored, cached);
  await favorite().locator('img').evaluate(img => img.decode());
  assert.equal(await favorite().locator('img').getAttribute('src'), cached);
  await ui.screenshot({ path: join(output, 'offline-restart.png') });
  checks.push('服务器关闭后重启进程，SQLite 恢复 PNG 图标，无需站点网络');
  await writeFile(join(output, 'result.json'), JSON.stringify({ passed: true, profile, checks }, null, 2));
  console.log(JSON.stringify({ passed: true, profile, checks }, null, 2));
} catch (error) {
  await ui?.screenshot({ path: join(output, 'failure.png') }).catch(() => {});
  throw error;
} finally {
  await pageBrowser?.close().catch(() => {});
  await browser?.close().catch(() => {});
  if (child && !child.killed) child.kill();
  server.close();
}
