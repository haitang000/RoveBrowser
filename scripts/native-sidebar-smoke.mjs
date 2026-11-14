// Check sidebar resizing against real WebView2 viewports and SQLite restart.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { access, mkdtemp, mkdir, writeFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { DatabaseSync } from 'node:sqlite';
import { chromium } from '@playwright/test';
import { createLegacyNativeProfile } from './native-profile-fixture.mjs';

const profile = await mkdtemp(join(tmpdir(), 'rove-sidebar-'));
createLegacyNativeProfile(profile);
const port = Number(process.env.ERA_SMOKE_PORT || 19946);
const output = resolve(process.env.ERA_SMOKE_OUTPUT || 'test-results/native-sidebar');
await mkdir(output, { recursive: true });
function savedState() {
  const db = new DatabaseSync(join(profile, 'era.sqlite'), { readOnly: true });
  try { return JSON.parse(db.prepare('SELECT json FROM session WHERE id=1').get().json); }
  finally { db.close(); }
}
// No external websites are needed by this fixture.
{
  const state = savedState();
  state.favorites = [];
  state.tabs = state.tabs.filter(tab => tab.id === 'start');
  const db = new DatabaseSync(join(profile, 'era.sqlite'));
  try { db.prepare('UPDATE session SET json=? WHERE id=1').run(JSON.stringify(state)); }
  finally { db.close(); }
}
const server = createServer((req, res) => {
  res.setHeader('Content-Type', 'text/html; charset=utf-8');
  res.end(`<!doctype html><title>Sidebar fixture ${req.url}</title>
    <style>html{background:#164e63;color:white;font:24px system-ui}body{margin:32px}</style>
    <h1>Live viewport ${req.url}</h1><p id="width"></p><script>
    window.marker=crypto.randomUUID();function tick(){document.querySelector('#width').textContent=innerWidth;requestAnimationFrame(tick)}tick();</script>`);
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
const base = `http://127.0.0.1:${server.address().port}`;
async function waitFor(fn, label) {
  for (let i = 0; i < 150; i++) {
    try { const value = await fn(); if (value) return value; } catch {}
    await new Promise(resolve => setTimeout(resolve, 50));
  }
  throw Error('Timed out: ' + label);
}
let child, uiBrowser, pageBrowser, ui;
const send = command => ui.evaluate(command => window.ipc.postMessage(JSON.stringify(command)), command);
const sidebarWidth = () => ui.locator('.sidebar-track').evaluate(e => e.getBoundingClientRect().width);
async function start() {
  child = spawn(resolve(process.env.ERA_SMOKE_EXE || 'native/target/debug/rove.exe'), [], {
    env: { ...process.env, ERA_DATA_DIR: profile, ERA_DEBUG_PORT: String(port) }, windowsHide: true, stdio: 'pipe',
  });
  child.stderr.on('data', data => process.stderr.write(data));
  uiBrowser = await waitFor(() => chromium.connectOverCDP(`http://127.0.0.1:${port}`), 'UI debugger');
  ui = await waitFor(() => uiBrowser.contexts().flatMap(c => c.pages()).find(p => p.url() === 'https://era.ui/index.html'), 'shell');
  await ui.waitForSelector('.sidebar-resize-handle');
}
async function stop() {
  const stopped = child && child.exitCode === null && child.signalCode === null && once(child, 'exit');
  child?.kill();
  await stopped;
  await pageBrowser?.close().catch(() => {});
  await uiBrowser?.close().catch(() => {});
  child = uiBrowser = pageBrowser = undefined;
}
async function drag(pages, target) {
  await ui.bringToFront();
  await ui.evaluate(() => {
    window.__sidebarEvents = [];
    for (const type of ['blur', 'pointercancel', 'lostpointercapture']) {
      window.addEventListener(type, event => window.__sidebarEvents.push({ type, pointerId: event.pointerId }), { once: true, capture: true });
    }
  });
  const markers = await Promise.all(pages.map(page => page.evaluate(() => window.marker)));
  const before = savedState().sidebarWidth;
  const handle = ui.getByRole('separator', { name: '调整侧栏宽度' });
  const bounds = await handle.boundingBox();
  const initial = await sidebarWidth();
  await ui.mouse.move(bounds.x + 1, bounds.y + 100);
  await ui.mouse.down();
  await waitFor(() => ui.locator('.browser-shell').evaluate(e => e.classList.contains('sidebar-resizing')), 'drag captured');
  for (const delta of [1, 100, -80, target - initial]) {
    await ui.mouse.move(bounds.x + 1 + delta, bounds.y + 100, { steps: 4 });
    const expected = Math.max(192, Math.min(480, initial + delta));
    try {
      await waitFor(async () => Math.abs(await sidebarWidth() - expected) < 0.1, `sidebar tracks pointer (${initial} + ${delta})`);
    } catch (error) {
      console.error(JSON.stringify(await ui.evaluate(() => ({
        events: window.__sidebarEvents,
        width: document.querySelector('.sidebar-track').getBoundingClientRect().width,
        resizing: document.querySelector('.browser-shell').classList.contains('sidebar-resizing'),
      }))));
      throw error;
    }
    const widths = await ui.locator('.page-pane > .webview-slot').evaluateAll(slots => slots.map(e => e.getBoundingClientRect().width));
    for (let i = 0; i < pages.length; i++) {
      await waitFor(async () => Math.abs(await pages[i].evaluate(() => innerWidth) - widths[i]) <= 1, 'live viewport');
      assert.equal(await pages[i].evaluate(() => document.visibilityState), 'visible');
      assert.equal(await pages[i].evaluate(() => window.marker), markers[i]);
    }
    assert.equal(savedState().sidebarWidth, before, 'drag does not write preferences every frame: ' + JSON.stringify(await ui.evaluate(() => ({
      events: window.__sidebarEvents,
      resizing: document.querySelector('.browser-shell').classList.contains('sidebar-resizing'),
    }))));
  }
  await ui.mouse.up();
  await waitFor(() => savedState().sidebarWidth === target, 'width saved on release');
}
try {
  await start();
  await send({ type: 'navigate', url: base + '/left' });
  pageBrowser = await waitFor(() => chromium.connectOverCDP(`http://127.0.0.1:${port + 1}`), 'page debugger');
  const site = path => waitFor(() => pageBrowser.contexts().flatMap(c => c.pages()).find(p => p.url() === base + path), path);
  const left = await site('/left');
  await left.waitForSelector('h1');
  if (process.env.ERA_SMOKE_REVIEW === '1') {
    console.log(JSON.stringify({ reviewReady: true, pid: child.pid, port, output }));
    for (let i = 0; i < 300; i++) {
      if (await access(join(output, 'resume')).then(() => true, () => false)) break;
      await new Promise(resolve => setTimeout(resolve, 1000));
    }
    console.log(JSON.stringify({ desktopDragWidth: await sidebarWidth(), savedWidth: savedState().sidebarWidth }));
    await send({ type: 'sidebarWidth', width: 240 });
    await waitFor(async () => await sidebarWidth() === 240, 'reset review width');
  }
  await drag([left], 336);
  const leftId = await ui.getByRole('tab', { selected: true }).getAttribute('data-tab-id');
  await send({ type: 'new', url: base + '/right' });
  const right = await site('/right');
  await right.waitForSelector('h1');
  const rightId = await ui.getByRole('tab', { selected: true }).getAttribute('data-tab-id');
  await send({ type: 'split', id: leftId, other: rightId });
  await ui.locator('.split-divider').waitFor();
  await drag([left, right], 360);
  await send({ type: 'sidebar' });
  await waitFor(async () => await sidebarWidth() === 0, 'collapsed');
  await send({ type: 'sidebar' });
  await waitFor(async () => await sidebarWidth() === 360, 'expanded at saved width');
  await stop();
  await start();
  await waitFor(async () => await sidebarWidth() === 360, 'restored after restart');
  await ui.getByRole('separator', { name: '调整侧栏宽度' }).dblclick();
  await waitFor(() => savedState().sidebarWidth === undefined, 'reset clears preference');
  await waitFor(async () => await sidebarWidth() === 240, 'default restored');
  const report = { passed: true, profile, checks: ['single viewport', 'split viewports', 'save on release', 'collapse/expand', 'restart', 'reset'] };
  await writeFile(join(output, 'result.json'), JSON.stringify(report, null, 2));
  console.log(JSON.stringify(report));
} finally { await stop(); server.close(); }
