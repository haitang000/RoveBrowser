// Check sidebar resizing against real WebView2 viewports and SQLite restart.
import assert from 'node:assert/strict';
import { execFile, spawn } from 'node:child_process';
import { once } from 'node:events';
import { access, mkdtemp, mkdir, writeFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { promisify } from 'node:util';
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
    <style>html{background:#164e63;color:white;font:24px system-ui}body{margin:32px}
    .columns{display:grid;grid-template-columns:repeat(auto-fit,minmax(260px,1fr));gap:20px}
    .panel{background:#ffffff18;border:1px solid #ffffff40;border-radius:12px;padding:20px;font-size:18px}</style>
    <h1>Live viewport ${req.url}</h1><p id="width"></p>
    <div class="columns">${['Frozen snapshot', 'Fresh layout', 'Final size'].map(title => `<section class="panel"><h2>${title}</h2><p>Resize the sidebar to change this grid. The renderer should lay out these cards once on release while the old image scales in perspective.</p></section>`).join('')}</div><script>
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
const motionEvents = [];
let desktopReview = process.env.ERA_SMOKE_REVIEW === '1';
const send = command => ui.evaluate(command => window.ipc.postMessage(JSON.stringify(command)), command);
const sidebarWidth = () => ui.locator('.sidebar-track').evaluate(e => e.getBoundingClientRect().width);
const execFileAsync = promisify(execFile);
async function focusShortcut(pages, width) {
  const markers = await Promise.all(pages.map(page => page.evaluate(() => window.marker)));
  for (const collapsed of [true, false]) {
    const mark = motionEvents.length;
    for (const page of pages) await page.evaluate(() => {
      window.focusWidths = [];
      window.onresize = () => window.focusWidths.push(innerWidth);
    });
    // Trusted OS input reaches the focused page's native accelerator handler.
    await execFileAsync('powershell.exe', ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-File',
      resolve('scripts/real-keys.ps1'), '-ProcessId', String(child.pid), '-Action', 'ctrl-shift-f']);
    await waitFor(async () => await sidebarWidth() === (collapsed ? 0 : width), 'focus shortcut layout');
    assert.equal(await ui.locator('.reveal-sidebar').count(), 0, 'no reveal button in fully hidden mode');
    await waitFor(() => savedState().sidebarHidden === collapsed, 'fully hidden state saved');
    const widths = await ui.locator('.page-pane > .webview-slot').evaluateAll(slots => slots.map(e => e.getBoundingClientRect().width));
    for (let i = 0; i < pages.length; i++) {
      await waitFor(async () => Math.abs(await pages[i].evaluate(() => innerWidth) - widths[i]) <= 1, 'final focus viewport');
      assert.equal(await pages[i].evaluate(() => window.marker), markers[i], 'toggle preserves the open page');
      assert.ok((await pages[i].evaluate(() => window.focusWidths)).length <= 1, 'renderer reflows once after the snapshot animation');
    }
    assert.equal(await ui.locator('.find-bar').count(), 0, 'Ctrl+Shift+F does not open Find');
    const blurStart = motionEvents.findIndex((e, i) => i >= mark && e.phase === 'sidebar-blur');
    assert.ok(blurStart >= mark, 'sidebar starts its content blur');
    await waitFor(() => motionEvents.slice(blurStart).filter(e => e.phase === 'restored').length >= pages.length, 'blur settles for each pane');
    const samples = motionEvents.slice(blurStart).filter(e => e.phase === 'settling');
    assert.ok(samples.some(e => e.blur > 0), 'content blurs during the transition');
    assert.ok(samples.every(e => e.scale === 1 && e.depth.every(n => n === 0) && e.slide === 0), 'sidebar motion keeps the page flat and at full scale');
    if (collapsed) {
      assert.deepEqual(await ui.locator('.page-area').evaluate(e => {
        const r = e.getBoundingClientRect();
        return [r.left, innerWidth - r.right];
      }), [8, 8], 'fully hidden pages have matching outer margins');
    }
  }
}
async function useDomInput() {
  await ui.evaluate(() => {
    window.__nativeSidebarFeedback = [];
    const receive = window.__ERA_RECEIVE__;
    window.__ERA_RECEIVE__ = event => {
      if (event.type === 'sidebarResize') window.__nativeSidebarFeedback.push(event);
      receive(event);
    };
    const post = window.chrome.webview.postMessage.bind(window.chrome.webview);
    window.chrome.webview.postMessage = message => {
      const command = JSON.parse(message);
      if (command.type === 'layout') { command.sidebar = null; post(JSON.stringify(command)); }
      else post(message);
    };
    window.dispatchEvent(new Event('resize'));
  });
}
async function start() {
  child = spawn(resolve(process.env.ERA_SMOKE_EXE || 'native/target/debug/rove.exe'), [], {
    env: { ...process.env, ERA_DATA_DIR: profile, ERA_DEBUG_PORT: String(port), ERA_RESIZE_TRACE: '1' }, windowsHide: true, stdio: 'pipe',
  });
  let partial = '';
  child.stderr.on('data', data => {
    partial += data.toString();
    const lines = partial.split(/\r?\n/); partial = lines.pop();
    for (const line of lines) {
      if (line.startsWith('Rove resize: ')) motionEvents.push(JSON.parse(line.slice('Rove resize: '.length)));
      else if (line) process.stderr.write(line + '\n');
    }
  });
  uiBrowser = await waitFor(() => chromium.connectOverCDP(`http://127.0.0.1:${port}`), 'UI debugger');
  ui = await waitFor(() => uiBrowser.contexts().flatMap(c => c.pages()).find(p => p.url() === 'https://era.ui/index.html'), 'shell');
  await ui.waitForSelector('.sidebar-resize-handle');
  // CDP injects input into a WebView, bypassing the native strip and potentially
  // stealing its focus/capture. This fixture deliberately checks the DOM fallback
  // with real page controllers; native mouse/capture is covered by sidebar.rs
  // and desktop input against the same fixture in review mode.
  if (!desktopReview) await useDomInput();
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
        native: window.__nativeSidebarFeedback,
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
    console.log(JSON.stringify({ reviewReady: true, pid: child.pid, port, output, profile }));
    for (let i = 0; i < 300; i++) {
      if (await access(join(output, 'resume')).then(() => true, () => false)) break;
      await new Promise(resolve => setTimeout(resolve, 1000));
    }
    console.log(JSON.stringify({ desktopDragWidth: await sidebarWidth(), savedWidth: savedState().sidebarWidth }));
    desktopReview = false;
    await useDomInput();
    await send({ type: 'sidebarWidth', width: 240 });
    await waitFor(async () => await sidebarWidth() === 240, 'reset review width');
  }
  if (process.env.ERA_SMOKE_REVIEW_ONLY !== '1') {
  await drag([left], 336);
  await focusShortcut([left], 336);
  const leftId = await ui.getByRole('tab', { selected: true }).getAttribute('data-tab-id');
  await send({ type: 'new', url: base + '/right' });
  const right = await site('/right');
  await right.waitForSelector('h1');
  const rightId = await ui.getByRole('tab', { selected: true }).getAttribute('data-tab-id');
  await send({ type: 'split', id: leftId, other: rightId });
  await ui.locator('.split-divider').waitFor();
  await drag([left, right], 360);
  await focusShortcut([left, right], 360);
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
  const report = { passed: true, profile, checks: ['single viewport', 'split viewports', 'save on release', 'collapse/expand', 'Ctrl+Shift+F from a focused page', 'fully hidden sidebar', 'blur without perspective or scale', 'matching outer margins', 'one reflow after sidebar motion', 'restart', 'reset'] };
  await writeFile(join(output, 'result.json'), JSON.stringify(report, null, 2));
  console.log(JSON.stringify(report));
  }
} finally { await stop(); server.close(); }
