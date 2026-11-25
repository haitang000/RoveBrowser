// Compare the same large session in two native builds using ERA_PERF_EXE.
import { chromium } from '@playwright/test';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdtemp, mkdir, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { DatabaseSync } from 'node:sqlite';
import assert from 'node:assert/strict';
import { createLegacyNativeProfile } from './native-profile-fixture.mjs';

const profile = await mkdtemp(join(tmpdir(), 'rove-performance-'));
createLegacyNativeProfile(profile);
const db = new DatabaseSync(join(profile, 'era.sqlite'));
const state = JSON.parse(db.prepare('SELECT json FROM session').get().json);
const icon = `data:image/svg+xml,${encodeURIComponent(`<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32"><rect width="32" height="32" fill="skyblue"/><!--${'x'.repeat(2048)}--></svg>`)}`;
state.libraryVersion = 1;
state.iconVersion = 2;
state.settings = { ...state.settings, askBeforeQuit: false, searchSuggestions: false };
state.favorites = [];
state.tabs = Array.from({ length: 200 }, (_, i) => ({ id: `tab-${i}`, spaceId: 'personal', title: `Tab ${i}`, url: 'era://newtab', pinned: false, favicon: icon, lastActivated: Math.floor(Date.now() / 1000) }));
state.pins = Array.from({ length: 200 }, (_, i) => ({ id: `pin-${i}`, spaceId: 'personal', title: `Saved ${i}`, url: `https://example.com/${i}`, favicon: icon }));
state.activeTab = state.tabs[0].id;
db.prepare('UPDATE session SET json=? WHERE id=1').run(JSON.stringify(state));
db.close();
const executable = resolve(process.env.ERA_PERF_EXE || 'native/target/debug/rove.exe');
const port = Number(process.env.ERA_PERF_PORT || 19936);
const output = resolve(process.env.ERA_PERF_OUTPUT || 'test-results/native-performance');
await mkdir(output, { recursive: true });
let app, browser, ui;
async function waitFor(check, label) {
  const deadline = Date.now() + 30000;
  while (Date.now() < deadline) {
    try { const value = await check(); if (value) return value; } catch {}
    await new Promise(done => setTimeout(done, 40));
  }
  throw Error(`Timed out: ${label}`);
}
async function launch() {
  const started = performance.now();
  app = spawn(executable, [], { env: { ...process.env, ERA_DATA_DIR: profile, ERA_DEBUG_PORT: String(port) }, windowsHide: true, stdio: 'pipe' });
  app.stderr.on('data', data => process.stderr.write(data));
  await waitFor(async () => {
    const targets = await fetch(`http://127.0.0.1:${port}/json/list`, { signal: AbortSignal.timeout(500) }).then(r => r.json());
    return targets.some(t => t.url === 'https://era.ui/index.html');
  }, 'UI target');
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
  ui = await waitFor(() => browser.contexts().flatMap(c => c.pages()).find(p => p.url() === 'https://era.ui/index.html'), 'UI page');
  await ui.locator('.sidebar').waitFor();
  await ui.evaluate(() => document.fonts.ready);
  await ui.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  return Math.round(performance.now() - started);
}
async function close() {
  if (!app || app.exitCode !== null) return;
  const exited = once(app, 'exit');
  await ui.evaluate(() => window.ipc.postMessage(JSON.stringify({ type: 'window', action: 'close' })));
  await Promise.race([exited, new Promise((_, reject) => setTimeout(() => reject(Error('Graceful quit timed out')), 10000).unref())]);
  await browser.close();
}
try {
  const startupMs = await launch();
  await ui.waitForTimeout(600);
  let profiler;
  if (process.env.ERA_PERF_PROFILE) {
    profiler = await ui.context().newCDPSession(ui);
    await profiler.send('Profiler.enable');
    await profiler.send('Profiler.start');
  }
  const measurements = await ui.evaluate(async () => {
    const samples = [];
    const stats = { paletteReads: 0, stateMessages: 0, stateBytes: 0 };
    const read = CanvasRenderingContext2D.prototype.getImageData;
    CanvasRenderingContext2D.prototype.getImageData = function (...args) { stats.paletteReads++; return read.apply(this, args); };
    const record = event => {
      if (event.data?.type === 'state' || event.data?.type === 'statePatch') {
        stats.stateMessages++;
        stats.stateBytes += new TextEncoder().encode(JSON.stringify(event.data)).length;
      }
    };
    window.chrome?.webview?.addEventListener('message', record);
    let done, receivedAt = 0, receiveMs = 0;
    const receive = window.__ERA_RECEIVE__;
    window.__ERA_RECEIVE__ = event => {
      const started = performance.now();
      receive(event);
      if (event.type === 'state') { receivedAt = started; receiveMs = performance.now() - started; window.__perfState = event.state; done?.(event.state.activeTab); }
    };
    for (let i = 1; i <= 40; i++) {
      const id = `tab-${i}`;
      const started = performance.now();
      await new Promise((resolve, reject) => {
        const timeout = setTimeout(() => reject(Error(`Activation timed out: ${id}`)), 10000);
        done = active => { if (active === id) { clearTimeout(timeout); resolve(); } };
        window.ipc.postMessage(JSON.stringify({ type: 'activate', id }));
      });
      await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
      samples.push({ total: performance.now() - started, native: receivedAt - started, receive: receiveMs });
    }
    window.chrome?.webview?.removeEventListener('message', record);
    CanvasRenderingContext2D.prototype.getImageData = read;
    return { samples, stats };
  });
  if (profiler) {
    const { profile } = await profiler.send('Profiler.stop');
    await writeFile(join(output, 'ui-profile.json'), JSON.stringify(profile));
    await profiler.detach();
  }
  const samples = measurements.samples.map(sample => sample.total);
  if (process.env.ERA_PERF_EXPECT_OPTIMIZED === '1') assert.equal(measurements.stats.paletteReads, 0, 'Tab switching must reuse the caption palette');
  const burstMs = await ui.evaluate(async () => {
    const started = performance.now();
    const receive = window.__ERA_RECEIVE__;
    await new Promise((resolve, reject) => {
      const timeout = setTimeout(() => reject(Error('Activation burst timed out')), 30000);
      window.__ERA_RECEIVE__ = event => {
        receive(event);
        if (event.type === 'state' && event.state.activeTab === 'tab-100') { clearTimeout(timeout); requestAnimationFrame(() => requestAnimationFrame(resolve)); }
      };
      for (let i = 41; i <= 100; i++) window.ipc.postMessage(JSON.stringify({ type: 'activate', id: `tab-${i}` }));
    });
    return performance.now() - started;
  });
  // Quit immediately after the last update: shutdown must flush the last tab.
  await close();
  const saved = new DatabaseSync(join(profile, 'era.sqlite'), { readOnly: true });
  assert.equal(JSON.parse(saved.prepare('SELECT json FROM session').get().json).activeTab, 'tab-100');
  saved.close();
  await launch();
  assert.equal(await ui.evaluate(() => window.__ERA_INITIAL_STATE__.activeTab), 'tab-100');
  await close();
  const sorted = [...samples].sort((a, b) => a - b);
  const median = values => [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)];
  const result = { executable, profile, tabs: 200, pins: 200, sessionBytes: Buffer.byteLength(JSON.stringify(state)), startupMs, activationMedianMs: Math.round(sorted[Math.floor(sorted.length / 2)]), activationP95Ms: Math.round(sorted[Math.floor(sorted.length * .95)]), nativeRoundTripMedianMs: Math.round(median(measurements.samples.map(sample => sample.native))), receiveMedianMs: Math.round(median(measurements.samples.map(sample => sample.receive))), ...measurements.stats, burst60Ms: Math.round(burstMs), checks: ['last update flushed on graceful quit', 'active tab restored on restart'] };
  await writeFile(join(output, 'result.json'), JSON.stringify(result, null, 2));
  console.log(JSON.stringify(result, null, 2));
} finally {
  if (app?.exitCode === null) app.kill();
  await browser?.close().catch(() => {});
}
