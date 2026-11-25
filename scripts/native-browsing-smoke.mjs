// Exercise normal link navigation, SPA updates and tab changes in real WebView2.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { createServer } from 'node:http';
import { mkdtemp, mkdir, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { DatabaseSync } from 'node:sqlite';
import { chromium } from '@playwright/test';
import { createLegacyNativeProfile } from './native-profile-fixture.mjs';

const profile = await mkdtemp(join(tmpdir(), 'rove-browsing-'));
createLegacyNativeProfile(profile);
const db = new DatabaseSync(join(profile, 'era.sqlite'));
const state = JSON.parse(db.prepare('SELECT json FROM session WHERE id=1').get().json);
state.favorites = [];
state.tabs = state.tabs.filter(tab => tab.id === 'start');
state.settings = { ...state.settings, askBeforeQuit: false, searchSuggestions: false };
db.prepare('UPDATE session SET json=? WHERE id=1').run(JSON.stringify(state));
db.close();
const output = resolve(process.env.ERA_SMOKE_OUTPUT || 'test-results/native-browsing');
await mkdir(output, { recursive: true });
const port = Number(process.env.ERA_SMOKE_PORT || 19984);
const rounds = Number(process.env.ERA_BROWSING_ROUNDS || 100);
const checks = [];
const samples = [];
const server = createServer((req, res) => {
  if (req.url === '/favicon.ico') { res.writeHead(204); res.end(); return; }
  const index = Number(new URL(req.url, 'http://localhost').pathname.split('/').pop()) || 0;
  res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8', 'Cache-Control': 'no-store' });
  res.end(`<!doctype html><title>Browsing fixture ${index}</title>
    <style>body{background:#164e63;color:white;font:24px system-ui;padding:32px}a{color:white}</style>
    <h1>Browsing fixture ${index}</h1><a id="next" href="/page/${index + 1}">Next article</a>
    <button id="route">Change route</button><div style="height:1800px"></div>
    <script>window.documentId=${index};window.marker=crypto.randomUUID();
    document.querySelector('#route').onclick=()=>{
      history.pushState({},'', '/route/'+(++window.documentId));
      document.title='Route fixture '+window.documentId;
    };</script>`);
});
await new Promise(done => server.listen(0, '127.0.0.1', done));
const base = `http://127.0.0.1:${server.address().port}`;
let child, uiBrowser, pageBrowser, ui;
async function bounded(promise, label, timeout = 5000) {
  let timer;
  try {
    return await Promise.race([promise, new Promise((_, reject) => {
      timer = setTimeout(() => reject(Error(`Timed out: ${label}`)), timeout);
    })]);
  } finally { clearTimeout(timer); }
}
async function waitFor(fn, label) {
  const end = Date.now() + 15000;
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
  child.on('error', error => console.error(error));
  uiBrowser = await waitFor(() => chromium.connectOverCDP(`http://127.0.0.1:${port}`), 'shell debugger');
  ui = await waitFor(() => uiBrowser.contexts().flatMap(c => c.pages()).find(p => p.url() === 'https://era.ui/index.html'), 'shell');
  ui.setDefaultTimeout(5000);
  await ui.waitForSelector('.sidebar');
  const send = command => bounded(ui.evaluate(command => window.ipc.postMessage(JSON.stringify(command)), command), 'send command');
  await ui.evaluate(() => {
    window.__browsingState = window.__ERA_INITIAL_STATE__;
    window.__browsingHistory = 0;
    const receive = window.__ERA_RECEIVE__;
    window.__ERA_RECEIVE__ = event => {
      if (event.type === 'state') window.__browsingState = event.state;
      if (event.type === 'history') window.__browsingHistory++;
      receive(event);
    };
  });
  const heartbeat = async () => {
    const before = await ui.evaluate(() => window.__browsingHistory);
    const started = performance.now();
    await send({ type: 'history', query: 'fixture' });
    await ui.waitForFunction(before => window.__browsingHistory > before, before, { timeout: 1500 });
    samples.push(Math.round(performance.now() - started));
  };
  await send({ type: 'navigate', url: base + '/page/0' });
  pageBrowser = await waitFor(() => chromium.connectOverCDP(`http://127.0.0.1:${port + 1}`), 'page debugger');
  const web = await waitFor(() => pageBrowser.contexts().flatMap(c => c.pages()).find(p => p.url() === base + '/page/0'), 'fixture');
  web.setDefaultTimeout(5000);
  await web.waitForSelector('#next');
  for (let index = 1; index <= rounds; index++) {
    await web.locator('#next').click();
    await web.waitForFunction(index => window.documentId === index, index, { timeout: 5000 });
    await heartbeat();
    if (index % 20 === 0) console.log(`Navigated ${index}/${rounds} articles`);
  }
  checks.push(`${rounds} ordinary link navigations keep the host responsive`);
  for (let index = 1; index <= 30; index++) {
    await web.locator('#route').click();
    await ui.waitForFunction(url => window.__browsingState.tabs.some(tab => tab.url === url), base + '/route/' + (rounds + index));
    await heartbeat();
  }
  checks.push('30 SPA route/title changes keep the host responsive');
  const marker = await web.evaluate(() => window.marker);
  const first = await ui.evaluate(() => window.__browsingState.activeTab);
  for (let index = 0; index < 20; index++) {
    await send({ type: 'new', url: base + '/tab/' + index });
    await ui.waitForFunction(url => window.__browsingState.tabs.some(tab => tab.url === url && !tab.loading), base + '/tab/' + index);
    await heartbeat();
    await send({ type: 'close' });
    await ui.waitForFunction(id => window.__browsingState.activeTab === id, first);
  }
  assert.equal(await web.evaluate(() => window.marker), marker);
  checks.push('20 tab open/close cycles preserve the original page');
  const exited = once(child, 'exit');
  await send({ type: 'window', action: 'close' });
  await bounded(exited, 'graceful shutdown');
  const saved = new DatabaseSync(join(profile, 'era.sqlite'), { readOnly: true });
  assert.equal(JSON.parse(saved.prepare('SELECT json FROM session WHERE id=1').get().json).activeTab, first);
  assert(saved.prepare('SELECT count(*) AS count FROM history').get().count >= rounds);
  saved.close();
  checks.push('history and the final active tab are saved on shutdown');
  const result = { passed: true, profile, checks, maxHostResponseMs: Math.max(...samples) };
  await writeFile(join(output, 'result.json'), JSON.stringify(result, null, 2));
  console.log(JSON.stringify(result, null, 2));
} catch (error) {
  await writeFile(join(output, 'failure.json'), JSON.stringify({ profile, pid: child?.pid, checks, error: String(error) }, null, 2));
  throw error;
} finally {
  if (child?.exitCode === null) child.kill();
  await pageBrowser?.close().catch(() => {});
  await uiBrowser?.close().catch(() => {});
  server.closeAllConnections();
  server.close();
}
