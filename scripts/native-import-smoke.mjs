// Real import and credential-use integration; all browser sources are synthetic.
import { chromium } from '@playwright/test';
import { spawn } from 'node:child_process';
import { mkdtemp, mkdir, writeFile, readFile, unlink } from 'node:fs/promises';
import { createServer } from 'node:http';
import { DatabaseSync } from 'node:sqlite';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { once } from 'node:events';
import assert from 'node:assert/strict';

const profile = await mkdtemp(join(tmpdir(), 'rove-import-smoke-'));
const fixtures = join(profile, 'sources');
const output = resolve('test-results/native-import');
await mkdir(output, { recursive: true });
const checks = [];
const server = createServer((req, res) => {
  res.setHeader('Content-Type', 'text/html; charset=utf-8');
  const action = req.url === '/cross-action' ? ' action="https://example.org/login"' : '';
  const autocomplete = req.url === '/new' ? 'new-password' : 'current-password';
  res.end(`<!doctype html><title>Import login fixture</title><h1>Login</h1>
    <form${action} onsubmit="event.preventDefault();window.submitted=true">
    <label>User<input id="user" autocomplete="username" value="different-user"></label>
    <label>Password<input id="pass" type="password" autocomplete="${autocomplete}"></label>
    <button>Submit</button></form>`);
});
await new Promise(done => server.listen(0, '127.0.0.1', done));
const site = `http://127.0.0.1:${server.address().port}`;
const csvSecret = 'native-fixture-only-secret';
const edge = join(fixtures, 'Local/Microsoft/Edge/User Data/Default');
const chrome = join(fixtures, 'Local/Google/Chrome/User Data/Profile 1');
const firefox = join(fixtures, 'Roaming/Mozilla/Firefox/Profiles/test.default');
await Promise.all([edge, chrome, firefox].map(path => mkdir(path, { recursive: true })));
const bookmarkJson = url => JSON.stringify({ roots: { bookmark_bar: { name: 'Toolbar', children: [{ name: 'Imported docs', url }] } } });
await writeFile(join(edge, 'Bookmarks'), bookmarkJson('https://edge.example/docs'));
await writeFile(join(chrome, 'Bookmarks'), bookmarkJson('https://chrome.example/docs'));
await writeFile(join(chrome, 'History'), 'broken fixture database');
function historyAt(path, url) {
  const db = new DatabaseSync(path);
  db.exec('CREATE TABLE urls(url TEXT,title TEXT,last_visit_time INTEGER)');
  db.prepare('INSERT INTO urls VALUES(?,?,?)').run(url, 'Imported history', (1700000000 + 11644473600) * 1000000);
  db.close();
}
historyAt(join(edge, 'History'), 'https://edge.example/history');
const ff = new DatabaseSync(join(firefox, 'places.sqlite'));
ff.exec(`CREATE TABLE moz_places(id INTEGER,url TEXT,title TEXT,last_visit_date INTEGER);
  CREATE TABLE moz_bookmarks(id INTEGER,parent INTEGER,title TEXT,fk INTEGER,type INTEGER,position INTEGER);
  INSERT INTO moz_places VALUES(1,'https://firefox.example/','Firefox',1700000000000000);
  INSERT INTO moz_bookmarks VALUES(1,0,'Toolbar',NULL,2,0),(2,1,'Firefox bookmark',1,1,0);`);
ff.close();
await writeFile(join(fixtures, 'passwords.csv'), `name,url,username,password\nFixture,${site}/login,fixture-user,${csvSecret}\n`);

