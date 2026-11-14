// A stalled shell must not hold the real page viewports at the previous size.
import assert from 'node:assert/strict';
import { spawn, execFile } from 'node:child_process';
import { once } from 'node:events';
import { mkdtemp, mkdir, writeFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { promisify } from 'node:util';
import { DatabaseSync } from 'node:sqlite';
import { chromium } from '@playwright/test';
import { createLegacyNativeProfile } from './native-profile-fixture.mjs';

const profile = await mkdtemp(join(tmpdir(), 'rove-resize-'));
createLegacyNativeProfile(profile);
const db = new DatabaseSync(join(profile, 'era.sqlite'));
const state = JSON.parse(db.prepare('SELECT json FROM session WHERE id=1').get().json);
state.favorites = [];
state.tabs = state.tabs.filter(tab => tab.id === 'start');
db.prepare('UPDATE session SET json=? WHERE id=1').run(JSON.stringify(state));
db.close();
const output = resolve(process.env.ERA_SMOKE_OUTPUT || 'test-results/native-resize');
await mkdir(output, { recursive: true });
const port = Number(process.env.ERA_SMOKE_PORT || 19958);
const run = promisify(execFile);
const server = createServer((req, res) => {
  res.setHeader('Content-Type', 'text/html; charset=utf-8');
  res.end(`<!doctype html><title>Resize ${req.url}</title>
    <style>html{background:#164e63;color:white;font:24px system-ui}body{margin:32px}</style>
    <h1>Real viewport ${req.url}</h1><script>window.marker=crypto.randomUUID()</script>`);
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
const base = `http://127.0.0.1:${server.address().port}`;
async function waitFor(fn, label, timeout = 8000) {
  const end = Date.now() + timeout;
  do {
    try { const value = await fn(); if (value) return value; } catch {}
    await new Promise(resolve => setTimeout(resolve, 40));
  } while (Date.now() < end);
  throw Error(`Timed out: ${label}`);
}
let child, uiBrowser, pageBrowser;
const checks = [];
try {
  child = spawn(resolve(process.env.ERA_SMOKE_EXE || 'native/target/debug/rove.exe'), [], {
    env: { ...process.env, ERA_DATA_DIR: profile, ERA_DEBUG_PORT: String(port) }, windowsHide: true, stdio: 'pipe',
  });
  child.stderr.on('data', data => process.stderr.write(data));
  uiBrowser = await waitFor(() => chromium.connectOverCDP(`http://127.0.0.1:${port}`), 'shell debugger');
  const ui = await waitFor(() => uiBrowser.contexts().flatMap(c => c.pages()).find(p => p.url() === 'https://era.ui/index.html'), 'shell');
  await ui.waitForSelector('.sidebar-resize-handle');
  const send = command => ui.evaluate(command => window.ipc.postMessage(JSON.stringify(command)), command);
  await send({ type: 'navigate', url: base + '/left' });
  pageBrowser = await waitFor(() => chromium.connectOverCDP(`http://127.0.0.1:${port + 1}`), 'page debugger');
  const site = path => waitFor(() => pageBrowser.contexts().flatMap(c => c.pages()).find(p => p.url() === base + path), path);
  const left = await site('/left');
  await left.waitForSelector('h1');
  const client = await ui.context().newCDPSession(ui);
  await client.send('Runtime.enable');
  async function resize(pages, width, height) {
    const previous = await ui.evaluate(() => ({ width: innerWidth, height: innerHeight }));
    const rects = await ui.locator('.page-pane > .webview-slot').evaluateAll(slots => slots.map(e => {
      const r = e.getBoundingClientRect();
      return { x: r.x, y: r.y, width: r.width, height: r.height };
    }));
    const markers = await Promise.all(pages.map(p => p.evaluate(() => window.marker)));
    let shellFinished = false;
    const started = new Promise(resolve => client.once('Runtime.consoleAPICalled', resolve));
    const blocked = client.send('Runtime.evaluate', { expression: `console.log('resize-blocked'); { const end=performance.now()+5000; while(performance.now()<end){} }` })
      .finally(() => shellFinished = true);
    await started;
    try {
      const result = await run('powershell.exe', ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', resolve('scripts/resize-window.ps1'),
        '-ProcessId', String(child.pid), '-Width', String(width), '-Height', String(height)], { windowsHide: true });
      const current = JSON.parse(result.stdout.trim());
      const total = rects.reduce((sum, r) => sum + r.width, 0);
      const sidebar = w => Math.min(w <= 740 ? 216 : 240, Math.min(480, Math.max(192, w - 248)));
      const dx = sidebar(current.width) - sidebar(previous.width);
      const dw = current.width - previous.width - dx;
      const dh = current.height - previous.height;
      const expected = rects.map(r => ({ width: r.width + dw * r.width / total, height: r.height + dh }));
      await waitFor(async () => {
        const sizes = await Promise.all(pages.map(p => p.evaluate(() => ({ width: innerWidth, height: innerHeight }))));
        return sizes.every((s, i) => Math.abs(s.width - expected[i].width) <= 1 && Math.abs(s.height - expected[i].height) <= 1);
      }, 'live pages resize while the shell is blocked', 2500);
      assert.equal(shellFinished, false, 'native resizing must finish before shell layout resumes');
      checks.push({ pages: pages.length, current, expected, independentOfShell: true });
    } finally { await blocked; }
    // Crossing the responsive breakpoint can start the sidebar's CSS width
    // transition once the shell unblocks. Let that intentional motion finish
    // before checking that a stale IPC message leaves the final bounds intact.
    await ui.evaluate(async () => {
      await new Promise(resolve => requestAnimationFrame(resolve));
      await Promise.all(document.querySelector('.sidebar-track').getAnimations().map(a => a.finished.catch(() => {})));
    });
    await waitFor(async () => {
      const rects = await ui.locator('.page-pane > .webview-slot').evaluateAll(slots => slots.map(e => ({ width: e.clientWidth, height: e.clientHeight })));
      const sizes = await Promise.all(pages.map(p => p.evaluate(() => ({ width: innerWidth, height: innerHeight }))));
      return sizes.every((s, i) => Math.abs(s.width - rects[i].width) <= 1 && Math.abs(s.height - rects[i].height) <= 1);
    }, 'authoritative shell layout');
    for (let i = 0; i < pages.length; i++) {
      assert.equal(await pages[i].evaluate(() => window.marker), markers[i]);
      assert.equal(await pages[i].evaluate(() => document.visibilityState), 'visible');
    }
    // An IPC report from before the resize must not roll back the real viewport.
    const before = await Promise.all(pages.map(p => p.evaluate(() => innerWidth)));
    await send({ type: 'layout', primary: rects[0], secondary: rects[1] ?? null, viewport: previous });
    await new Promise(resolve => setTimeout(resolve, 150));
    const after = await Promise.all(pages.map(p => p.evaluate(() => innerWidth)));
    // The current DOM report can still settle a fractional CSS border by one
    // pixel; the old viewport (hundreds of pixels away) must never return.
    assert.ok(after.every((width, i) => Math.abs(width - before[i]) <= 1),
      'reject stale layout: ' + JSON.stringify({ previous, rects, before, after, current: await ui.evaluate(() => ({ width: innerWidth, height: innerHeight })) }));
  }
  await resize([left], 1000, 700);
  await resize([left], 700, 600);
  await resize([left], 1280, 820);
  const leftId = await ui.getByRole('tab', { selected: true }).getAttribute('data-tab-id');
  await send({ type: 'new', url: base + '/right' });
  const right = await site('/right');
  await right.waitForSelector('h1');
  const rightId = await ui.getByRole('tab', { selected: true }).getAttribute('data-tab-id');
  await send({ type: 'split', id: leftId, other: rightId });
  await ui.locator('.split-divider').waitFor();
  await resize([left, right], 1000, 700);
  await resize([left, right], 1280, 820);
  const report = { passed: true, profile, checks };
  await writeFile(join(output, 'result.json'), JSON.stringify(report, null, 2));
  console.log(JSON.stringify(report));
} finally {
  const stopped = child && child.exitCode === null && child.signalCode === null && once(child, 'exit');
  child?.kill();
  await stopped;
  await pageBrowser?.close().catch(() => {});
  await uiBrowser?.close().catch(() => {});
  server.close();
}
