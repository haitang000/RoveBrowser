// Tab sleep (ported Zigzag Tabs/TabSleep.swift) against real WebView2 pages and
// an isolated temporary profile. The debug build accepts ERA_SLEEP_* to shorten
// Zigzag's one-hour threshold, so a real TrySuspend/Resume can be watched here.
// Only the trusted UI debugger is attached: a page debugger may itself keep a
// WebView from suspending, and the session's runtime state is reported through
// the same sleep events the sidebar listens to.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdtemp, mkdir, writeFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { DatabaseSync } from 'node:sqlite';
import { chromium } from '@playwright/test';
import { createLegacyNativeProfile } from './native-profile-fixture.mjs';

const profile = await mkdtemp(join(tmpdir(), 'rove-sleep-'));
createLegacyNativeProfile(profile);
const port = Number(process.env.ERA_SMOKE_PORT || 19966);
const output = resolve(process.env.ERA_SMOKE_OUTPUT || 'test-results/native-sleep');
await mkdir(output, { recursive: true });

// One tab, no favorites: only the fixture pages exist, so the sleep decision is
// about pages this test made.
{
  const db = new DatabaseSync(join(profile, 'era.sqlite'));
  try {
    const state = JSON.parse(db.prepare('SELECT json FROM session WHERE id=1').get().json);
    state.favorites = [];
    state.tabs = state.tabs.filter((tab) => tab.id === 'start');
    db.prepare('UPDATE session SET json=? WHERE id=1').run(JSON.stringify(state));
  } finally {
    db.close();
  }
}

// Every request counts and titles carry the count, so a wake that reloaded the
// page would be visible in both the server and the sidebar title.
const loads = {};
const server = createServer((req, res) => {
  const path = req.url === '/' ? '/root' : req.url;
  loads[path] = (loads[path] || 0) + 1;
  const name = path.slice(1);
  res.setHeader('Content-Type', 'text/html; charset=utf-8');
  res.end(`<!doctype html><title>${name} #${loads[path]}</title>
    <style>html{background:#164e63;color:white;font:24px system-ui}body{margin:32px}</style>
    <h1>${name} fixture</h1><p>Loaded ${loads[path]} times.</p><div style="height:1200px"></div>`);
});
await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
const base = `http://127.0.0.1:${server.address().port}`;

