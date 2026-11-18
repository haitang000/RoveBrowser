// Verify that pending download details stay in Rove's popup while WebView2
// owns the operation and decides when it has completed.
import { createLegacyNativeProfile } from './native-profile-fixture.mjs';
import { chromium } from '@playwright/test';
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { mkdtemp, mkdir, readFile, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import assert from 'node:assert/strict';

const executable = resolve(process.env.ERA_SMOKE_EXE || 'native/target/debug/rove.exe');
const content = await readFile(executable);
let pendingResponse;
const server = createServer((req, res) => {
  if (req.url === '/review.exe') {
    res.writeHead(200, {
      'Content-Type': 'application/octet-stream',
      'Content-Disposition': 'attachment; filename="review.exe"',
      'Content-Length': String(content.length),
    });
    // Keep a real operation alive while checking Rove's dialog.
    res.write(content.subarray(0, -1));
    pendingResponse = res;
  } else {
    res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' });
    res.end('<!doctype html><title>Download review fixture</title><a id="download" href="/review.exe">Download EXE</a>');
  }
});
await new Promise(done => server.listen(0, '127.0.0.1', done));
const base = `http://127.0.0.1:${server.address().port}`;
const profile = await mkdtemp(join(tmpdir(), 'rove-download-review-'));
const folder = join(profile, 'Downloads');
await mkdir(folder);
createLegacyNativeProfile(profile);
const output = resolve(process.env.ERA_SMOKE_OUTPUT || 'test-results/native-download-review');
await mkdir(output, { recursive: true });
const port = Number(process.env.ERA_SMOKE_PORT || 19856);
let child, uiBrowser, pageBrowser, ui, popup;
const checks = [];
async function waitFor(fn, label) {
  for (let i = 0; i < 180; i++) {
    try { const value = await fn(); if (value) return value; } catch {}
    await new Promise(done => setTimeout(done, 100));
  }
  throw Error(`Timed out: ${label}`);
}
const send = command => ui.evaluate(value => window.ipc.postMessage(JSON.stringify(value)), command);
const items = () => ui.evaluate(() => window.__downloads || []);

async function isNativeDialogOpen() {
  const browserSession = await pageBrowser.newBrowserCDPSession();
  try {
    const { targetInfos } = await browserSession.send('Target.getTargets');
    return targetInfos.some(target => target.type === 'browser_ui' && target.url.startsWith('edge://downloads-hub'));
  } finally { await browserSession.detach(); }
}

try {
  child = spawn(executable, [], {
    env: { ...process.env, ERA_DATA_DIR: profile, ERA_DEBUG_PORT: String(port) },
    windowsHide: true, stdio: 'pipe',
  });
  child.stderr.on('data', data => process.stderr.write(data));
  uiBrowser = await waitFor(() => chromium.connectOverCDP(`http://127.0.0.1:${port}`), 'UI debugger');
  ui = await waitFor(() => uiBrowser.contexts().flatMap(context => context.pages()).find(page => page.url() === 'https://era.ui/index.html'), 'UI page');
  await ui.waitForSelector('.sidebar');
  popup = await waitFor(() => uiBrowser.contexts().flatMap(context => context.pages()).find(page => page.url().endsWith('?popup')), 'popup renderer');
  await ui.evaluate(() => {
    window.__downloads = [];
    window.__browserState = window.__ERA_INITIAL_STATE__;
    const receive = window.__ERA_RECEIVE__;
    window.__ERA_RECEIVE__ = event => {
      if (event.type === 'downloads') window.__downloads = event.items;
      if (event.type === 'state') window.__browserState = event.state;
      receive?.(event);
    };
  });
  const settings = await ui.evaluate(() => window.__ERA_INITIAL_STATE__.settings);
  await send({ type: 'settingsUpdate', settings: { ...settings, downloadFolder: folder, askBeforeQuit: false } });
  await send({ type: 'new', url: base });
  pageBrowser = await waitFor(() => chromium.connectOverCDP(`http://127.0.0.1:${port + 1}`), 'page debugger');
  const page = await waitFor(() => pageBrowser.contexts().flatMap(context => context.pages()).find(page => page.url().startsWith(base)), 'fixture page');
  await page.waitForSelector('#download');
  const source = await ui.evaluate(() => window.__browserState.activeTab);
  const session = await page.context().newCDPSession(page);
  await session.send('Browser.setDownloadBehavior', { behavior: 'default' });
  await page.click('#download');
  const download = await waitFor(async () => (await items()).find(item => item.received > 0), 'active executable download');

  // Present the full-byte waiting state in the popup only. The engine still
  // owns a live, unfinished transfer; this does not skip a security check.
  await popup.evaluate(id => {
    const receive = window.__ERA_RECEIVE__;
    window.__ERA_RECEIVE__ = event => {
      if (event.type === 'downloads') event = { ...event, items: event.items.map(item => item.id === id ? { ...item, received: item.total } : item) };
      receive?.(event);
    };
  }, download.id);
  await send({ type: 'new', url: 'era://newtab' });
  const active = await waitFor(async () => {
    const id = await ui.evaluate(() => window.__browserState.activeTab);
    return id !== source && id;
  }, 'another tab is active');
  const review = popup.getByRole('button', { name: '查看下载提示', exact: true });
  const notice = popup.getByRole('dialog', { name: '正在等待下载结果' });
  await review.click();
  await notice.getByText('review.exe', { exact: true }).waitFor();
  assert.equal(await isNativeDialogOpen(), false);
  assert.equal(await ui.evaluate(() => window.__browserState.activeTab), active);
  assert.equal((await items())[0].state, 'inProgress');
  await notice.getByRole('button', { name: '继续等待' }).click();
  await notice.waitFor({ state: 'detached' });
  await review.click();
  await notice.waitFor();
  checks.push('查看下载提示在 Rove 窗口内打开，可继续等待，且不会切换标签或打开原生弹窗');

  pendingResponse.end(content.subarray(-1));
  const complete = await waitFor(async () => (await items()).find(item => item.id === download.id && item.state === 'finished'), 'download finishes after its final byte');
  await notice.waitFor({ state: 'detached' });
  assert.equal(await isNativeDialogOpen(), false);
  assert.equal(complete.exists, true);
  assert.equal(complete.received, content.length);
  assert.deepEqual(await readFile(complete.path), content);
  checks.push('WebView2 完成下载后自动收起提示，保存内容与源文件一致');
  const result = { passed: true, profile, checks };
  await writeFile(join(output, 'result.json'), JSON.stringify(result, null, 2));
  console.log(JSON.stringify(result, null, 2));
} catch (error) {
  await writeFile(join(output, 'failure.json'), JSON.stringify({ checks, error: String(error), items: ui ? await items().catch(() => []) : [] }, null, 2));
  throw error;
} finally {
  await pageBrowser?.close().catch(() => {});
  await uiBrowser?.close().catch(() => {});
  child?.kill();
  server.closeAllConnections();
  server.close();
}