let child, browser, pageBrowser, ui, port = 19860;
async function waitFor(fn, label) {
  const end = Date.now() + 25000;
  while (Date.now() < end) {
    try { const result = await fn(); if (result) return result; } catch {}
    await new Promise(done => setTimeout(done, 100));
  }
  throw Error(`Timed out: ${label}`);
}
async function launch() {
  port += 4;
  child = spawn(resolve(process.env.ERA_TEST_EXE || 'native/target/debug/rove.exe'), [], {
    env: { ...process.env, ERA_DATA_DIR: profile, ERA_IMPORT_FIXTURES: fixtures, ERA_DEBUG_PORT: String(port) },
    windowsHide: true, stdio: 'pipe',
  });
  child.stderr.on('data', data => process.stderr.write(data));
  browser = await waitFor(() => chromium.connectOverCDP(`http://127.0.0.1:${port}`), 'UI debugger');
  ui = await waitFor(() => browser.contexts().flatMap(c => c.pages()).find(p => p.url() === 'https://era.ui/index.html'), 'UI page');
  await ui.locator('.onboarding, .sidebar').first().waitFor();
  await ui.evaluate(() => {
    window.__importTest = { state: window.__ERA_INITIAL_STATE__, busy: false, sources: [], errors: [] };
    const receive = window.__ERA_RECEIVE__;
    window.__ERA_RECEIVE__ = event => {
      if (event.state) window.__importTest.state = event.state;
      if (event.type === 'importState') {
        window.__importTest.busy = event.importBusy;
        if (event.importSources) window.__importTest.sources = event.importSources;
      }
      if (event.type === 'error') window.__importTest.errors.push(event.message);
      if (event.type === 'history') window.__importTest.history = event.entries;
      receive(event);
    };
  });
}
async function send(command) { await ui.evaluate(c => window.ipc.postMessage(JSON.stringify(c)), command); }
async function state() { return ui.evaluate(() => window.__importTest.state); }
async function report(id) { return (await state()).onboarding.imports.find(r => r.id === id); }
async function idle() { await waitFor(() => ui.evaluate(() => !window.__importTest.busy), 'import completion'); }
async function scan() {
  await send({ type: 'importScan' });
  return waitFor(async () => {
    const sources = await ui.evaluate(() => window.__importTest.sources);
    return sources.length === 3 && sources;
  }, 'fixture sources');
}
async function stop() {
  if (child?.exitCode === null) { const exited = once(child, 'exit'); child.kill(); await exited; }
  await pageBrowser?.close().catch(() => {}); await browser?.close().catch(() => {});
  browser = pageBrowser = undefined;
}
function database(fn) {
  const db = new DatabaseSync(join(profile, 'era.sqlite'), { readOnly: true });
  try { return fn(db); } finally { db.close(); }
}
async function navigate(path, host = '127.0.0.1') {
  const url = `${site.replace('127.0.0.1', host)}${path}`;
  await send({ type: 'navigate', url });
  const page = await waitFor(async () => {
    const found = pageBrowser?.contexts().flatMap(c => c.pages()).find(p => p.url() === url);
    if (found) return found;
    pageBrowser = await chromium.connectOverCDP(`http://127.0.0.1:${port + 1}`);
  }, 'login page');
  await page.locator('#pass').waitFor();
  await waitFor(async () => (await state()).tabs.some(t => t.url === url && !t.loading), 'page finished');
  return page;
}

