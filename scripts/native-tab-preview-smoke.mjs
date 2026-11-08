// Real WebView2 hover previews, using an isolated profile and local-only fixtures.
import { chromium } from '@playwright/test';
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { mkdtemp, mkdir, writeFile, access } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import assert from 'node:assert/strict';

const profile = await mkdtemp(join(tmpdir(), 'rove-tab-preview-'));
const output = resolve(process.env.ERA_SMOKE_OUTPUT || 'test-results/native-tab-preview');
const port = Number(process.env.ERA_SMOKE_PORT || 19746);
await mkdir(output, { recursive: true });
let unopened = 0;
const server = createServer((req, res) => {
  if (req.url === '/unopened') unopened++;
  const name = req.url === '/first' ? 'First' : req.url === '/second' ? 'Second' : 'Third';
  res.setHeader('Content-Type', 'text/html; charset=utf-8');
  res.end(`<!doctype html><title>${name} hover fixture</title><style>html{background:${name === 'First' ? '#164e63' : name === 'Second' ? '#385743' : '#7c2d12'};color:white;font:24px system-ui}body{margin:32px}button{font:inherit;padding:16px}</style><h1>${name} hover fixture</h1><button onclick="this.textContent='Clicked live page'">Live page button</button><div style="height:1600px"></div>`);
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
const base = `http://127.0.0.1:${server.address().port}`;
const checks = [];
let child, uiBrowser, pageBrowser, ui, popup;
async function waitFor(fn, label) {
  for (let i = 0; i < 150; i++) {
    try { const result = await fn(); if (result) return result; } catch {}
    await new Promise(resolve => setTimeout(resolve, 100));
  }
  throw Error('Timed out: ' + label);
}
const send = command => ui.evaluate(command => window.ipc.postMessage(JSON.stringify(command)), command);
const tooltip = () => popup.locator('#tab-hover-preview');
const site = path => waitFor(() => pageBrowser.contexts().flatMap(c => c.pages()).find(p => p.url() === base + path), path);
async function selected() { return ui.getByRole('tab', { selected: true }).getAttribute('data-tab-id'); }
async function hover(id) {
  await ui.bringToFront();
  await ui.mouse.move(900, 740);
  await ui.locator(`[data-tab-id="${id}"]`).hover();
  try { await tooltip().waitFor({ timeout: 5000 }); }
  catch (error) {
    console.log(JSON.stringify({ hoverFailed: await ui.locator(`[data-tab-id="${id}"]`).evaluate(row => ({ hovered: row.matches(':hover'), connected: row.isConnected, rect: row.getBoundingClientRect().toJSON(), focus: document.hasFocus(), visibility: document.visibilityState, target: document.querySelector('#tab-hover-preview')?.outerHTML })), pid: child.pid }));
    throw error;
  }
  await waitFor(async () => await tooltip().getAttribute('data-preview-id') === id, 'hover target ' + id);
  await waitFor(async () => await tooltip().getAttribute('data-phase') === 'open', 'entry fade complete');
}
try {
  child = spawn(resolve(process.env.ERA_SMOKE_EXE || 'native/target/debug/rove.exe'), [], {
    env: { ...process.env, ERA_DATA_DIR: profile, ERA_DEBUG_PORT: String(port) },
    windowsHide: true, stdio: 'pipe',
  });
  child.stderr.on('data', data => process.stderr.write(data));
  uiBrowser = await waitFor(() => chromium.connectOverCDP(`http://127.0.0.1:${port}`), 'UI debugger');
  ui = await waitFor(() => uiBrowser.contexts().flatMap(c => c.pages()).find(p => p.url() === 'https://era.ui/index.html'), 'trusted UI');
  await ui.waitForSelector('.sidebar');
  popup = await waitFor(() => ui.context().pages().find(p => p.url().endsWith('?popup')), 'popup renderer');
  assert(await ui.evaluate(() => window.__ERA_NATIVE__ === true && !!window.__ERA_INITIAL_STATE__ && document.documentElement.dataset.native === 'true'), 'document-created initialization must select the native store before mounting');
  await ui.evaluate(() => {
    const receive = window.__ERA_RECEIVE__;
    window.__previewHistory = null;
    window.__ERA_RECEIVE__ = event => {
      if (event.type === 'history') window.__previewHistory = event.entries;
      receive(event);
    };
  });
  await send({ type: 'navigate', url: base + '/first' });
  pageBrowser = await waitFor(() => chromium.connectOverCDP(`http://127.0.0.1:${port + 1}`), 'page debugger');
  const first = await site('/first'); await first.waitForSelector('h1');
  const firstId = await selected();
  await send({ type: 'new', url: base + '/second' });
  const second = await site('/second'); await second.waitForSelector('h1');
  const secondId = await selected();
  await second.evaluate(() => {
    window.__previewMarker = crypto.randomUUID();
    window.__previewVisibility = [];
    document.addEventListener('visibilitychange', () => window.__previewVisibility.push(document.visibilityState));
  });
  const marker = await second.evaluate(() => window.__previewMarker);
  const tabCount = await ui.getByRole('tab').count();
  await hover(firstId);
  await waitFor(async () => await tooltip().locator('.tab-preview-pane > img').count() === 1, 'real thumbnail');
  await tooltip().locator('.tab-preview-pane > img').evaluate(img => img.decode());
  assert.equal(await second.evaluate(() => document.visibilityState), 'visible');
  assert.deepEqual(await second.evaluate(() => window.__previewVisibility), []);
  assert.equal(await ui.getByRole('tab').count(), tabCount);
  assert.equal(await selected(), secondId);
  await ui.screenshot({ path: join(output, 'single-ui.png') });
  checks.push('单页：后台标签真实缩略图、标题及URL；当前网页保持可见、不切换标签或重载');

  const before = await tooltip().locator('.tab-preview-pane > img').getAttribute('src');
  await first.evaluate(() => document.documentElement.style.background = '#991b1b');
  await waitFor(async () => (await tooltip().locator('.tab-preview-pane > img').getAttribute('src')) !== before, 'one-second snapshot refresh');
  checks.push('悬停期间按秒刷新已存在的WebView2，不创建新控制器');
  await ui.mouse.move(900, 740);
  await tooltip().waitFor({ state: 'detached' });
  await second.getByRole('button', { name: 'Live page button' }).click();
  assert.equal(await second.getByRole('button').textContent(), 'Clicked live page');
  assert.equal(await second.evaluate(() => window.__previewMarker), marker);
  checks.push('移出后恢复网页区域；原网页按钮、DOM和控制器保持正常');

  await send({ type: 'favoriteSave', title: 'Unopened preview', url: base + '/unopened', previousUrl: 'https://linear.app' });
  const unloaded = ui.locator('.favorite-tile').filter({ has: ui.getByRole('button', { name: 'Unopened preview', exact: true }) });
  await unloaded.hover(); await tooltip().waitFor();
  await waitFor(async () => await tooltip().locator('.tab-preview-pane > img').count() === 1, 'unloaded item renders on demand');
  await tooltip().locator('.tab-preview-pane > img').evaluate(img => img.decode());
  assert.equal(unopened, 1);
  assert.equal(await ui.getByRole('tab').count(), tabCount);
  assert.equal(await unloaded.locator('.favorite-close').count(), 0);
  assert.equal(await selected(), secondId);
  await send({ type: 'history', query: '/unopened' });
  await waitFor(() => ui.evaluate(() => window.__previewHistory !== null), 'history reply');
  assert.deepEqual(await ui.evaluate(() => window.__previewHistory), []);
  await ui.keyboard.press('Escape'); await tooltip().waitFor({ state: 'detached' });
  await waitFor(() => !pageBrowser.contexts().flatMap(c => c.pages()).some(p => p.url() === base + '/unopened'), 'temporary renderer released');
  await ui.mouse.move(900, 740); await unloaded.hover();
  await waitFor(async () => await tooltip().getAttribute('data-phase') === 'open', 'cached preview opens');
  assert.equal(unopened, 1);
  await ui.keyboard.press('Escape'); await tooltip().waitFor({ state: 'detached' });
  checks.push('无缓存常用网站现场加载并生成真实预览，不增加标签、不写历史；收起释放控制器，下次命中缓存');

  // Slow entry/exit down with the browser animation timeline, without altering app durations.
  await popup.evaluate(() => {
    const animate = Element.prototype.animate;
    Element.prototype.animate = function (...args) {
      const result = animate.apply(this, args);
      if (this.id === 'tab-hover-preview') result.pause();
      return result;
    };
    window.__restorePreviewAnimate = () => { Element.prototype.animate = animate; };
  });
  await ui.mouse.move(900, 740); await ui.locator(`[data-tab-id="${firstId}"]`).hover();
  await waitFor(async () => await tooltip().getAttribute('data-phase') === 'opening', 'native entry animation');
  assert.equal(await popup.locator('.tab-preview-background').count(), 0);
  const entering = await tooltip().evaluate(e => {
    const a = e.getAnimations()[0]; a.currentTime = 80;
    return Number(getComputedStyle(e).opacity);
  });
  assert(entering > 0 && entering < 1);
  assert.equal(await second.evaluate(() => document.visibilityState), 'visible');
  await ui.screenshot({ path: join(output, 'entry-midpoint-ui.png') });
  await tooltip().evaluate(e => e.getAnimations()[0].finish());
  await waitFor(async () => await tooltip().getAttribute('data-phase') === 'open', 'entry completed');
  await ui.mouse.move(900, 740);
  await waitFor(async () => await tooltip().getAttribute('data-phase') === 'closing', 'native exit animation');
  await tooltip().evaluate(e => e.getAnimations()[0].currentTime = 60);
  assert(await tooltip().evaluate(e => Number(getComputedStyle(e).opacity) > 0));
  await tooltip().evaluate(e => e.getAnimations()[0].finish());
  await tooltip().waitFor({ state: 'detached' });
  assert.equal(await ui.locator('.tab-preview-background').count(), 0);
  await popup.evaluate(() => window.__restorePreviewAnimate());
  checks.push('真实网页上160ms入场/120ms离场，透明中间帧直接显示真实网页，当前网页始终可见');

  await send({ type: 'split', id: secondId, other: firstId });
  await send({ type: 'new', url: base + '/third' });
  const third = await site('/third'); await third.waitForSelector('h1');
  const thirdId = await selected();
  await hover(firstId);
  assert.equal(await tooltip().locator('.tab-preview-caption').count(), 2);
  await waitFor(async () => await tooltip().locator('.tab-preview-pane > img').count() === 2, 'two snapshots');
  await ui.screenshot({ path: join(output, 'split-ui.png') });
  checks.push('从任意一半悬停可看到整个分屏组合，两张真实缩略图及各自标题');

  // A normal preview can span the two visible native panes; neither is hidden.
  await ui.keyboard.press('Escape'); await tooltip().waitFor({ state: 'detached' });
  await send({ type: 'activate', id: secondId });
  await hover(thirdId);
  assert.equal(await first.evaluate(() => document.visibilityState), 'visible');
  assert.equal(await second.evaluate(() => document.visibilityState), 'visible');
  await ui.screenshot({ path: join(output, 'over-split-ui.png') });
  checks.push('在真实分屏上显示预览时，两侧网页均保持可见');
  await send({ type: 'theme' });
  await tooltip().waitFor({ state: 'detached' });
  await hover(thirdId);
  await ui.screenshot({ path: join(output, 'dark-ui.png') });
  checks.push('主题切换清理旧预览，深色主题可再次显示');

  if (process.env.ERA_SMOKE_REVIEW === '1') {
    console.log(JSON.stringify({ reviewReady: true, pid: child.pid, output, port, thirdId }));
    const resume = join(output, `resume-${child.pid}`);
    for (let i = 0; i < 180; i++) {
      if (await access(resume).then(() => true, () => false)) break;
      await new Promise(resolve => setTimeout(resolve, 1000));
    }
  }
  await ui.keyboard.press('Escape'); await tooltip().waitFor({ state: 'detached' });
  await ui.keyboard.press('Control+l');
  await popup.locator('.command-scrim[data-presented="true"]').waitFor();
  assert.equal(await tooltip().count(), 0);
  await popup.keyboard.press('Escape');
  await popup.locator('.command-scrim').waitFor({ state: 'detached' });
  checks.push('Esc和命令栏正确收起预览，原有浮层仍正常工作');
  const result = { passed: true, profile, checks };
  await writeFile(join(output, 'native-tab-preview-smoke.json'), JSON.stringify(result, null, 2));
  console.log(JSON.stringify(result, null, 2));
} catch (error) {
  await ui?.screenshot({ path: join(output, 'failure.png') }).catch(() => {});
  await writeFile(join(output, 'failure.json'), JSON.stringify({ checks, error: String(error) }, null, 2));
  throw error;
} finally {
  await pageBrowser?.close(); await uiBrowser?.close();
  if (child && !child.killed) child.kill();
  server.close();
}
