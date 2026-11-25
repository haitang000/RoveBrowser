// Use the Agent protocol for hidden tabs and debug CDP only for the user interface.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { randomBytes } from 'node:crypto';
import { once } from 'node:events';
import { mkdtemp } from 'node:fs/promises';
import { createServer } from 'node:http';
import { createServer as createTcpServer } from 'node:net';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { DatabaseSync } from 'node:sqlite';
import { setTimeout as delay } from 'node:timers/promises';
import { chromium } from '@playwright/test';
import { createLegacyNativeProfile } from './native-profile-fixture.mjs';
import { requestAgent } from './rove-agent.mjs';

const profile = await mkdtemp(join(tmpdir(), 'rove-native-agent-'));
createLegacyNativeProfile(profile);
const db = new DatabaseSync(join(profile, 'era.sqlite'));
const state = JSON.parse(db.prepare('SELECT json FROM session WHERE id=1').get().json);
state.favorites = [];
state.tabs = state.tabs.filter(tab => tab.id === 'start');
state.settings = { ...state.settings, askBeforeQuit: false, searchSuggestions: false };
db.prepare('UPDATE session SET json=? WHERE id=1').run(JSON.stringify(state));
db.close();

const hits = new Map();
let slowResponse;
const server = createServer((req, res) => {
  hits.set(req.url, req.headers.cookie ?? '');
  if (req.url === '/favicon.ico') { res.writeHead(204).end(); return; }
  if (req.url === '/redirect') { res.writeHead(302, { Location: '/landed' }).end(); return; }
  if (req.url === '/failed') { res.destroy(); return; }
  if (req.url === '/user') res.setHeader('Set-Cookie', 'user_session=private; Path=/');
  if (req.url === '/agent') res.setHeader('Set-Cookie', 'agent_session=separate; Path=/');
  res.setHeader('Content-Type', 'text/html; charset=utf-8');
  if (req.url === '/slow') { slowResponse = res; return; }
  if (req.url === '/controls/pagination') {
    res.end(`<!doctype html><title>Snapshot pagination</title>
      <span id="primary">Label</span><span id="secondary">  reference </span>
      <button aria-labelledby="primary missing secondary" aria-label="Ignored">Fallback</button>
      <button aria-label=" ">Visible <span>fallback</span></button>
      ${Array.from({ length: 98 }, (_, index) => `<button aria-label="Entry ${index + 3}">Button</button>`).join('')}
      <input type="password"><div role="heading">Unsupported role</div>
      <div aria-hidden="true"><button>Hidden by ancestor</button></div>
      <button disabled>Disabled</button><button hidden>Hidden</button>
      <button id="last" aria-label="Last entry">Final button</button><p id="result"></p>
      <script>document.querySelector('#last').onclick = () => {
        document.querySelector('#result').textContent = 'Clicked the last entry';
      };</script>`);
    return;
  }
  res.end(`<!doctype html><title>${req.url}</title><input id="typing">${req.url === '/agent' ? '<script>alert("hidden"); window.open("/popup");</script>' : ''}`);
});
await new Promise(done => server.listen(0, '127.0.0.1', done));
const base = `http://127.0.0.1:${server.address().port}`;
const reserve = createTcpServer();
await new Promise(done => reserve.listen(0, '127.0.0.1', done));
const port = reserve.address().port;
await new Promise(done => reserve.close(done));
const token = randomBytes(32).toString('hex');
const debug = Number(process.env.ERA_SMOKE_PORT || 19860);
const call = command => requestAgent(command, { port, token });
const run = async command => {
  const response = await call(command);
  assert.equal(response.status, 'ok', JSON.stringify(response));
  return response.result;
};
const list = async () => (await run({ op: 'list' })).tabs;
const failure = async (command, code) => assert.deepEqual(await call(command), { status: 'error', code });

async function waitFor(fn, description, retryErrors = false) {
  const deadline = Date.now() + 15_000;
  let lastError;
  while (Date.now() < deadline) {
    try {
      const result = await fn();
      if (result) return result;
    } catch (error) {
      if (!retryErrors) throw error;
      lastError = error;
    }
    await delay(80);
  }
  throw new Error(`Timed out: ${description}`, { cause: lastError });
}