try {
  await launch();
  await ui.getByRole('button', { name: '开始使用', exact: true }).click();
  await idle();
  const sources = await scan();
  const edgeId = sources.find(s => s.browser === 'Microsoft Edge').id;
  const chromeId = sources.find(s => s.browser === 'Google Chrome').id;
  const firefoxId = sources.find(s => s.browser === 'Firefox').id;
  assert(!JSON.stringify(sources).includes(fixtures));
  for (const source of sources) await ui.getByRole('checkbox', { name: new RegExp(source.name.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')) }).uncheck();
  await ui.getByRole('checkbox', { name: /Microsoft Edge/ }).check();
  await ui.getByRole('checkbox', { name: '历史记录', exact: true }).uncheck();
  await ui.getByRole('button', { name: '导入所选数据', exact: true }).click();
  await waitFor(async () => (await report(edgeId))?.bookmarks === 1, 'bookmark import');
  assert.equal(database(db => db.prepare('SELECT COUNT(*) AS n FROM history').get().n), 0);
  checks.push('Detected isolated Edge, Chrome and Firefox profiles; UI type selection imports real bookmarks only');

  await ui.getByRole('checkbox', { name: '历史记录', exact: true }).check();
  await ui.getByRole('button', { name: '导入所选数据', exact: true }).click();
  await waitFor(async () => (await report(edgeId))?.history === 1, 'history import');
  await idle();
  await send({ type: 'importRun', request: { sources: [chromeId, firefoxId], bookmarks: true, history: true } });
  await waitFor(async () => (await report(chromeId))?.problems.length && (await report(firefoxId))?.history === 1, 'partial failure');
  await idle();
  assert.equal((await state()).pins.length, 3);
  await unlink(join(chrome, 'History')); historyAt(join(chrome, 'History'), 'https://chrome.example/history');
  await ui.getByRole('button', { name: '重试这个来源', exact: true }).click();
  await waitFor(async () => (await report(chromeId))?.history === 1 && !(await report(chromeId)).problems.length, 'retry repaired source');
  await idle();
  assert.equal((await state()).pins.length, 3);
  assert.equal(database(db => db.prepare('SELECT COUNT(*) AS n FROM history').get().n), 3);
  assert.equal(database(db => db.prepare('SELECT visited FROM history WHERE url=?').get('https://edge.example/history').visited), 1700000000);
  checks.push('History timestamps preserved; corrupt source reports partial failure; retry deduplicates bookmarks');

  await ui.getByRole('button', { name: '导入密码文件…', exact: true }).click();
  await waitFor(async () => (await report('password-file'))?.passwords === 1, 'password import'); await idle();
  await ui.getByRole('button', { name: '导入密码文件…', exact: true }).click();
  await waitFor(async () => (await report('password-file'))?.skipped === 1, 'password duplicate'); await idle();
  const secret = database(db => db.prepare('SELECT secret FROM imported_logins').get().secret);
  assert(!Buffer.from(secret).includes(Buffer.from(csvSecret)));
  assert(!JSON.stringify(await state()).includes(csvSecret));
  await send({ type: 'importRun', request: { sources: ['../../arbitrary'], bookmarks: true, history: true } });
  await waitFor(() => ui.evaluate(() => window.__importTest.errors.length > 0), 'invalid source rejected');
  checks.push('CSV passwords stored as DPAPI ciphertext; repeat import and arbitrary source IDs rejected');
  await ui.screenshot({ path: join(output, 'import-results.png') });
  await stop(); await launch();
  assert.equal((await state()).onboarding.step, 'bring');
  assert.equal((await state()).pins.length, 3); assert.equal((await state()).onboarding.imports.length, 4);
  await ui.getByRole('button', { name: '跳过', exact: true }).click();
  await ui.getByRole('button', { name: '跳过', exact: true }).click();
  await ui.getByRole('button', { name: '跳过', exact: true }).click();
  await ui.getByRole('button', { name: '暂时不用', exact: true }).click();
  await ui.getByRole('button', { name: '开始浏览', exact: true }).click();
  await ui.locator('.sidebar').waitFor();
  await send({ type: 'space', id: `import-${edgeId}` });
  await ui.locator('.space-name').filter({ hasText: '导入 · Microsoft Edge' }).waitFor();
  await ui.getByRole('tab', { name: /Toolbar \/ Imported docs/ }).waitFor();
  await send({ type: 'history', query: 'Imported history' });
  await waitFor(() => ui.evaluate(() => window.__importTest.history?.length === 2), 'imported history searchable');
  checks.push('Imported workspace pins and history search are available in the browsing interface');
  let login = await navigate('/login');
  const fill = login.getByRole('button', { name: 'Rove · 填写登录信息', exact: true });
  await fill.waitFor();
  assert.equal(await login.locator('#pass').inputValue(), '');
  await fill.evaluate(button => button.click());
  assert.equal(await login.getByRole('button', { name: 'fixture-user', exact: true }).isVisible(), false);
  await login.locator('#pass').fill('previously-entered-fixture');
  await fill.click(); await login.getByRole('button', { name: 'fixture-user', exact: true }).click();
  assert.equal(await login.locator('#user').inputValue(), 'fixture-user');
  assert.equal(await login.locator('#pass').inputValue(), csvSecret);
  assert.equal(await login.evaluate(() => !!window.submitted), false);
  checks.push('After restart, user gesture selects and fills the matching account; no automatic submission');

  for (const [path, host] of [['/new','127.0.0.1'], ['/cross-action','127.0.0.1'], ['/login','localhost']]) {
    login = await navigate(path, host);
    assert.equal(await login.locator('#__rove_imported_login').count(), 0);
    assert.equal(await login.locator('#pass').inputValue(), '');
  }
  login = await navigate('/login');
  await login.getByRole('button', { name: 'Rove · 填写登录信息', exact: true }).waitFor();
  const settings = { ...(await state()).settings, autofill: false };
  await send({ type: 'settingsUpdate', settings });
  await login.locator('#__rove_imported_login').waitFor({ state: 'detached' });
  login = await navigate('/disabled');
  assert.equal(await login.locator('#__rove_imported_login').count(), 0);
  checks.push('New-password forms, cross-origin actions and other origins excluded; disabling autofill removes existing picker');
  assert.equal(await readFile(join(edge, 'Bookmarks'), 'utf8'), bookmarkJson('https://edge.example/docs'));
  await writeFile(join(output, 'checks.json'), JSON.stringify({ profile, checks }, null, 2));
  console.log(JSON.stringify({ profile, checks }, null, 2));
} finally { await stop(); await new Promise(done => server.close(done)); }
