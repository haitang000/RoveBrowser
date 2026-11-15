// Reload must keep processing host commands while SQLite's writer is busy.
import assert from 'node:assert/strict';
import { spawn, execFile } from 'node:child_process';
import { once } from 'node:events';
import { createServer } from 'node:http';
import { mkdtemp, mkdir, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { promisify } from 'node:util';
import { DatabaseSync } from 'node:sqlite';
import { chromium } from '@playwright/test';
import { createLegacyNativeProfile } from './native-profile-fixture.mjs';

const profile = await mkdtemp(join(tmpdir(), 'rove-reload-'));
createLegacyNativeProfile(profile);
const db = new DatabaseSync(join(profile, 'era.sqlite'));
const state = JSON.parse(db.prepare('SELECT json FROM session WHERE id=1').get().json);
state.favorites = [];
state.tabs = state.tabs.filter(tab => tab.id === 'start');
db.prepare('UPDATE session SET json=? WHERE id=1').run(JSON.stringify(state));
db.close();
const output = resolve(process.env.ERA_SMOKE_OUTPUT || 'test-results/native-reload');
await mkdir(output, { recursive: true });
let documentId = 0;
const server = createServer((req, res) => {
  if (req.url === '/favicon.ico') { res.writeHead(204); res.end(); return; }
  if (req.url === '/redirect') { res.writeHead(302, { Location: '/page' }); res.end(); return; }
  res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8', 'Cache-Control': 'no-store' });
  const id = ++documentId;
  res.end(`<!doctype html><title>Reload fixture ${id}</title><h1>Reload fixture</h1><script>window.documentId=${id}</script>`);
});
await new Promise(done => server.listen(0, '127.0.0.1', done));
const base = `http://127.0.0.1:${server.address().port}`;
const port = Number(process.env.ERA_SMOKE_PORT || 19974);
const checks = [];
const run = promisify(execFile);
let child, uiBrowser, pageBrowser, locker;
async function bounded(promise, label, timeout = 8000) {
  let timer;
  try {
    return await Promise.race([promise, new Promise((_, reject) => {
      timer = setTimeout(() => reject(Error(`Timed out: ${label}`)), timeout);
    })]);
  } finally { clearTimeout(timer); }
}
async function waitFor(fn, label) {
  const end = Date.now() + 10000;
  do {
    try { const value = await bounded(Promise.resolve().then(fn), label, 1000); if (value) return value; } catch {}
    await new Promise(done => setTimeout(done, 40));
  } while (Date.now() < end);
  throw Error(`Timed out: ${label}`);
}
try {
  child = spawn(resolve(process.env.ERA_SMOKE_EXE || 'native/target/debug/rove.exe'), [], {
    env: { ...process.env, ERA_DATA_DIR: profile, ERA_DEBUG_PORT: String(port) }, windowsHide: true, stdio: 'pipe',
  });
  child.stderr.on('data', data => process.stderr.write(data));
  uiBrowser = await waitFor(() => chromium.connectOverCDP(`http://127.0.0.1:${port}`), 'shell debugger');
  const ui = await waitFor(() => uiBrowser.contexts().flatMap(c => c.pages()).find(p => p.url() === 'https://era.ui/index.html'), 'shell');
  await ui.waitForSelector('.sidebar');
  const send = command => ui.evaluate(command => window.ipc.postMessage(JSON.stringify(command)), command);
  await ui.evaluate(() => {
    window.__reloadEvents = [];
    const receive = window.__ERA_RECEIVE__;
    window.__ERA_RECEIVE__ = event => { window.__reloadEvents.push(event); receive(event); };
  });
  await send({ type: 'settingsUpdate', settings: { ...await ui.evaluate(() => window.__ERA_INITIAL_STATE__.settings), askBeforeQuit: false } });
  await send({ type: 'navigate', url: base + '/page' });
  pageBrowser = await waitFor(() => chromium.connectOverCDP(`http://127.0.0.1:${port + 1}`), 'page debugger');
  const web = await waitFor(() => pageBrowser.contexts().flatMap(c => c.pages()).find(p => p.url() === base + '/page'), 'fixture');
  await web.waitForFunction(() => window.documentId > 0);
  const history = async (timeout = 1000) => {
    await ui.evaluate(() => { window.__reloadEvents = []; });
    await send({ type: 'history', query: 'Reload fixture' });
    await ui.waitForFunction(() => window.__reloadEvents.some(e => e.type === 'history'), null, { timeout });
    return ui.evaluate(() => window.__reloadEvents.find(e => e.type === 'history').entries);
  };
  await waitFor(async () => (await history()).length === 1, 'initial history saved');
  locker = new DatabaseSync(join(profile, 'era.sqlite'));
  locker.exec('PRAGMA busy_timeout=5000');
  // Force contention with another database writer instead of relying on chance.
  for (const trigger of ['toolbar', 'command', 'ctrl-r', 'f5']) {
    const before = await web.evaluate(() => window.documentId);
    locker.exec('BEGIN IMMEDIATE');
    try {
      if (trigger === 'toolbar') await ui.getByTitle(/^刷新/).click();
      else if (trigger === 'command') await send({ type: 'reload' });
      else await run('powershell.exe', ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-File',
        resolve('scripts/real-keys.ps1'), '-ProcessId', String(child.pid), '-Action', trigger], { windowsHide: true });
      await web.waitForFunction(id => window.documentId > id, before, { timeout: 3000 });
      const started = Date.now();
      await history();
      checks.push({ trigger, responsiveWhileDatabaseLockedMs: Date.now() - started });
    } finally { locker.exec('ROLLBACK'); }
    const expectedTitle = await web.title();
    await waitFor(async () => (await history()).some(e => e.title === expectedTitle), 'queued visit persisted');
  }
  for (let i = 0; i < 30; i++) {
    const before = await web.evaluate(() => window.documentId);
    await send({ type: 'reload' });
    await web.waitForFunction(id => window.documentId > id, before);
  }
  await history();
  checks.push('30 repeated reloads finish and host history queries still respond');
  const beforeRedirect = await web.evaluate(() => window.documentId);
  await send({ type: 'navigate', url: base + '/redirect' });
  await web.waitForFunction(id => window.documentId > id, beforeRedirect);
  await web.waitForURL(base + '/page');
  const redirectTitle = await web.title();
  await waitFor(async () => (await history()).some(e => e.title === redirectTitle), 'redirect visit persisted');
  const beforeClear = await web.evaluate(() => window.documentId);
  locker.exec('BEGIN IMMEDIATE');
  try {
    await send({ type: 'reload' });
    await web.waitForFunction(id => window.documentId > id, beforeClear);
    await history();
    await send({ type: 'clearBrowsingData', kind: 'history' });
    await history();
    assert.equal(await ui.evaluate(() => window.__reloadEvents.some(e => e.type === 'dataCleared')), false,
      'clear completion must wait for the database');
  } finally { locker.exec('ROLLBACK'); }
  await waitFor(() => ui.evaluate(() => window.__reloadEvents.some(e => e.type === 'dataCleared' && e.action === 'history')), 'history cleared');
  assert.deepEqual(await history(), [], 'queued visits must not reappear after clearing history');
  checks.push('clearing history remains responsive under lock and discards all earlier queued visits');
  const exited = once(child, 'exit');
  await send({ type: 'window', action: 'close' });
  await bounded(exited, 'graceful shutdown');
  const saved = new DatabaseSync(join(profile, 'era.sqlite'), { readOnly: true });
  assert.equal(saved.prepare('SELECT count(*) AS count FROM history').get().count, 0);
  saved.close();
  const result = { passed: true, profile, checks };
  await writeFile(join(output, 'result.json'), JSON.stringify(result, null, 2));
  console.log(JSON.stringify(result, null, 2));
} finally {
  locker?.close();
  if (child?.exitCode === null) child.kill();
  await pageBrowser?.close().catch(() => {});
  await uiBrowser?.close().catch(() => {});
  server.closeAllConnections();
  server.close();
}