let child, uiBrowser, pageBrowser, disabledBrowser;
const exe = resolve(process.env.ERA_SMOKE_EXE || 'native/target/debug/rove.exe');
try {
  child = spawn(exe, [], {
    env: {
      ...process.env, ERA_DATA_DIR: profile, ERA_DEBUG_PORT: String(debug),
      ROVE_AGENT_PORT: String(port), ROVE_AGENT_TOKEN: token,
      ROVE_AGENT_BROWSER_TABS: '1', ROVE_AGENT_PAGE_ACTIONS: '1',
    },
    windowsHide: true, stdio: ['ignore', 'ignore', 'inherit'],
  });
  const exited = once(child, 'exit');
  const connect = port => chromium.connectOverCDP(`http://127.0.0.1:${port}`, { timeout: 1500 });
  uiBrowser = await waitFor(() => connect(debug), 'UI debugger', true);
  const pages = browser => browser.contexts().flatMap(context => context.pages());
  const ui = await waitFor(() => pages(uiBrowser).find(page => page.url() === 'https://era.ui/index.html'), 'UI');
  await ui.waitForSelector('.sidebar', { timeout: 10_000 });
  const send = command => ui.evaluate(command => window.ipc.postMessage(JSON.stringify(command)), command);
  await ui.evaluate(() => {
    window.__agentSmokeState = window.__ERA_INITIAL_STATE__;
    const receive = window.__ERA_RECEIVE__;
    window.__ERA_RECEIVE__ = event => {
      if (event.type === 'state') window.__agentSmokeState = event.state;
      receive(event);
    };
  });
  await send({ type: 'navigate', url: base + '/user' });
  pageBrowser = await waitFor(() => connect(debug + 1), 'user page debugger', true);
  const web = await waitFor(() => pages(pageBrowser).find(page => page.url() === base + '/user'), 'user page');
  const input = web.locator('#typing');
  await input.fill('before');
  assert.equal(await web.evaluate(() => document.cookie), 'user_session=private');
  assert.deepEqual(await list(), []);
  assert.deepEqual(await requestAgent({ op: 'list' }, { port, token: randomBytes(32).toString('hex') }), { status: 'error', code: 'unauthorized' });
  await failure({ op: 'eval', script: '1' }, 'invalid_request');
  await failure({ op: 'open', url: 'file:///C:/secret' }, 'invalid_url');
  await failure({ op: 'navigate', id: 'start', url: base + '/agent' }, 'tab_not_found');
  await failure({ op: 'close', id: 'start' }, 'tab_not_found');

  const { id } = await run({ op: 'open', url: base + '/agent' });
  const ready = pathname => waitFor(async () => (await list()).find(tab => tab.id === id && tab.load === 'ready' && tab.url === base + pathname), pathname);
  assert.equal((await ready('/agent')).title, '/agent');
  assert.equal(hits.get('/agent'), '');
  assert.equal(hits.has('/popup'), false);

  await run({ op: 'navigate', id, url: base + '/slow' });
  await waitFor(() => slowResponse, 'pending Agent navigation');
  assert.deepEqual(await list(), [{ id, load: 'loading', url: null, title: null }]);
  assert.equal(await input.evaluate(element => element === document.activeElement), true);
  await web.keyboard.type(' while loading');
  assert.equal(await input.inputValue(), 'before while loading');
  slowResponse.end('<!doctype html><title>Slow Agent page</title>');
  assert.equal((await ready('/slow')).title, 'Slow Agent page');
  await run({ op: 'navigate', id, url: base + '/redirect' });
  assert.equal((await ready('/landed')).title, '/landed');
  assert.equal(hits.get('/landed'), 'agent_session=separate');
  await run({ op: 'navigate', id, url: base + '/landed#hash' });
  assert.equal((await ready('/landed#hash')).title, '/landed');
  await run({ op: 'navigate', id, url: base + '/landed#hash' });
  await ready('/landed#hash');
  await run({ op: 'navigate', id, url: base + '/failed' });
  assert.deepEqual(await waitFor(async () => (await list()).find(tab => tab.load === 'failed'), 'failed navigation'),
    { id, load: 'failed', url: null, title: null });
  await run({ op: 'navigate', id, url: base + '/landed' });
  await ready('/landed');
  assert.equal(await web.evaluate(() => document.cookie), 'user_session=private');
  assert.deepEqual(pages(pageBrowser).map(page => page.url()), [base + '/user']);
  const foreground = await ui.evaluate(() => ({ active: window.__agentSmokeState.activeTab, tabs: window.__agentSmokeState.tabs.map(tab => tab.id) }));
  assert.deepEqual(foreground, { active: 'start', tabs: ['start'] });
  await run({ op: 'close', id });
  await failure({ op: 'close', id }, 'tab_not_found');
  assert.deepEqual(await list(), []);
  assert.equal(await input.inputValue(), 'before while loading');

  const paginationUrl = base + '/controls/pagination';
  const { id: paginationTab } = await run({ op: 'browser_open', url: paginationUrl });
  const pagination = (op, fields = {}) => run({ op, id: paginationTab, ...fields });
  const first = await waitFor(async () => {
    const response = await call({ op: 'page_snapshot', id: paginationTab });
    return response.status === 'ok' && response.result.url === paginationUrl && response.result;
  }, 'pagination fixture');
  assert.equal(first.elements.length, 100);
  assert.deepEqual(first.elements.slice(0, 3).map(entry => entry.label), ['Label reference', 'Visible fallback', 'Entry 3']);
  assert.equal(first.elements.at(-1).label, 'Entry 100');
  assert.equal(first.nextOffset, 100);
  const last = await pagination('page_snapshot', { offset: first.nextOffset });
  assert.equal(last.nextOffset, null);
  assert.deepEqual(last.elements.map(({ element, label }) => [element, label]), [[1, 'Last entry']]);
  await failure({ op: 'page_click', id: paginationTab, snapshot: first.snapshot, element: 1 }, 'stale_snapshot');
  await pagination('page_click', { snapshot: last.snapshot, element: 1 });
  const pastEnd = await pagination('page_snapshot', { offset: 101 });
  assert.deepEqual(pastEnd.elements, []);
  assert.equal(pastEnd.nextOffset, null);
  assert.match(pastEnd.text, /Clicked the last entry/);
  await pagination('browser_close');

  // Leave tabs open: they must not restore with the user's session.
  for (let i = 0; i < 4; i++) await run({ op: 'open', url: base + '/agent' });
  await failure({ op: 'open', url: base + '/agent' }, 'tab_limit');
  await send({ type: 'window', action: 'close' }).catch(() => {});
  await Promise.race([exited, delay(7_000, null, { ref: false }).then(() => { throw new Error('Rove did not exit'); })]);
  const saved = new DatabaseSync(join(profile, 'era.sqlite'), { readOnly: true });
  try {
    const session = JSON.parse(saved.prepare('SELECT json FROM session WHERE id=1').get().json);
    assert.equal(session.activeTab, 'start');
    assert.deepEqual(session.tabs.map(tab => tab.url), [base + '/user']);
    assert.equal(saved.prepare('SELECT count(*) AS count FROM history WHERE url LIKE ? AND url NOT IN (?, ?)')
      .get(base + '/%', base + '/user', paginationUrl).count, 0);
  } finally { saved.close(); }
  const env = { ...process.env, ERA_DATA_DIR: profile, ERA_DEBUG_PORT: String(debug + 2) };
  delete env.ROVE_AGENT_PORT;
  delete env.ROVE_AGENT_TOKEN;
  child = spawn(exe, [], { env, windowsHide: true, stdio: ['ignore', 'ignore', 'inherit'] });
  const disabledExit = once(child, 'exit');
  disabledBrowser = await waitFor(() => connect(debug + 2), 'disabled UI debugger', true);
  const disabledUi = await waitFor(() => pages(disabledBrowser).find(page => page.url() === 'https://era.ui/index.html'), 'disabled UI');
  await disabledUi.waitForSelector('.sidebar', { timeout: 10_000 });
  await assert.rejects(call({ op: 'list' }), { code: 'ECONNREFUSED' });
  await disabledUi.evaluate(() => window.ipc.postMessage(JSON.stringify({ type: 'window', action: 'close' }))).catch(() => {});
  await Promise.race([disabledExit, delay(7_000, null, { ref: false }).then(() => { throw new Error('Rove did not exit'); })]);
  console.log('Agent opt-in, navigation, foreground input, cookies, popup blocking, and session isolation passed.');
} finally {
  if (child?.exitCode === null) child.kill();
  await pageBrowser?.close().catch(() => {});
  await uiBrowser?.close().catch(() => {});
  await disabledBrowser?.close().catch(() => {});
  server.closeAllConnections();
  server.close();
}