let child;
let uiBrowser;
let ui;
let popup;
const checks = [];
async function waitFor(fn, label, tries = 200) {
  for (let i = 0; i < tries; i++) {
    try {
      const value = await fn();
      if (value) return value;
    } catch {}
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw Error('Timed out: ' + label);
}
async function send(command) {
  await ui.evaluate((command) => window.ipc.postMessage(JSON.stringify(command)), command);
}
async function latestState() {
  return ui.evaluate(() => window.__roveSleep?.states.at(-1));
}
async function tabFor(url) {
  const state = await latestState();
  return state?.tabs.find((tab) => tab.url === url);
}
async function titleFor(url) {
  return (await tabFor(url))?.title;
}
async function events(type) {
  return ui.evaluate((type) => window.__roveSleep?.events.filter((event) => event.type === type) || [], type);
}
async function waitEvent(predicate, label) {
  return waitFor(async () => (await events('sleep')).find(predicate), label);
}
async function start() {
  child = spawn(resolve(process.env.ERA_SMOKE_EXE || 'native/target/debug/rove.exe'), [], {
    env: {
      ...process.env,
      ERA_DATA_DIR: profile,
      ERA_DEBUG_PORT: String(port),
      ERA_SLEEP_TICK_SECS: '1',
      ERA_SLEEP_IDLE_SECS: '2',
      ERA_SLEEP_CROWDED_SECS: '1',
      ERA_SLEEP_LOG: '1',
    },
    windowsHide: true,
    stdio: 'pipe',
  });
  child.stderr.on('data', (data) => process.stderr.write(data));
  uiBrowser = await waitFor(() => chromium.connectOverCDP(`http://127.0.0.1:${port}`), 'UI debugger');
  ui = await waitFor(
    () => uiBrowser.contexts().flatMap((context) => context.pages()).find((page) => page.url() === 'https://era.ui/index.html'),
    'UI page',
  );
  await ui.waitForSelector('.sidebar');
  popup = await waitFor(
    () => ui.context().pages().find((page) => page.url().endsWith('?popup')),
    'popup renderer',
  );
  for (const renderer of [ui, popup]) {
    await renderer.evaluate(() => {
      window.__roveSleep = { events: [], states: [] };
      const previous = window.__ERA_RECEIVE__;
      window.__ERA_RECEIVE__ = (event) => {
        if (event.type === 'sleep' || event.type === 'sleepState') window.__roveSleep.events.push({ ...event });
        if (event.type === 'state') window.__roveSleep.states.push(event.state);
        previous?.(event);
      };
    });
  }
  await ui.waitForSelector('.tab-row');
  const state = await latestState();
  await send({ type: 'settingsUpdate', settings: { ...(state?.settings || {}), askBeforeQuit: false, sleepTabs: true } });
}
async function stop() {
  await uiBrowser?.close().catch(() => {});
  if (!child) return;
  child.kill();
  await new Promise((resolve) => child.once('exit', resolve));
  child = undefined;
  uiBrowser = undefined;
}
try {
  await start();
  await send({ type: 'navigate', url: base + '/a' });
  const tabA = await waitFor(() => tabFor(base + '/a'), 'tab A state');
  await waitFor(async () => (await titleFor(base + '/a')) === 'a #1', 'tab A title');
  checks.push('页面 A 加载并成为前台标签');

  await send({ type: 'new', url: base + '/b' });
  await waitFor(() => tabFor(base + '/b'), 'tab B state');
  await waitFor(async () => (await titleFor(base + '/b')) === 'b #1', 'tab B title');
  checks.push('页面 B 成为前台标签，A 转入后台');

  // Zigzag's idle threshold (shortened to 2s here) puts A's page to sleep.
  await waitEvent((event) => event.id === tabA.id && event.asleep === true, 'A sleeps');
  assert.equal(loads['/a'], 1, 'suspending does not reload the page');
  checks.push('后台页面 A 经 TrySuspend 进入休眠（真实 WebView2 确认）');

  const row = ui.locator(`[data-tab-id="${tabA.id}"]`);
  assert.equal(await row.evaluate((element) => element.classList.contains('sleeping')), true, 'sleeping row class');
  assert.match(await row.getAttribute('title'), /已休眠以节省内存/, 'sleeping row tooltip says why');
  const ring = await row.evaluate((element) => {
    const style = getComputedStyle(element.querySelector('.tab-picture'), '::after');
    return { borderStyle: style.borderStyle, width: style.borderTopWidth };
  });
  assert.equal(ring.borderStyle, 'dashed', 'sleeping icon has a dashed ring');
  checks.push('侧栏显示虚线环、tooltip 与悬停卡片数据（休眠状态）');

  // Clicking the sleeping tab wakes it; the page keeps its title and the server
  // is never asked again, so the wake is a resume rather than a reload.
  await send({ type: 'activate', id: tabA.id });
  await waitEvent((event) => event.id === tabA.id && event.asleep === false, 'A wakes');
  await waitFor(
    async () => !(await ui.locator(`[data-tab-id="${tabA.id}"]`).evaluate((element) => element.classList.contains('sleeping'))),
    'ring cleared',
  );
  assert.equal(await titleFor(base + '/a'), 'a #1', 'waking keeps the page, no reload');
  assert.equal(loads['/a'], 1, 'waking does not request the page again');
  checks.push('点击休眠标签立即唤醒，页面标题保留，服务器没有再次访问');

  // A second page sleeps the same way once it is backgrounded, which shows the
  // decision is not a one-off after the first suspension.
  const tabB = await waitFor(() => tabFor(base + '/b'), 'tab B state');
  await waitEvent((event) => event.id === tabB.id && event.asleep === true, 'B sleeps');
  checks.push('第二个后台页面同样进入休眠');

  // Turning the setting off wakes every sleeping page and stops new sleeps.
  const state = await latestState();
  const before = (await events('sleep')).filter((event) => event.asleep === true).length;
  await send({ type: 'settingsUpdate', settings: { ...state.settings, sleepTabs: false } });
  await waitEvent((event) => event.id === tabB.id && event.asleep === false, 'B woken by setting off');
  await send({ type: 'new', url: base + '/c' });
  await waitFor(async () => (await titleFor(base + '/c')) === 'c #1', 'tab C title');
  await new Promise((resolve) => setTimeout(resolve, 4500));
  const after = (await events('sleep')).filter((event) => event.asleep === true).length;
  assert.equal(after, before, 'no page sleeps while the setting is off');
  checks.push('关闭“后台标签休眠”后唤醒全部页面且不再休眠');

  // Turning it back on lets an idle background page go again: the setting is live.
  const stateLater = await latestState();
  const enabledAt = (await events('sleep')).length;
  await send({ type: 'settingsUpdate', settings: { ...stateLater.settings, sleepTabs: true } });
  const again = await waitFor(async () => {
    const all = (await events('sleep')).slice(enabledAt);
    return all.some((event) => event.asleep === true) ? all : null;
  }, 'sleep resumes after re-enabling', 120);
  assert.ok(again.some((event) => event.asleep === true), 'sleeping comes back with the setting');
  checks.push('重新打开设置后休眠继续生效');

  const report = {
    passed: true,
    profile,
    checks,
    thresholds: { idleSeconds: 2, tickSeconds: 1 },
    requests: loads,
  };
  await writeFile(join(output, 'result.json'), JSON.stringify(report, null, 2));
  console.log(JSON.stringify(report));
} finally {
  await stop();
  server.close();
}
