// Exercise painted split viewports with two real WebView2 controllers.
import assert from 'node:assert/strict';
import { createLegacyNativeProfile } from './native-profile-fixture.mjs';
import { chromium } from '@playwright/test';
import { execFile, spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { access, mkdtemp, mkdir, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { promisify } from 'node:util';
import { checkSplitResize } from './native-split-checks.mjs';

const profile = await mkdtemp(join(tmpdir(), 'rove-split-'));
createLegacyNativeProfile(profile);
const port = Number(process.env.ERA_SMOKE_PORT || 19876);
const output = resolve(process.env.ERA_SMOKE_OUTPUT || 'test-results/native-split');
await mkdir(output, { recursive: true });
const hits = {};
const input = promisify(execFile);
const server = createServer((req, res) => {
  hits[req.url] = (hits[req.url] || 0) + 1;
  const right = req.url === '/right';
  res.setHeader('Content-Type', 'text/html; charset=utf-8');
  res.end(`<!doctype html><title>${right ? 'Right' : 'Left'} split fixture</title>
    <style>html{background:${right ? '#385743' : '#164e63'};color:white;font:24px system-ui}
    body{margin:32px}h1{font-size:32px}</style>
    <h1>${right ? 'Right' : 'Left'} live page</h1><p>Viewport: <b id="width"></b>px</p>
    <p>Frames: <b id="frames"></b></p><script>let frame=0;function tick(){
    document.querySelector('#width').textContent=innerWidth;
    document.querySelector('#frames').textContent=++frame;requestAnimationFrame(tick)}tick();</script>`);
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
const base = `http://127.0.0.1:${server.address().port}`;
async function waitFor(fn, label) {
  for (let i = 0; i < 150; i++) {
    try { const result = await fn(); if (result) return result; } catch {}
    await new Promise(resolve => setTimeout(resolve, 50));
  }
  throw Error('Timed out: ' + label);
}
let child, uiBrowser, pageBrowser;
try {
  child = spawn(resolve(process.env.ERA_SMOKE_EXE || 'native/target/debug/rove.exe'), [], {
    env: { ...process.env, ERA_DATA_DIR: profile, ERA_DEBUG_PORT: String(port) },
    windowsHide: true, stdio: 'pipe',
  });
  child.stderr.on('data', data => process.stderr.write(data));
  uiBrowser = await waitFor(() => chromium.connectOverCDP(`http://127.0.0.1:${port}`), 'UI debugger');
  const ui = await waitFor(() => uiBrowser.contexts().flatMap(c => c.pages()).find(p => p.url() === 'https://era.ui/index.html'), 'shell');
  await ui.waitForSelector('.sidebar');
  const send = command => ui.evaluate(command => window.ipc.postMessage(JSON.stringify(command)), command);
  await send({ type: 'navigate', url: base + '/left' });
  pageBrowser = await waitFor(() => chromium.connectOverCDP(`http://127.0.0.1:${port + 1}`), 'page debugger');
  const site = path => waitFor(() => pageBrowser.contexts().flatMap(c => c.pages()).find(p => p.url() === base + path), path);
  const left = await site('/left'); await left.waitForSelector('h1');
  const leftId = await ui.getByRole('tab', { selected: true }).getAttribute('data-tab-id');
  await send({ type: 'new', url: base + '/right' });
  const right = await site('/right'); await right.waitForSelector('h1');
  const rightId = await ui.getByRole('tab', { selected: true }).getAttribute('data-tab-id');
  await send({ type: 'activate', id: leftId });
  await waitFor(async () => (await ui.getByRole('tab', { selected: true }).getAttribute('data-tab-id')) === leftId, 'primary activation');
  const from = await ui.locator(`.tab-list [data-tab-id="${rightId}"]`).boundingBox();
  const area = await ui.locator('.page-area').boundingBox();
  const viewport = await ui.evaluate(() => ({ width: innerWidth, height: innerHeight }));
  const tabDropInput = action => input('powershell.exe', [
    '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', resolve('scripts/real-keys.ps1'),
    '-ProcessId', String(child.pid), '-Action', action,
    '-FromXFrac', String((from.x + 45) / viewport.width), '-FromYFrac', String((from.y + from.height / 2) / viewport.height),
    '-xFrac', String((area.x + area.width * 0.75) / viewport.width), '-yFrac', String((area.y + area.height / 2) / viewport.height),
  ], { windowsHide: true, timeout: 15000 });
  await left.evaluate(() => {
    window.tabDropClicks = 0;
    window.tabDropMarker = crypto.randomUUID();
    addEventListener('pointerdown', () => window.tabDropClicks++);
  });
  const marker = await left.evaluate(() => window.tabDropMarker);
  const cancelled = tabDropInput('cancel-tab-drag');
  try {
    await ui.locator('.split-drop-target').waitFor({ timeout: 5000 });
    assert.equal(await left.evaluate(() => document.visibilityState), 'visible');
  } finally {
    await cancelled;
  }
  await ui.locator('.split-drop-target').waitFor({ state: 'detached' });
  assert.equal(await ui.locator('.split-divider').count(), 0, 'Esc cancels the drop');
  await tabDropInput('click-page');
  await waitFor(() => left.evaluate(() => window.tabDropClicks === 1), 'live page receives clicks after cancelling');
  await tabDropInput('drag-tab');
  await ui.locator('.split-divider').waitFor({ timeout: 5000 });
  await ui.locator('.split-drop-target').waitFor({ state: 'detached' });
  assert.equal(await left.evaluate(() => window.tabDropMarker), marker, 'dragging keeps the live page document');
  const widths = await ui.locator('.webview-slot').evaluateAll(slots => slots.map(slot => slot.getBoundingClientRect().width));
  await waitFor(async () => {
    const viewports = await Promise.all([left, right].map(page => page.evaluate(() => innerWidth)));
    return viewports.every((width, index) => Math.abs(width - widths[index]) <= 1);
  }, 'split entry commits both viewports');
  if (process.env.ERA_SMOKE_REVIEW === '1') {
    await ui.evaluate(() => {
      window.__nativeSplitEvents = [];
      const receive = window.__ERA_RECEIVE__;
      window.__ERA_RECEIVE__ = event => {
        if (event.type === 'splitResize') window.__nativeSplitEvents.push(event);
        receive(event);
      };
    });
    console.log(JSON.stringify({ reviewReady: true, pid: child.pid, port, output }));
    for (let i = 0; i < 300; i++) {
      if (await access(join(output, 'resume')).then(() => true, () => false)) break;
      await new Promise(resolve => setTimeout(resolve, 1000));
    }
    console.log(JSON.stringify({ nativeMouseEvents: await ui.evaluate(() => window.__nativeSplitEvents) }));
  }
  const metrics = await checkSplitResize(ui, [left, right], { waitFor, capturePaints: true });
  const rightPane = ui.getByRole('region', { name: '右侧窗格' });
  const popup = await waitFor(() => ui.context().pages().find(p => p.url().endsWith('?popup')), 'popup renderer');
  assert.equal(await ui.locator('.titlebar').count(), 0);
  await rightPane.getByRole('button', { name: '搜索或输入地址' }).click();
  const field = popup.getByRole('combobox');
  await field.waitFor();
  assert.equal(await field.inputValue(), base + '/right');
  await field.fill(base + '/right-next');
  await field.press('Enter');
  await right.waitForURL(base + '/right-next');
  assert.equal(left.url(), base + '/left');
  await rightPane.getByRole('button', { name: '后退' }).click();
  await right.waitForURL(base + '/right');
  await rightPane.getByRole('button', { name: '前进' }).click();
  await right.waitForURL(base + '/right-next');
  const reloads = hits['/right-next'];
  const leftLoads = hits['/left'];
  await rightPane.getByRole('button', { name: '刷新' }).click();
  await waitFor(() => hits['/right-next'] > reloads, 'right pane refresh');
  assert.equal(hits['/left'], leftLoads);
  await ui.getByRole('button', { name: '关闭分屏' }).click();
  await ui.locator('.titlebar').waitFor();
  const report = { passed: true, profile, hits, ...metrics };
  await mkdir(output, { recursive: true });
  await writeFile(join(output, 'result.json'), JSON.stringify(report, null, 2));
  console.log(JSON.stringify(report));
} finally {
  await pageBrowser?.close(); await uiBrowser?.close(); child?.kill(); server.close();
}
