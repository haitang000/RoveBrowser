// Real WebView2 Reader integration: isolated extraction, independent rendering,
// script sanitization, original DOM preservation, preferences, find and splits.
import { createLegacyNativeProfile } from './native-profile-fixture.mjs';
import { chromium } from '@playwright/test';
import { spawn, execFile } from 'node:child_process';
import { createServer } from 'node:http';
import { mkdtemp, mkdir, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import assert from 'node:assert/strict';
import { promisify } from 'node:util';

const profile = await mkdtemp(join(tmpdir(), 'rove-reader-'));
createLegacyNativeProfile(profile);
const output = resolve(process.env.ERA_SMOKE_OUTPUT || 'test-results/native-reader');
await mkdir(output, { recursive: true });
const port = Number(process.env.ERA_SMOKE_PORT || 19846);
let requests = 0;
const execFileAsync = promisify(execFile);
const text = '阅读模式保留文章的段落与图片，并去除导航与广告。This is a substantive article paragraph about reading, software and the web. A reader can focus on the words and follow the story without distraction. ';
const server = createServer((req, res) => {
  res.setHeader('Content-Type', 'text/html; charset=utf-8');
  if (req.url === '/empty') { res.end('<!doctype html><title>Empty page</title><nav>Menu only</nav>'); return; }
  if (req.url === '/image.svg') { res.setHeader('Content-Type', 'image/svg+xml'); res.end('<svg xmlns="http://www.w3.org/2000/svg" width="600" height="100"><rect width="600" height="100" fill="#ccc"/></svg>'); return; }
  requests++;
  const html = `<!doctype html><html lang="en"><head><title>Reader fixture - Fixture</title><meta property="og:site_name" content="Fixture"><meta name="author" content="Alice"><meta property="article:published_time" content="2024-03-03T10:00:00-05:00"></head><body><nav>Navigation poison-nav</nav><input id="draft" value="original"><article><h1>Reader fixture</h1><p>By Alice</p>${Array.from({ length: 20 }, (_, i) => `<p>${text.repeat(3)} ${i === 3 ? 'article-only-needle' : ''}</p>`).join('')}<figure><img src="/image.svg" width="600" height="100" onerror="window.__readerAttack=true"><figcaption>图片说明</figcaption></figure><p><a href="/second">Read next</a> <a href="javascript:alert('unsafe')">Unsafe link</a></p></article><aside>Advertisement poison-ad</aside><script>window.__originalMarker=crypto.randomUUID();window.Readability=function(){throw Error('page parser interference')};window.__roveReader={take:()=>null};</script></body></html>`;
  if (req.url === '/slow') {
    res.write(html.replace('</body></html>', '<img src="/stalled.png">'));
    setTimeout(() => res.end('</body></html>'), 1800);
  } else res.end(html);
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
const base = `http://127.0.0.1:${server.address().port}`;
const checks = [];
const keyboard = [];
let child, uiBrowser, pageBrowser, ui, popup;
async function waitFor(fn, label) {
  for (let i = 0; i < 160; i++) {
    try { const value = await fn(); if (value) return value; } catch { /* Renderer may be navigating. */ }
    await new Promise(resolve => setTimeout(resolve, 100));
  }
  throw Error(`Timed out: ${label}`);
}
const send = command => ui.evaluate(command => window.ipc.postMessage(JSON.stringify(command)), command);
const pages = () => pageBrowser.contexts().flatMap(c => c.pages());
const readerState = () => ui.evaluate(() => window.__readerState);
const keys = async action => {
  for (const page of [ui, popup, ...pages()]) await page.evaluate(() => {
    window.__keyLog = [];
    if (!window.__listeningKeys) {
      window.__listeningKeys = true;
      document.addEventListener('keydown', e => window.__keyLog.push({key:e.key, ctrl:e.ctrlKey, alt:e.altKey, shift:e.shiftKey}), true);
    }
  });
  const result = await execFileAsync('powershell.exe', ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', resolve('scripts/real-keys.ps1'), '-ProcessId', String(child.pid), '-Action', action], { windowsHide: true, timeout: 30000 });
  const documents = await Promise.all([ui, popup, ...pages()].map(page => page.evaluate(() => ({title:document.title, keys:window.__keyLog})).catch(String)));
  keyboard.push({ action, input: result.stdout.trim(), documents });
};
async function shownReader() {
  await waitFor(async () => (await readerState())?.id, 'reader active');
  return waitFor(async () => {
    for (const page of pages()) {
      if (await page.locator('h1.title').count().catch(() => 0)) return page;
    }
  }, 'reader document');
}
async function openReader() {
  await send({ type: 'reader' });
  return shownReader();
}
async function connectUI() {
  child = spawn(resolve(process.env.ERA_SMOKE_EXE || 'native/target/debug/rove.exe'), [], {
    env: { ...process.env, ERA_DATA_DIR: profile, ERA_DEBUG_PORT: String(port) }, windowsHide: true, stdio: 'pipe',
  });
  child.stderr.on('data', data => process.stderr.write(data));
  uiBrowser = await waitFor(() => chromium.connectOverCDP(`http://127.0.0.1:${port}`), 'UI debugger');
  ui = await waitFor(() => uiBrowser.contexts().flatMap(c => c.pages()).find(p => p.url() === 'https://era.ui/index.html'), 'UI');
  await ui.waitForSelector('.sidebar');
  popup = await waitFor(() => ui.context().pages().find(p => p.url().endsWith('?popup')), 'popup');
  await ui.evaluate(() => {
    const receive = window.__ERA_RECEIVE__;
    window.__ERA_RECEIVE__ = event => {
      if (event.type === 'readerState') window.__readerState = event.reader;
      receive?.(event);
    };
  });
}
try {
  await connectUI();
  await send({ type: 'navigate', url: base + '/article' });
  pageBrowser = await waitFor(() => chromium.connectOverCDP(`http://127.0.0.1:${port + 1}`), 'page debugger');
  const original = await waitFor(() => pages().find(p => p.url() === base + '/article'), 'article');
  await original.waitForSelector('#draft');
  const marker = await original.evaluate(() => { document.querySelector('#draft').value = 'unsaved draft'; scrollTo(0, 500); return window.__originalMarker; });
  await ui.getByRole('button', { name: '进入阅读模式', exact: true }).waitFor();
  checks.push('正文识别后地址栏显示阅读入口；站点伪造的解析器不影响隔离世界');
  const initialRequests = requests;
  let reader = await openReader();
  assert.equal(await reader.locator('h1.title').innerText(), 'Reader fixture');
  assert.ok(await reader.locator('main > article').innerText().then(t => t.includes('article-only-needle')));
  assert.ok(!(await reader.locator('main > article').innerText()).includes('poison-nav'));
  assert.equal(await reader.locator('script,iframe,[onerror],[onclick]').count(), 0);
  assert.equal(await reader.locator('a[href^="javascript:"]').count(), 0);
  assert.equal(await reader.evaluate(() => typeof window.ipc), 'undefined');
  assert.equal(await reader.locator('article img').getAttribute('src'), base + '/image.svg');
  checks.push('独立阅读 WebView 提取标题、正文、图片；移除导航、脚本、事件与危险链接，无宿主 IPC');
  await popup.getByRole('button', { name: '阅读外观', exact: true }).click();
  await popup.getByRole('button', { name: '纸色', exact: true }).click();
  await popup.getByRole('button', { name: '无衬线', exact: true }).click();
  await popup.getByRole('button', { name: '宽', exact: true }).click();
  await popup.getByRole('button', { name: '放大阅读字号', exact: true }).click();
  await waitFor(() => reader.evaluate(() => document.documentElement.dataset.theme === 'sepia' && document.documentElement.dataset.face === 'sans' && document.documentElement.style.getPropertyValue('--size') === '22px' && document.documentElement.style.getPropertyValue('--measure') === '40em'), 'live preferences');
  await popup.getByRole('button', { name: '阅读外观', exact: true }).click();
  await send({ type: 'zoom', action: 'in' });
  await waitFor(() => reader.evaluate(() => document.documentElement.style.getPropertyValue('--size') === '24px'), 'reader text zoom');
  checks.push('文字、颜色、字体和栏宽即时生效，Ctrl 缩放命令调整阅读字号');
  await send({ type: 'findOpen' });
  await popup.locator('.find-bar input').fill('article-only-needle');
  await waitFor(async () => await popup.locator('.find-count').innerText() === '1 / 1', 'reader find');
  await send({ type: 'findClose' });
  await popup.locator('.find-bar').waitFor({ state: 'detached' });
  await waitFor(() => reader.evaluate(() => document.hasFocus()), 'reader focus after closing find');
  checks.push('原生 Find API 搜索当前阅读正文');
  await popup.screenshot({ path: join(output, 'reader-controls.png') });
  await reader.screenshot({ path: join(output, 'reader-article.png') });
  await keys('escape');
  await waitFor(async () => !(await readerState())?.id, 'reader closed');
  assert.equal(await original.evaluate(() => window.__originalMarker), marker);
  assert.equal(await original.locator('#draft').inputValue(), 'unsaved draft');
  assert.equal(await original.evaluate(() => scrollY), 500);
  assert.equal(requests, initialRequests);
  checks.push('退出后保留原网页 DOM、未提交表单、滚动位置，未重新请求原文');
  await keys('ctrl-alt-r');
  reader = await shownReader();
  checks.push('真实 Windows 键盘事件：Esc 退出，Ctrl+Alt+R 从网页重新进入');
  await reader.getByRole('link', { name: 'Read next', exact: true }).click();
  await waitFor(() => original.url() === base + '/second', 'reader link navigation');
  await waitFor(async () => !(await readerState())?.id, 'link closes reader');
  checks.push('正文中的相对链接由宿主路由到原标签');
  await send({ type: 'navigate', url: base + '/empty' });
  await waitFor(() => original.url() === base + '/empty', 'empty page');
  await ui.getByRole('button', { name: '进入阅读模式', exact: true }).waitFor({ state: 'detached' });
  await send({ type: 'reader' });
  await waitFor(async () => !(await readerState())?.waiting, 'empty extract finished');
  assert.equal((await readerState()).id, null);
  checks.push('无正文页面隐藏阅读入口，手动调用也不替换原网页');
  await send({ type: 'navigate', url: base + '/slow' });
  await waitFor(() => original.url() === base + '/slow', 'loading document');
  await send({ type: 'reader' });
  await waitFor(async () => (await readerState())?.waiting, 'wait for article load');
  reader = await shownReader();
  await send({ type: 'readerClose' });
  await waitFor(async () => !(await readerState())?.id, 'close loaded reader');
  checks.push('页面加载中请求阅读模式，加载完成后显示正文');
  await send({ type: 'navigate', url: base + '/article' });
  await waitFor(() => original.url() === base + '/article', 'article again');
  await ui.getByRole('button', { name: '进入阅读模式', exact: true }).waitFor();
  const leftId = await ui.getByRole('tab', { selected: true }).getAttribute('data-tab-id');
  await send({ type: 'new', url: base + '/second' });
  const rightId = await waitFor(async () => {
    const id = await ui.getByRole('tab', { selected: true }).getAttribute('data-tab-id');
    return id !== leftId && id;
  }, 'second tab selected');
  await waitFor(() => pages().find(p => p.url() === base + '/second'), 'second split page');
  await send({ type: 'split', id: leftId, other: rightId });
  await ui.locator('.split-divider').waitFor();
  reader = await openReader();
  const geometry = await readerState();
  assert.ok(geometry.rect.width > 100 && geometry.rect.width < 1000);
  await send({ type: 'new', url: 'era://newtab' });
  await waitFor(async () => !(await readerState())?.id, 'switch closes reader');
  checks.push('阅读视图按分屏窗格布局；切换到其他标签后自动关闭');
  // A fresh native process reads the same SQLite profile and preferences.
  await new Promise(resolve => setTimeout(resolve, 500));
  await pageBrowser.close(); pageBrowser = undefined;
  await uiBrowser.close(); uiBrowser = undefined;
  child.kill(); await new Promise(resolve => child.once('exit', resolve));
  await connectUI();
  const restored = await ui.evaluate(() => window.__ERA_INITIAL_STATE__.readerPreferences);
  assert.deepEqual(restored, { size: 24, face: 'sans', theme: 'sepia', width: 'wide' });
  checks.push('SQLite 重启后恢复阅读偏好');
  await writeFile(join(output, 'native-reader-smoke.json'), JSON.stringify({ passed: true, profile, checks, keyboard }, null, 2));
  console.log(JSON.stringify({ passed: true, profile, checks }, null, 2));
} catch (error) {
  await ui?.screenshot({ path: join(output, 'failure-ui.png') }).catch(() => {});
  await popup?.screenshot({ path: join(output, 'failure-popup.png') }).catch(() => {});
  const documents = pageBrowser ? await Promise.all(pages().map(async p => ({url:p.url(), html:await p.content().catch(String), title:await p.title().catch(String)}))) : [];
  await writeFile(join(output, 'failure.json'), JSON.stringify({ checks, keyboard, error: String(error), state: await readerState().catch(() => null), documents }, null, 2));
  throw error;
} finally {
  await pageBrowser?.close(); await uiBrowser?.close();
  if (child && !child.killed) child.kill();
  server.close();
}
